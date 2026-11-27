use sciplot_core::{
    column_3d, Column3D, CompiledExpr, GridData, Project, SurfaceMode, SurfacePointStyle, SurfaceSource, SurfaceSpec,
    TablePlot3D,
};

use crate::color::viridis;
use crate::scene::{HAlign, Prim, Scene, VAlign};

pub fn render_surface(project: &Project, spec: &SurfaceSpec, scene: &mut Scene, scale: f32) {
    if let SurfaceSource::Table { table, column, kind } = &spec.source {
        let Some(table) = project.table(*table) else {
            scene.notes.push(sciplot_core::t("找不到三维图对应的表格").to_string());
            return;
        };
        match column_3d(table, *column, *kind) {
            Ok(data) => render_column(spec, &data, scene, scale),
            Err(err) => scene.notes.push(err),
        }
        return;
    }
    let grid = match surface_grid(project, &spec.source) {
        Ok(grid) => grid,
        Err(err) => {
            scene.notes.push(err);
            return;
        }
    };
    if grid.rows < 2 || grid.cols < 2 {
        scene.notes.push(sciplot_core::t("三维网格至少需要 2×2").to_string());
        return;
    }
    let mut zmin = f64::INFINITY;
    let mut zmax = f64::NEG_INFINITY;
    for value in &grid.z {
        if value.is_finite() {
            zmin = zmin.min(*value);
            zmax = zmax.max(*value);
        }
    }
    if !zmin.is_finite() {
        scene.notes.push(sciplot_core::t("曲面上没有有效数值").to_string());
        return;
    }
    let zspan = (zmax - zmin).abs().max(1e-9);
    let mut points = Vec::with_capacity(grid.rows * grid.cols);
    for row in 0..grid.rows {
        for col in 0..grid.cols {
            let x = axis_at(grid.x0, grid.x1, col, grid.cols);
            let y = axis_at(grid.y0, grid.y1, row, grid.rows);
            let z = grid.at(row, col);
            let nx = norm(x, grid.x0, grid.x1);
            let ny = norm(y, grid.y0, grid.y1);
            let nz = if z.is_finite() { ((z - zmin) / zspan * 2.0 - 1.0) as f32 } else { f32::NAN };
            points.push((nx, ny, nz, z));
        }
    }

    let cx = scene.width * 0.5;
    let cy = scene.height * 0.54;
    let zoom = spec.zoom.max(0.2);
    let span = fitted_span(spec, &points, scene.width, scene.height, scale) * zoom;

    if !spec.title.is_empty() {
        scene.primitives.push(Prim::Text {
            x: scene.width * 0.5,
            y: 10.0 * scale,
            text: spec.title.clone(),
            size: 16.0 * scale,
            color: [20, 20, 20, 255],
            halign: HAlign::Center,
            valign: VAlign::Top,
            vertical: false,
        });
    }

    match spec.mode {
        SurfaceMode::Surface | SurfaceMode::Polygons | SurfaceMode::HiddenLines | SurfaceMode::WireSurface => {
            let flat = matches!(spec.mode, SurfaceMode::Polygons | SurfaceMode::HiddenLines);
            let mut tris = Vec::new();
            for row in 0..grid.rows - 1 {
                for col in 0..grid.cols - 1 {
                    let i00 = row * grid.cols + col;
                    let i10 = i00 + 1;
                    let i01 = i00 + grid.cols;
                    let i11 = i01 + 1;
                    push_tri(&mut tris, spec, &points, i00, i10, i01, zmin, zspan, flat);
                    push_tri(&mut tris, spec, &points, i10, i11, i01, zmin, zspan, flat);
                }
            }
            tris.sort_by(|a, b| a.depth.total_cmp(&b.depth));
            let hidden = spec.mode == SurfaceMode::HiddenLines;
            let wire = matches!(spec.mode, SurfaceMode::HiddenLines | SurfaceMode::WireSurface);
            for tri in tris {
                let a = project_pt(spec, tri.a, cx, cy, span);
                let b = project_pt(spec, tri.b, cx, cy, span);
                let c = project_pt(spec, tri.c, cx, cy, span);
                if point_ok(a) && point_ok(b) && point_ok(c) {
                    let fill = if hidden { [248, 248, 248, 255] } else { tri.color };
                    scene.primitives.push(Prim::Triangle { a, b, c, fill });
                    if wire {
                        stroke_tri(scene, a, b, c, scale);
                    }
                }
            }
        }
        SurfaceMode::Wireframe => {
            for row in 0..grid.rows {
                let mut pts = Vec::new();
                for col in 0..grid.cols {
                    let p = points[row * grid.cols + col];
                    if p.2.is_finite() {
                        pts.push(project_pt(spec, (p.0, p.1, p.2), cx, cy, span));
                    }
                }
                if pts.len() >= 2 {
                    scene.primitives.push(Prim::Polyline {
                        pts,
                        color: [40, 90, 160, 255],
                        width: 1.0 * scale,
                        dash: sciplot_core::Dash::Solid,
                    });
                }
            }
            for col in 0..grid.cols {
                let mut pts = Vec::new();
                for row in 0..grid.rows {
                    let p = points[row * grid.cols + col];
                    if p.2.is_finite() {
                        pts.push(project_pt(spec, (p.0, p.1, p.2), cx, cy, span));
                    }
                }
                if pts.len() >= 2 {
                    scene.primitives.push(Prim::Polyline {
                        pts,
                        color: [40, 90, 160, 220],
                        width: 1.0 * scale,
                        dash: sciplot_core::Dash::Solid,
                    });
                }
            }
        }
        SurfaceMode::Scatter => {
            for point in &points {
                if !point.2.is_finite() {
                    continue;
                }
                let (x, y) = project_pt(spec, (point.0, point.1, point.2), cx, cy, span);
                let t = ((point.3 - zmin) / zspan) as f32;
                draw_marker(scene, spec.point_style, x, y, viridis(t), scale);
            }
        }
    }
    draw_surface_guides(scene, spec, &grid, zmin, zmax, cx, cy, span, scale);
}

