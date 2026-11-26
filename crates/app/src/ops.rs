use std::f64::consts::PI;

use sciplot_analysis::{
    cumulative_integral, definite_integral, derivative, fft_filter, fft_spectrum, fit_series, interpolate,
    intersect, invert_matrix, moving_average, pearson, savitzky_golay, summarize, CurveInput, FitModel,
    InterpMethod, IntersectMode, IntersectOptions,
};
use sciplot_core::{
    format_number, resolve_curve, unique_name, Annotation, AnnotationKind, ColumnMode, Curve, CurveKind,
    CurveStyle, DataSource, GraphId, Project,
};

pub fn curve_input(project: &Project, curve: &Curve) -> Result<CurveInput, String> {
    match &curve.source {
        DataSource::Function { expr, x_min, x_max, samples } => Ok(CurveInput::Analytic {
            name: curve.name.clone(),
            expr: expr.clone(),
            x_min: *x_min,
            x_max: *x_max,
            samples: (*samples).max(8),
        }),
        _ => {
            let series = resolve_curve(project, curve)?;
            let points = series.pairs();
            if points.len() < 2 {
                return Err(format!("曲线「{}」的有效点少于 2 个", curve.name));
            }
            Ok(CurveInput::Polyline {
                name: curve.name.clone(),
                points,
            })
        }
    }
}

pub fn horizontal_line(y: f64) -> CurveInput {
    CurveInput::Horizontal { name: format!("y={}", format_number(y)), y }
}

pub fn vertical_line(x: f64) -> CurveInput {
    CurveInput::Vertical { name: format!("x={}", format_number(x)), x }
}

/// 把交点标到图上，并写入一张新表。返回交点个数。
pub fn apply_intersection(
    project: &mut Project,
    graph_id: GraphId,
    layer: usize,
    output_name: &str,
    result: &sciplot_analysis::IntersectOutput,
) -> Result<usize, String> {
    let color = [214, 39, 40, 255];
    {
        let graph = project.graph_mut(graph_id).ok_or("找不到图形")?;
        let layer = graph.layers.get_mut(layer).ok_or("找不到图层")?;
        for point in &result.points {
            layer.annotations.push(Annotation {
                id: 0,
                kind: AnnotationKind::Marker {
                    x: point.x,
                    y: point.y,
                    label: format!("({}, {})", format_number(point.x), format_number(point.y)),
                    color,
                },
            });
        }
    }
    reassign_zero_ids(project, graph_id, layer);

    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let mut sa = Vec::new();
    let mut sb = Vec::new();
    let mut index = Vec::new();
    for (i, point) in result.points.iter().enumerate() {
        index.push((i + 1) as f64);
        xs.push(point.x);
        ys.push(point.y);
        sa.push(point.seg_a.map(|v| v as f64).unwrap_or(f64::NAN));
        sb.push(point.seg_b.map(|v| v as f64).unwrap_or(f64::NAN));
    }
    let table_id = project.new_table(output_name);
    {
        let table = project.table_mut(table_id).unwrap();
        table.columns.clear();
        table.add_numeric_column("序号", ColumnMode::None, &index);
        table.add_numeric_column("X", ColumnMode::X, &xs);
        table.add_numeric_column("Y", ColumnMode::Y, &ys);
        table.add_numeric_column("线段A", ColumnMode::None, &sa);
        table.add_numeric_column("线段B", ColumnMode::None, &sb);
        for (i, point) in result.points.iter().enumerate() {
            if point.seg_a.is_none() {
                table.columns[3].set_text(i, "—");
            }
            if point.seg_b.is_none() {
                table.columns[4].set_text(i, "—");
            }
        }
        table.comment = result.note.clone();
    }
    let count = result.points.len();
    let note = if result.note.is_empty() {
        format!("求得 {count} 个交点，已写入表格「{output_name}」。")
    } else {
        format!("求得 {count} 个交点。{}\n结果表：{output_name}", result.note)
    };
    project.log_line(note);
    Ok(count)
}

fn reassign_zero_ids(project: &mut Project, graph_id: GraphId, layer: usize) {
    let mut zeros = 0usize;
    if let Some(graph) = project.graph(graph_id) {
        if let Some(layer) = graph.layers.get(layer) {
            zeros = layer.annotations.iter().filter(|a| a.id == 0).count();
        }
    }
    let mut ids = Vec::new();
    for _ in 0..zeros {
        ids.push(project.alloc_annotation_id());
    }
    if let Some(graph) = project.graph_mut(graph_id) {
        if let Some(layer) = graph.layers.get_mut(layer) {
            let mut next = ids.into_iter();
            for annotation in &mut layer.annotations {
                if annotation.id == 0 {
                    annotation.id = next.next().unwrap_or(1);
                }
            }
        }
    }
}

