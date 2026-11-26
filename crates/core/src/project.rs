use serde::{Deserialize, Serialize};

use crate::graph::{Curve, CurveKind, CurveStyle, DataSource, Graph, Layer, SurfaceMode, SurfaceSpec, SurfaceSource};
use crate::matrix::Matrix;
use crate::table::{ColumnMode, Table, TableId};
use crate::util::unique_name;
use crate::{GraphId, MatrixId, NoteId};

pub const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Note {
    pub id: NoteId,
    pub name: String,
    pub text: String,
}

impl Default for Note {
    fn default() -> Self {
        Self {
            id: NoteId(0),
            name: crate::locale::t("备注").to_string(),
            text: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Project {
    pub name: String,
    pub tables: Vec<Table>,
    pub matrices: Vec<Matrix>,
    pub graphs: Vec<Graph>,
    pub notes: Vec<Note>,
    pub log: Vec<String>,
    pub next_id: u64,
}

impl Default for Project {
    fn default() -> Self {
        Self::new(crate::locale::t("未命名项目"))
    }
}

impl Project {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tables: Vec::new(),
            matrices: Vec::new(),
            graphs: Vec::new(),
            notes: Vec::new(),
            log: Vec::new(),
            next_id: 1,
        }
    }

    pub fn log_line(&mut self, line: impl Into<String>) {
        self.log.push(line.into());
        if self.log.len() > 2000 {
            let extra = self.log.len() - 2000;
            self.log.drain(0..extra);
        }
    }

    fn alloc(&mut self) -> u64 {
        let id = self.next_id.max(1);
        self.next_id = id + 1;
        id
    }

    pub fn fix_ids(&mut self) {
        let mut max_id = 0u64;
        let mut bump = |id: u64| {
            if id > max_id {
                max_id = id;
            }
        };
        for table in &self.tables {
            bump(table.id.0);
        }
        for matrix in &self.matrices {
            bump(matrix.id.0);
        }
        for graph in &self.graphs {
            bump(graph.id.0);
            for layer in &graph.layers {
                for annotation in &layer.annotations {
                    bump(annotation.id);
                }
            }
        }
        for note in &self.notes {
            bump(note.id.0);
        }
        if self.next_id <= max_id {
            self.next_id = max_id + 1;
        }
    }

    pub fn new_table(&mut self, name: &str) -> TableId {
        let id = TableId(self.alloc());
        let name = unique_name(&self.table_names(), name);
        self.tables.push(Table::new(id, name));
        id
    }

    pub fn new_matrix(&mut self, name: &str, rows: usize, cols: usize) -> MatrixId {
        let id = MatrixId(self.alloc());
        let name = unique_name(&self.matrix_names(), name);
        self.matrices.push(Matrix::new(id, name, rows, cols));
        id
    }

    pub fn new_graph(&mut self, name: &str) -> GraphId {
        let id = GraphId(self.alloc());
        let name = unique_name(&self.graph_names(), name);
        self.graphs.push(Graph::new(id, name));
        id
    }

    pub fn new_note(&mut self, name: &str) -> NoteId {
        let id = NoteId(self.alloc());
        let name = unique_name(&self.notes.iter().map(|n| n.name.clone()).collect::<Vec<_>>(), name);
        self.notes.push(Note {
            id,
            name,
            text: String::new(),
        });
        id
    }

    pub fn table(&self, id: TableId) -> Option<&Table> {
        self.tables.iter().find(|t| t.id == id)
    }

    pub fn table_mut(&mut self, id: TableId) -> Option<&mut Table> {
        self.tables.iter_mut().find(|t| t.id == id)
    }

    pub fn matrix(&self, id: MatrixId) -> Option<&Matrix> {
        self.matrices.iter().find(|m| m.id == id)
    }

    pub fn matrix_mut(&mut self, id: MatrixId) -> Option<&mut Matrix> {
        self.matrices.iter_mut().find(|m| m.id == id)
    }

    pub fn graph(&self, id: GraphId) -> Option<&Graph> {
        self.graphs.iter().find(|g| g.id == id)
    }

    pub fn graph_mut(&mut self, id: GraphId) -> Option<&mut Graph> {
        self.graphs.iter_mut().find(|g| g.id == id)
    }

    pub fn note_mut(&mut self, id: NoteId) -> Option<&mut Note> {
        self.notes.iter_mut().find(|n| n.id == id)
    }

    pub fn delete_table(&mut self, id: TableId) -> bool {
        let before = self.tables.len();
        self.tables.retain(|t| t.id != id);
        self.tables.len() != before
    }

    pub fn delete_matrix(&mut self, id: MatrixId) -> bool {
        let before = self.matrices.len();
        self.matrices.retain(|m| m.id != id);
        self.matrices.len() != before
    }

    pub fn delete_graph(&mut self, id: GraphId) -> bool {
        let before = self.graphs.len();
        self.graphs.retain(|g| g.id != id);
        self.graphs.len() != before
    }

    pub fn delete_note(&mut self, id: NoteId) -> bool {
        let before = self.notes.len();
        self.notes.retain(|n| n.id != id);
        self.notes.len() != before
    }

    /// 插入列，并把引用这张表的曲线列号一起后移。
    pub fn insert_table_column(&mut self, id: TableId, index: usize, name: impl Into<String>, mode: ColumnMode) -> Result<(), String> {
        let at = self.table_mut(id).ok_or(crate::t("找不到表格"))?.insert_column(index, name, mode);
        self.retarget_columns(id, at, true);
        Ok(())
    }

    /// 删除列。引用被删的 X 或 Y 的曲线会去掉，误差列和向量分量则清空。
    pub fn remove_table_column(&mut self, id: TableId, index: usize) -> Result<(), String> {
        let removed = self.table_mut(id).ok_or(crate::t("找不到表格"))?.remove_column(index);
        if !removed {
            return Err(crate::t("没有这一列").into());
        }
        self.retarget_columns(id, index, false);
        Ok(())
    }

    pub fn graphs_using_table(&self, id: TableId) -> Vec<String> {
        self.graphs
            .iter()
            .filter(|graph| {
                graph.layers.iter().any(|layer| {
                    layer.curves.iter().any(|curve| matches!(curve.source, DataSource::Columns { table, .. } if table == id))
                })
            })
            .map(|graph| graph.name.clone())
            .collect()
    }

    pub fn graphs_using_matrix(&self, id: MatrixId) -> Vec<String> {
        self.graphs
            .iter()
            .filter(|graph| {
                let surface = matches!(graph.surface.as_ref().map(|spec| &spec.source), Some(SurfaceSource::Matrix(matrix)) if *matrix == id);
                let curve = graph.layers.iter().any(|layer| {
                    layer.curves.iter().any(|curve| matches!(curve.source, DataSource::Matrix { matrix } if matrix == id))
                });
                surface || curve
            })
            .map(|graph| graph.name.clone())
            .collect()
    }

    fn retarget_columns(&mut self, id: TableId, at: usize, inserted: bool) {
        let mut dropped = Vec::new();
        for graph in &mut self.graphs {
            for layer in &mut graph.layers {
                let mut kept = Vec::with_capacity(layer.curves.len());
                for mut curve in layer.curves.drain(..) {
                    let drop_curve = match &mut curve.source {
                        DataSource::Columns {
                            table,
                            x,
                            y,
                            x_err,
                            y_err,
                            dx,
                            dy,
                        } if *table == id => {
                            let lost_x = shift_index(x, at, inserted);
                            let lost_y = shift_index(y, at, inserted);
                            shift_optional(x_err, at, inserted);
                            shift_optional(y_err, at, inserted);
                            shift_optional(dx, at, inserted);
                            shift_optional(dy, at, inserted);
                            !inserted && (lost_x || lost_y)
                        }
                        _ => false,
                    };
                    if drop_curve {
                        dropped.push(curve.name);
                    } else {
                        kept.push(curve);
                    }
                }
                layer.curves = kept;
            }
        }
        for name in dropped {
            self.log_line(crate::trf!(
                "曲线「{name}」因列被删除而移除",
                "Curve \"{name}\" was removed because its column was deleted",
                name = name
            ));
        }
    }

    pub fn rename_table(&mut self, id: TableId, name: &str) -> Result<(), String> {
        if name.trim().is_empty() {
            return Err(crate::locale::t("名称不能为空").into());
        }
        if self.tables.iter().any(|t| t.id != id && t.name == name) {
            return Err(crate::locale::t("已有同名表格").into());
        }
        self.table_mut(id).ok_or(crate::locale::t("找不到表格"))?.name = name.to_string();
        Ok(())
    }

    pub fn to_json(&self) -> Result<String, String> {
        let file = ProjectFile {
            format: FORMAT_VERSION,
            project: self.clone(),
        };
        serde_json::to_string_pretty(&file).map_err(|err| err.to_string())
    }

    pub fn from_json(text: &str) -> Result<Project, String> {
        let value: serde_json::Value = serde_json::from_str(text).map_err(|err| format!("不是合法的 JSON：{err}"))?;
        let obj = value.as_object().ok_or(crate::t("不是 SciPlot 项目文件"))?;
        if obj.contains_key("format") || obj.contains_key("project") {
            let format = obj.get("format").and_then(json_format).ok_or(crate::t("不是 SciPlot 项目文件：缺少 format"))?;
            if format > FORMAT_VERSION {
                return Err(crate::trf!(
                    "项目由更新版本创建（format {format}），请升级 SciPlot",
                    "This project was written by a newer SciPlot (format {format}). Please upgrade.",
                    format = format
                ));
            }
            let project = obj.get("project").cloned().ok_or(crate::t("不是 SciPlot 项目文件：缺少 project"))?;
            let mut project: Project = serde_json::from_value(project).map_err(|err| {
                crate::trf!("不是 SciPlot 项目文件：{err}", "This is not a SciPlot project: {err}", err = err)
            })?;
            project.fix_ids();
            return Ok(project);
        }
        let legacy = ["tables", "graphs", "matrices", "notes"].iter().any(|key| obj.contains_key(*key));
        if !legacy {
            return Err(crate::t("不是 SciPlot 项目文件").into());
        }
        let mut project: Project = serde_json::from_value(value).map_err(|err| {
            crate::trf!("不是 SciPlot 项目文件：{err}", "This is not a SciPlot project: {err}", err = err)
        })?;
        project.fix_ids();
        Ok(project)
    }

    /// 用表的 X 列和全部 Y 列新建一张图。直方图、箱线图、饼图按 Y 列的数值作图。
    /// 只有一条 Y 时，图名和标题用「Y-X」，坐标轴标题用列名。
    pub fn plot_table(&mut self, table_id: TableId, kind: CurveKind) -> Result<GraphId, String> {
        let (table_name, x_label, y_labels) = {
            let table = self.table(table_id).ok_or(crate::locale::t("找不到表格"))?;
            let x_label = table
                .columns
                .iter()
                .find(|column| column.mode == ColumnMode::X)
                .map(|column| column.name.clone())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "X".into());
            let y_labels: Vec<String> = table
                .columns
                .iter()
                .filter(|column| column.mode == ColumnMode::Y || column.mode == ColumnMode::Z)
                .map(|column| column.name.clone())
                .filter(|name| !name.is_empty())
                .collect();
            (table.name.clone(), x_label, y_labels)
        };
        let graph_name = if y_labels.len() == 1 {
            format!("{}-{}", y_labels[0], x_label)
        } else {
            format!("{table_name}-{}", kind.label())
        };
        let graph_id = self.new_graph(&graph_name);
        self.add_curves(graph_id, 0, table_id, kind)?;
        if let Some(graph) = self.graph_mut(graph_id) {
            if let Some(layer) = graph.layers.first_mut() {
                layer.title = if y_labels.len() == 1 {
                    format!("{}-{}", y_labels[0], x_label)
                } else {
                    format!("{table_name} {}", kind.label())
                };
                if layer.x_axis.label.is_empty() || layer.x_axis.label == "x" {
                    layer.x_axis.label = x_label.clone();
                }
                if y_labels.len() == 1 && (layer.y_axis.label.is_empty() || layer.y_axis.label == "y") {
                    layer.y_axis.label = y_labels[0].clone();
                }
            }
        }
        Ok(graph_id)
    }