#[allow(clippy::too_many_arguments)]
fn draw_surface_guides(
    scene: &mut Scene,
    spec: &SurfaceSpec,
    grid: &GridData,
    zmin: f64,
    zmax: f64,
    cx: f32,
    cy: f32,
    span: f32,
    scale: f32,
) {
    let color = [30, 30, 30, 255];
    let axes = [
        ((-1.0, -1.0, -1.0), (1.0, -1.0, -1.0)),
        ((-1.0, -1.0, -1.0), (-1.0, 1.0, -1.0)),
        ((-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0)),
    ];
    for (from, to) in axes {
        let a = project_pt(spec, from, cx, cy, span);
        let b = project_pt(spec, to, cx, cy, span);
        scene.primitives.push(Prim::Line {
            x1: a.0,
            y1: a.1,
            x2: b.0,
            y2: b.1,
            color,
            width: 1.1 * scale,
            dash: sciplot_core::Dash::Solid,
        });
    }
    let range = format!(
        "x [{}, {}]    y [{}, {}]    z [{}, {}]",
        sciplot_core::format_short(grid.x0),
        sciplot_core::format_short(grid.x1),
        sciplot_core::format_short(grid.y0),
        sciplot_core::format_short(grid.y1),
        sciplot_core::format_short(zmin),
        sciplot_core::format_short(zmax)
    );
    scene.primitives.push(Prim::Text {
        x: 12.0 * scale,
        y: 28.0 * scale,
        text: range,
        size: 12.0 * scale,
        color,
        halign: HAlign::Left,
        valign: VAlign::Top,
        vertical: false,
    });
    let bar_x = scene.width - 36.0 * scale;
    let bar_y = 48.0 * scale;
    let bar_w = 12.0 * scale;
    let bar_h = (scene.height - 96.0 * scale).max(40.0);
    let steps = 32;
    for i in 0..steps {
        let t = 1.0 - i as f32 / steps as f32;
        let yy = bar_y + bar_h * i as f32 / steps as f32;
        scene.primitives.push(Prim::Rect {
            x: bar_x,
            y: yy,
            w: bar_w,
            h: bar_h / steps as f32 + 0.6,
            fill: Some(viridis(t)),
            stroke: None,
        });
    }
    scene.primitives.push(Prim::Text {
        x: bar_x - 4.0 * scale,
        y: bar_y,
        text: sciplot_core::format_short(zmax),
        size: 11.0 * scale,
        color,
        halign: HAlign::Right,
        valign: VAlign::Top,
        vertical: false,
    });
    scene.primitives.push(Prim::Text {
        x: bar_x - 4.0 * scale,
        y: bar_y + bar_h,
        text: sciplot_core::format_short(zmin),
        size: 11.0 * scale,
        color,
        halign: HAlign::Right,
        valign: VAlign::Bottom,
        vertical: false,
    });
}

