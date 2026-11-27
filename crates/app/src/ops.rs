use std::f64::consts::PI;

use sciplot_analysis::{
    autocorrelate, convolve, correlate, cumulative_integral, deconvolve, definite_integral, derivative, fft_filter_band,
    fft_forward_complex, fft_inverse, fft_spectrum, fit_with, interpolate, intersect, invert_matrix, moving_average,
    pearson, savitzky_golay, summarize, CurveInput, FftBand, FftShape, FitModel, FitOptions, InterpMethod, IntersectMode,
    IntersectOptions, Weighting,
};
use sciplot_core::{
    format_number, resolve_curve, unique_name, Annotation, AnnotationKind, ColumnMode, Curve,
    CurveKind, CurveStyle, DataSource, GraphId, Project,
};

pub fn curve_input(project: &Project, curve: &Curve) -> Result<CurveInput, String> {
    match &curve.source {
        DataSource::Function {
            expr,
            x_min,
            x_max,
            samples,
        } => Ok(CurveInput::Analytic {
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
                return Err(sciplot_core::trf!(
                    "曲线「{}」的有效点少于 2 个",
                    "Curve \"{}\" has fewer than 2 finite points",
                    curve.name
                ));
            }
            Ok(CurveInput::Polyline {
                name: curve.name.clone(),
                points,
            })
        }
    }
}

pub fn horizontal_line(y: f64) -> CurveInput {
    CurveInput::Horizontal {
        name: format!("y={}", format_number(y)),
        y,
    }
}

pub fn vertical_line(x: f64) -> CurveInput {
    CurveInput::Vertical {
        name: format!("x={}", format_number(x)),
        x,
    }
}