    pub fn add_curves(&mut self, graph_id: GraphId, layer: usize, table_id: TableId, kind: CurveKind) -> Result<(), String> {
        let curves = {
            let table = self.table(table_id).ok_or(crate::locale::t("找不到表格"))?;
            curves_from_table(table, kind)?
        };
        let graph = self.graph_mut(graph_id).ok_or(crate::locale::t("找不到图形"))?;
        if graph.layers.is_empty() {
            graph.layers.push(Layer::default());
        }
        let layer = graph.layers.get_mut(layer).ok_or(crate::locale::t("找不到图层"))?;
        let color_start = layer.curves.len();
        for (offset, mut curve) in curves.into_iter().enumerate() {
            curve.style = CurveStyle::from_index(color_start + offset);
            layer.curves.push(curve);
        }
        Ok(())
    }

    pub fn plot_matrix(&mut self, matrix_id: MatrixId, kind: CurveKind) -> Result<GraphId, String> {
        if !matches!(kind, CurveKind::Heatmap | CurveKind::Contour) {
            return Err(crate::locale::t("矩阵可以直接画成热力图或等高线").into());
        }
        let name = self.matrix(matrix_id).ok_or(crate::locale::t("找不到矩阵"))?.name.clone();
        let graph_id = self.new_graph(&format!("{name}-{}", kind.label()));
        let style = CurveStyle::from_index(0);
        if let Some(graph) = self.graph_mut(graph_id) {
            let layer = &mut graph.layers[0];
            layer.title = format!("{name} {}", kind.label());
            layer.curves.push(Curve {
                name: name.clone(),
                kind,
                style,
                source: DataSource::Matrix { matrix: matrix_id },
                visible: true,
            });
        }
        Ok(graph_id)
    }

