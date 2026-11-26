use sciplot_core::{
    format_number, palette_color, resolve_curve, Axis, AxisScale, CurveKind, Dash, Graph, LegendPos,
    Project, SeriesData, Symbol,
};

use crate::color::{viridis, with_alpha};
use crate::contour::contour_segments;
use crate::scale::{minor_ticks_for, ticks_for_axis, unit, Tick};
use crate::scene::{HAlign, LayerFrame, LayerView, Prim, PxRect, Scene, VAlign};
use crate::surface::render_surface;

struct Item {
    series: SeriesData,
    color: [u8; 4],
    fill: Option<[u8; 4]>,
    width: f32,
    dash: Dash,
    symbol: Symbol,
    symbol_size: f32,
    bins: usize,
    kind: CurveKind,
    name: String,
    curve: usize,
}

struct LegendEntry {
    name: String,
    color: [u8; 4],
    line: bool,
    hidden: bool,
    curve: Option<usize>,
}

pub fn render(
    project: &Project,
    graph: &Graph,
    views: &[LayerView],
    width: f32,
    height: f32,
    scale: f32,
) -> Scene {
    let mut scene = Scene {
        width: width.max(1.0),
        height: height.max(1.0),
        primitives: Vec::new(),
        layers: Vec::new(),
        notes: Vec::new(),
        legend_hits: Vec::new(),
    };
    scene.primitives.push(Prim::Rect {
        x: 0.0,
        y: 0.0,
        w: scene.width,
        h: scene.height,
        fill: Some([255, 255, 255, 255]),
        stroke: None,
    });
    if let Some(surface) = &graph.surface {
        render_surface(project, surface, &mut scene, scale);
        draw_notes(&mut scene, scale);
        return scene;
    }
    if graph.layers.is_empty() {
        scene.notes.push(sciplot_core::t("图形没有图层").to_string());
        draw_notes(&mut scene, scale);
        return scene;
    }
    for (index, layer) in graph.layers.iter().enumerate() {
        let view = views.get(index).copied().unwrap_or_default();
        render_layer(project, layer, index, view, &mut scene, scale);
    }
    draw_notes(&mut scene, scale);
    scene
}