/// 把交点标到图上，并写入一张新表。没有交点时不建表、不标点，返回 0。
pub fn apply_intersection(
    project: &mut Project,
    graph_id: GraphId,
    layer: usize,
    output_name: &str,
    result: &sciplot_analysis::IntersectOutput,
) -> Result<usize, String> {
    let count = result.points.len();
    if count == 0 {
        let why = if result.note.is_empty() {
            sciplot_core::t("没有找到交点").to_string()
        } else {
            result.note.clone()
        };
        project.log_line(sciplot_core::trf!(
            "{why}，没有新建表格。",
            "{why} No table was created.",
            why = why
        ));
        return Ok(0);
    }
    let color = [214, 39, 40, 255];
    {
        let graph = project
            .graph_mut(graph_id)
            .ok_or(sciplot_core::t("找不到图形"))?;
        let layer = graph
            .layers
            .get_mut(layer)
            .ok_or(sciplot_core::t("找不到图层"))?;
        for point in &result.points {
            layer.annotations.push(Annotation {
                id: 0,
                kind: AnnotationKind::Marker {
                    x: point.x,
                    y: point.y,
                    label: format!(
                        "({}, {})",
                        sciplot_core::format_short(point.x),
                        sciplot_core::format_short(point.y)
                    ),
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
        table.add_numeric_column(sciplot_core::t("序号"), ColumnMode::None, &index);
        table.add_numeric_column("X", ColumnMode::X, &xs);
        table.add_numeric_column("Y", ColumnMode::Y, &ys);
        table.add_numeric_column(sciplot_core::t("线段A"), ColumnMode::None, &sa);
        table.add_numeric_column(sciplot_core::t("线段B"), ColumnMode::None, &sb);
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
    let note = if result.note.is_empty() {
        sciplot_core::trf!(
            "求得 {count} 个交点，已写入表格「{output_name}」。",
            "Found {count} intersections, written to \"{output_name}\".",
            count = count,
            output_name = output_name
        )
    } else {
        sciplot_core::trf!(
            "求得 {count} 个交点。{}\n结果表：{output_name}",
            "Found {count} intersections. {}\nTable: {output_name}",
            result.note,
            count = count,
            output_name = output_name
        )
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
    let Some(graph_id) = project
        .graphs
        .iter()
        .find(|g| g.name == "正弦与余弦" || g.name == "Sine and cosine")
        .map(|g| g.id)
    else {
        return;
    };
    let (a, b) = {
        let Some(graph) = project.graph(graph_id) else {
            return;
        };
        let Some(layer) = graph.layers.first() else {
            return;
        };
        if layer.curves.len() < 2 {
            return;
        }
        let Ok(a) = curve_input(project, &layer.curves[0]) else {
            return;
        };
        let Ok(b) = curve_input(project, &layer.curves[1]) else {
            return;
        };
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
    let name = unique_name(
        &project
            .tables
            .iter()
            .map(|t| t.name.clone())
            .collect::<Vec<_>>(),
        sciplot_core::t("交点_sin_cos"),
    );
    apply_intersection(project, graph_id, 0, &name, &result).ok();
}

pub fn xy_of(
    project: &Project,
    graph: GraphId,
    layer: usize,
    curve: usize,
) -> Result<(String, Vec<f64>, Vec<f64>), String> {
    let graph = project.graph(graph).ok_or(sciplot_core::t("找不到图形"))?;
    let curve = graph
        .layers
        .get(layer)
        .ok_or(sciplot_core::t("找不到图层"))?
        .curves
        .get(curve)
        .ok_or(sciplot_core::t("找不到曲线"))?;
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
        return Err(sciplot_core::trf!(
            "曲线「{name}」的有效点少于 2 个",
            "Curve \"{name}\" has fewer than 2 finite points",
            name = name
        ));
    }
    Ok((name, x, y))
}

#[allow(clippy::too_many_arguments)]
pub fn fit_on_graph(
    project: &mut Project,
    graph: GraphId,
    layer: usize,
    curve: usize,
    model: &FitModel,
    x_min: Option<f64>,
    x_max: Option<f64>,
    options: &FitOptions,
) -> Result<String, String> {
    let (name, x, y, y_err) = aligned_xy(project, graph, layer, curve)?;
    let column = if options.column.is_empty() {
        vec![f64::NAN; x.len()]
    } else if options.column.len() == x.len() {
        options.column.clone()
    } else {
        return Err(sciplot_core::tr!(
            "权重列和曲线的行数对不上",
            "The weight column does not line up with the curve"
        )
        .into());
    };
    let (x, y, y_err, column) = restrict_rows(&x, &y, &y_err, &column, x_min, x_max)?;
    let mut options = options.clone();
    match options.weighting {
        Weighting::Instrumental => {
            if y_err.iter().all(|value| !value.is_finite() || *value == 0.0) {
                return Err(sciplot_core::tr!(
                    "这条曲线没有 Y 误差，不能用仪器权重",
                    "This curve has no Y errors, so instrumental weights cannot be used"
                )
                .into());
            }
            options.errors = y_err;
        }
        Weighting::Dataset | Weighting::Direct => {
            if column.iter().all(|value| !value.is_finite()) {
                return Err(sciplot_core::tr!(
                    "请选择用来加权的数据列",
                    "Choose a data column to use as weights"
                )
                .into());
            }
            options.column = column;
        }
        Weighting::None | Weighting::Statistical => {}
    }
    let result = fit_with(&x, &y, model, &options)?;
    let xmin = x.iter().copied().fold(f64::INFINITY, f64::min);
    let xmax = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let report = sciplot_core::trf!(
        "拟合「{name}」\n{}",
        "Fit \"{name}\"\n{}",
        result.report(),
        name = name
    );
    let span = if x_min.is_none() && x_max.is_none() {
        sciplot_core::t("全区间").to_string()
    } else {
        format!("{}–{}", format_number(xmin), format_number(xmax))
    };
    if let Some(graph) = project.graph_mut(graph) {
        let layer = graph
            .layers
            .get_mut(layer)
            .ok_or(sciplot_core::t("找不到图层"))?;
        let style = CurveStyle::from_index(layer.curves.len());
        let base = sciplot_core::trf!(
            "拟合 {name}（{span}）",
            "Fit {name} ({span})",
            name = name,
            span = span
        );
        let existing: Vec<String> = layer
            .curves
            .iter()
            .map(|curve| curve.name.clone())
            .collect();
        layer.curves.push(Curve {
            name: unique_name(&existing, &base),
            kind: CurveKind::Function,
            style,
            source: DataSource::Function {
                expr: result.expr.clone(),
                x_min: xmin,
                x_max: xmax,
                samples: 240,
            },
            visible: true,
            ..Curve::default()
        });
    }
    project.log_line(&report);
    Ok(report)
}

#[allow(clippy::type_complexity)]
fn aligned_xy(
    project: &Project,
    graph: GraphId,
    layer: usize,
    curve: usize,
) -> Result<(String, Vec<f64>, Vec<f64>, Vec<f64>), String> {
    let graph = project.graph(graph).ok_or(sciplot_core::t("找不到图形"))?;
    let curve = graph
        .layers
        .get(layer)
        .ok_or(sciplot_core::t("找不到图层"))?
        .curves
        .get(curve)
        .ok_or(sciplot_core::t("找不到曲线"))?;
    let name = curve.name.clone();
    let series = resolve_curve(project, curve)?;
    if series.x.len() < 2 {
        return Err(sciplot_core::trf!(
            "曲线「{name}」的有效点少于 2 个",
            "Curve \"{name}\" has fewer than 2 finite points",
            name = name
        ));
    }
    let y_err = if series.y_err.len() == series.x.len() {
        series.y_err
    } else {
        vec![f64::NAN; series.x.len()]
    };
    Ok((name, series.x, series.y, y_err))
}

/// 按曲线所在表格的行，取出和曲线对齐的一列。曲线不是表格列时不能用。
pub fn weight_column(
    project: &Project,
    graph: GraphId,
    layer: usize,
    curve: usize,
    column_name: &str,
) -> Result<Vec<f64>, String> {
    let graph = project.graph(graph).ok_or(sciplot_core::t("找不到图形"))?;
    let curve = graph
        .layers
        .get(layer)
        .ok_or(sciplot_core::t("找不到图层"))?
        .curves
        .get(curve)
        .ok_or(sciplot_core::t("找不到曲线"))?;
    let DataSource::Columns { table, .. } = &curve.source else {
        return Err(sciplot_core::tr!(
            "这条曲线不是表格列，不能用数据列做权重",
            "This curve is not table columns, so a data column cannot weight it"
        )
        .into());
    };
    let table = project.table(*table).ok_or(sciplot_core::t("找不到表格"))?;
    let column = table
        .columns
        .iter()
        .find(|column| column.name == column_name)
        .ok_or_else(|| {
            sciplot_core::trf!(
                "找不到权重列「{name}」",
                "Weight column \"{name}\" was not found",
                name = column_name
            )
        })?;
    let series = resolve_curve(project, curve)?;
    Ok((0..series.x.len()).map(|row| column.number_at(row)).collect())
}

#[allow(clippy::type_complexity)]
fn restrict_rows(
    x: &[f64],
    y: &[f64],
    y_err: &[f64],
    column: &[f64],
    x_min: Option<f64>,
    x_max: Option<f64>,
) -> Result<(Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>), String> {
    let lo = x_min.unwrap_or(f64::NEG_INFINITY);
    let hi = x_max.unwrap_or(f64::INFINITY);
    let (lo, hi) = (lo.min(hi), lo.max(hi));
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let mut errs = Vec::new();
    let mut weights = Vec::new();
    for index in 0..x.len() {
        let x = x[index];
        if (x_min.is_some() || x_max.is_some()) && !(x >= lo && x <= hi) {
            continue;
        }
        xs.push(x);
        ys.push(y.get(index).copied().unwrap_or(f64::NAN));
        errs.push(y_err.get(index).copied().unwrap_or(f64::NAN));
        weights.push(column.get(index).copied().unwrap_or(f64::NAN));
    }
    if xs.len() < 2 {
        return Err(sciplot_core::trf!(
            "区间 [{}, {}] 里的有效点少于 2 个",
            "Fewer than 2 finite points in [{}, {}]",
            format_number(lo),
            format_number(hi)
        ));
    }
    Ok((xs, ys, errs, weights))
}

pub fn add_xy_table(
    project: &mut Project,
    name: &str,
    x_name: &str,
    x: &[f64],
    y_name: &str,
    y: &[f64],
) -> sciplot_core::TableId {
    let id = project.new_table(name);
    let table = project.table_mut(id).unwrap();
    table.columns.clear();
    table.add_numeric_column(x_name, ColumnMode::X, x);
    table.add_numeric_column(y_name, ColumnMode::Y, y);
    id
}

pub fn derivative_table(
    project: &mut Project,
    graph: GraphId,
    layer: usize,
    curve: usize,
) -> Result<sciplot_core::TableId, String> {
    let (name, x, y) = xy_of(project, graph, layer, curve)?;
    let dy = derivative(&x, &y);
    let id = add_xy_table(
        project,
        &sciplot_core::trf!("{name}-微分", "{name}-derivative", name = name),
        "X",
        &x,
        "dY/dX",
        &dy,
    );
    project.log_line(sciplot_core::trf!(
        "已计算「{name}」的数值导数，写入新表。",
        "Computed the derivative of \"{name}\" into a new table.",
        name = name
    ));
    Ok(id)
}

pub fn integral_report(
    project: &mut Project,
    graph: GraphId,
    layer: usize,
    curve: usize,
    xmin: f64,
    xmax: f64,
) -> Result<String, String> {
    let (name, x, y) = xy_of(project, graph, layer, curve)?;
    let value = definite_integral(&x, &y, xmin, xmax)?;
    let cum = cumulative_integral(&x, &y);
    add_xy_table(
        project,
        &sciplot_core::trf!("{name}-积分", "{name}-integral", name = name),
        "X",
        &x,
        sciplot_core::t("累积积分"),
        &cum,
    );
    let text = sciplot_core::trf!(
        "∫ {name} dx，区间 [{}, {}] = {}",
        "∫ {name} dx over [{}, {}] = {}",
        format_number(xmin),
        format_number(xmax),
        format_number(value),
        name = name
    );
    project.log_line(&text);
    Ok(text)
}

pub fn interpolate_table(
    project: &mut Project,
    graph: GraphId,
    layer: usize,
    curve: usize,
    method: InterpMethod,
    n: usize,
) -> Result<sciplot_core::TableId, String> {
    let (name, x, y) = xy_of(project, graph, layer, curve)?;
    let n = n.clamp(2, 5000);
    let xmin = x[0];
    let xmax = x[x.len() - 1];
    let xs: Vec<f64> = (0..n)
        .map(|i| xmin + (xmax - xmin) * i as f64 / (n as f64 - 1.0))
        .collect();
    let ys = interpolate(&x, &y, &xs, method)?;
    let id = add_xy_table(
        project,
        &format!("{name}-{}", method.label()),
        "X",
        &xs,
        "Y",
        &ys,
    );
    project.log_line(sciplot_core::trf!(
        "已用{}插值「{name}」，共 {n} 点。",
        "Interpolated \"{name}\" with {}, {n} points.",
        method.label(),
        name = name,
        n = n
    ));
    Ok(id)
}

pub fn smooth_table(
    project: &mut Project,
    graph: GraphId,
    layer: usize,
    curve: usize,
    sg: bool,
    window: usize,
    order: usize,
) -> Result<sciplot_core::TableId, String> {
    let (name, x, y) = xy_of(project, graph, layer, curve)?;
    let smoothed = if sg {
        savitzky_golay(&y, window, order)?
    } else {
        moving_average(&y, window)?
    };
    let label = if sg {
        sciplot_core::t("SG平滑")
    } else {
        sciplot_core::t("移动平均")
    };
    let id = add_xy_table(
        project,
        &format!("{name}-{label}"),
        "X",
        &x,
        label,
        &smoothed,
    );
    project.log_line(sciplot_core::trf!(
        "已对「{name}」做{label}，窗口 {window}。",
        "Applied {label} to \"{name}\", window {window}.",
        name = name,
        label = label,
        window = window
    ));
    Ok(id)
}

pub fn fft_table(
    project: &mut Project,
    graph: GraphId,
    layer: usize,
    curve: usize,
) -> Result<sciplot_core::TableId, String> {
    let (name, x, y) = xy_of(project, graph, layer, curve)?;
    let spectrum = fft_spectrum(&x, &y)?;
    let id = project.new_table(&format!("{name}-FFT"));
    {
        let table = project.table_mut(id).unwrap();
        table.columns.clear();
        table.add_numeric_column(sciplot_core::t("频率"), ColumnMode::X, &spectrum.frequency);
        table.add_numeric_column(sciplot_core::t("振幅"), ColumnMode::Y, &spectrum.amplitude);
        table.add_numeric_column(sciplot_core::t("相位"), ColumnMode::Y, &spectrum.phase);
    }
    project.log_line(sciplot_core::trf!(
        "已计算「{name}」的 FFT。",
        "Computed the FFT of \"{name}\".",
        name = name
    ));
    Ok(id)
}

pub fn filter_table(
    project: &mut Project,
    graph: GraphId,
    layer: usize,
    curve: usize,
    band: FftBand,
) -> Result<sciplot_core::TableId, String> {
    let (name, x, y) = xy_of(project, graph, layer, curve)?;
    let filtered = fft_filter_band(&x, &y, band)?;
    let label = band.label();
    let id = add_xy_table(
        project,
        &format!("{name}-{label}"),
        "X",
        &x,
        label,
        &filtered,
    );
    project.log_line(sciplot_core::trf!(
        "已对「{name}」做 FFT {label}。",
        "FFT {label} on \"{name}\".",
        name = name,
        label = label
    ));
    Ok(id)
}

/// 完整 FFT 或逆 FFT。正向结果含实部、虚部、振幅、相位。逆变换读实部和虚部，写回时间序列。
/// 采样间隔留空时用 X 的中位正间隔；逆变换如果输入已经是频率轴，需要自己填原来的采样间隔。
#[allow(clippy::too_many_arguments)]
pub fn fft_complex_table(
    project: &mut Project,
    graph: GraphId,
    layer: usize,
    curve: usize,
    inverse: bool,
    shape: FftShape,
    imag_name: Option<&str>,
    dt: Option<f64>,
) -> Result<sciplot_core::TableId, String> {
    let (name, x, y, y_err) = aligned_xy(project, graph, layer, curve)?;
    let imag = match imag_name.filter(|name| !name.is_empty()) {
        Some(column) => weight_column(project, graph, layer, curve, column)?,
        None => vec![0.0; x.len()],
    };
    let (x, y, _, imag) = restrict_rows(&x, &y, &y_err, &imag, None, None)?;
    let mut x_kept = Vec::new();
    let mut y_kept = Vec::new();
    let mut imag_kept = Vec::new();
    for index in 0..x.len() {
        if x[index].is_finite() && y[index].is_finite() && imag[index].is_finite() {
            x_kept.push(x[index]);
            y_kept.push(y[index]);
            imag_kept.push(imag[index]);
        }
    }
    if y_kept.len() < 4 {
        return Err(sciplot_core::tr!(
            "FFT 至少需要 4 个有限点",
            "FFT needs at least 4 finite points"
        )
        .into());
    }
    let dt = match dt {
        Some(dt) if dt.is_finite() && dt > 0.0 => dt,
        Some(_) => {
            return Err(sciplot_core::tr!("采样间隔必须是正数", "The sample spacing must be positive").into());
        }
        None => median_step(&x_kept)?,
    };
    let y = y_kept;
    let imag = imag_kept;
    let series = if inverse {
        fft_inverse(&y, &imag, dt, shape)?
    } else {
        fft_forward_complex(&y, &imag, dt, shape)?
    };
    let id = project.new_table(&format!("{name}-{}", if inverse { "IFFT" } else { "FFT" }));
    {
        let table = project.table_mut(id).unwrap();
        table.columns.clear();
        let axis = if inverse {
            sciplot_core::t("时间")
        } else {
            sciplot_core::t("频率")
        };
        table.add_numeric_column(axis, ColumnMode::X, &series.axis);
        table.add_numeric_column(sciplot_core::t("实部"), ColumnMode::Y, &series.real);
        table.add_numeric_column(sciplot_core::t("虚部"), ColumnMode::Y, &series.imag);
        if !inverse {
            table.add_numeric_column(sciplot_core::t("振幅"), ColumnMode::Y, &series.amplitude);
            table.add_numeric_column(sciplot_core::t("相位"), ColumnMode::Y, &series.phase);
        }
    }
    project.log_line(if inverse {
        sciplot_core::trf!(
            "已计算「{name}」的逆 FFT。",
            "Computed the inverse FFT of \"{name}\".",
            name = name
        )
    } else {
        sciplot_core::trf!("已计算「{name}」的 FFT。", "Computed the FFT of \"{name}\".", name = name)
    });
    Ok(id)
}

fn median_step(x: &[f64]) -> Result<f64, String> {
    let mut steps: Vec<f64> = x
        .windows(2)
        .filter_map(|pair| {
            let step = pair[1] - pair[0];
            (step.is_finite() && step > 0.0).then_some(step)
        })
        .collect();
    if steps.is_empty() {
        return Err(sciplot_core::tr!(
            "无法从 X 得到采样间隔，请填写一个正数",
            "Could not read a sample spacing from X. Enter a positive number"
        )
        .into());
    }
    steps.sort_by(f64::total_cmp);
    Ok(steps[steps.len() / 2])
}

/// 两列的线性卷积、反卷积，或信号的互相关 / 自相关。横轴是样本序号，不是 Pearson 相关。
pub fn signal_table(
    project: &mut Project,
    table: sciplot_core::TableId,
    kind: &str,
    column_a: usize,
    column_b: Option<usize>,
) -> Result<sciplot_core::TableId, String> {
    let (name_a, a) = signal_column(project, table, column_a)?;
    let (name_b, b) = if kind == "autocorrelate" {
        (name_a.clone(), a.clone())
    } else {
        signal_column(project, table, column_b.ok_or_else(|| {
            sciplot_core::tr!("请再选择一列", "Choose a second column").to_string()
        })?)?
    };
    let (x_name, x, y_name, y, title) = match kind {
        "convolve" => {
            let y = convolve(&a, &b)?;
            (
                sciplot_core::t("序号").to_string(),
                (0..y.len()).map(|i| i as f64).collect(),
                sciplot_core::t("卷积").to_string(),
                y,
                format!("{name_a}*{name_b}"),
            )
        }
        "deconvolve" => {
            let y = deconvolve(&a, &b)?;
            (
                sciplot_core::t("序号").to_string(),
                (0..y.len()).map(|i| i as f64).collect(),
                sciplot_core::t("反卷积").to_string(),
                y,
                format!("{name_a}/{name_b}"),
            )
        }
        "correlate" | "autocorrelate" => {
            let values = if kind == "autocorrelate" {
                autocorrelate(&a)?
            } else {
                correlate(&a, &b)?
            };
            (
                sciplot_core::t("滞后").to_string(),
                values.lag,
                sciplot_core::t("协方差").to_string(),
                values.value,
                if kind == "autocorrelate" {
                    format!("{name_a}-auto")
                } else {
                    format!("{name_a}×{name_b}")
                },
            )
        }
        _ => {
            return Err(sciplot_core::tr!("不认识的信号处理", "Unknown signal operation").into());
        }
    };
    let id = add_xy_table(project, &title, &x_name, &x, &y_name, &y);
    project.log_line(sciplot_core::trf!(
        "已计算「{title}」。",
        "Computed \"{title}\".",
        title = title
    ));
    Ok(id)
}

fn signal_column(project: &Project, table: sciplot_core::TableId, column: usize) -> Result<(String, Vec<f64>), String> {
    let table = project.table(table).ok_or(sciplot_core::t("找不到表格"))?;
    let column = table
        .columns
        .get(column)
        .ok_or_else(|| sciplot_core::tr!("找不到列", "Column not found").to_string())?;
    let mut values = Vec::new();
    for row in 0..column.len() {
        let value = column.number_at(row);
        if !value.is_finite() {
            return Err(sciplot_core::trf!(
                "列「{name}」里有空单元格或非数值，信号处理需要一整列有限数值",
                "Column \"{name}\" has an empty or non-numeric cell. Signal processing needs a finite column",
                name = column.name
            ));
        }
        values.push(value);
    }
    if values.len() < 2 {
        return Err(sciplot_core::tr!("这一列至少需要两个数值", "The column needs at least two numbers").into());
    }
    Ok((column.name.clone(), values))
}

fn stat_labels() -> Vec<String> {
    vec![
        sciplot_core::t("个数").into(),
        sciplot_core::t("最小值").into(),
        sciplot_core::t("最大值").into(),
        sciplot_core::t("求和").into(),
        sciplot_core::t("均值").into(),
        sciplot_core::t("中位数").into(),
        sciplot_core::t("标准差").into(),
        sciplot_core::t("方差").into(),
    ]
}

fn summary_values(summary: &sciplot_analysis::Summary) -> [f64; 8] {
    [
        summary.n as f64,
        summary.min,
        summary.max,
        summary.sum,
        summary.mean,
        summary.median,
        summary.stdev,
        summary.variance,
    ]
}

fn numeric_columns(table: &sciplot_core::Table, columns: &[usize]) -> Vec<usize> {
    let picked: Vec<usize> = if columns.is_empty() {
        (0..table.columns.len()).collect()
    } else {
        columns.to_vec()
    };
    picked
        .into_iter()
        .filter(|index| {
            table
                .columns
                .get(*index)
                .is_some_and(|column| column.ty == sciplot_core::ColumnType::Numeric)
        })
        .collect()
}

/// 把所选数值列的描述统计写成一张新表。行是统计量，列是原来的每一列，包含方差。
pub fn column_stats_table(
    project: &mut Project,
    table: sciplot_core::TableId,
    columns: &[usize],
) -> Result<sciplot_core::TableId, String> {
    let source = project.table(table).ok_or(sciplot_core::t("找不到表格"))?.clone();
    let indexes = numeric_columns(&source, columns);
    if indexes.is_empty() {
        return Err(sciplot_core::t("没有可统计的数值列").into());
    }
    let mut summaries = Vec::new();
    for index in indexes {
        let column = &source.columns[index];
        let Some(summary) = summarize(&column.finite_values()) else {
            continue;
        };
        summaries.push((column.name.clone(), summary));
    }
    if summaries.is_empty() {
        return Err(sciplot_core::t("这些列没有数值").into());
    }
    let id = project.new_table(&sciplot_core::trf!("列统计_{}", "Column stats {}", source.name));
    let stat_table = project.table_mut(id).ok_or(sciplot_core::t("找不到表格"))?;
    stat_table.columns.clear();
    stat_table.add_column(sciplot_core::t("统计量"), sciplot_core::ColumnMode::X);
    stat_table.columns[0].ty = sciplot_core::ColumnType::Text;
    for (row, label) in stat_labels().iter().enumerate() {
        stat_table.columns[0].set_text(row, label);
    }
    for (name, summary) in summaries {
        let index = stat_table.add_column(name, sciplot_core::ColumnMode::Y);
        stat_table.columns[index].set_numbers(&summary_values(&summary));
    }
    project.log_line(sciplot_core::trf!(
        "已把「{}」的列统计写入新表。",
        "Wrote column statistics for \"{}\" into a new table.",
        source.name
    ));
    Ok(id)
}

/// 按行汇总所选数值列，每个原行在新表里占一行。
pub fn row_stats_table(
    project: &mut Project,
    table: sciplot_core::TableId,
    columns: &[usize],
) -> Result<sciplot_core::TableId, String> {
    let source = project.table(table).ok_or(sciplot_core::t("找不到表格"))?.clone();
    let indexes = numeric_columns(&source, columns);
    if indexes.is_empty() {
        return Err(sciplot_core::t("没有可统计的数值列").into());
    }
    let nrows = source.nrows();
    if nrows == 0 {
        return Err(sciplot_core::t("表格没有数据行").into());
    }
    let id = project.new_table(&sciplot_core::trf!("行统计_{}", "Row stats {}", source.name));
    let stat_table = project.table_mut(id).ok_or(sciplot_core::t("找不到表格"))?;
    stat_table.columns.clear();
    stat_table.add_column(sciplot_core::t("行"), sciplot_core::ColumnMode::X);
    for label in stat_labels() {
        stat_table.add_column(label, sciplot_core::ColumnMode::Y);
    }
    let row_index: Vec<f64> = (1..=nrows).map(|n| n as f64).collect();
    stat_table.columns[0].set_numbers(&row_index);
    for row in 0..nrows {
        let values: Vec<f64> = indexes
            .iter()
            .filter_map(|index| {
                let value = source.columns[*index].number_at(row);
                value.is_finite().then_some(value)
            })
            .collect();
        if let Some(summary) = summarize(&values) {
            for (offset, value) in summary_values(&summary).into_iter().enumerate() {
                stat_table.columns[offset + 1].set_text(row, &format_number(value));
            }
        } else {
            stat_table.columns[1].set_text(row, "0");
        }
    }
    project.log_line(sciplot_core::trf!(
        "已把「{}」的行统计写入新表。",
        "Wrote row statistics for \"{}\" into a new table.",
        source.name
    ));
    Ok(id)
}

pub fn column_stats(
    project: &Project,
    table: sciplot_core::TableId,
    column: usize,
) -> Result<String, String> {
    let table = project.table(table).ok_or(sciplot_core::t("找不到表格"))?;
    let column = table
        .columns
        .get(column)
        .ok_or(sciplot_core::t("找不到列"))?;
    let summary = summarize(&column.finite_values()).ok_or(sciplot_core::t("这一列没有数值"))?;
    let text = summary.report(&format!("{} / {}", table.name, column.name));
    Ok(text)
}

pub fn correlation_report(
    project: &Project,
    table: sciplot_core::TableId,
) -> Result<String, String> {
    let table = project.table(table).ok_or(sciplot_core::t("找不到表格"))?;
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
        return Err(sciplot_core::t("至少需要两列数值才能算相关").into());
    }
    let mut text = sciplot_core::trf!("相关性：{}\n", "Correlation: {}\n", table.name);
    for i in 0..cols.len() {
        for j in (i + 1)..cols.len() {
            match pearson(&cols[i].2, &cols[j].2) {
                Ok(r) => text.push_str(&sciplot_core::trf!(
                    "  {} 与 {}：r = {}\n",
                    "  {} vs {}: r = {}\n",
                    cols[i].1,
                    cols[j].1,
                    format_number(r)
                )),
                Err(err) => text.push_str(&sciplot_core::trf!(
                    "  {} 与 {}：{err}\n",
                    "  {} vs {}: {err}\n",
                    cols[i].1,
                    cols[j].1,
                    err = err
                )),
            }
        }
    }
    Ok(text)
}

pub fn invert_into_new(
    project: &mut Project,
    matrix: sciplot_core::MatrixId,
) -> Result<sciplot_core::MatrixId, String> {
    let source = project
        .matrix(matrix)
        .ok_or(sciplot_core::t("找不到矩阵"))?
        .clone();
    let inverse = invert_matrix(&source)?;
    let id = project.insert_matrix(inverse);
    project.log_line(sciplot_core::trf!(
        "已求「{}」的逆矩阵。",
        "Inverted \"{}\".",
        source.name
    ));
    Ok(id)
}

pub fn transpose_into_new(
    project: &mut Project,
    matrix: sciplot_core::MatrixId,
) -> Result<sciplot_core::MatrixId, String> {
    let source = project
        .matrix(matrix)
        .ok_or(sciplot_core::t("找不到矩阵"))?;
    let transposed = source.transpose();
    let name = source.name.clone();
    let id = project.insert_matrix(transposed);
    project.log_line(sciplot_core::trf!(
        "已转置「{name}」。",
        "Transposed \"{name}\".",
        name = name
    ));
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
        let table = project
            .tables
            .iter()
            .find(|t| t.name.contains(sciplot_core::t("交点")))
            .expect("交点表");
        assert!(table.nrows() >= 2, "rows={}", table.nrows());
        let graph = project
            .graphs
            .iter()
            .find(|g| g.name == sciplot_core::t("正弦与余弦"))
            .unwrap();
        let markers = graph.layers[0]
            .annotations
            .iter()
            .filter(|a| matches!(a.kind, AnnotationKind::Marker { .. }))
            .count();
        assert!(markers >= 2);
    }

    #[test]
    fn empty_intersection_does_not_create_a_table() {
        let mut project = demo_project();
        let before = project.tables.len();
        let graph_id = project
            .graphs
            .iter()
            .find(|graph| graph.name == sciplot_core::t("正弦与余弦"))
            .unwrap()
            .id;
        let result = sciplot_analysis::IntersectOutput {
            points: Vec::new(),
            note: sciplot_core::t("区间内没有交点").into(),
        };
        let count = apply_intersection(
            &mut project,
            graph_id,
            0,
            sciplot_core::t("交点_空"),
            &result,
        )
        .unwrap();
        assert_eq!(count, 0);
        assert_eq!(project.tables.len(), before);
        assert!(project.log.iter().any(|line| line.contains("没有新建表格")));
        let graph = project
            .graphs
            .iter()
            .find(|graph| graph.id == graph_id)
            .unwrap();
        assert!(graph.layers[0]
            .annotations
            .iter()
            .all(|annotation| !matches!(annotation.kind, AnnotationKind::Marker { .. })));
    }

    #[test]
    fn fit_uses_the_requested_interval() {
        let mut project = sciplot_core::demo_project();
        let graph = project
            .graphs
            .iter()
            .find(|graph| graph.name == sciplot_core::t("正弦与余弦"))
            .unwrap()
            .id;
        let wide = fit_on_graph(
            &mut project,
            graph,
            0,
            0,
            &FitModel::Linear,
            None,
            None,
            &FitOptions::default(),
        )
        .unwrap();
        let curves_after_wide = project.graph(graph).unwrap().layers[0].curves.len();
        let narrow = fit_on_graph(
            &mut project,
            graph,
            0,
            0,
            &FitModel::Linear,
            Some(0.0),
            Some(1.0),
            &FitOptions::default(),
        )
        .unwrap();
        assert!(
            wide.contains("n = 200") || wide.contains("n = 200"),
            "{wide}"
        );
        assert!(
            narrow.contains("n = ") && !narrow.contains("n = 200"),
            "{narrow}"
        );
        let names: Vec<String> = project.graph(graph).unwrap().layers[0]
            .curves
            .iter()
            .map(|curve| curve.name.clone())
            .collect();
        let tagged = names
            .iter()
            .filter(|name| name.contains('–') || name.contains(sciplot_core::t("全区间")))
            .count();
        assert!(tagged >= 2, "{names:?}");
        assert!(names.last().unwrap().contains('–'), "{names:?}");
        let curve = project.graph(graph).unwrap().layers[0]
            .curves
            .last()
            .unwrap();
        match &curve.source {
            sciplot_core::DataSource::Function { x_min, x_max, .. } => {
                assert!(*x_min >= -1e-6 && *x_max <= 1.0 + 1e-6, "{x_min} {x_max}");
            }
            other => panic!("拟合线应是函数，实际 {other:?}"),
        }
        assert!(project.graph(graph).unwrap().layers[0].curves.len() > curves_after_wide);
    }

    #[test]
    fn convolution_writes_the_linear_result_and_correlation_is_not_pearson() {
        let mut project = Project::new("s");
        let id = project.new_table("信号");
        {
            let table = project.table_mut(id).unwrap();
            table.columns[0].set_numbers(&[1.0, 2.0, 3.0]);
            table.columns[1].set_numbers(&[1.0, 1.0]);
        }
        let out = signal_table(&mut project, id, "convolve", 0, Some(1)).unwrap();
        let values: Vec<f64> = project.table(out).unwrap().columns[1]
            .numbers
            .iter()
            .map(|value| value.unwrap())
            .collect();
        assert_eq!(values, vec![1.0, 3.0, 5.0, 3.0]);
        let corr = signal_table(&mut project, id, "autocorrelate", 0, None).unwrap();
        let table = project.table(corr).unwrap();
        let zero = table.columns[0]
            .numbers
            .iter()
            .position(|value| *value == Some(0.0))
            .unwrap();
        assert!((table.columns[1].numbers[zero].unwrap() - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn stats_tables_include_variance_and_rows() {
        let mut project = Project::new("t");
        let id = project.new_table("测量");
        {
            let table = project.table_mut(id).unwrap();
            table.columns[0].set_numbers(&[1.0, 2.0, 3.0]);
            table.columns[1].set_numbers(&[10.0, 20.0, 30.0]);
        }
        let stats = column_stats_table(&mut project, id, &[0, 1]).unwrap();
        let table = project.table(stats).unwrap();
        assert_eq!(table.columns[0].texts[7], "方差");
        assert!((table.columns[1].numbers[7].unwrap() - 1.0).abs() < 1e-9);
        assert!((table.columns[2].numbers[4].unwrap() - 20.0).abs() < 1e-9);
        let rows = row_stats_table(&mut project, id, &[0, 1]).unwrap();
        let table = project.table(rows).unwrap();
        assert!((table.columns[0].numbers[1].unwrap() - 2.0).abs() < 1e-9);
        assert!((table.columns[5].number_at(0) - 5.5).abs() < 1e-9);
    }
}
