use sciplot_core::{AxisScale, Project};

use crate::scale::{forward, inverse};
use crate::scene::LayerFrame;

pub fn zoom_limits(scale: AxisScale, min: f64, max: f64, cursor: f64, factor: f64) -> Option<(f64, f64)> {
    let t0 = forward(scale, min)?;
    let t1 = forward(scale, max)?;
    let tc = forward(scale, cursor).unwrap_or((t0 + t1) * 0.5);
    let factor = factor.clamp(0.2, 8.0);
    let n0 = tc + (t0 - tc) / factor;
    let n1 = tc + (t1 - tc) / factor;
    Some((inverse(scale, n0)?, inverse(scale, n1)?))
}

pub fn pan_limits(frame: &LayerFrame, dx: f32, dy: f32) -> Option<((f64, f64), (f64, f64))> {
    let x0 = forward(frame.x_scale, frame.x_min)?;
    let x1 = forward(frame.x_scale, frame.x_max)?;
    let y0 = forward(frame.y_scale, frame.y_min)?;
    let y1 = forward(frame.y_scale, frame.y_max)?;
    let mut x_shift = dx as f64 / frame.plot.w.max(1.0) as f64 * (x1 - x0);
    let mut y_shift = dy as f64 / frame.plot.h.max(1.0) as f64 * (y1 - y0);
    if frame.x_inverted {
        x_shift = -x_shift;
    }
    if frame.y_inverted {
        y_shift = -y_shift;
    }
    Some((
        (inverse(frame.x_scale, x0 - x_shift)?, inverse(frame.x_scale, x1 - x_shift)?),
        (inverse(frame.y_scale, y0 + y_shift)?, inverse(frame.y_scale, y1 + y_shift)?),
    ))
}

pub fn screen_to_data(frame: &LayerFrame, px: f32, py: f32) -> Option<(f64, f64)> {
    if frame.plot.w < 1.0 || frame.plot.h < 1.0 {
        return None;
    }
    let mut ux = ((px - frame.plot.x) / frame.plot.w) as f64;
    let mut uy = (1.0 - (py - frame.plot.y) / frame.plot.h) as f64;
    if frame.x_inverted {
        ux = 1.0 - ux;
    }
    if frame.y_inverted {
        uy = 1.0 - uy;
    }
    let x0 = forward(frame.x_scale, frame.x_min)?;
    let x1 = forward(frame.x_scale, frame.x_max)?;
    let y0 = forward(frame.y_scale, frame.y_min)?;
    let y1 = forward(frame.y_scale, frame.y_max)?;
    Some((inverse(frame.x_scale, x0 + (x1 - x0) * ux)?, inverse(frame.y_scale, y0 + (y1 - y0) * uy)?))
}

#[derive(Clone, Debug)]
pub struct NearestPoint {
    pub curve: usize,
    pub index: usize,
    pub x: f64,
    pub y: f64,
    pub px: f32,
    pub py: f32,
}

