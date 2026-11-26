use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use tiny_skia::{FillRule, LineCap, Paint, PathBuilder, Pixmap, Stroke, StrokeDash, Transform};

use crate::scene::{dash_intervals, HAlign, Prim, Scene, VAlign};

pub struct GlyphFont {
    font: FontVec,
}

impl GlyphFont {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let owned = prepare_font(bytes)?;
        FontVec::try_from_vec(owned).ok().map(|font| Self { font })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, pixmap: &mut Pixmap, text: &str, x: f32, y: f32, size: f32, color: [u8; 4], halign: HAlign, valign: VAlign, vertical: bool) {
        if text.is_empty() || size < 1.0 {
            return;
        }
        if vertical {
            let chars: Vec<char> = text.chars().collect();
            let start = match valign {
                VAlign::Top => y,
                VAlign::Center => y - chars.len() as f32 * size * 0.5,
                VAlign::Bottom => y - chars.len() as f32 * size,
            };
            for (index, ch) in chars.into_iter().enumerate() {
                self.draw(pixmap, &ch.to_string(), x, start + index as f32 * size, size, color, HAlign::Center, VAlign::Top, false);
            }
            return;
        }
        let scale = PxScale::from(size.max(1.0));
        let scaled = self.font.as_scaled(scale);
        let mut width = 0.0;
        let mut prev = None;
        for ch in text.chars() {
            let id = scaled.glyph_id(ch);
            if let Some(previous) = prev {
                width += scaled.kern(previous, id);
            }
            width += scaled.h_advance(id);
            prev = Some(id);
        }
        let origin_x = match halign {
            HAlign::Left => x,
            HAlign::Center => x - width * 0.5,
            HAlign::Right => x - width,
        };
        let origin_y = match valign {
            VAlign::Top => y + scaled.ascent(),
            VAlign::Center => y + scaled.ascent() * 0.5 - scaled.descent() * 0.15,
            VAlign::Bottom => y,
        };
        let mut cursor = origin_x;
        let mut prev = None;
        for ch in text.chars() {
            let id = scaled.glyph_id(ch);
            if let Some(previous) = prev {
                cursor += scaled.kern(previous, id);
            }
            let glyph = id.with_scale_and_position(scale, ab_glyph::point(cursor, origin_y));
            if let Some(outlined) = scaled.outline_glyph(glyph) {
                outlined.draw(|dx, dy, coverage| {
                    blend(pixmap, outlined.px_bounds().min.x as i32 + dx as i32, outlined.px_bounds().min.y as i32 + dy as i32, color, coverage);
                });
            }
            cursor += scaled.h_advance(id);
            prev = Some(id);
        }
    }
}

fn blend(pixmap: &mut Pixmap, x: i32, y: i32, color: [u8; 4], coverage: f32) {
    if x < 0 || y < 0 {
        return;
    }
    let (x, y) = (x as u32, y as u32);
    if x >= pixmap.width() || y >= pixmap.height() {
        return;
    }
    let alpha = (coverage.clamp(0.0, 1.0) * color[3] as f32 / 255.0).clamp(0.0, 1.0);
    if alpha <= 0.0 {
        return;
    }
    let width = pixmap.width();
    if let Some(pixel) = pixmap.pixels_mut().get_mut((y * width + x) as usize) {
        let src_a = alpha;
        let dst_a = pixel.alpha() as f32 / 255.0;
        let out_a = src_a + dst_a * (1.0 - src_a);
        if out_a <= 0.0 {
            return;
        }
        let mix = |src: u8, dst: u8| -> u8 {
            let value = (src as f32 * src_a + dst as f32 * dst_a * (1.0 - src_a)) / out_a;
            value.round().clamp(0.0, 255.0) as u8
        };
        let dem = pixel.demultiply();
        let (r, g, b) = (mix(color[0], dem.red()), mix(color[1], dem.green()), mix(color[2], dem.blue()));
        *pixel = tiny_skia::ColorU8::from_rgba(r, g, b, (out_a * 255.0).round() as u8).premultiply();
    }
}