struct Tri {
    a: (f32, f32, f32),
    b: (f32, f32, f32),
    c: (f32, f32, f32),
    color: [u8; 4],
    depth: f32,
}

#[allow(clippy::too_many_arguments)]
fn push_tri(
    tris: &mut Vec<Tri>,
    spec: &SurfaceSpec,
    points: &[(f32, f32, f32, f64)],
    i: usize,
    j: usize,
    k: usize,
    zmin: f64,
    zspan: f64,
    flat: bool,
) {
    let a = points[i];
    let b = points[j];
    let c = points[k];
    if !a.2.is_finite() || !b.2.is_finite() || !c.2.is_finite() {
        return;
    }
    let e1 = (b.0 - a.0, b.1 - a.1, b.2 - a.2);
    let e2 = (c.0 - a.0, c.1 - a.1, c.2 - a.2);
    let n = cross(e1, e2);
    let light = (0.35, 0.55, 0.76);
    let ndot = (n.0 * light.0 + n.1 * light.1 + n.2 * light.2).abs().clamp(0.35, 1.0);
    let z = (a.3 + b.3 + c.3) / 3.0;
    let mut color = viridis(((z - zmin) / zspan) as f32);
    if !flat {
        color[0] = (color[0] as f32 * ndot) as u8;
        color[1] = (color[1] as f32 * ndot) as u8;
        color[2] = (color[2] as f32 * ndot) as u8;
    }
    let ra = rotate(spec, (a.0, a.1, a.2));
    let rb = rotate(spec, (b.0, b.1, b.2));
    let rc = rotate(spec, (c.0, c.1, c.2));
    tris.push(Tri {
        a: (a.0, a.1, a.2),
        b: (b.0, b.1, b.2),
        c: (c.0, c.1, c.2),
        color,
        depth: (ra.2 + rb.2 + rc.2) / 3.0,
    });
}

fn cross(a: (f32, f32, f32), b: (f32, f32, f32)) -> (f32, f32, f32) {
    let n = (
        a.1 * b.2 - a.2 * b.1,
        a.2 * b.0 - a.0 * b.2,
        a.0 * b.1 - a.1 * b.0,
    );
    let len = (n.0 * n.0 + n.1 * n.1 + n.2 * n.2).sqrt().max(1e-6);
    (n.0 / len, n.1 / len, n.2 / len)
}

fn rotate(spec: &SurfaceSpec, p: (f32, f32, f32)) -> (f32, f32, f32) {
    let az = spec.azimuth.to_radians();
    let el = spec.elevation.to_radians();
    let x1 = p.0 * az.cos() - p.1 * az.sin();
    let y1 = p.0 * az.sin() + p.1 * az.cos();
    let y2 = y1 * el.cos() - p.2 * el.sin();
    let z2 = y1 * el.sin() + p.2 * el.cos();
    (x1, y2, z2)
}

fn point_ok(p: (f32, f32)) -> bool {
    p.0.is_finite() && p.1.is_finite()
}

