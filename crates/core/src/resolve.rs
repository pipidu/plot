use crate::formula::CompiledExpr;
use crate::graph::{Curve, CurveKind, DataSource, ErrorSource, VectorMode};
use crate::matrix::Matrix;
use crate::table::{Column, ColumnMode};
use crate::Project;

#[derive(Clone, Debug)]
pub struct GridData {
    pub rows: usize,
    pub cols: usize,
    pub x0: f64,
    pub x1: f64,
    pub y0: f64,
    pub y1: f64,
    pub z: Vec<f64>,
}

impl GridData {
    pub fn from_matrix(matrix: &Matrix) -> Self {
        let mut z = Vec::with_capacity(matrix.rows * matrix.cols);
        for row in 0..matrix.rows {
            for col in 0..matrix.cols {
                z.push(matrix.get(row, col).unwrap_or(f64::NAN));
            }
        }
        Self {
            rows: matrix.rows,
            cols: matrix.cols,
            x0: matrix.x_min,
            x1: matrix.x_max,
            y0: matrix.y_min,
            y1: matrix.y_max,
            z,
        }
    }

    pub fn at(&self, row: usize, col: usize) -> f64 {
        self.z.get(row * self.cols + col).copied().unwrap_or(f64::NAN)
    }
}

#[derive(Clone, Debug)]
pub struct SeriesData {
    pub name: String,
    pub kind: CurveKind,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub x_err: Vec<f64>,
    pub y_err: Vec<f64>,
    pub dx: Vec<f64>,
    pub dy: Vec<f64>,
    pub labels: Vec<String>,
    /// 点旁边的文字，来自角色为「标签」的列。和分类轴用的 `labels` 分开。
    pub point_labels: Vec<String>,
    pub grid: Option<GridData>,
}

impl SeriesData {
    pub fn pairs(&self) -> Vec<(f64, f64)> {
        self.x
            .iter()
            .zip(self.y.iter())
            .filter(|(x, y)| x.is_finite() && y.is_finite())
            .map(|(x, y)| (*x, *y))
            .collect()
    }
}