pub fn to_png(scene: &Scene, font: Option<&GlyphFont>) -> Result<Vec<u8>, String> {
    draw_pixmap(scene, font)?.encode_png().map_err(|err| err.to_string())
}

pub fn to_rgba(scene: &Scene, font: Option<&GlyphFont>) -> Result<(u32, u32, Vec<u8>), String> {
    let pixmap = draw_pixmap(scene, font)?;
    let mut rgba = Vec::with_capacity((pixmap.width() * pixmap.height() * 4) as usize);
    for pixel in pixmap.pixels() {
        let color = pixel.demultiply();
        rgba.extend_from_slice(&[color.red(), color.green(), color.blue(), color.alpha()]);
    }
    Ok((pixmap.width(), pixmap.height(), rgba))
}

fn draw_pixmap(scene: &Scene, font: Option<&GlyphFont>) -> Result<Pixmap, String> {
    let width = scene.width.max(1.0).round() as u32;
    let height = scene.height.max(1.0).round() as u32;
    let mut pixmap = Pixmap::new(width, height).ok_or("无法创建位图")?;
    pixmap.fill(tiny_skia::Color::from_rgba8(255, 255, 255, 255));
    for prim in &scene.primitives {
        match prim {
            Prim::Rect { x, y, w, h, fill, stroke } => {
                if let Some(path) = rect_path(*x, *y, *w, *h) {
                    if let Some(color) = fill {
                        fill_path(&mut pixmap, &path, *color);
                    }
                    if let Some((color, width)) = stroke {
                        stroke_path(&mut pixmap, &path, *color, *width, None);
                    }
                }
            }
            Prim::Circle { x, y, r, fill, stroke } => {
                if let Some(path) = circle_path(*x, *y, *r) {
                    if let Some(color) = fill {
                        fill_path(&mut pixmap, &path, *color);
                    }
                    if let Some((color, width)) = stroke {
                        stroke_path(&mut pixmap, &path, *color, *width, None);
                    }
                }
            }
            Prim::Line { x1, y1, x2, y2, color, width, dash } => {
                if let Some(path) = line_path(*x1, *y1, *x2, *y2) {
                    stroke_path(&mut pixmap, &path, *color, *width, dash_intervals(*dash, *width).as_deref());
                }
            }
            Prim::Polyline { pts, color, width, dash } => {
                if let Some(path) = poly_path(pts, false) {
                    stroke_path(&mut pixmap, &path, *color, *width, dash_intervals(*dash, *width).as_deref());
                }
            }
            Prim::Polygon { pts, fill } => {
                if let Some(path) = poly_path(pts, true) {
                    fill_path(&mut pixmap, &path, *fill);
                }
            }
            Prim::Triangle { a, b, c, fill } => {
                if let Some(path) = poly_path(&[*a, *b, *c], true) {
                    fill_path(&mut pixmap, &path, *fill);
                }
            }
            Prim::Text { x, y, text, size, color, halign, valign, vertical } => {
                if let Some(font) = font {
                    font.draw(&mut pixmap, text, *x, *y, *size, *color, *halign, *valign, *vertical);
                }
            }
        }
    }
    Ok(pixmap)
}

fn fill_path(pixmap: &mut Pixmap, path: &tiny_skia::Path, color: [u8; 4]) {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    paint.anti_alias = true;
    pixmap.fill_path(path, &paint, FillRule::Winding, Transform::identity(), None);
}

fn stroke_path(pixmap: &mut Pixmap, path: &tiny_skia::Path, color: [u8; 4], width: f32, dash: Option<&[f32]>) {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    paint.anti_alias = true;
    let mut stroke = Stroke {
        width: width.max(0.5),
        line_cap: if dash.is_some() { LineCap::Butt } else { LineCap::Round },
        ..Stroke::default()
    };
    if let Some(pattern) = dash {
        stroke.dash = StrokeDash::new(pattern.to_vec(), 0.0);
    }
    pixmap.stroke_path(path, &paint, &stroke, Transform::identity(), None);
}