pub fn enrich_demo(project: &mut Project) {
    let Some(graph_id) = project.graphs.iter().find(|g| g.name == "正弦与余弦").map(|g| g.id) else {
        return;
    };
    let (a, b) = {
        let Some(graph) = project.graph(graph_id) else { return };
        let Some(layer) = graph.layers.first() else { return };
        if layer.curves.len() < 2 {
            return;
        }
        let Ok(a) = curve_input(project, &layer.curves[0]) else { return };
        let Ok(b) = curve_input(project, &layer.curves[1]) else { return };
        (a, b)
    };
    let result = intersect(
        &a,
        &b,
        &IntersectOptions {
            mode: IntersectMode::Smooth,
            x_min: Some(0.0),
            x_max: Some(2.0 * PI),
            tolerance: 1e-7,
        },
    );
    let name = unique_name(&project.tables.iter().map(|t| t.name.clone()).collect::<Vec<_>>(), "交点_sin_cos");
    apply_intersection(project, graph_id, 0, &name, &result).ok();
}

pub fn xy_of(project: &Project, graph: GraphId, layer: usize, curve: usize) -> Result<(String, Vec<f64>, Vec<f64>), String> {
    let graph = project.graph(graph).ok_or("找不到图形")?;
    let curve = graph.layers.get(layer).ok_or("找不到图层")?.curves.get(curve).ok_or("找不到曲线")?;
    let name = curve.name.clone();
    let series = resolve_curve(project, curve)?;
    let mut x = Vec::new();
    let mut y = Vec::new();
    for (a, b) in series.x.iter().zip(&series.y) {
        if a.is_finite() && b.is_finite() {
            x.push(*a);
            y.push(*b);
        }
    }
    if x.len() < 2 {
        return Err(format!("曲线「{name}」的有效点少于 2 个"));
    }
    Ok((name, x, y))
}

pub fn fit_on_graph(project: &mut Project, graph: GraphId, layer: usize, curve: usize, model: &FitModel) -> Result<String, String> {
    let (name, x, y) = xy_of(project, graph, layer, curve)?;
    let result = fit_series(&x, &y, model)?;
    let xmin = x.iter().copied().fold(f64::INFINITY, f64::min);
    let xmax = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let report = format!("拟合「{name}」\n{}", result.report());
    if let Some(graph) = project.graph_mut(graph) {
        let layer = graph.layers.get_mut(layer).ok_or("找不到图层")?;
        let style = CurveStyle::from_index(layer.curves.len());
        layer.curves.push(Curve {
            name: format!("拟合 {name}"),
            kind: CurveKind::Function,
            style,
            source: DataSource::Function {
                expr: result.expr.clone(),
                x_min: xmin,
                x_max: xmax,
                samples: 240,
            },
            visible: true,
        });
    }
    project.log_line(&report);
    Ok(report)
}

pub fn add_xy_table(project: &mut Project, name: &str, x_name: &str, x: &[f64], y_name: &str, y: &[f64]) -> sciplot_core::TableId {
    let id = project.new_table(name);
    let table = project.table_mut(id).unwrap();
    table.columns.clear();
    table.add_numeric_column(x_name, ColumnMode::X, x);
    table.add_numeric_column(y_name, ColumnMode::Y, y);
    id
}

pub fn derivative_table(project: &mut Project, graph: GraphId, layer: usize, curve: usize) -> Result<sciplot_core::TableId, String> {
    let (name, x, y) = xy_of(project, graph, layer, curve)?;
    let dy = derivative(&x, &y);
    let id = add_xy_table(project, &format!("{name}-微分"), "X", &x, "dY/dX", &dy);
    project.log_line(format!("已计算「{name}」的数值导数，写入新表。"));
    Ok(id)
}

pub fn integral_report(project: &mut Project, graph: GraphId, layer: usize, curve: usize, xmin: f64, xmax: f64) -> Result<String, String> {
    let (name, x, y) = xy_of(project, graph, layer, curve)?;
    let value = definite_integral(&x, &y, xmin, xmax)?;
    let cum = cumulative_integral(&x, &y);
    add_xy_table(project, &format!("{name}-积分"), "X", &x, "累积积分", &cum);
    let text = format!("∫ {name} dx，区间 [{}, {}] = {}", format_number(xmin), format_number(xmax), format_number(value));
    project.log_line(&text);
    Ok(text)
}

pub fn interpolate_table(project: &mut Project, graph: GraphId, layer: usize, curve: usize, method: InterpMethod, n: usize) -> Result<sciplot_core::TableId, String> {
    let (name, x, y) = xy_of(project, graph, layer, curve)?;
    let n = n.clamp(2, 5000);
    let xmin = x[0];
    let xmax = x[x.len() - 1];
    let xs: Vec<f64> = (0..n).map(|i| xmin + (xmax - xmin) * i as f64 / (n as f64 - 1.0)).collect();
    let ys = interpolate(&x, &y, &xs, method)?;
    let id = add_xy_table(project, &format!("{name}-{}", method.label()), "X", &xs, "Y", &ys);
    project.log_line(format!("已用{}插值「{name}」，共 {n} 点。", method.label()));
    Ok(id)
}