fn fitted_span(spec: &SurfaceSpec, points: &[(f32, f32, f32, f64)], width: f32, height: f32, scale: f32) -> f32 {
    let mut reach = 0.2f32;
    for point in points {
        if !point.0.is_finite() || !point.1.is_finite() || !point.2.is_finite() {
            continue;
        }
        let (x, y, z) = rotate(spec, (point.0, point.1, point.2));
        let k = if spec.perspective { 3.4 / (3.4 - z).max(0.4) } else { 1.0 };
        reach = reach.max((x * k).abs()).max((y * k).abs());
    }
    let margin = 48.0 * scale;
    let half = (width.min(height) * 0.5 - margin).max(40.0);
    half / reach
}

fn project_pt(spec: &SurfaceSpec, p: (f32, f32, f32), cx: f32, cy: f32, span: f32) -> (f32, f32) {
    let (x, y, z) = rotate(spec, p);
    let k = if spec.perspective {
        3.4 / (3.4 - z).max(0.4)
    } else {
        1.0
    };
    (cx + x * k * span, cy - y * k * span)
}

fn norm(value: f64, min: f64, max: f64) -> f32 {
    if (max - min).abs() < 1e-12 {
        return 0.0;
    }
    (((value - min) / (max - min)) * 2.0 - 1.0) as f32
}

fn axis_at(min: f64, max: f64, index: usize, count: usize) -> f64 {
    if count <= 1 {
        return (min + max) * 0.5;
    }
    min + (max - min) * index as f64 / (count as f64 - 1.0)
}

fn surface_grid(project: &Project, source: &SurfaceSource) -> Result<GridData, String> {
    match source {
        SurfaceSource::Matrix(id) => {
            let matrix = project.matrix(*id).ok_or(sciplot_core::t("找不到三维图对应的矩阵"))?;
            Ok(decimate_grid(GridData::from_matrix(matrix)))
        }
        SurfaceSource::Table { .. } => Err(sciplot_core::t("表格三维图不使用网格").into()),
        SurfaceSource::Function { expr, x_min, x_max, y_min, y_max, n } => {
            let compiled = CompiledExpr::compile(expr)?;
            let n = (*n).clamp(8, 80);
            let mut z = Vec::with_capacity(n * n);
            for row in 0..n {
                let y = axis_at(*y_min, *y_max, row, n);
                for col in 0..n {
                    let x = axis_at(*x_min, *x_max, col, n);
                    let value = compiled.eval(&[("x", x), ("y", y)]).unwrap_or(f64::NAN);
                    z.push(if value.is_finite() { value } else { f64::NAN });
                }
            }
            Ok(GridData {
                rows: n,
                cols: n,
                x0: *x_min,
                x1: *x_max,
                y0: *y_min,
                y1: *y_max,
                z,
            })
        }
    }
}

fn decimate_grid(grid: GridData) -> GridData {
    const MAX: usize = 160;
    if grid.rows <= MAX && grid.cols <= MAX {
        return grid;
    }
    let step_r = (grid.rows as f64 / MAX as f64).ceil().max(1.0) as usize;
    let step_c = (grid.cols as f64 / MAX as f64).ceil().max(1.0) as usize;
    let rows = grid.rows.div_ceil(step_r).max(2);
    let cols = grid.cols.div_ceil(step_c).max(2);
    let mut z = Vec::with_capacity(rows * cols);
    for row in 0..rows {
        let src_r = (row * step_r).min(grid.rows - 1);
        for col in 0..cols {
            let src_c = (col * step_c).min(grid.cols - 1);
            z.push(grid.at(src_r, src_c));
        }
    }
    let x1 = axis_at(grid.x0, grid.x1, ((cols - 1) * step_c).min(grid.cols - 1), grid.cols);
    let y1 = axis_at(grid.y0, grid.y1, ((rows - 1) * step_r).min(grid.rows - 1), grid.rows);
    GridData {
        rows,
        cols,
        x0: grid.x0,
        x1,
        y0: grid.y0,
        y1,
        z,
    }
}