fn rect_path(x: f32, y: f32, w: f32, h: f32) -> Option<tiny_skia::Path> {
    let rect = tiny_skia::Rect::from_xywh(x, y, w.max(0.2), h.max(0.2))?;
    let mut path = PathBuilder::new();
    path.push_rect(rect);
    path.finish()
}

fn circle_path(x: f32, y: f32, r: f32) -> Option<tiny_skia::Path> {
    let mut path = PathBuilder::new();
    path.push_circle(x, y, r.max(0.2));
    path.finish()
}

fn line_path(x1: f32, y1: f32, x2: f32, y2: f32) -> Option<tiny_skia::Path> {
    let mut path = PathBuilder::new();
    path.move_to(x1, y1);
    path.line_to(x2, y2);
    path.finish()
}

fn poly_path(pts: &[(f32, f32)], close: bool) -> Option<tiny_skia::Path> {
    if pts.len() < 2 {
        return None;
    }
    let mut path = PathBuilder::new();
    path.move_to(pts[0].0, pts[0].1);
    for point in pts.iter().skip(1) {
        path.line_to(point.0, point.1);
    }
    if close {
        path.close();
    }
    path.finish()
}

/// 单字体 TTF/OTF 原样返回。TTC 取出第 0 个字面，改写表偏移。
pub fn prepare_font(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() >= 4 && &bytes[..4] == b"ttcf" {
        extract_ttc(bytes, 0)
    } else if bytes.len() > 12 {
        Some(bytes.to_vec())
    } else {
        None
    }
}

fn extract_ttc(bytes: &[u8], index: usize) -> Option<Vec<u8>> {
    if bytes.len() < 16 {
        return None;
    }
    let count = u32be(bytes, 8)? as usize;
    if index >= count {
        return None;
    }
    let offset = u32be(bytes, 12 + index * 4)? as usize;
    let num_tables = u16be(bytes, offset + 4)? as usize;
    if num_tables == 0 || offset + 12 + num_tables * 16 > bytes.len() {
        return None;
    }
    struct Rec {
        tag: [u8; 4],
        checksum: u32,
        offset: u32,
        length: u32,
    }
    let mut records = Vec::with_capacity(num_tables);
    for i in 0..num_tables {
        let at = offset + 12 + i * 16;
        let mut tag = [0; 4];
        tag.copy_from_slice(&bytes[at..at + 4]);
        records.push(Rec {
            tag,
            checksum: u32be(bytes, at + 4)?,
            offset: u32be(bytes, at + 8)?,
            length: u32be(bytes, at + 12)?,
        });
    }
    let mut out = bytes[offset..offset + 12].to_vec();
    out.resize(12 + num_tables * 16, 0);
    let mut cursor = out.len() as u32;
    for (i, rec) in records.iter().enumerate() {
        let aligned = (cursor + 3) & !3;
        out.resize(aligned as usize, 0);
        let start = rec.offset as usize;
        let end = start + rec.length as usize;
        if end > bytes.len() {
            return None;
        }
        out.extend_from_slice(&bytes[start..end]);
        let dir = 12 + i * 16;
        out[dir..dir + 4].copy_from_slice(&rec.tag);
        put_u32(&mut out, dir + 4, rec.checksum);
        put_u32(&mut out, dir + 8, aligned);
        put_u32(&mut out, dir + 12, rec.length);
        cursor = aligned + rec.length;
    }
    Some(out)
}

fn u16be(bytes: &[u8], at: usize) -> Option<u16> {
    bytes.get(at..at + 2).map(|s| u16::from_be_bytes([s[0], s[1]]))
}

fn u32be(bytes: &[u8], at: usize) -> Option<u32> {
    bytes.get(at..at + 4).map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

fn put_u32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