    pub fn plot_surface(&mut self, source: SurfaceSource, mode: SurfaceMode, title: &str) -> GraphId {
        let graph_id = self.new_graph(title);
        if let Some(graph) = self.graph_mut(graph_id) {
            graph.surface = Some(SurfaceSpec {
                source,
                mode,
                title: title.to_string(),
                ..SurfaceSpec::default()
            });
        }
        graph_id
    }

    pub fn matrix_from_table(&mut self, table_id: TableId) -> Result<MatrixId, String> {
        let table = self.table(table_id).ok_or(crate::locale::t("找不到表格"))?;
        let cols: Vec<usize> = table
            .columns
            .iter()
            .enumerate()
            .filter(|(_, column)| column.numbers.iter().any(|v| v.is_some()))
            .map(|(i, _)| i)
            .collect();
        if cols.is_empty() {
            return Err(crate::locale::t("表中没有数值列").into());
        }
        let rows = table.nrows().max(1);
        let name = crate::trf!("{}-矩阵", "{}-matrix", table.name);
        let matrix_id = self.new_matrix(&name, rows, cols.len());
        let table = self.table(table_id).unwrap();
        let snapshot: Vec<Vec<Option<f64>>> = cols
            .iter()
            .map(|&index| table.columns[index].numbers.clone())
            .collect();
        let matrix = self.matrix_mut(matrix_id).unwrap();
        matrix.x_min = 1.0;
        matrix.x_max = cols.len() as f64;
        matrix.y_min = 1.0;
        matrix.y_max = rows as f64;
        for (c, column) in snapshot.iter().enumerate() {
            for r in 0..rows {
                matrix.set(r, c, column.get(r).copied().flatten());
            }
        }
        Ok(matrix_id)
    }