fn render_layer(
    project: &Project,
    layer: &sciplot_core::Layer,
    layer_index: usize,
    view: LayerView,
    scene: &mut Scene,
    scale: f32,
) {
    let mut items = Vec::new();
    let mut hidden_legend = Vec::new();
    for (index, curve) in layer.curves.iter().enumerate() {
        if !curve.visible {
            hidden_legend.push(LegendEntry {
                name: sciplot_core::trf!("{}（已隐藏）", "{} (hidden)", curve.name),
                color: [150, 150, 150, 255],
                line: legend_has_line(curve.kind),
                hidden: true,
                curve: Some(index),
            });
            continue;
        }
        match resolve_curve(project, curve) {
            Ok(series) => items.push(Item {
                series,
                color: curve.style.color,
                fill: curve.style.fill,
                width: curve.style.line_width.max(0.6) * scale,
                dash: curve.style.dash,
                symbol: curve.style.symbol,
                symbol_size: curve.style.symbol_size.max(2.0) * scale,
                bins: curve.style.histogram_bins.max(1),
                kind: curve.kind,
                name: curve.name.clone(),
                curve: index,
            }),
            Err(err) => scene.notes.push(format!("{}：{err}", curve.name)),
        }
    }

    let fx = layer.frame.x * scene.width;
    let fy = layer.frame.y * scene.height;
    let fw = (layer.frame.w * scene.width).max(1.0);
    let fh = (layer.frame.h * scene.height).max(1.0);
    let wants_bar = items.iter().any(|item| matches!(item.kind, CurveKind::Heatmap | CurveKind::Contour));
    let left = 64.0 * scale;
    let right = if wants_bar { 72.0 * scale } else { 16.0 * scale };
    let top = 30.0 * scale;
    let bottom = 46.0 * scale;
    let plot = PxRect {
        x: fx + left,
        y: fy + top,
        w: (fw - left - right).max(8.0),
        h: (fh - top - bottom).max(8.0),
    };

    let only_pie = !items.is_empty() && items.iter().all(|item| item.kind == CurveKind::Pie);
    let mut x_auto = Range::new(layer.x_axis.scale);
    let mut y_auto = Range::new(layer.y_axis.scale);
    let mut category_x: Option<Vec<(f64, String)>> = None;
    let mut category_y: Option<Vec<(f64, String)>> = None;
    let mut z_span: Option<(f64, f64)> = None;
    let mut box_slot = 0.0;

    for item in &items {
        match item.kind {
            CurveKind::Pie => {}
            CurveKind::Heatmap | CurveKind::Contour => {
                if let Some(grid) = &item.series.grid {
                    x_auto.add(grid.x0.min(grid.x1));
                    x_auto.add(grid.x0.max(grid.x1));
                    y_auto.add(grid.y0.min(grid.y1));
                    y_auto.add(grid.y0.max(grid.y1));
                    let mut lo = f64::INFINITY;
                    let mut hi = f64::NEG_INFINITY;
                    for value in &grid.z {
                        if value.is_finite() {
                            lo = lo.min(*value);
                            hi = hi.max(*value);
                        }
                    }
                    if lo.is_finite() {
                        z_span = Some(match z_span {
                            Some((a, b)) => (a.min(lo), b.max(hi)),
                            None => (lo, hi),
                        });
                    }
                }
            }
            CurveKind::Histogram => {
                let values = finite_values(hist_source(item));
                if let Some((edges, counts)) = histogram(&values, item.bins) {
                    x_auto.add(edges[0]);
                    x_auto.add(*edges.last().unwrap_or(&edges[0]));
                    y_auto.add(0.0);
                    y_auto.add(counts.iter().copied().fold(0.0, f64::max));
                }
            }
            CurveKind::BoxPlot => {
                box_slot += 1.0;
                x_auto.add(box_slot);
                for value in item.series.y.iter().copied().filter(|v| v.is_finite()) {
                    y_auto.add(value);
                }
            }
            CurveKind::HorizontalBars => {
                y_auto.add(0.0);
                for (index, value) in item.series.y.iter().enumerate() {
                    if value.is_finite() {
                        x_auto.add(0.0);
                        x_auto.add(*value);
                    }
                    if item.series.x.get(index).copied().unwrap_or(f64::NAN).is_finite() {
                        y_auto.add(item.series.x[index]);
                    }
                }
                if !item.series.labels.is_empty() {
                    category_y = Some(labeled(&item.series));
                }
            }
            CurveKind::Bars => {
                y_auto.add(0.0);
                for (index, value) in item.series.y.iter().enumerate() {
                    if value.is_finite() {
                        y_auto.add(*value);
                    }
                    if item.series.x.get(index).copied().unwrap_or(f64::NAN).is_finite() {
                        x_auto.add(item.series.x[index]);
                    }
                }
                if !item.series.labels.is_empty() {
                    category_x = Some(labeled(&item.series));
                }
            }
            CurveKind::Vector => {
                let n = item.series.x.len().min(item.series.y.len());
                for i in 0..n {
                    let (x, y) = (item.series.x[i], item.series.y[i]);
                    if x.is_finite() && y.is_finite() {
                        x_auto.add(x);
                        y_auto.add(y);
                        let dx = item.series.dx.get(i).copied().unwrap_or(0.0);
                        let dy = item.series.dy.get(i).copied().unwrap_or(0.0);
                        if dx.is_finite() {
                            x_auto.add(x + dx);
                        }
                        if dy.is_finite() {
                            y_auto.add(y + dy);
                        }
                    }
                }
            }
            _ => {
                let n = item.series.x.len().min(item.series.y.len());
                for i in 0..n {
                    let (x, y) = (item.series.x[i], item.series.y[i]);
                    if !x.is_finite() || !y.is_finite() {
                        continue;
                    }
                    x_auto.add(x);
                    y_auto.add(y);
                    if let Some(err) = item.series.x_err.get(i).copied().filter(|v| v.is_finite()) {
                        x_auto.add(x - err);
                        x_auto.add(x + err);
                    }
                    if let Some(err) = item.series.y_err.get(i).copied().filter(|v| v.is_finite()) {
                        y_auto.add(y - err);
                        y_auto.add(y + err);
                    }
                }
            }
        }
    }

    let (x_min, x_max, x_note) = finish_range(&x_auto, &layer.x_axis, view.x_min, view.x_max);
    let (y_min, y_max, y_note) = finish_range(&y_auto, &layer.y_axis, view.y_min, view.y_max);
    push_note(scene, x_note);
    push_note(scene, y_note);
    let frame = LayerFrame {
        plot,
        x_min,
        x_max,
        y_min,
        y_max,
        x_scale: layer.x_axis.scale,
        y_scale: layer.y_axis.scale,
        x_inverted: layer.x_axis.inverted,
        y_inverted: layer.y_axis.inverted,
    };

    push_rect(scene, plot.x, plot.y, plot.w, plot.h, Some([252, 252, 252, 255]), None);
    if !only_pie {
        draw_grid(scene, &frame, &layer.x_axis, &layer.y_axis, category_x.is_some(), category_y.is_some());
    }

    let mut box_slot = 0.0;
    let mut legend: Vec<LegendEntry> = Vec::new();
    for item in &items {
        match item.kind {
            CurveKind::Pie => draw_pie(scene, plot, item, &mut legend, scale),
            CurveKind::Heatmap => draw_heatmap(scene, &frame, item),
            CurveKind::Contour => draw_contour(scene, &frame, item, z_span),
            CurveKind::Histogram => draw_histogram(scene, &frame, item),
            CurveKind::BoxPlot => {
                box_slot += 1.0;
                draw_box(scene, &frame, item, box_slot);
            }
            CurveKind::Bars => draw_bars(scene, &frame, item, false),
            CurveKind::HorizontalBars => draw_bars(scene, &frame, item, true),
            CurveKind::Area => draw_area(scene, &frame, item),
            CurveKind::Steps => draw_steps(scene, &frame, item),
            CurveKind::Vector => draw_vectors(scene, &frame, item),
            CurveKind::Scatter => draw_markers(scene, &frame, item),
            CurveKind::ErrorBars => {
                draw_errors(scene, &frame, item);
                draw_polyline(scene, &frame, item);
                draw_markers(scene, &frame, item);
            }
            CurveKind::LineSymbol => {
                draw_polyline(scene, &frame, item);
                draw_markers(scene, &frame, item);
            }
            _ => draw_polyline(scene, &frame, item),
        }
        if item.kind != CurveKind::Pie {
            legend.push(LegendEntry {
                name: item.name.clone(),
                color: item.color,
                line: legend_has_line(item.kind),
                hidden: false,
                curve: Some(item.curve),
            });
        }
    }
    legend.extend(hidden_legend);

    push_rect(scene, plot.x, plot.y, plot.w, plot.h, None, Some(([50, 50, 50, 255], 1.1 * scale)));
    if !only_pie {
        draw_axis_decor(
            scene,
            &frame,
            layer,
            category_x.as_deref(),
            category_y.as_deref(),
            scale,
        );
    }
    if !layer.title.is_empty() {
        text(
            scene,
            plot.x + plot.w * 0.5,
            fy + 4.0 * scale,
            &layer.title,
            layer.title_size * scale,
            [20, 20, 20, 255],
            HAlign::Center,
            VAlign::Top,
            false,
        );
    }
    if let Some((z0, z1)) = z_span {
        if wants_bar {
            draw_colorbar(scene, plot, z0, z1, scale);
        }
    }
    if layer.legend.visible {
        draw_legend(scene, plot, layer_index, &legend, layer.legend.position, scale);
    }
    for annotation in &layer.annotations {
        draw_annotation(scene, &frame, annotation, scale);
    }
    scene.layers.push(frame);
}

struct Range {
    scale: AxisScale,
    min: f64,
    max: f64,
    pos_min: f64,
    pos_max: f64,
    neg_min: f64,
    neg_max: f64,
    dropped: bool,
}

impl Range {
    fn new(scale: AxisScale) -> Self {
        Self {
            scale,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            pos_min: f64::INFINITY,
            pos_max: f64::NEG_INFINITY,
            neg_min: f64::INFINITY,
            neg_max: f64::NEG_INFINITY,
            dropped: false,
        }
    }

    fn add(&mut self, value: f64) {
        if !value.is_finite() {
            return;
        }
        match self.scale {
            AxisScale::Log10 => {
                if value > 0.0 {
                    self.min = self.min.min(value);
                    self.max = self.max.max(value);
                } else {
                    self.dropped = true;
                }
            }
            AxisScale::Reciprocal => {
                if value == 0.0 {
                    self.dropped = true;
                } else if value > 0.0 {
                    self.pos_min = self.pos_min.min(value);
                    self.pos_max = self.pos_max.max(value);
                } else {
                    self.neg_min = self.neg_min.min(value);
                    self.neg_max = self.neg_max.max(value);
                }
            }
            AxisScale::Linear => {
                self.min = self.min.min(value);
                self.max = self.max.max(value);
            }
        }
    }
}

