//! 从表格里选中的一列取出三维色带、三维柱和三维轨迹的坐标。
//!
//! 选中的列是高度 Z。X 用它左侧最近的数值 X 列，没有就用从 1 开始的行号。
//! 三维柱和三维轨迹再找另一列数值 Y 当作第二维；找不到时 Y 取 0，画在一个平面上。
//! 色带不使用 Y 数据，渲染时自己加一条窄带。

use crate::graph::TablePlot3D;
use crate::table::{ColumnMode, Table};

#[derive(Clone, Debug)]
pub struct Column3D {
    pub kind: TablePlot3D,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub z: Vec<f64>,
    pub x_name: String,
    pub y_name: String,
    pub z_name: String,
    /// Y 没有单独的列，所有点的 Y 都是 0。
    pub y_is_plane: bool,
}

pub fn column_3d(table: &Table, column: usize, kind: TablePlot3D) -> Result<Column3D, String> {
    let z_col = table
        .columns
        .get(column)
        .ok_or_else(|| crate::tr!("找不到要绘制的列", "The column to plot is missing"))?;
    let z_name = z_col.name.clone();
    let n = table.nrows().max(z_col.len());
    if n < 2 {
        return Err(crate::tr!("这一列至少需要两个数值", "The column needs at least two numbers").into());
    }
    let x_index = numeric_role(table, column, ColumnMode::X);
    let (x_all, x_name) = match x_index {
        Some(index) => (
            (0..n).map(|row| table.columns[index].number_at(row)).collect::<Vec<_>>(),
            table.columns[index].name.clone(),
        ),
        None => ((1..=n).map(|row| row as f64).collect(), crate::tr!("行号", "Row").to_string()),
    };
    let y_index = match kind {
        TablePlot3D::Ribbon => None,
        TablePlot3D::Bars | TablePlot3D::Trajectory => numeric_role(table, column, ColumnMode::Y)
            .filter(|index| Some(*index) != x_index),
    };
    let (y_all, y_name, y_is_plane) = match y_index {
        Some(index) => (
            (0..n).map(|row| table.columns[index].number_at(row)).collect::<Vec<_>>(),
            table.columns[index].name.clone(),
            false,
        ),
        None => (vec![0.0; n], "Y".to_string(), true),
    };
    let z_all: Vec<f64> = (0..n).map(|row| z_col.number_at(row)).collect();
    let mut x = Vec::new();
    let mut y = Vec::new();
    let mut z = Vec::new();
    for row in 0..n {
        let xv = x_all.get(row).copied().unwrap_or(f64::NAN);
        let yv = y_all.get(row).copied().unwrap_or(f64::NAN);
        let zv = z_all.get(row).copied().unwrap_or(f64::NAN);
        if xv.is_finite() && yv.is_finite() && zv.is_finite() {
            x.push(xv);
            y.push(yv);
            z.push(zv);
        }
    }
    if z.len() < 2 {
        return Err(crate::tr!(
            "这一列至少需要两个数值",
            "The column needs at least two numbers"
        )
        .into());
    }
    Ok(Column3D {
        kind,
        x,
        y,
        z,
        x_name,
        y_name,
        z_name,
        y_is_plane,
    })
}

fn numeric_role(table: &Table, selected: usize, mode: ColumnMode) -> Option<usize> {
    let usable = |index: usize| {
        index != selected
            && table.columns.get(index).is_some_and(|column| {
                column.mode == mode && column.numbers.iter().any(|value| value.is_some())
            })
    };
    if selected > 0 {
        if let Some(index) = (0..selected).rev().find(|index| usable(*index)) {
            return Some(index);
        }
    }
    (0..table.columns.len()).find(|index| usable(*index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::{ColumnMode, Table, TableId};

    fn sample() -> Table {
        let mut table = Table::new(TableId(1), "t");
        table.columns.clear();
        table.add_numeric_column("X", ColumnMode::X, &[0.0, 1.0, 2.0, 3.0]);
        table.add_numeric_column("Y", ColumnMode::Y, &[1.0, 2.0, 3.0, 4.0]);
        table.add_numeric_column("Z", ColumnMode::Z, &[0.0, 1.0, 0.0, 1.0]);
        table
    }

    #[test]
    fn selected_z_uses_x_and_y_and_a_single_y_falls_back_to_a_plane() {
        let table = sample();
        let bars = column_3d(&table, 2, TablePlot3D::Bars).unwrap();
        assert_eq!(bars.x, vec![0.0, 1.0, 2.0, 3.0]);
        assert_eq!(bars.y, vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(bars.z, vec![0.0, 1.0, 0.0, 1.0]);
        assert!(!bars.y_is_plane);
        assert_eq!(bars.z_name, "Z");

        let ribbon = column_3d(&table, 1, TablePlot3D::Ribbon).unwrap();
        assert_eq!(ribbon.x, vec![0.0, 1.0, 2.0, 3.0]);
        assert_eq!(ribbon.z, vec![1.0, 2.0, 3.0, 4.0]);
        assert!(ribbon.y_is_plane);
        assert!(ribbon.y.iter().all(|value| *value == 0.0));

        let path = column_3d(&table, 1, TablePlot3D::Trajectory).unwrap();
        assert!(path.y_is_plane, "the only Y column is the height, so the path lies in a plane");
        assert_eq!(path.z, vec![1.0, 2.0, 3.0, 4.0]);
    }
}