pub fn nearest_point(project: &Project, graph: &sciplot_core::Graph, layer: usize, frame: &LayerFrame, px: f32, py: f32) -> Option<NearestPoint> {
    let layer = graph.layers.get(layer)?;
    let mut best: Option<NearestPoint> = None;
    let mut best_d = f32::MAX;
    for (curve_index, curve) in layer.curves.iter().enumerate() {
        if !curve.visible {
            continue;
        }
        let Ok(series) = sciplot_core::resolve_curve(project, curve) else {
            continue;
        };
        let n = series.x.len().min(series.y.len());
        for index in 0..n {
            let Some((sx, sy)) = data_to_screen(frame, series.x[index], series.y[index]) else {
                continue;
            };
            let dist = (sx - px).hypot(sy - py);
            if dist < best_d {
                best_d = dist;
                best = Some(NearestPoint {
                    curve: curve_index,
                    index,
                    x: series.x[index],
                    y: series.y[index],
                    px: sx,
                    py: sy,
                });
            }
        }
    }
    best.filter(|_| best_d < 18.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CurveHit {
    pub layer: usize,
    pub curve: usize,
}

/// 离指针最近的可见曲线。点到采样点、以及点到相邻采样点连成的线段，都会参与比较。
pub fn nearest_curve(project: &Project, graph: &sciplot_core::Graph, frames: &[LayerFrame], px: f32, py: f32) -> Option<CurveHit> {
    let mut best: Option<(CurveHit, f32)> = None;
    for (layer_index, frame) in frames.iter().enumerate() {
        let Some(layer) = graph.layers.get(layer_index) else {
            continue;
        };
        for (curve_index, curve) in layer.curves.iter().enumerate() {
            if !curve.visible {
                continue;
            }
            let Ok(series) = sciplot_core::resolve_curve(project, curve) else {
                continue;
            };
            let n = series.x.len().min(series.y.len());
            let mut prev: Option<(f32, f32)> = None;
            for index in 0..n {
                let Some(point) = data_to_screen(frame, series.x[index], series.y[index]) else {
                    prev = None;
                    continue;
                };
                consider(&mut best, layer_index, curve_index, (point.0 - px).hypot(point.1 - py));
                if let Some(start) = prev {
                    consider(&mut best, layer_index, curve_index, segment_distance(px, py, start.0, start.1, point.0, point.1));
                }
                prev = Some(point);
            }
        }
    }
    best.filter(|(_, distance)| *distance <= 12.0).map(|(hit, _)| hit)
}

fn consider(best: &mut Option<(CurveHit, f32)>, layer: usize, curve: usize, distance: f32) {
    if best.as_ref().map(|(_, previous)| distance < *previous).unwrap_or(true) {
        *best = Some((CurveHit { layer, curve }, distance));
    }
}

pub fn data_to_screen(frame: &LayerFrame, x: f64, y: f64) -> Option<(f32, f32)> {
    let ux = crate::scale::unit(frame.x_scale, x, frame.x_min, frame.x_max, frame.x_inverted)?;
    let uy = crate::scale::unit(frame.y_scale, y, frame.y_min, frame.y_max, frame.y_inverted)?;
    Some((frame.plot.x + ux * frame.plot.w, frame.plot.y + frame.plot.h - uy * frame.plot.h))
}

fn segment_distance(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = bx - ax;
    let dy = by - ay;
    let len2 = dx * dx + dy * dy;
    if len2 < 1e-6 {
        return (px - ax).hypot(py - ay);
    }
    let t = (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0);
    (px - (ax + t * dx)).hypot(py - (ay + t * dy))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sciplot_core::{ColumnMode, CurveKind, Project};

    #[test]
    fn nearest_curve_hits_the_middle_of_a_segment() {
        let mut project = Project::new("t");
        let table = project.new_table("t");
        {
            let table = project.table_mut(table).unwrap();
            table.columns.clear();
            table.add_numeric_column("X", ColumnMode::X, &[0.0, 1.0]);
            table.add_numeric_column("Y", ColumnMode::Y, &[0.0, 1.0]);
        }
        let graph_id = project.plot_table(table, CurveKind::Line).unwrap();
        let graph = project.graph(graph_id).unwrap();
        let scene = crate::render(&project, graph, &[], 400.0, 300.0, 1.0);
        let frame = &scene.layers[0];
        let (x0, y0) = data_to_screen(frame, 0.0, 0.0).unwrap();
        let (x1, y1) = data_to_screen(frame, 1.0, 1.0).unwrap();
        let hit = nearest_curve(&project, graph, &scene.layers, (x0 + x1) / 2.0, (y0 + y1) / 2.0).unwrap();
        assert_eq!(hit.curve, 0);
        assert!(nearest_curve(&project, graph, &scene.layers, 1.0, 1.0).is_none());
    }
}