fn resolved_auto(auto: &Range) -> (f64, f64, Option<&'static str>) {
    match auto.scale {
        AxisScale::Linear => (auto.min, auto.max, None),
        AxisScale::Log10 => {
            let note = if !auto.min.is_finite() { Some(sciplot_core::t("对数坐标没有正数数据")) } else { None };
            (auto.min, auto.max, note)
        }
        AxisScale::Reciprocal => {
            let has_pos = auto.pos_min.is_finite();
            let has_neg = auto.neg_min.is_finite();
            if has_pos && has_neg {
                let pos_abs = auto.pos_max.abs().max(auto.pos_min.abs());
                let neg_abs = auto.neg_min.abs().max(auto.neg_max.abs());
                if pos_abs >= neg_abs {
                    (auto.pos_min, auto.pos_max, Some(sciplot_core::t("倒数坐标不能穿过 0，已只用正值")))
                } else {
                    (auto.neg_min, auto.neg_max, Some(sciplot_core::t("倒数坐标不能穿过 0，已只用负值")))
                }
            } else if has_pos {
                (auto.pos_min, auto.pos_max, None)
            } else if has_neg {
                (auto.neg_min, auto.neg_max, None)
            } else {
                (f64::NAN, f64::NAN, if auto.dropped { Some(sciplot_core::t("倒数坐标没有可用数据")) } else { None })
            }
        }
    }
}

fn finish_range(auto: &Range, axis: &Axis, view_min: Option<f64>, view_max: Option<f64>) -> (f64, f64, Option<&'static str>) {
    let (data_min, data_max, mut note) = resolved_auto(auto);
    let (padded_min, padded_max) = if data_min.is_finite() && data_max.is_finite() {
        crate::scale::pad_range(axis.scale, data_min, data_max)
    } else {
        crate::scale::pad_range(axis.scale, f64::NAN, f64::NAN)
    };
    let user_min = view_min.or(axis.min);
    let user_max = view_max.or(axis.max);
    let mut lo = user_min.unwrap_or(padded_min);
    let mut hi = user_max.unwrap_or(padded_max);
    if axis.scale == AxisScale::Log10 {
        if lo <= 0.0 {
            note = Some(sciplot_core::t("对数坐标的最小值必须大于 0"));
            lo = if padded_min > 0.0 { padded_min } else { 0.1 };
        }
        if hi <= 0.0 {
            note = Some(sciplot_core::t("对数坐标的最大值必须大于 0"));
            hi = lo * 10.0;
        }
    }
    if axis.scale == AxisScale::Reciprocal && lo < 0.0 && hi > 0.0 {
        note = Some(sciplot_core::t("倒数坐标的范围不能穿过 0"));
        if lo.abs() >= hi.abs() {
            hi = (lo * 0.2).min(-1e-6);
            if hi <= lo {
                hi = lo - lo.abs().max(1.0);
            }
        } else {
            lo = (hi * 0.2).max(1e-6);
            if lo >= hi {
                lo = hi * 0.5;
            }
        }
    }
    if hi <= lo {
        if axis.scale == AxisScale::Log10 {
            hi = lo * 10.0;
        } else {
            let span = (padded_max - padded_min).abs().max(1.0);
            if user_max.is_none() {
                hi = lo + span;
            } else if user_min.is_none() {
                lo = hi - span;
            } else {
                hi = lo + span;
            }
        }
    }
    (lo, hi, note)
}

fn push_note(scene: &mut Scene, note: Option<&str>) {
    let Some(note) = note else {
        return;
    };
    if scene.notes.iter().any(|existing| existing == note) {
        return;
    }
    scene.notes.push(note.to_string());
}

fn draw_notes(scene: &mut Scene, scale: f32) {
    if scene.notes.is_empty() {
        return;
    }
    let notes: Vec<String> = scene.notes.iter().take(4).cloned().collect();
    let banner = (16.0 * notes.len() as f32 + 8.0) * scale;
    push_rect(scene, 0.0, 0.0, scene.width, banner, Some([255, 243, 224, 230]), None);
    let mut y = 4.0 * scale;
    for note in &notes {
        text(scene, 8.0 * scale, y, note, 12.0 * scale, [150, 45, 30, 255], HAlign::Left, VAlign::Top, false);
        y += 16.0 * scale;
    }
}

fn labeled(series: &SeriesData) -> Vec<(f64, String)> {
    series
        .x
        .iter()
        .zip(series.labels.iter())
        .filter(|(x, label)| x.is_finite() && !label.is_empty())
        .map(|(x, label)| (*x, label.clone()))
        .collect()
}

fn hist_source(item: &Item) -> &[f64] {
    if item.series.y.iter().any(|v| v.is_finite()) {
        &item.series.y
    } else {
        &item.series.x
    }
}

fn finite_values(values: &[f64]) -> Vec<f64> {
    values.iter().copied().filter(|v| v.is_finite()).collect()
}

fn histogram(values: &[f64], bins: usize) -> Option<(Vec<f64>, Vec<f64>)> {
    if values.is_empty() {
        return None;
    }
    let mut lo = values.iter().copied().fold(f64::INFINITY, f64::min);
    let mut hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if (hi - lo).abs() < 1e-12 {
        lo -= 0.5;
        hi += 0.5;
    }
    let bins = bins.max(1);
    let width = (hi - lo) / bins as f64;
    let mut counts = vec![0.0; bins];
    for value in values {
        let mut bin = ((*value - lo) / width).floor() as isize;
        if bin < 0 {
            bin = 0;
        }
        if bin >= bins as isize {
            bin = bins as isize - 1;
        }
        counts[bin as usize] += 1.0;
    }
    let edges: Vec<f64> = (0..=bins).map(|i| lo + width * i as f64).collect();
    Some((edges, counts))
}

fn map_xy(frame: &LayerFrame, x: f64, y: f64) -> Option<(f32, f32)> {
    let ux = unit(frame.x_scale, x, frame.x_min, frame.x_max, frame.x_inverted)?;
    let uy = unit(frame.y_scale, y, frame.y_min, frame.y_max, frame.y_inverted)?;
    Some((
        frame.plot.x + ux * frame.plot.w,
        frame.plot.y + frame.plot.h - uy * frame.plot.h,
    ))
}

fn draw_grid(scene: &mut Scene, frame: &LayerFrame, x_axis: &Axis, y_axis: &Axis, cat_x: bool, cat_y: bool) {
    if x_axis.major_grid && !cat_x {
        for tick in axis_ticks(scene, x_axis, frame.x_min, frame.x_max) {
            if !tick.major {
                continue;
            }
            if let Some((px, _)) = map_xy(frame, tick.value, frame.y_min) {
                vline(scene, frame, px, [226, 226, 226, 255], 1.0);
            }
        }
    }
    if y_axis.major_grid && !cat_y {
        for tick in axis_ticks(scene, y_axis, frame.y_min, frame.y_max) {
            if !tick.major {
                continue;
            }
            if let Some((_, py)) = map_xy(frame, frame.x_min, tick.value) {
                hline(scene, frame, py, [226, 226, 226, 255], 1.0);
            }
        }
    }
    if x_axis.minor_grid && !cat_x {
        let majors: Vec<Tick> = axis_ticks(scene, x_axis, frame.x_min, frame.x_max)
            .into_iter()
            .filter(|tick| tick.major)
            .collect();
        for tick in minor_ticks_for(x_axis.scale, &majors, minor_parts(x_axis)) {
            if let Some((px, _)) = map_xy(frame, tick.value, frame.y_min) {
                vline(scene, frame, px, [238, 238, 238, 255], 1.0);
            }
        }
    }
    if y_axis.minor_grid && !cat_y {
        let majors: Vec<Tick> = axis_ticks(scene, y_axis, frame.y_min, frame.y_max)
            .into_iter()
            .filter(|tick| tick.major)
            .collect();
        for tick in minor_ticks_for(y_axis.scale, &majors, minor_parts(y_axis)) {
            if let Some((_, py)) = map_xy(frame, frame.x_min, tick.value) {
                hline(scene, frame, py, [238, 238, 238, 255], 1.0);
            }
        }
    }
}