fn stroke_tri(scene: &mut Scene, a: (f32, f32), b: (f32, f32), c: (f32, f32), scale: f32) {
    let color = [30, 40, 70, 220];
    let width = 0.8 * scale;
    for (p, q) in [(a, b), (b, c), (c, a)] {
        scene.primitives.push(Prim::Line {
            x1: p.0,
            y1: p.1,
            x2: q.0,
            y2: q.1,
            color,
            width,
            dash: sciplot_core::Dash::Solid,
        });
    }
}

fn draw_marker(scene: &mut Scene, style: SurfacePointStyle, x: f32, y: f32, color: [u8; 4], scale: f32) {
    let arm = 4.2 * scale;
    match style {
        SurfacePointStyle::Dot => {
            scene.primitives.push(Prim::Circle {
                x,
                y,
                r: 2.4 * scale,
                fill: Some(color),
                stroke: None,
            });
        }
        SurfacePointStyle::Cross => {
            for (x1, y1, x2, y2) in [(x - arm, y, x + arm, y), (x, y - arm, x, y + arm)] {
                scene.primitives.push(Prim::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    color,
                    width: 1.3 * scale,
                    dash: sciplot_core::Dash::Solid,
                });
            }
        }
        SurfacePointStyle::Cone => {
            scene.primitives.push(Prim::Triangle {
                a: (x, y - arm),
                b: (x - arm * 0.65, y + arm * 0.55),
                c: (x + arm * 0.65, y + arm * 0.55),
                fill: color,
            });
        }
        SurfacePointStyle::Bar => {
            scene.primitives.push(Prim::Line {
                x1: x,
                y1: y - arm,
                x2: x,
                y2: y + arm * 1.4,
                color,
                width: 1.6 * scale,
                dash: sciplot_core::Dash::Solid,
            });
        }
    }
}

fn render_column(spec: &SurfaceSpec, data: &Column3D, scene: &mut Scene, scale: f32) {
    if !spec.title.is_empty() {
        scene.primitives.push(Prim::Text {
            x: scene.width * 0.5,
            y: 10.0 * scale,
            text: spec.title.clone(),
            size: 16.0 * scale,
            color: [20, 20, 20, 255],
            halign: HAlign::Center,
            valign: VAlign::Top,
            vertical: false,
        });
    }
    let mut zmin = data.z.iter().copied().fold(f64::INFINITY, f64::min);
    let mut zmax = data.z.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if data.kind == TablePlot3D::Bars {
        zmin = zmin.min(0.0);
        zmax = zmax.max(0.0);
    }
    let zspan = (zmax - zmin).abs().max(1e-9);
    let (xmin, xmax) = span_of(&data.x);
    let (ymin, ymax) = if data.kind == TablePlot3D::Ribbon || data.y_is_plane {
        (-1.0, 1.0)
    } else {
        span_of(&data.y)
    };
    let bounds = Bounds { xmin, xmax, ymin, ymax, zmin, zmax };
    let cx = scene.width * 0.5;
    let cy = scene.height * 0.54;
    let samples: Vec<(f32, f32, f32, f64)> = data
        .x
        .iter()
        .zip(&data.y)
        .zip(&data.z)
        .map(|((x, y), z)| {
            let y = if data.kind == TablePlot3D::Ribbon || data.y_is_plane { 0.0 } else { *y };
            let (nx, ny, nz) = normalize(*x, y, *z, &bounds);
            (nx, ny, nz, *z)
        })
        .collect();
    let zoom = spec.zoom.max(0.2);
    let span = fitted_span(spec, &samples, scene.width, scene.height, scale) * zoom;
    match data.kind {
        TablePlot3D::Ribbon => draw_ribbon(spec, &samples, zmin, zspan, cx, cy, span, scene, scale),
        TablePlot3D::Bars => draw_bars(spec, data, &bounds, zmin, zspan, cx, cy, span, scene),
        TablePlot3D::Trajectory => draw_trajectory(spec, &samples, zmin, zspan, cx, cy, span, scene, scale),
    }
    let grid = GridData {
        rows: 2,
        cols: 2,
        x0: xmin,
        x1: xmax,
        y0: if data.y_is_plane { 0.0 } else { ymin },
        y1: if data.y_is_plane { 0.0 } else { ymax },
        z: Vec::new(),
    };
    draw_surface_guides(scene, spec, &grid, zmin, zmax, cx, cy, span, scale);
}