pub fn resolve_curve(project: &Project, curve: &Curve) -> Result<SeriesData, String> {
    let mut series = SeriesData {
        name: curve.name.clone(),
        kind: curve.kind,
        x: Vec::new(),
        y: Vec::new(),
        x_err: Vec::new(),
        y_err: Vec::new(),
        dx: Vec::new(),
        dy: Vec::new(),
        labels: Vec::new(),
        point_labels: Vec::new(),
        grid: None,
    };
    match &curve.source {
        DataSource::Columns {
            table,
            x,
            y,
            x_err,
            y_err,
            dx,
            dy,
        } => {
            let table = project.table(*table).ok_or(crate::locale::t("找不到曲线对应的表格"))?;
            let (xs, labels) = column_x(table.columns.get(*x).ok_or(crate::locale::t("X 列不存在"))?)?;
            let ys = column_numbers(table.columns.get(*y).ok_or(crate::locale::t("Y 列不存在"))?);
            let n = xs.len().max(ys.len());
            series.x = pad(xs, n);
            series.y = pad(ys, n);
            series.labels = if labels.len() == n { labels } else { Vec::new() };
            if let Some(index) = label_column(table, *x, *y) {
                series.point_labels = (0..n)
                    .map(|row| table.columns[index].display(row))
                    .collect();
            }
            if let Some(index) = x_err {
                series.x_err = pad(column_numbers(table.columns.get(*index).ok_or(crate::locale::t("x 误差列不存在"))?), n);
            }
            if let Some(index) = y_err {
                series.y_err = pad(column_numbers(table.columns.get(*index).ok_or(crate::locale::t("y 误差列不存在"))?), n);
            }
            if let Some(index) = dx {
                series.dx = pad(column_numbers(table.columns.get(*index).ok_or(crate::locale::t("dx 列不存在"))?), n);
            }
            if let Some(index) = dy {
                series.dy = pad(column_numbers(table.columns.get(*index).ok_or(crate::locale::t("dy 列不存在"))?), n);
            }
        }
        DataSource::Matrix { matrix } => {
            let matrix = project.matrix(*matrix).ok_or(crate::locale::t("找不到曲线对应的矩阵"))?;
            series.grid = Some(GridData::from_matrix(matrix));
        }
        DataSource::Function {
            expr,
            x_min,
            x_max,
            samples,
        } => {
            let (x, y) = sample_expr(expr, *x_min, *x_max, *samples, "x")?;
            series.x = x;
            series.y = y;
        }
        DataSource::Parametric {
            x_expr,
            y_expr,
            t_min,
            t_max,
            samples,
        } => {
            let n = (*samples).max(2);
            let x_expr = CompiledExpr::compile(x_expr)?;
            let y_expr = CompiledExpr::compile(y_expr)?;
            for i in 0..n {
                let t = *t_min + (*t_max - *t_min) * (i as f64) / (n as f64 - 1.0);
                let x = x_expr.eval(&[("t", t), ("x", t)]).unwrap_or(f64::NAN);
                let y = y_expr.eval(&[("t", t), ("y", t)]).unwrap_or(f64::NAN);
                series.x.push(x);
                series.y.push(y);
            }
        }
        DataSource::Polar {
            expr,
            theta_min,
            theta_max,
            samples,
        } => {
            let compiled = CompiledExpr::compile(expr)?;
            let n = (*samples).max(2);
            for i in 0..n {
                let theta = if (theta_max - theta_min).abs() < f64::EPSILON {
                    *theta_min
                } else {
                    *theta_min + (*theta_max - *theta_min) * (i as f64) / (n as f64 - 1.0)
                };
                let r = compiled
                    .eval(&[("theta", theta), ("t", theta), ("x", theta)])
                    .unwrap_or(f64::NAN);
                if r.is_finite() && theta.is_finite() {
                    series.x.push(r * theta.cos());
                    series.y.push(r * theta.sin());
                } else {
                    series.x.push(f64::NAN);
                    series.y.push(f64::NAN);
                }
            }
        }
        DataSource::Points { x, y } => {
            let n = x.len().max(y.len());
            series.x = pad(x.clone(), n);
            series.y = pad(y.clone(), n);
        }
    }
    apply_vector(&mut series, curve.vector_mode);
    apply_error(&mut series, curve);
    Ok(series)
}

fn apply_vector(series: &mut SeriesData, mode: VectorMode) {
    let n = series.x.len().min(series.y.len()).min(series.dx.len()).min(series.dy.len());
    match mode {
        VectorMode::DxDy => {}
        VectorMode::EndPoint => {
            for i in 0..n {
                if series.x[i].is_finite() && series.dx[i].is_finite() {
                    series.dx[i] -= series.x[i];
                }
                if series.y[i].is_finite() && series.dy[i].is_finite() {
                    series.dy[i] -= series.y[i];
                }
            }
        }
        VectorMode::AngleMagnitude => {
            for i in 0..n {
                let angle = series.dx[i];
                let magnitude = series.dy[i];
                if angle.is_finite() && magnitude.is_finite() {
                    series.dx[i] = magnitude * angle.cos();
                    series.dy[i] = magnitude * angle.sin();
                }
            }
        }
    }
}

fn apply_error(series: &mut SeriesData, curve: &Curve) {
    series.x_err = expand_error(&series.x, &series.x_err, curve.error.x, curve.error.x_percent);
    series.y_err = expand_error(&series.y, &series.y_err, curve.error.y, curve.error.y_percent);
}

fn expand_error(values: &[f64], column: &[f64], source: ErrorSource, percent: f64) -> Vec<f64> {
    match source {
        ErrorSource::None | ErrorSource::Column => {
            if column.is_empty() {
                Vec::new()
            } else {
                column.to_vec()
            }
        }
        ErrorSource::Percent => {
            let scale = percent / 100.0;
            values
                .iter()
                .map(|value| if value.is_finite() { scale * value.abs() } else { f64::NAN })
                .collect()
        }
        ErrorSource::StandardDeviation | ErrorSource::StandardError => {
            let finite: Vec<f64> = values.iter().copied().filter(|value| value.is_finite()).collect();
            let mut err = sample_std(&finite);
            if source == ErrorSource::StandardError && finite.len() > 1 {
                err /= (finite.len() as f64).sqrt();
            }
            values
                .iter()
                .map(|value| if value.is_finite() { err } else { f64::NAN })
                .collect()
        }
    }
}