fn minor_parts(axis: &Axis) -> usize {
    axis.minor_count.unwrap_or(4).max(1) as usize
}

fn axis_ticks(scene: &mut Scene, axis: &Axis, min: f64, max: f64) -> Vec<Tick> {
    let (ticks, note) = ticks_for_axis(axis, min, max);
    push_note(scene, note);
    ticks
}

fn draw_axis_decor(
    scene: &mut Scene,
    frame: &LayerFrame,
    layer: &sciplot_core::Layer,
    category_x: Option<&[(f64, String)]>,
    category_y: Option<&[(f64, String)]>,
    scale: f32,
) {
    let tick = 5.0 * scale;
    if let Some(labels) = category_x {
        for (x, label) in labels {
            if let Some((px, py)) = map_xy(frame, *x, frame.y_min) {
                line(scene, px, frame.plot.bottom(), px, frame.plot.bottom() + tick, [40, 40, 40, 255], scale, Dash::Solid);
                text(scene, px, frame.plot.bottom() + tick + 2.0 * scale, label, layer.x_axis.tick_size * scale, [30, 30, 30, 255], HAlign::Center, VAlign::Top, false);
                let _ = py;
            }
        }
    } else {
        for tick_mark in axis_ticks(scene, &layer.x_axis, frame.x_min, frame.x_max) {
            if !tick_mark.major {
                continue;
            }
            if let Some((px, _)) = map_xy(frame, tick_mark.value, frame.y_min) {
                if px < frame.plot.x - 1.0 || px > frame.plot.right() + 1.0 {
                    continue;
                }
                line(scene, px, frame.plot.bottom(), px, frame.plot.bottom() + tick, [40, 40, 40, 255], scale, Dash::Solid);
                text(
                    scene,
                    px,
                    frame.plot.bottom() + tick + 2.0 * scale,
                    &format_number(tick_mark.value),
                    layer.x_axis.tick_size * scale,
                    [30, 30, 30, 255],
                    HAlign::Center,
                    VAlign::Top,
                    false,
                );
            }
        }
    }
    if let Some(labels) = category_y {
        for (y, label) in labels {
            if let Some((px, py)) = map_xy(frame, frame.x_min, *y) {
                line(scene, frame.plot.x - tick, py, frame.plot.x, py, [40, 40, 40, 255], scale, Dash::Solid);
                text(scene, frame.plot.x - tick - 3.0 * scale, py, label, layer.y_axis.tick_size * scale, [30, 30, 30, 255], HAlign::Right, VAlign::Center, false);
                let _ = px;
            }
        }
    } else {
        for tick_mark in axis_ticks(scene, &layer.y_axis, frame.y_min, frame.y_max) {
            if !tick_mark.major {
                continue;
            }
            if let Some((_, py)) = map_xy(frame, frame.x_min, tick_mark.value) {
                if py < frame.plot.y - 1.0 || py > frame.plot.bottom() + 1.0 {
                    continue;
                }
                line(scene, frame.plot.x - tick, py, frame.plot.x, py, [40, 40, 40, 255], scale, Dash::Solid);
                text(
                    scene,
                    frame.plot.x - tick - 3.0 * scale,
                    py,
                    &format_number(tick_mark.value),
                    layer.y_axis.tick_size * scale,
                    [30, 30, 30, 255],
                    HAlign::Right,
                    VAlign::Center,
                    false,
                );
            }
        }
    }
    if !layer.x_axis.label.is_empty() {
        text(
            scene,
            frame.plot.x + frame.plot.w * 0.5,
            frame.plot.bottom() + 28.0 * scale,
            &layer.x_axis.label,
            layer.x_axis.label_size * scale,
            [20, 20, 20, 255],
            HAlign::Center,
            VAlign::Top,
            false,
        );
    }
    if !layer.y_axis.label.is_empty() {
        text(
            scene,
            frame.plot.x - 52.0 * scale,
            frame.plot.y + frame.plot.h * 0.5,
            &layer.y_axis.label,
            layer.y_axis.label_size * scale,
            [20, 20, 20, 255],
            HAlign::Center,
            VAlign::Center,
            true,
        );
    }
}

fn draw_polyline(scene: &mut Scene, frame: &LayerFrame, item: &Item) {
    let pts = mapped_pairs(frame, &item.series);
    for part in clip_polyline(frame.plot, &pts) {
        if part.len() >= 2 {
            scene.primitives.push(Prim::Polyline {
                pts: part,
                color: item.color,
                width: item.width,
                dash: item.dash,
            });
        }
    }
}

fn draw_markers(scene: &mut Scene, frame: &LayerFrame, item: &Item) {
    if item.symbol == Symbol::None && item.kind != CurveKind::Scatter {
        return;
    }
    let symbol = if item.symbol == Symbol::None { Symbol::Circle } else { item.symbol };
    let n = item.series.x.len().min(item.series.y.len());
    for i in 0..n {
        if let Some((px, py)) = map_xy(frame, item.series.x[i], item.series.y[i]) {
            if frame.plot.contains(px, py) {
                draw_symbol(scene, symbol, px, py, item.symbol_size, item.color);
            }
        }
    }
}

fn draw_steps(scene: &mut Scene, frame: &LayerFrame, item: &Item) {
    let n = item.series.x.len().min(item.series.y.len());
    let mut raw: Vec<(f32, f32)> = Vec::new();
    for i in 0..n {
        if let Some(p) = map_xy(frame, item.series.x[i], item.series.y[i]) {
            if let Some(prev) = raw.last().copied() {
                raw.push((p.0, prev.1));
            }
            raw.push(p);
        }
    }
    for part in clip_polyline(frame.plot, &raw) {
        if part.len() >= 2 {
            scene.primitives.push(Prim::Polyline {
                pts: part,
                color: item.color,
                width: item.width,
                dash: item.dash,
            });
        }
    }
}

