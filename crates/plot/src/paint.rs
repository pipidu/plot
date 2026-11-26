use egui::{Color32, CornerRadius, Pos2, Shape, Stroke, StrokeKind, Vec2};

use crate::scene::{dash_intervals, HAlign, Prim, Scene, VAlign};

pub fn paint(painter: &egui::Painter, origin: Pos2, scene: &Scene) {
    let p = |x: f32, y: f32| origin + Vec2::new(x, y);
    for prim in &scene.primitives {
        match prim {
            Prim::Rect { x, y, w, h, fill, stroke } => {
                let rect = egui::Rect::from_min_size(p(*x, *y), Vec2::new(*w, *h));
                if let Some(color) = fill {
                    painter.add(Shape::rect_filled(rect, CornerRadius::ZERO, color32(*color)));
                }
                if let Some((color, width)) = stroke {
                    painter.add(Shape::rect_stroke(rect, CornerRadius::ZERO, Stroke::new(*width, color32(*color)), StrokeKind::Inside));
                }
            }
            Prim::Circle { x, y, r, fill, stroke } => {
                let center = p(*x, *y);
                if let Some(color) = fill {
                    painter.circle_filled(center, *r, color32(*color));
                }
                if let Some((color, width)) = stroke {
                    painter.circle_stroke(center, *r, Stroke::new(*width, color32(*color)));
                }
            }
            Prim::Line { x1, y1, x2, y2, color, width, dash } => {
                stroke_poly(painter, &[p(*x1, *y1), p(*x2, *y2)], *color, *width, *dash);
            }
            Prim::Polyline { pts, color, width, dash } => {
                let points: Vec<Pos2> = pts.iter().map(|(x, y)| p(*x, *y)).collect();
                stroke_poly(painter, &points, *color, *width, *dash);
            }
            Prim::Polygon { pts, fill } => {
                let points: Vec<Pos2> = pts.iter().map(|(x, y)| p(*x, *y)).collect();
                if points.len() >= 3 {
                    painter.add(Shape::convex_polygon(points, color32(*fill), Stroke::NONE));
                }
            }
            Prim::Triangle { a, b, c, fill } => {
                painter.add(Shape::convex_polygon(
                    vec![p(a.0, a.1), p(b.0, b.1), p(c.0, c.1)],
                    color32(*fill),
                    Stroke::NONE,
                ));
            }
            Prim::Text { x, y, text, size, color, halign, valign, vertical } => {
                let font = egui::FontId::proportional(*size);
                if *vertical {
                    let chars: Vec<char> = text.chars().collect();
                    let start = match valign {
                        VAlign::Top => *y,
                        VAlign::Center => *y - chars.len() as f32 * *size * 0.5,
                        VAlign::Bottom => *y - chars.len() as f32 * *size,
                    };
                    for (index, ch) in chars.into_iter().enumerate() {
                        painter.text(p(*x, start + index as f32 * *size), egui::Align2::CENTER_TOP, ch, font.clone(), color32(*color));
                    }
                } else {
                    painter.text(p(*x, *y), align2(*halign, *valign), text, font, color32(*color));
                }
            }
        }
    }
}

fn stroke_poly(painter: &egui::Painter, pts: &[Pos2], color: [u8; 4], width: f32, dash: sciplot_core::Dash) {
    if pts.len() < 2 {
        return;
    }
    let stroke = Stroke::new(width, color32(color));
    if let Some(pattern) = dash_intervals(dash, width) {
        for (a, b) in dash_pieces(pts, &pattern) {
            painter.line_segment([a, b], stroke);
        }
    } else {
        painter.add(Shape::line(pts.to_vec(), stroke));
    }
}

fn dash_pieces(pts: &[Pos2], pattern: &[f32]) -> Vec<(Pos2, Pos2)> {
    let mut out = Vec::new();
    if pattern.is_empty() {
        return out;
    }
    let mut draw = true;
    let mut remain = pattern[0];
    let mut slot = 0usize;
    for pair in pts.windows(2) {
        let mut a = pair[0];
        let b = pair[1];
        let mut left = a.distance(b);
        if left < 1e-3 {
            continue;
        }
        let dir = (b - a) / left;
        while left > 1e-3 {
            let step = remain.min(left);
            let next = a + dir * step;
            if draw {
                out.push((a, next));
            }
            a = next;
            left -= step;
            remain -= step;
            if remain <= 1e-3 {
                slot = (slot + 1) % pattern.len();
                remain = pattern[slot].max(0.5);
                draw = !draw;
            }
        }
    }
    out
}

fn align2(h: HAlign, v: VAlign) -> egui::Align2 {
    let x = match h {
        HAlign::Left => egui::Align::LEFT,
        HAlign::Center => egui::Align::Center,
        HAlign::Right => egui::Align::RIGHT,
    };
    let y = match v {
        VAlign::Top => egui::Align::TOP,
        VAlign::Center => egui::Align::Center,
        VAlign::Bottom => egui::Align::BOTTOM,
    };
    egui::Align2([x, y])
}

fn color32(color: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(color[0], color[1], color[2], color[3])
}