fn sample_std(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let var = values.iter().map(|value| (value - mean) * (value - mean)).sum::<f64>() / (values.len() - 1) as f64;
    var.sqrt()
}

pub fn sample_expr(expr: &str, min: f64, max: f64, samples: usize, var: &str) -> Result<(Vec<f64>, Vec<f64>), String> {
    let compiled = CompiledExpr::compile(expr)?;
    let n = samples.max(2);
    let mut xs = Vec::with_capacity(n);
    let mut ys = Vec::with_capacity(n);
    for i in 0..n {
        let x = if (max - min).abs() < f64::EPSILON {
            min
        } else {
            min + (max - min) * (i as f64) / (n as f64 - 1.0)
        };
        let y = compiled
            .eval(&[(var, x), ("x", x), ("t", x)])
            .unwrap_or(f64::NAN);
        xs.push(x);
        ys.push(y);
    }
    Ok((xs, ys))
}

fn label_column(table: &crate::table::Table, x: usize, y: usize) -> Option<usize> {
    let labels: Vec<usize> = table
        .columns
        .iter()
        .enumerate()
        .filter(|(index, column)| column.mode == ColumnMode::Label && *index != x && *index != y)
        .map(|(index, _)| index)
        .collect();
    if labels.is_empty() {
        return None;
    }
    if labels.contains(&(y.saturating_add(1))) {
        Some(y + 1)
    } else {
        labels.first().copied()
    }
}

fn column_numbers(column: &Column) -> Vec<f64> {
    (0..column.len()).map(|row| column.number_at(row)).collect()
}

fn column_x(column: &Column) -> Result<(Vec<f64>, Vec<String>), String> {
    let n = column.len();
    let numeric = column.numbers.iter().flatten().count();
    if numeric == 0 && column.texts.iter().any(|text| !text.is_empty()) {
        let labels = (0..n)
            .map(|row| column.texts.get(row).cloned().unwrap_or_default())
            .collect();
        let x = (0..n).map(|i| i as f64).collect();
        return Ok((x, labels));
    }
    let labels = if column.texts.iter().any(|text| !text.trim().is_empty()) && numeric < n {
        (0..n)
            .map(|row| column.texts.get(row).cloned().unwrap_or_default())
            .collect()
    } else {
        Vec::new()
    };
    Ok((column_numbers(column), labels))
}