fn draw_area(scene: &mut Scene, frame: &LayerFrame, item: &Item) {
    let pts = mapped_pairs(frame, &item.series);
    let baseline = if frame.y_scale == AxisScale::Linear { 0.0 } else { frame.y_min };
    let Some((_, base_y)) = map_xy(frame, frame.x_min, baseline) else {
        draw_polyline(scene, frame, item);
        return;
    };
    let base_y = base_y.clamp(frame.plot.y, frame.plot.bottom());
    for part in clip_polyline(frame.plot, &pts) {
        if part.len() < 2 {
            continue;
        }
        let mut poly = part.clone();
        let last = *part.last().unwrap();
        let first = part[0];
        poly.push((last.0, base_y));
        poly.push((first.0, base_y));
        if let Some(fill) = item.fill {
            scene.primitives.push(Prim::Polygon { pts: poly, fill });
        }
    }
    draw_polyline(scene, frame, item);
}

fn draw_errors(scene: &mut Scene, frame: &LayerFrame, item: &Item) {
    let n = item.series.x.len().min(item.series.y.len());
    let cap = item.symbol_size * 0.7;
    for i in 0..n {
        let (x, y) = (item.series.x[i], item.series.y[i]);
        if let Some(err) = item.series.y_err.get(i).copied().filter(|v| v.is_finite()) {
            if let (Some(a), Some(b)) = (map_xy(frame, x, y - err), map_xy(frame, x, y + err)) {
                for part in clip_polyline(frame.plot, &[a, b]) {
                    if part.len() == 2 {
                        line(scene, part[0].0, part[0].1, part[1].0, part[1].1, item.color, item.width, Dash::Solid);
                        line(scene, part[0].0 - cap, part[0].1, part[0].0 + cap, part[0].1, item.color, item.width, Dash::Solid);
                        line(scene, part[1].0 - cap, part[1].1, part[1].0 + cap, part[1].1, item.color, item.width, Dash::Solid);
                    }
                }
            }
        }
        if let Some(err) = item.series.x_err.get(i).copied().filter(|v| v.is_finite()) {
            if let (Some(a), Some(b)) = (map_xy(frame, x - err, y), map_xy(frame, x + err, y)) {
                for part in clip_polyline(frame.plot, &[a, b]) {
                    if part.len() == 2 {
                        line(scene, part[0].0, part[0].1, part[1].0, part[1].1, item.color, item.width, Dash::Solid);
                    }
                }
            }
        }
    }
}

fn draw_bars(scene: &mut Scene, frame: &LayerFrame, item: &Item, horizontal: bool) {
    let n = item.series.x.len().min(item.series.y.len()).max(item.series.x.len()).max(item.series.y.len());
    let xs: Vec<f64> = if horizontal { item.series.y.clone() } else { item.series.x.clone() };
    let mut sorted: Vec<f64> = xs.iter().copied().filter(|v| v.is_finite()).collect();
    sorted.sort_by(|a, b| a.total_cmp(b));
    sorted.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    let gap = sorted.windows(2).map(|w| w[1] - w[0]).filter(|d| *d > 0.0).fold(f64::INFINITY, f64::min);
    let gap = if gap.is_finite() { gap } else { 1.0 };
    let half = gap * 0.35;
    let baseline = if (if horizontal { frame.x_scale } else { frame.y_scale }) == AxisScale::Linear {
        0.0
    } else if horizontal {
        frame.x_min
    } else {
        frame.y_min
    };
    let fill = item.fill.unwrap_or(with_alpha(item.color, 170));
    for i in 0..n {
        let x = item.series.x.get(i).copied().unwrap_or(f64::NAN);
        let y = item.series.y.get(i).copied().unwrap_or(f64::NAN);
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        let (x0, y0, x1, y1) = if horizontal {
            (baseline, x - half, y, x + half)
        } else {
            (x - half, baseline, x + half, y)
        };
        let Some(a) = map_xy(frame, x0, y0) else { continue };
        let Some(b) = map_xy(frame, x1, y1) else { continue };
        let rx = a.0.min(b.0);
        let ry = a.1.min(b.1);
        let rw = (a.0 - b.0).abs().max(1.0);
        let rh = (a.1 - b.1).abs().max(1.0);
        if let Some((cx, cy, cw, ch)) = clip_rect(frame.plot, rx, ry, rw, rh) {
            push_rect(scene, cx, cy, cw, ch, Some(fill), Some((item.color, item.width.max(1.0))));
        }
    }
}

fn draw_histogram(scene: &mut Scene, frame: &LayerFrame, item: &Item) {
    let values = finite_values(hist_source(item));
    let Some((edges, counts)) = histogram(&values, item.bins) else {
        return;
    };
    let fill = item.fill.unwrap_or(with_alpha(item.color, 150));
    for (index, count) in counts.iter().enumerate() {
        let Some(a) = map_xy(frame, edges[index], 0.0) else { continue };
        let Some(b) = map_xy(frame, edges[index + 1], *count) else { continue };
        let rx = a.0.min(b.0);
        let ry = a.1.min(b.1);
        if let Some((cx, cy, cw, ch)) = clip_rect(frame.plot, rx, ry, (b.0 - a.0).abs(), (b.1 - a.1).abs()) {
            push_rect(scene, cx, cy, cw, ch, Some(fill), Some((item.color, 1.0)));
        }
    }
}

fn draw_box(scene: &mut Scene, frame: &LayerFrame, item: &Item, slot: f64) {
    let mut values = finite_values(&item.series.y);
    if values.is_empty() {
        return;
    }
    values.sort_by(|a, b| a.total_cmp(b));
    let q1 = quantile(&values, 0.25);
    let med = quantile(&values, 0.5);
    let q3 = quantile(&values, 0.75);
    let iqr = q3 - q1;
    let low = values.iter().copied().filter(|v| *v >= q1 - 1.5 * iqr).fold(f64::INFINITY, f64::min);
    let high = values.iter().copied().filter(|v| *v <= q3 + 1.5 * iqr).fold(f64::NEG_INFINITY, f64::max);
    let half = 0.28;
    let fill = item.fill.unwrap_or(with_alpha(item.color, 80));
    if let (Some(a), Some(b)) = (map_xy(frame, slot - half, q1), map_xy(frame, slot + half, q3)) {
        let rx = a.0.min(b.0);
        let ry = a.1.min(b.1);
        push_rect(scene, rx, ry, (b.0 - a.0).abs(), (b.1 - a.1).abs(), Some(fill), Some((item.color, item.width)));
    }
    if let (Some(a), Some(b)) = (map_xy(frame, slot - half, med), map_xy(frame, slot + half, med)) {
        line(scene, a.0, a.1, b.0, b.1, item.color, item.width + 0.6, Dash::Solid);
    }
    if let (Some(a), Some(b)) = (map_xy(frame, slot, low), map_xy(frame, slot, q1)) {
        line(scene, a.0, a.1, b.0, b.1, item.color, item.width, Dash::Solid);
    }
    if let (Some(a), Some(b)) = (map_xy(frame, slot, q3), map_xy(frame, slot, high)) {
        line(scene, a.0, a.1, b.0, b.1, item.color, item.width, Dash::Solid);
    }
    if let (Some(a), Some(b)) = (map_xy(frame, slot - half * 0.6, low), map_xy(frame, slot + half * 0.6, low)) {
        line(scene, a.0, a.1, b.0, b.1, item.color, item.width, Dash::Solid);
    }
    if let (Some(a), Some(b)) = (map_xy(frame, slot - half * 0.6, high), map_xy(frame, slot + half * 0.6, high)) {
        line(scene, a.0, a.1, b.0, b.1, item.color, item.width, Dash::Solid);
    }
    for value in &values {
        if *value < low || *value > high {
            if let Some((px, py)) = map_xy(frame, slot, *value) {
                scene.primitives.push(Prim::Circle {
                    x: px,
                    y: py,
                    r: 2.4,
                    fill: None,
                    stroke: Some((item.color, 1.2)),
                });
            }
        }
    }
}

