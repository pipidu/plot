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

pub fn data_to_screen(frame: &LayerFrame, x: f64, y: f64) -> Option<(f32, f32)> {
    let ux = crate::scale::unit(frame.x_scale, x, frame.x_min, frame.x_max, frame.x_inverted)?;
    let uy = crate::scale::unit(frame.y_scale, y, frame.y_min, frame.y_max, frame.y_inverted)?;
    Some((frame.plot.x + ux * frame.plot.w, frame.plot.y + frame.plot.h - uy * frame.plot.h))
}