struct Bounds {
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
    zmin: f64,
    zmax: f64,
}

fn span_of(values: &[f64]) -> (f64, f64) {
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if (max - min).abs() < 1e-9 {
        (min - 0.5, max + 0.5)
    } else {
        (min, max)
    }
}

fn normalize(x: f64, y: f64, z: f64, bounds: &Bounds) -> (f32, f32, f32) {
    (
        norm(x, bounds.xmin, bounds.xmax),
        norm(y, bounds.ymin, bounds.ymax),
        norm(z, bounds.zmin, bounds.zmax),
    )
}

#[allow(clippy::too_many_arguments)]
fn draw_ribbon(
    spec: &SurfaceSpec,
    samples: &[(f32, f32, f32, f64)],
    zmin: f64,
    zspan: f64,
    cx: f32,
    cy: f32,
    span: f32,
    scene: &mut Scene,
    scale: f32,
) {
    if spec.mode == SurfaceMode::Scatter {
        for point in samples {
            let (x, y) = project_pt(spec, (point.0, 0.0, point.2), cx, cy, span);
            draw_marker(scene, spec.point_style, x, y, viridis(((point.3 - zmin) / zspan) as f32), scale);
        }
        return;
    }
    let mut tris = Vec::new();
    let points: Vec<(f32, f32, f32, f64)> = samples
        .iter()
        .flat_map(|point| {
            [
                (point.0, -0.12, point.2, point.3),
                (point.0, 0.12, point.2, point.3),
            ]
        })
        .collect();
    for i in 0..samples.len().saturating_sub(1) {
        let i00 = i * 2;
        let i01 = i00 + 1;
        let i10 = i00 + 2;
        let i11 = i00 + 3;
        let flat = matches!(spec.mode, SurfaceMode::Polygons | SurfaceMode::HiddenLines);
        push_tri(&mut tris, spec, &points, i00, i01, i11, zmin, zspan, flat);
        push_tri(&mut tris, spec, &points, i00, i11, i10, zmin, zspan, flat);
    }
    tris.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    let hidden = spec.mode == SurfaceMode::HiddenLines;
    let wire = matches!(spec.mode, SurfaceMode::Wireframe | SurfaceMode::HiddenLines | SurfaceMode::WireSurface);
    let fill = !matches!(spec.mode, SurfaceMode::Wireframe);
    for tri in tris {
        let a = project_pt(spec, tri.a, cx, cy, span);
        let b = project_pt(spec, tri.b, cx, cy, span);
        let c = project_pt(spec, tri.c, cx, cy, span);
        if !(point_ok(a) && point_ok(b) && point_ok(c)) {
            continue;
        }
        if fill {
            let color = if hidden { [248, 248, 248, 255] } else { tri.color };
            scene.primitives.push(Prim::Triangle { a, b, c, fill: color });
        }
        if wire {
            stroke_tri(scene, a, b, c, scale);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_bars(
    spec: &SurfaceSpec,
    data: &Column3D,
    bounds: &Bounds,
    zmin: f64,
    zspan: f64,
    cx: f32,
    cy: f32,
    span: f32,
    scene: &mut Scene,
) {
    let hx = half_norm(&data.x);
    let hy = if data.y_is_plane { 0.08 } else { half_norm(&data.y) };
    let mut tris = Vec::new();
    for ((x, y), z) in data.x.iter().zip(&data.y).zip(&data.z) {
        let (nx, ny, nz) = normalize(*x, *y, *z, bounds);
        let nz0 = norm(0.0, bounds.zmin, bounds.zmax);
        let corners = [
            (nx - hx, ny - hy, nz0),
            (nx + hx, ny - hy, nz0),
            (nx + hx, ny + hy, nz0),
            (nx - hx, ny + hy, nz0),
            (nx - hx, ny - hy, nz),
            (nx + hx, ny - hy, nz),
            (nx + hx, ny + hy, nz),
            (nx - hx, ny + hy, nz),
        ];
        let faces = [(0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7), (4, 5, 6, 7)];
        let color = viridis(((z - zmin) / zspan) as f32);
        for (a, b, c, d) in faces {
            push_box(&mut tris, spec, &corners, a, b, c, color);
            push_box(&mut tris, spec, &corners, a, c, d, color);
        }
    }
    tris.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    for tri in tris {
        let a = project_pt(spec, tri.a, cx, cy, span);
        let b = project_pt(spec, tri.b, cx, cy, span);
        let c = project_pt(spec, tri.c, cx, cy, span);
        if point_ok(a) && point_ok(b) && point_ok(c) {
            scene.primitives.push(Prim::Triangle { a, b, c, fill: tri.color });
        }
    }
}

fn push_box(tris: &mut Vec<Tri>, spec: &SurfaceSpec, corners: &[(f32, f32, f32)], i: usize, j: usize, k: usize, color: [u8; 4]) {
    let a = corners[i];
    let b = corners[j];
    let c = corners[k];
    let e1 = (b.0 - a.0, b.1 - a.1, b.2 - a.2);
    let e2 = (c.0 - a.0, c.1 - a.1, c.2 - a.2);
    let n = cross(e1, e2);
    let light = (0.35, 0.55, 0.76);
    let ndot = (n.0 * light.0 + n.1 * light.1 + n.2 * light.2).abs().clamp(0.35, 1.0);
    let mut shaded = color;
    shaded[0] = (shaded[0] as f32 * ndot) as u8;
    shaded[1] = (shaded[1] as f32 * ndot) as u8;
    shaded[2] = (shaded[2] as f32 * ndot) as u8;
    let ra = rotate(spec, a);
    let rb = rotate(spec, b);
    let rc = rotate(spec, c);
    tris.push(Tri {
        a,
        b,
        c,
        color: shaded,
        depth: (ra.2 + rb.2 + rc.2) / 3.0,
    });
}

fn half_norm(values: &[f64]) -> f32 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mut diffs = Vec::new();
    for pair in sorted.windows(2) {
        let step = pair[1] - pair[0];
        if step > 1e-9 {
            diffs.push(step);
        }
    }
    let (min, max) = span_of(values);
    let data_half = if diffs.is_empty() {
        (max - min).abs() * 0.08
    } else {
        diffs.sort_by(f64::total_cmp);
        diffs[diffs.len() / 2] * 0.35
    };
    let span = (max - min).abs().max(1e-9);
    ((data_half / span) * 2.0).clamp(0.03, 0.28) as f32
}

#[allow(clippy::too_many_arguments)]
fn draw_trajectory(
    spec: &SurfaceSpec,
    samples: &[(f32, f32, f32, f64)],
    zmin: f64,
    zspan: f64,
    cx: f32,
    cy: f32,
    span: f32,
    scene: &mut Scene,
    scale: f32,
) {
    let mut previous: Option<(f32, f32)> = None;
    for point in samples {
        let (x, y) = project_pt(spec, (point.0, point.1, point.2), cx, cy, span);
        if !point_ok((x, y)) {
            previous = None;
            continue;
        }
        if spec.mode != SurfaceMode::Scatter {
            if let Some((x1, y1)) = previous {
                scene.primitives.push(Prim::Line {
                    x1,
                    y1,
                    x2: x,
                    y2: y,
                    color: viridis(((point.3 - zmin) / zspan) as f32),
                    width: 1.8 * scale,
                    dash: sciplot_core::Dash::Solid,
                });
            }
        }
        if spec.mode == SurfaceMode::Scatter || spec.point_style != SurfacePointStyle::Dot {
            draw_marker(scene, spec.point_style, x, y, viridis(((point.3 - zmin) / zspan) as f32), scale);
        }
        previous = Some((x, y));
    }
}