    pub fn table_from_matrix(&mut self, matrix_id: MatrixId) -> Result<TableId, String> {
        let matrix = self.matrix(matrix_id).ok_or(crate::locale::t("找不到矩阵"))?.clone();
        let table_id = self.new_table(&crate::trf!("{}-表", "{}-table", matrix.name));
        let table = self.table_mut(table_id).unwrap();
        table.columns.clear();
        for c in 0..matrix.cols {
            let values: Vec<f64> = (0..matrix.rows)
                .map(|r| matrix.get(r, c).unwrap_or(f64::NAN))
                .collect();
            let mode = if c == 0 { ColumnMode::X } else { ColumnMode::Y };
            table.add_numeric_column(format!("C{}", c + 1), mode, &values);
        }
        let shown = self.table(table_id).map(|table| table.name.clone()).unwrap_or_default();
        let source = matrix.name.clone();
        self.log_line(crate::trf!("已把矩阵「{source}」转为表格「{shown}」。", "Converted matrix \"{source}\" to table \"{shown}\".", source = source, shown = shown));
        Ok(table_id)
    }

    pub fn alloc_annotation_id(&mut self) -> u64 {
        self.alloc()
    }

    pub fn insert_table(&mut self, mut table: Table) -> TableId {
        table.id = TableId(self.alloc());
        table.name = unique_name(&self.table_names(), &table.name);
        let id = table.id;
        self.tables.push(table);
        id
    }

    pub fn insert_matrix(&mut self, mut matrix: Matrix) -> MatrixId {
        matrix.id = MatrixId(self.alloc());
        matrix.name = unique_name(&self.matrix_names(), &matrix.name);
        let id = matrix.id;
        self.matrices.push(matrix);
        id
    }