fn pad(mut values: Vec<f64>, n: usize) -> Vec<f64> {
    values.resize(n, f64::NAN);
    values
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::CurveKind;
    use crate::Project;

    #[test]
    fn resolves_function_and_columns() {
        let mut project = Project::new("p");
        let table_id = project.new_table("t");
        {
            let table = project.table_mut(table_id).unwrap();
            table.columns[0].set_numbers(&[0.0, 1.0]);
            table.columns[0].texts = vec![crate::locale::t("甲").into(), crate::locale::t("乙").into()];
            table.columns[1].set_numbers(&[2.0, 4.0]);
        }
        let graph_id = project.plot_table(table_id, CurveKind::Line).unwrap();
        let curve = project.graph(graph_id).unwrap().layers[0].curves[0].clone();
        let series = resolve_curve(&project, &curve).unwrap();
        assert_eq!(series.pairs(), vec![(0.0, 2.0), (1.0, 4.0)]);

        let curve = Curve {
            name: crate::locale::t("正弦").into(),
            kind: CurveKind::Function,
            style: Default::default(),
            source: DataSource::Function {
                expr: "sin(x)".into(),
                x_min: 0.0,
                x_max: std::f64::consts::PI,
                samples: 3,
            },
            visible: true,
            ..Curve::default()
        };
        let series = resolve_curve(&project, &curve).unwrap();
        assert!((series.y[0]).abs() < 1e-9);
        assert!((series.y[1] - 1.0).abs() < 1e-6);
        assert!(series.y[2].abs() < 1e-6);
    }

    #[test]
    fn polar_parametric_vectors_and_percent_errors() {
        let project = Project::new("p");
        let polar = Curve {
            name: "r".into(),
            kind: CurveKind::Polar,
            source: DataSource::Polar {
                expr: "1".into(),
                theta_min: 0.0,
                theta_max: std::f64::consts::FRAC_PI_2,
                samples: 2,
            },
            ..Curve::default()
        };
        let series = resolve_curve(&project, &polar).unwrap();
        assert!((series.x[0] - 1.0).abs() < 1e-9, "{}", series.x[0]);
        assert!(series.y[0].abs() < 1e-9);
        assert!(series.x[1].abs() < 1e-6);
        assert!((series.y[1] - 1.0).abs() < 1e-6);

        let parametric = Curve {
            name: "p".into(),
            kind: CurveKind::Parametric,
            source: DataSource::Parametric {
                x_expr: "cos(t)".into(),
                y_expr: "sin(t)".into(),
                t_min: 0.0,
                t_max: std::f64::consts::FRAC_PI_2,
                samples: 2,
            },
            ..Curve::default()
        };
        let series = resolve_curve(&project, &parametric).unwrap();
        assert!((series.x[0] - 1.0).abs() < 1e-9);
        assert!(series.y[0].abs() < 1e-9);
        assert!(series.x[1].abs() < 1e-6);
        assert!((series.y[1] - 1.0).abs() < 1e-6);

        let mut project = Project::new("p");
        let table_id = project.new_table("t");
        {
            let table = project.table_mut(table_id).unwrap();
            table.columns.clear();
            table.add_numeric_column("X", crate::table::ColumnMode::X, &[0.0, 1.0]);
            table.add_numeric_column("Y", crate::table::ColumnMode::Y, &[0.0, 0.0]);
            table.add_numeric_column("x2", crate::table::ColumnMode::None, &[1.0, 3.0]);
            table.add_numeric_column("y2", crate::table::ColumnMode::None, &[2.0, 4.0]);
        }
        let graph_id = project.plot_table_mode(table_id, CurveKind::Vector, crate::graph::VectorMode::EndPoint).unwrap();
        let curve = project.graph(graph_id).unwrap().layers[0].curves[0].clone();
        let series = resolve_curve(&project, &curve).unwrap();
        assert!((series.dx[0] - 1.0).abs() < 1e-9 && (series.dy[0] - 2.0).abs() < 1e-9);

        let mut curve = curve.clone();
        curve.vector_mode = crate::graph::VectorMode::AngleMagnitude;
        if let DataSource::Columns { dx, dy, .. } = &mut curve.source {
            // 角度 0、模长 2，以及 π/2、模长 3。
            let table = project.table_mut(table_id).unwrap();
            table.columns[2].set_numbers(&[0.0, std::f64::consts::FRAC_PI_2]);
            table.columns[3].set_numbers(&[2.0, 3.0]);
            let _ = (dx, dy);
        }
        let series = resolve_curve(&project, &curve).unwrap();
        assert!((series.dx[0] - 2.0).abs() < 1e-9, "{}", series.dx[0]);
        assert!(series.dy[0].abs() < 1e-6, "{}", series.dy[0]);
        assert!(series.dx[1].abs() < 1e-6, "{}", series.dx[1]);
        assert!((series.dy[1] - 3.0).abs() < 1e-6, "{}", series.dy[1]);

        let mut curve = Curve {
            name: "e".into(),
            kind: CurveKind::Line,
            source: DataSource::Points {
                x: vec![0.0, 1.0],
                y: vec![10.0, -20.0],
            },
            ..Curve::default()
        };
        curve.error.y = ErrorSource::Percent;
        curve.error.y_percent = 10.0;
        let series = resolve_curve(&project, &curve).unwrap();
        assert!((series.y_err[0] - 1.0).abs() < 1e-9);
        assert!((series.y_err[1] - 2.0).abs() < 1e-9);
        curve.error.y = ErrorSource::StandardDeviation;
        let series = resolve_curve(&project, &curve).unwrap();
        let expected = sample_std(&[10.0, -20.0]);
        assert!((series.y_err[0] - expected).abs() < 1e-9);
    }
}