fn draw_pie(scene: &mut Scene, plot: PxRect, item: &Item, legend: &mut Vec<LegendEntry>, scale: f32) {
    let values: Vec<(f64, String)> = item
        .series
        .y
        .iter()
        .enumerate()
        .filter(|(_, v)| v.is_finite() && **v > 0.0)
        .map(|(i, v)| {
            let label = item.series.labels.get(i).cloned().filter(|s| !s.is_empty()).unwrap_or_else(|| format!("{}", i + 1));
            (*v, label)
        })
        .collect();
    let total: f64 = values.iter().map(|(v, _)| *v).sum();
    if total <= 0.0 {
        return;
    }
    let cx = plot.x + plot.w * 0.5;
    let cy = plot.y + plot.h * 0.5;
    let radius = plot.w.min(plot.h) * 0.34;
    let mut angle = -std::f32::consts::FRAC_PI_2;
    for (index, (value, label)) in values.iter().enumerate() {
        let sweep = (*value / total) as f32 * std::f32::consts::TAU;
        let color = palette_color(index);
        let mut poly = vec![(cx, cy)];
        let steps = ((sweep.abs() * 18.0).ceil() as usize).clamp(2, 48);
        for step in 0..=steps {
            let t = angle + sweep * step as f32 / steps as f32;
            poly.push((cx + t.cos() * radius, cy + t.sin() * radius));
        }
        scene.primitives.push(Prim::Polygon { pts: poly, fill: color });
        let mid = angle + sweep * 0.5;
        text(
            scene,
            cx + mid.cos() * radius * 0.62,
            cy + mid.sin() * radius * 0.62,
            &format_number(*value),
            11.0 * scale,
            [255, 255, 255, 255],
            HAlign::Center,
            VAlign::Center,
            false,
        );
        legend.push(LegendEntry {
            name: label.clone(),
            color,
            line: false,
            hidden: false,
            curve: Some(item.curve),
        });
        angle += sweep;
    }
}

fn draw_heatmap(scene: &mut Scene, frame: &LayerFrame, item: &Item) {
    let Some(grid) = &item.series.grid else { return };
    if grid.rows == 0 || grid.cols == 0 {
        return;
    }
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for value in &grid.z {
        if value.is_finite() {
            lo = lo.min(*value);
            hi = hi.max(*value);
        }
    }
    if !lo.is_finite() {
        return;
    }
    let span = (hi - lo).abs().max(1e-12);
    for row in 0..grid.rows {
        for col in 0..grid.cols {
            let z = grid.at(row, col);
            if !z.is_finite() {
                continue;
            }
            let x0 = cell_edge(grid.x0, grid.x1, col, grid.cols, false);
            let x1 = cell_edge(grid.x0, grid.x1, col, grid.cols, true);
            let y0 = cell_edge(grid.y0, grid.y1, row, grid.rows, false);
            let y1 = cell_edge(grid.y0, grid.y1, row, grid.rows, true);
            let Some(a) = map_xy(frame, x0, y0) else { continue };
            let Some(b) = map_xy(frame, x1, y1) else { continue };
            let t = ((z - lo) / span) as f32;
            if let Some((x, y, w, h)) = clip_rect(frame.plot, a.0.min(b.0), a.1.min(b.1), (a.0 - b.0).abs().max(1.0), (a.1 - b.1).abs().max(1.0)) {
                push_rect(scene, x, y, w, h, Some(viridis(t)), None);
            }
        }
    }
}

fn draw_contour(scene: &mut Scene, frame: &LayerFrame, item: &Item, z_span: Option<(f64, f64)>) {
    let Some(grid) = &item.series.grid else { return };
    let (lo, hi) = z_span.unwrap_or((0.0, 1.0));
    if hi <= lo {
        return;
    }
    let levels = 8;
    for level_index in 1..=levels {
        let level = lo + (hi - lo) * level_index as f64 / (levels as f64 + 1.0);
        let color = palette_color(level_index);
        for (a, b) in contour_segments(grid, level) {
            if let (Some(p), Some(q)) = (map_xy(frame, a.0, a.1), map_xy(frame, b.0, b.1)) {
                for part in clip_polyline(frame.plot, &[p, q]) {
                    if part.len() == 2 {
                        line(scene, part[0].0, part[0].1, part[1].0, part[1].1, color, item.width, Dash::Solid);
                    }
                }
            }
        }
    }
}

fn draw_vectors(scene: &mut Scene, frame: &LayerFrame, item: &Item) {
    let n = item.series.x.len().min(item.series.y.len());
    for i in 0..n {
        let (x, y) = (item.series.x[i], item.series.y[i]);
        let dx = item.series.dx.get(i).copied().unwrap_or(0.0);
        let dy = item.series.dy.get(i).copied().unwrap_or(0.0);
        if let (Some(a), Some(b)) = (map_xy(frame, x, y), map_xy(frame, x + dx, y + dy)) {
            for part in clip_polyline(frame.plot, &[a, b]) {
                if part.len() == 2 {
                    line(scene, part[0].0, part[0].1, part[1].0, part[1].1, item.color, item.width, Dash::Solid);
                    arrow_head(scene, part[0], part[1], item.color, 8.0);
                }
            }
        }
    }
}