    fn table_names(&self) -> Vec<String> {
        self.tables.iter().map(|t| t.name.clone()).collect()
    }

    fn matrix_names(&self) -> Vec<String> {
        self.matrices.iter().map(|m| m.name.clone()).collect()
    }

    fn graph_names(&self) -> Vec<String> {
        self.graphs.iter().map(|g| g.name.clone()).collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectFile {
    pub format: u32,
    pub project: Project,
}

fn json_format(value: &serde_json::Value) -> Option<u32> {
    value
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .or_else(|| value.as_i64().and_then(|n| u32::try_from(n).ok()))
}

/// 插入时把 `>= at` 的列号加一。删除时若正好是这一列则返回 true，更大的列号减一。
fn shift_index(index: &mut usize, at: usize, inserted: bool) -> bool {
    if inserted {
        if *index >= at {
            *index += 1;
        }
        false
    } else if *index == at {
        true
    } else {
        if *index > at {
            *index -= 1;
        }
        false
    }
}

fn shift_optional(slot: &mut Option<usize>, at: usize, inserted: bool) {
    let Some(value) = *slot else {
        return;
    };
    if inserted {
        if value >= at {
            *slot = Some(value + 1);
        }
    } else if value == at {
        *slot = None;
    } else if value > at {
        *slot = Some(value - 1);
    }
}

fn curves_from_table(table: &Table, kind: CurveKind) -> Result<Vec<Curve>, String> {
    if table.columns.is_empty() {
        return Err(crate::locale::t("表格没有列").into());
    }
    if kind == CurveKind::Vector {
        return Ok(vec![vector_curve(table)?]);
    }
    let x = table
        .columns
        .iter()
        .position(|column| column.mode == ColumnMode::X)
        .unwrap_or(0);
    let mut y_cols: Vec<usize> = table
        .columns
        .iter()
        .enumerate()
        .filter(|(_, column)| column.mode == ColumnMode::Y || column.mode == ColumnMode::Z)
        .map(|(index, _)| index)
        .collect();
    if y_cols.is_empty() {
        y_cols = table
            .columns
            .iter()
            .enumerate()
            .filter(|(index, column)| *index != x && column.numbers.iter().any(|v| v.is_some()))
            .map(|(index, _)| index)
            .collect();
    }
    if y_cols.is_empty() {
        return Err(crate::locale::t("没有可绘制的 Y 列。请把一列设为 Y，或填入数值。").into());
    }
    let x_err = table.columns.iter().position(|column| column.mode == ColumnMode::XErr);
    let y_errs: Vec<usize> = table
        .columns
        .iter()
        .enumerate()
        .filter(|(_, column)| column.mode == ColumnMode::YErr)
        .map(|(index, _)| index)
        .collect();
    let mut curves = Vec::new();
    for y in y_cols {
        let y_err = if table.columns.get(y + 1).map(|c| c.mode) == Some(ColumnMode::YErr) {
            Some(y + 1)
        } else if y_errs.len() == 1 {
            Some(y_errs[0])
        } else {
            None
        };
        curves.push(Curve {
            name: table.columns[y].name.clone(),
            kind,
            style: CurveStyle::default(),
            source: DataSource::Columns {
                table: table.id,
                x,
                y,
                x_err: if kind == CurveKind::ErrorBars { x_err } else { None },
                y_err: if kind == CurveKind::ErrorBars { y_err } else { None },
                dx: None,
                dy: None,
            },
            visible: true,
        });
    }
    Ok(curves)
}

fn vector_curve(table: &Table) -> Result<Curve, String> {
    let x = table
        .columns
        .iter()
        .position(|column| column.mode == ColumnMode::X)
        .unwrap_or(0);
    let y = table
        .columns
        .iter()
        .position(|column| column.mode == ColumnMode::Y)
        .unwrap_or(1);
    let find = |names: &[&str]| {
        table.columns.iter().position(|column| {
            names.iter().any(|name| column.name.eq_ignore_ascii_case(name))
        })
    };
    let mut dx = find(&["dx", "dX", "vx"]);
    let mut dy = find(&["dy", "dY", "vy"]);
    if dx.is_none() || dy.is_none() {
        let extra: Vec<usize> = table
            .columns
            .iter()
            .enumerate()
            .filter(|(index, column)| {
                *index != x && *index != y && column.numbers.iter().any(|v| v.is_some())
            })
            .map(|(index, _)| index)
            .collect();
        if extra.len() >= 2 {
            dx = Some(extra[0]);
            dy = Some(extra[1]);
        }
    }
    let (Some(dx), Some(dy)) = (dx, dy) else {
        return Err(crate::locale::t("向量场需要 X、Y、dx、dy 四列（后两列可命名为 dx、dy）").into());
    };
    if y >= table.columns.len() {
        return Err(crate::locale::t("向量场需要 Y 列").into());
    }
    Ok(Curve {
        name: crate::locale::t("向量").to_string(),
        kind: CurveKind::Vector,
        style: CurveStyle::default(),
        source: DataSource::Columns {
            table: table.id,
            x,
            y,
            x_err: None,
            y_err: None,
            dx: Some(dx),
            dy: Some(dy),
        },
        visible: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_plot() {
        let mut project = Project::new("P");
        let table_id = project.new_table(crate::locale::t("数据"));
        {
            let table = project.table_mut(table_id).unwrap();
            table.columns[0].set_numbers(&[0.0, 1.0, 2.0]);
            table.columns[1].set_numbers(&[0.0, 1.0, 4.0]);
        }
        let graph_id = project.plot_table(table_id, CurveKind::Line).unwrap();
        assert_eq!(project.graph(graph_id).unwrap().layers[0].curves.len(), 1);
        let json = project.to_json().unwrap();
        let loaded = Project::from_json(&json).unwrap();
        assert_eq!(loaded.name, "P");
        assert_eq!(loaded.tables[0].columns[1].numbers[2], Some(4.0));
        assert!(loaded.next_id > project.graphs[0].id.0);
        let graph = loaded.graph(graph_id).unwrap();
        assert_eq!(graph.name, "B-A");
        assert_eq!(graph.layers[0].title, "B-A");
        assert_eq!(graph.layers[0].x_axis.label, "A");
        assert_eq!(graph.layers[0].y_axis.label, "B");
    }

    #[test]
    fn inserting_or_removing_columns_keeps_curve_bindings() {
        let mut project = Project::new("实验");
        let table_id = project.new_table("温度电阻");
        {
            let table = project.table_mut(table_id).unwrap();
            table.columns[0].name = "温度".into();
            table.columns[1].name = "电阻".into();
            table.columns[0].set_numbers(&[10.0, 20.0, 30.0]);
            table.columns[1].set_numbers(&[100.0, 200.0, 300.0]);
        }
        let graph_id = project.plot_table(table_id, CurveKind::Line).unwrap();
        project.insert_table_column(table_id, 0, "备注", ColumnMode::None).unwrap();
        let source = project.graph(graph_id).unwrap().layers[0].curves[0].source.clone();
        match source {
            DataSource::Columns { x, y, .. } => {
                assert_eq!((x, y), (1, 2));
            }
            other => panic!("unexpected {other:?}"),
        }
        let series = crate::resolve_curve(&project, &project.graph(graph_id).unwrap().layers[0].curves[0]).unwrap();
        assert_eq!(series.x, vec![10.0, 20.0, 30.0]);
        assert_eq!(series.y, vec![100.0, 200.0, 300.0]);

        project.remove_table_column(table_id, 2).unwrap();
        assert!(project.graph(graph_id).unwrap().layers[0].curves.is_empty());
        assert!(project.log.iter().any(|line| line.contains("因列被删除而移除")));
    }

    #[test]
    fn from_json_rejects_unknown_format_and_plain_objects() {
        assert!(Project::from_json("{}").is_err());
        assert!(Project::from_json(r#"{"foo":1,"bar":"baz"}"#).is_err());
        assert!(Project::from_json(r#"{"format":1}"#).is_err());
        let err = Project::from_json(r#"{"format":99,"project":{"name":"未来"}}"#).unwrap_err();
        assert!(err.contains("升级"), "{err}");
        let loaded = Project::from_json(r#"{"format":1,"project":{"name":"实验"}}"#).unwrap();
        assert_eq!(loaded.name, "实验");
        let legacy = Project::from_json(r#"{"name":"旧","tables":[]}"#).unwrap();
        assert_eq!(legacy.name, "旧");
    }

    #[test]
    fn old_axis_json_still_loads() {
        let axis: crate::Axis = serde_json::from_str(r#"{"label":"x","scale":"Linear"}"#).unwrap();
        assert!(axis.step.is_none());
        assert!(axis.minor_count.is_none());
        assert_eq!(axis.label, "x");
    }
}
