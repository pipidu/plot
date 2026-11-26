use sciplot_core::{CompiledExpr, GridData, Project, SurfaceMode, SurfaceSource, SurfaceSpec};

use crate::color::viridis;
use crate::scene::{HAlign, Prim, Scene, VAlign};

pub fn render_surface(project: &Project, spec: &SurfaceSpec, scene: &mut Scene, scale: f32) {
    let grid = match surface_grid(project, &spec.source) {
        Ok(grid) => grid,
        Err(err) => {
            scene.notes.push(err);
            return;
        }
    };
    if grid.rows < 2 || grid.cols < 2 {
        scene.notes.push("三维网格至少需要 2×2".to_string());
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
        scene.notes.push("曲面上没有有效数值".to_string());
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
        SurfaceMode::Surface => {
            let mut tris = Vec::new();
            for row in 0..grid.rows - 1 {
                for col in 0..grid.cols - 1 {
                    let i00 = row * grid.cols + col;
                    let i10 = i00 + 1;
                    let i01 = i00 + grid.cols;
                    let i11 = i01 + 1;
                    push_tri(&mut tris, spec, &points, i00, i10, i01, zmin, zspan);
                    push_tri(&mut tris, spec, &points, i10, i11, i01, zmin, zspan);
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
                scene.primitives.push(Prim::Circle {
                    x,
                    y,
                    r: 2.4 * scale,
                    fill: Some(viridis(t)),
                    stroke: None,
                });
            }
        }
    }
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
    color[0] = (color[0] as f32 * ndot) as u8;
    color[1] = (color[1] as f32 * ndot) as u8;
    color[2] = (color[2] as f32 * ndot) as u8;
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
            let matrix = project.matrix(*id).ok_or("找不到三维图对应的矩阵")?;
            Ok(GridData::from_matrix(matrix))
        }
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