fn draw_annotation(scene: &mut Scene, frame: &LayerFrame, annotation: &sciplot_core::Annotation, scale: f32) {
    use sciplot_core::AnnotationKind;
    match &annotation.kind {
        AnnotationKind::Text { x, y, text: label, size, color } => {
            if let Some((px, py)) = map_xy(frame, *x, *y) {
                text(scene, px, py, label, *size * scale, *color, HAlign::Left, VAlign::Bottom, false);
            }
        }
        AnnotationKind::Arrow { x1, y1, x2, y2, color } => {
            if let (Some(a), Some(b)) = (map_xy(frame, *x1, *y1), map_xy(frame, *x2, *y2)) {
                line(scene, a.0, a.1, b.0, b.1, *color, 1.4 * scale, Dash::Solid);
                arrow_head(scene, a, b, *color, 9.0 * scale);
            }
        }
        AnnotationKind::Line { x1, y1, x2, y2, color } => {
            if let (Some(a), Some(b)) = (map_xy(frame, *x1, *y1), map_xy(frame, *x2, *y2)) {
                line(scene, a.0, a.1, b.0, b.1, *color, 1.4 * scale, Dash::Solid);
            }
        }
        AnnotationKind::Marker { x, y, label, color } => {
            if let Some((px, py)) = map_xy(frame, *x, *y) {
                scene.primitives.push(Prim::Circle {
                    x: px,
                    y: py,
                    r: 5.0 * scale,
                    fill: Some(*color),
                    stroke: Some(([255, 255, 255, 255], 1.2 * scale)),
                });
                if !label.is_empty() {
                    let size = 12.0 * scale;
                    let gap = 7.0 * scale;
                    let width = label.chars().count() as f32 * size * 0.62;
                    let (tx, halign) = if px + gap + width > frame.plot.right() {
                        (px - gap, HAlign::Right)
                    } else {
                        (px + gap, HAlign::Left)
                    };
                    let (ty, valign) = if py - gap - size < frame.plot.y {
                        (py + gap, VAlign::Top)
                    } else {
                        (py - gap, VAlign::Bottom)
                    };
                    text(scene, tx, ty, label, size, *color, halign, valign, false);
                }
            }
        }
    }
}

fn draw_legend(scene: &mut Scene, plot: PxRect, layer_index: usize, entries: &[LegendEntry], pos: LegendPos, scale: f32) {
    if entries.is_empty() {
        return;
    }
    let font = 12.0 * scale;
    let row = 18.0 * scale;
    let pad = 6.0 * scale;
    let swatch = 18.0 * scale;
    let text_w = entries.iter().map(|entry| text_width(&entry.name, font)).fold(0.0, f32::max);
    let width = swatch + text_w + pad * 3.0;
    let height = entries.len() as f32 * row + pad;
    let (x, y) = match pos {
        LegendPos::TopRight => (plot.right() - width - 8.0 * scale, plot.y + 8.0 * scale),
        LegendPos::TopLeft => (plot.x + 8.0 * scale, plot.y + 8.0 * scale),
        LegendPos::BottomRight => (plot.right() - width - 8.0 * scale, plot.bottom() - height - 8.0 * scale),
        LegendPos::BottomLeft => (plot.x + 8.0 * scale, plot.bottom() - height - 8.0 * scale),
    };
    push_rect(scene, x, y, width, height, Some([255, 255, 255, 230]), Some(([160, 160, 160, 255], 1.0)));
    for (index, entry) in entries.iter().enumerate() {
        let top = y + pad * 0.5 + row * index as f32;
        let cy = top + row * 0.5;
        let x0 = x + pad;
        let color = if entry.hidden { [150, 150, 150, 255] } else { entry.color };
        let ink = if entry.hidden { [120, 120, 120, 255] } else { [20, 20, 20, 255] };
        if entry.line {
            line_prim(scene, x0, cy, x0 + swatch, cy, color, 1.6 * scale, Dash::Solid);
        }
        scene.primitives.push(Prim::Circle {
            x: x0 + swatch * 0.5,
            y: cy,
            r: 3.2 * scale,
            fill: Some(color),
            stroke: None,
        });
        text(scene, x0 + swatch + pad, cy, &entry.name, font, ink, HAlign::Left, VAlign::Center, false);
        if let Some(curve) = entry.curve {
            scene.legend_hits.push(crate::scene::LegendHit {
                layer: layer_index,
                curve,
                rect: PxRect { x, y: top, w: width, h: row },
            });
        }
    }
}

fn legend_has_line(kind: CurveKind) -> bool {
    matches!(
        kind,
        CurveKind::Line | CurveKind::LineSymbol | CurveKind::Steps | CurveKind::Function | CurveKind::Area | CurveKind::ErrorBars
    )
}

fn draw_colorbar(scene: &mut Scene, plot: PxRect, z0: f64, z1: f64, scale: f32) {
    let x = plot.right() + 12.0 * scale;
    let y = plot.y;
    let w = 12.0 * scale;
    let h = plot.h;
    let steps = 48;
    for i in 0..steps {
        let t = 1.0 - i as f32 / steps as f32;
        let yy = y + h * i as f32 / steps as f32;
        let hh = h / steps as f32 + 0.5;
        push_rect(scene, x, yy, w, hh, Some(viridis(t)), None);
    }
    push_rect(scene, x, y, w, h, None, Some(([80, 80, 80, 255], 1.0)));
    let span = (z1 - z0).abs().max(1e-12);
    for tick in crate::scale::ticks_for(sciplot_core::AxisScale::Linear, z0, z1) {
        if !tick.major {
            continue;
        }
        let t = ((tick.value - z0) / span) as f32;
        let yy = y + h * (1.0 - t.clamp(0.0, 1.0));
        text(scene, x + w + 4.0 * scale, yy, &format_number(tick.value), 11.0 * scale, [30, 30, 30, 255], HAlign::Left, VAlign::Center, false);
    }
}

fn mapped_pairs(frame: &LayerFrame, series: &SeriesData) -> Vec<(f32, f32)> {
    let n = series.x.len().min(series.y.len());
    let mut pts = Vec::new();
    for i in 0..n {
        if let Some(p) = map_xy(frame, series.x[i], series.y[i]) {
            pts.push(p);
        } else if !pts.is_empty() {
            pts.push((f32::NAN, f32::NAN));
        }
    }
    pts
}

fn draw_symbol(scene: &mut Scene, symbol: Symbol, x: f32, y: f32, size: f32, color: [u8; 4]) {
    let r = size * 0.5;
    match symbol {
        Symbol::None => {}
        Symbol::Circle => scene.primitives.push(Prim::Circle {
            x,
            y,
            r,
            fill: Some(color),
            stroke: Some(([255, 255, 255, 255], 1.0)),
        }),
        Symbol::Square => push_rect(scene, x - r, y - r, size, size, Some(color), Some(([255, 255, 255, 255], 1.0))),
        Symbol::Diamond => scene.primitives.push(Prim::Polygon {
            pts: vec![(x, y - r), (x + r, y), (x, y + r), (x - r, y)],
            fill: color,
        }),
        Symbol::Triangle => scene.primitives.push(Prim::Polygon {
            pts: vec![(x, y - r), (x + r, y + r * 0.8), (x - r, y + r * 0.8)],
            fill: color,
        }),
        Symbol::Cross => {
            line(scene, x - r, y - r, x + r, y + r, color, 1.4, Dash::Solid);
            line(scene, x - r, y + r, x + r, y - r, color, 1.4, Dash::Solid);
        }
        Symbol::Plus => {
            line(scene, x - r, y, x + r, y, color, 1.4, Dash::Solid);
            line(scene, x, y - r, x, y + r, color, 1.4, Dash::Solid);
        }
    }
}