pub fn smooth_table(project: &mut Project, graph: GraphId, layer: usize, curve: usize, sg: bool, window: usize, order: usize) -> Result<sciplot_core::TableId, String> {
    let (name, x, y) = xy_of(project, graph, layer, curve)?;
    let smoothed = if sg {
        savitzky_golay(&y, window, order)?
    } else {
        moving_average(&y, window)?
    };
    let label = if sg { "SG平滑" } else { "移动平均" };
    let id = add_xy_table(project, &format!("{name}-{label}"), "X", &x, label, &smoothed);
    project.log_line(format!("已对「{name}」做{label}，窗口 {window}。"));
    Ok(id)
}

pub fn fft_table(project: &mut Project, graph: GraphId, layer: usize, curve: usize) -> Result<sciplot_core::TableId, String> {
    let (name, x, y) = xy_of(project, graph, layer, curve)?;
    let spectrum = fft_spectrum(&x, &y)?;
    let id = project.new_table(&format!("{name}-FFT"));
    {
        let table = project.table_mut(id).unwrap();
        table.columns.clear();
        table.add_numeric_column("频率", ColumnMode::X, &spectrum.frequency);
        table.add_numeric_column("振幅", ColumnMode::Y, &spectrum.amplitude);
        table.add_numeric_column("相位", ColumnMode::Y, &spectrum.phase);
    }
    project.log_line(format!("已计算「{name}」的 FFT。"));
    Ok(id)
}

pub fn filter_table(project: &mut Project, graph: GraphId, layer: usize, curve: usize, cutoff: f64, highpass: bool) -> Result<sciplot_core::TableId, String> {
    let (name, x, y) = xy_of(project, graph, layer, curve)?;
    let filtered = fft_filter(&x, &y, cutoff, highpass)?;
    let label = if highpass { "高通" } else { "低通" };
    let id = add_xy_table(project, &format!("{name}-{label}"), "X", &x, label, &filtered);
    project.log_line(format!("已对「{name}」做 FFT {label}，截止频率 {}。", format_number(cutoff)));
    Ok(id)
}

pub fn column_stats(project: &Project, table: sciplot_core::TableId, column: usize) -> Result<String, String> {
    let table = project.table(table).ok_or("找不到表格")?;
    let column = table.columns.get(column).ok_or("找不到列")?;
    let summary = summarize(&column.finite_values()).ok_or("这一列没有数值")?;
    let text = summary.report(&format!("{} / {}", table.name, column.name));
    Ok(text)
}

pub fn correlation_report(project: &Project, table: sciplot_core::TableId) -> Result<String, String> {
    let table = project.table(table).ok_or("找不到表格")?;
    let cols: Vec<(usize, String, Vec<f64>)> = table
        .columns
        .iter()
        .enumerate()
        .filter_map(|(i, column)| {
            let values = column.finite_values();
            if values.len() >= 2 {
                Some((i, column.name.clone(), values))
            } else {
                None
            }
        })
        .collect();
    if cols.len() < 2 {
        return Err("至少需要两列数值才能算相关".into());
    }
    let mut text = format!("相关性：{}\n", table.name);
    for i in 0..cols.len() {
        for j in (i + 1)..cols.len() {
            match pearson(&cols[i].2, &cols[j].2) {
                Ok(r) => text.push_str(&format!("  {} 与 {}：r = {}\n", cols[i].1, cols[j].1, format_number(r))),
                Err(err) => text.push_str(&format!("  {} 与 {}：{err}\n", cols[i].1, cols[j].1)),
            }
        }
    }
    Ok(text)
}

pub fn invert_into_new(project: &mut Project, matrix: sciplot_core::MatrixId) -> Result<sciplot_core::MatrixId, String> {
    let source = project.matrix(matrix).ok_or("找不到矩阵")?.clone();
    let inverse = invert_matrix(&source)?;
    let id = project.insert_matrix(inverse);
    project.log_line(format!("已求「{}」的逆矩阵。", source.name));
    Ok(id)
}

pub fn transpose_into_new(project: &mut Project, matrix: sciplot_core::MatrixId) -> Result<sciplot_core::MatrixId, String> {
    let source = project.matrix(matrix).ok_or("找不到矩阵")?;
    let transposed = source.transpose();
    let name = source.name.clone();
    let id = project.insert_matrix(transposed);
    project.log_line(format!("已转置「{name}」。"));
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sciplot_core::demo_project;

    #[test]
    fn demo_intersection_finds_two_points() {
        let mut project = demo_project();
        enrich_demo(&mut project);
        let table = project.tables.iter().find(|t| t.name.contains("交点")).expect("交点表");
        assert!(table.nrows() >= 2, "rows={}", table.nrows());
        let graph = project.graphs.iter().find(|g| g.name == "正弦与余弦").unwrap();
        let markers = graph.layers[0].annotations.iter().filter(|a| matches!(a.kind, AnnotationKind::Marker { .. })).count();
        assert!(markers >= 2);
    }
}