fn arrow_head(scene: &mut Scene, from: (f32, f32), to: (f32, f32), color: [u8; 4], size: f32) {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1.0 {
        return;
    }
    let ux = dx / len;
    let uy = dy / len;
    let px = -uy;
    let py = ux;
    let base = (to.0 - ux * size, to.1 - uy * size);
    scene.primitives.push(Prim::Triangle {
        a: to,
        b: (base.0 + px * size * 0.45, base.1 + py * size * 0.45),
        c: (base.0 - px * size * 0.45, base.1 - py * size * 0.45),
        fill: color,
    });
}

fn quantile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let pos = q * (sorted.len() - 1) as f64;
    let i = pos.floor() as usize;
    let frac = pos - i as f64;
    let j = (i + 1).min(sorted.len() - 1);
    sorted[i] * (1.0 - frac) + sorted[j] * frac
}

fn cell_edge(min: f64, max: f64, index: usize, count: usize, upper: bool) -> f64 {
    if count <= 1 {
        return if upper { max } else { min };
    }
    let step = (max - min) / (count as f64 - 1.0);
    let center = min + step * index as f64;
    if upper {
        if index + 1 == count { max } else { center + step * 0.5 }
    } else if index == 0 {
        min
    } else {
        center - step * 0.5
    }
}

fn text_width(text: &str, size: f32) -> f32 {
    text.chars().map(|ch| if ch.is_ascii() { 0.58 } else { 1.0 }).sum::<f32>() * size
}

fn hline(scene: &mut Scene, frame: &LayerFrame, y: f32, color: [u8; 4], width: f32) {
    if y >= frame.plot.y && y <= frame.plot.bottom() {
        line(scene, frame.plot.x, y, frame.plot.right(), y, color, width, Dash::Solid);
    }
}

fn vline(scene: &mut Scene, frame: &LayerFrame, x: f32, color: [u8; 4], width: f32) {
    if x >= frame.plot.x && x <= frame.plot.right() {
        line(scene, x, frame.plot.y, x, frame.plot.bottom(), color, width, Dash::Solid);
    }
}

#[allow(clippy::too_many_arguments)]
fn line(scene: &mut Scene, x1: f32, y1: f32, x2: f32, y2: f32, color: [u8; 4], width: f32, dash: Dash) {
    line_prim(scene, x1, y1, x2, y2, color, width, dash);
}

#[allow(clippy::too_many_arguments)]
fn line_prim(scene: &mut Scene, x1: f32, y1: f32, x2: f32, y2: f32, color: [u8; 4], width: f32, dash: Dash) {
    scene.primitives.push(Prim::Line { x1, y1, x2, y2, color, width, dash });
}

fn push_rect(scene: &mut Scene, x: f32, y: f32, w: f32, h: f32, fill: Option<[u8; 4]>, stroke: Option<([u8; 4], f32)>) {
    scene.primitives.push(Prim::Rect { x, y, w, h, fill, stroke });
}

#[allow(clippy::too_many_arguments)]
fn text(scene: &mut Scene, x: f32, y: f32, value: &str, size: f32, color: [u8; 4], halign: HAlign, valign: VAlign, vertical: bool) {
    if value.is_empty() {
        return;
    }
    scene.primitives.push(Prim::Text {
        x,
        y,
        text: value.to_string(),
        size,
        color,
        halign,
        valign,
        vertical,
    });
}

fn clip_rect(plot: PxRect, x: f32, y: f32, w: f32, h: f32) -> Option<(f32, f32, f32, f32)> {
    let x2 = x + w;
    let y2 = y + h;
    let nx = x.max(plot.x);
    let ny = y.max(plot.y);
    let nx2 = x2.min(plot.right());
    let ny2 = y2.min(plot.bottom());
    if nx2 - nx < 0.4 || ny2 - ny < 0.4 {
        return None;
    }
    Some((nx, ny, nx2 - nx, ny2 - ny))
}

fn clip_polyline(plot: PxRect, pts: &[(f32, f32)]) -> Vec<Vec<(f32, f32)>> {
    let mut parts = Vec::new();
    let mut current: Vec<(f32, f32)> = Vec::new();
    let mut previous: Option<(f32, f32)> = None;
    for point in pts {
        if !point.0.is_finite() || !point.1.is_finite() {
            if current.len() >= 2 {
                parts.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
            previous = None;
            continue;
        }
        if let Some(prev) = previous {
            if let Some((a, b)) = clip_segment(plot, prev, *point) {
                if current.last().map(|p| (p.0 - a.0).hypot(p.1 - a.1) > 0.4).unwrap_or(true) {
                    if current.len() >= 2 {
                        parts.push(std::mem::take(&mut current));
                    } else {
                        current.clear();
                    }
                    current.push(a);
                }
                current.push(b);
            } else if current.len() >= 2 {
                parts.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
        }
        previous = Some(*point);
    }
    if current.len() >= 2 {
        parts.push(current);
    }
    parts
}

fn clip_segment(plot: PxRect, mut a: (f32, f32), mut b: (f32, f32)) -> Option<((f32, f32), (f32, f32))> {
    let mut ca = code(plot, a);
    let mut cb = code(plot, b);
    for _ in 0..8 {
        if ca | cb == 0 {
            return Some((a, b));
        }
        if ca & cb != 0 {
            return None;
        }
        let c = if ca != 0 { ca } else { cb };
        let (x, y) = if c & 8 != 0 {
            let y = plot.bottom();
            (a.0 + (b.0 - a.0) * (y - a.1) / (b.1 - a.1), y)
        } else if c & 4 != 0 {
            let y = plot.y;
            (a.0 + (b.0 - a.0) * (y - a.1) / (b.1 - a.1), y)
        } else if c & 2 != 0 {
            let x = plot.right();
            (x, a.1 + (b.1 - a.1) * (x - a.0) / (b.0 - a.0))
        } else {
            let x = plot.x;
            (x, a.1 + (b.1 - a.1) * (x - a.0) / (b.0 - a.0))
        };
        if c == ca {
            a = (x, y);
            ca = code(plot, a);
        } else {
            b = (x, y);
            cb = code(plot, b);
        }
    }
    None
}

fn code(plot: PxRect, p: (f32, f32)) -> u8 {
    let mut c = 0;
    if p.0 < plot.x {
        c |= 1;
    } else if p.0 > plot.right() {
        c |= 2;
    }
    if p.1 < plot.y {
        c |= 4;
    } else if p.1 > plot.bottom() {
        c |= 8;
    }
    c
}
