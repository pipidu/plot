//! 把图元写成 PDF 和 EPS。几何是矢量；文字按字体轮廓填色，也是矢量。
//! 位图标注在这里画成它的平均色块。PNG 和 SVG 仍保留原始像素。

use std::fmt::Write as _;

use ttf_parser::{Face, GlyphId, OutlineBuilder};

use crate::export::{ExportOptions, PageOrientation};
use crate::scene::{stack_vertical, HAlign, Prim, Scene, VAlign};
use sciplot_core::Dash;

struct Mapper {
    scale: f32,
    ox: f32,
    oy: f32,
    scene_h: f32,
}

impl Mapper {
    fn new(scene: &Scene, page_w: f32, page_h: f32) -> Self {
        let scene_w = scene.width.max(1.0);
        let scene_h = scene.height.max(1.0);
        let margin = (page_w.min(page_h) * 0.05).clamp(18.0, 36.0);
        let avail_w = (page_w - margin * 2.0).max(10.0);
        let avail_h = (page_h - margin * 2.0).max(10.0);
        let scale = (avail_w / scene_w).min(avail_h / scene_h);
        let used_w = scene_w * scale;
        let used_h = scene_h * scale;
        Self {
            scale,
            ox: margin + (avail_w - used_w) * 0.5,
            oy: margin + (avail_h - used_h) * 0.5,
            scene_h,
        }
    }

    fn pt(&self, x: f32, y: f32) -> (f32, f32) {
        let x = if x.is_finite() { x } else { 0.0 };
        let y = if y.is_finite() { y } else { 0.0 };
        (self.ox + x * self.scale, self.oy + (self.scene_h - y) * self.scale)
    }
}

enum PathOp {
    Move(f32, f32),
    Line(f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Close,
}

struct Drawn {
    ops: Vec<PathOp>,
    fill: Option<[u8; 4]>,
    stroke: Option<([u8; 4], f32, Dash)>,
}

pub fn pdf_document(scenes: &[Scene], faces: &[&[u8]], options: &ExportOptions) -> Vec<u8> {
    let (page_w, page_h) = options.paper.size_pt(options.orientation);
    let mut contents = Vec::new();
    for scene in scenes {
        contents.push(page_content(scene, faces, options, page_w, page_h, true));
    }
    if contents.is_empty() {
        contents.push(String::new());
    }
    assemble_pdf(&contents, page_w, page_h, options)
}

pub fn eps_document(scene: &Scene, faces: &[&[u8]], options: &ExportOptions) -> Vec<u8> {
    let (page_w, page_h) = options.paper.size_pt(options.orientation);
    let body = page_content(scene, faces, options, page_w, page_h, false);
    let orientation = match options.orientation {
        PageOrientation::Portrait => "Portrait",
        PageOrientation::Landscape => "Landscape",
    };
    let mut out = format!(
        "%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 {w:.0} {h:.0}\n%%HiResBoundingBox: 0 0 {w:.2} {h:.2}\n%%Orientation: {orientation}\n%%LanguageLevel: 2\n%%Pages: 1\n%%DocumentPaperSizes: {paper}\n%%SciPlotResolution: {dpi:.0}\n%%SciPlotTransparent: {transparent}\n%%SciPlotQuality: {quality}\n%%EndComments\n%%Page: 1 1\n",
        w = page_w,
        h = page_h,
        paper = options.paper.name(),
        dpi = options.dpi.clamp(36.0, 600.0),
        transparent = options.transparent,
        quality = options.quality.clamp(1, 100),
    );
    out.push_str(&body);
    out.push_str("showpage\n%%EOF\n");
    out.into_bytes()
}

fn page_content(scene: &Scene, faces: &[&[u8]], options: &ExportOptions, page_w: f32, page_h: f32, pdf: bool) -> String {
    let map = Mapper::new(scene, page_w, page_h);
    let mut drawn = Vec::new();
    if !options.transparent {
        let (x1, y1) = (0.0, 0.0);
        let (x2, y2) = (page_w, page_h);
        drawn.push(Drawn {
            ops: vec![
                PathOp::Move(x1, y1),
                PathOp::Line(x2, y1),
                PathOp::Line(x2, y2),
                PathOp::Line(x1, y2),
                PathOp::Close,
            ],
            fill: Some([255, 255, 255, 255]),
            stroke: None,
        });
    }
    for prim in &scene.primitives {
        if options.transparent && is_page_background(scene, prim) {
            continue;
        }
        push_prim(&mut drawn, prim, &map, faces);
    }
    if pdf {
        emit_pdf(&drawn)
    } else {
        emit_ps(&drawn)
    }
}

fn is_page_background(scene: &Scene, prim: &Prim) -> bool {
    match prim {
        Prim::Rect { x, y, w, h, fill, .. } => {
            let Some(color) = fill else {
                return false;
            };
            color[0] > 250 && color[1] > 250 && color[2] > 250 && color[3] > 250 && *x <= 1.0 && *y <= 1.0 && *w >= scene.width - 2.0 && *h >= scene.height - 2.0
        }
        _ => false,
    }
}

fn push_prim(out: &mut Vec<Drawn>, prim: &Prim, map: &Mapper, faces: &[&[u8]]) {
    match prim {
        Prim::Line { x1, y1, x2, y2, color, width, dash } => {
            let (a, b) = (map.pt(*x1, *y1), map.pt(*x2, *y2));
            out.push(Drawn {
                ops: vec![PathOp::Move(a.0, a.1), PathOp::Line(b.0, b.1)],
                fill: None,
                stroke: Some((*color, (*width * map.scale).max(0.2), *dash)),
            });
        }
        Prim::Polyline { pts, color, width, dash } => {
            if let Some(ops) = poly_ops(pts, map, false) {
                out.push(Drawn {
                    ops,
                    fill: None,
                    stroke: Some((*color, (*width * map.scale).max(0.2), *dash)),
                });
            }
        }
        Prim::Polygon { pts, fill } => {
            if let Some(ops) = poly_ops(pts, map, true) {
                out.push(Drawn { ops, fill: Some(*fill), stroke: None });
            }
        }
        Prim::Triangle { a, b, c, fill } => {
            if let Some(ops) = poly_ops(&[*a, *b, *c], map, true) {
                out.push(Drawn { ops, fill: Some(*fill), stroke: None });
            }
        }
        Prim::Rect { x, y, w, h, fill, stroke } => {
            if *w <= 0.0 || *h <= 0.0 {
                return;
            }
            let (x0, y0) = map.pt(*x, *y + *h);
            let (x1, y1) = map.pt(*x + *w, *y);
            out.push(Drawn {
                ops: vec![
                    PathOp::Move(x0, y0),
                    PathOp::Line(x1, y0),
                    PathOp::Line(x1, y1),
                    PathOp::Line(x0, y1),
                    PathOp::Close,
                ],
                fill: *fill,
                stroke: stroke.map(|(color, width)| (color, (width * map.scale).max(0.2), Dash::Solid)),
            });
        }
        Prim::Circle { x, y, r, fill, stroke } => {
            let (cx, cy) = map.pt(*x, *y);
            let radius = (*r * map.scale).max(0.2);
            out.push(Drawn {
                ops: circle_ops(cx, cy, radius),
                fill: *fill,
                stroke: stroke.map(|(color, width)| (color, (width * map.scale).max(0.2), Dash::Solid)),
            });
        }
        Prim::Text { x, y, text, size, color, halign, valign, vertical } => {
            text_ops(out, faces, text, *x, *y, *size, *color, *halign, *valign, *vertical, map);
        }
        Prim::Image { x, y, w, h, rgba, cols, rows, .. } => {
            if *w <= 0.0 || *h <= 0.0 {
                return;
            }
            let color = average_color(rgba, *cols, *rows);
            let (x0, y0) = map.pt(*x, *y + *h);
            let (x1, y1) = map.pt(*x + *w, *y);
            out.push(Drawn {
                ops: vec![
                    PathOp::Move(x0, y0),
                    PathOp::Line(x1, y0),
                    PathOp::Line(x1, y1),
                    PathOp::Line(x0, y1),
                    PathOp::Close,
                ],
                fill: Some(color),
                stroke: Some(([80, 80, 80, 255], 0.4, Dash::Solid)),
            });
        }
    }
}

fn poly_ops(pts: &[(f32, f32)], map: &Mapper, close: bool) -> Option<Vec<PathOp>> {
    if pts.len() < 2 {
        return None;
    }
    let mut ops = Vec::with_capacity(pts.len() + 1);
    let (x, y) = map.pt(pts[0].0, pts[0].1);
    ops.push(PathOp::Move(x, y));
    for point in pts.iter().skip(1) {
        let (x, y) = map.pt(point.0, point.1);
        ops.push(PathOp::Line(x, y));
    }
    if close {
        ops.push(PathOp::Close);
    }
    Some(ops)
}

fn circle_ops(cx: f32, cy: f32, r: f32) -> Vec<PathOp> {
    let k = 0.552_284_8 * r;
    vec![
        PathOp::Move(cx + r, cy),
        PathOp::Cubic(cx + r, cy + k, cx + k, cy + r, cx, cy + r),
        PathOp::Cubic(cx - k, cy + r, cx - r, cy + k, cx - r, cy),
        PathOp::Cubic(cx - r, cy - k, cx - k, cy - r, cx, cy - r),
        PathOp::Cubic(cx + k, cy - r, cx + r, cy - k, cx + r, cy),
        PathOp::Close,
    ]
}

fn average_color(rgba: &[u8], cols: u32, rows: u32) -> [u8; 4] {
    if cols == 0 || rows == 0 || rgba.len() < 4 {
        return [180, 180, 180, 255];
    }
    let mut r = 0u64;
    let mut g = 0u64;
    let mut b = 0u64;
    let mut n = 0u64;
    for pixel in rgba.chunks_exact(4) {
        if pixel[3] < 16 {
            continue;
        }
        r += pixel[0] as u64;
        g += pixel[1] as u64;
        b += pixel[2] as u64;
        n += 1;
    }
    if n == 0 {
        return [200, 200, 200, 255];
    }
    [(r / n) as u8, (g / n) as u8, (b / n) as u8, 255]
}

struct Recorder {
    ops: Vec<PathOp>,
    cx: f32,
    cy: f32,
    scale: f32,
    pen_x: f32,
    baseline: f32,
    rotate: bool,
    anchor_x: f32,
    anchor_y: f32,
    map_scale: f32,
    map_ox: f32,
    map_oy: f32,
    map_scene_h: f32,
}

impl Recorder {
    fn place(&self, gx: f32, gy: f32) -> (f32, f32) {
        let x = self.pen_x + gx * self.scale;
        let y = self.baseline - gy * self.scale;
        let (sx, sy) = if self.rotate { (self.anchor_x + y, self.anchor_y - x) } else { (x, y) };
        (self.map_ox + sx * self.map_scale, self.map_oy + (self.map_scene_h - sy) * self.map_scale)
    }
}

impl OutlineBuilder for Recorder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.cx = x;
        self.cy = y;
        let (px, py) = self.place(x, y);
        self.ops.push(PathOp::Move(px, py));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.cx = x;
        self.cy = y;
        let (px, py) = self.place(x, y);
        self.ops.push(PathOp::Line(px, py));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let cx1 = self.cx + (x1 - self.cx) * (2.0 / 3.0);
        let cy1 = self.cy + (y1 - self.cy) * (2.0 / 3.0);
        let cx2 = x + (x1 - x) * (2.0 / 3.0);
        let cy2 = y + (y1 - y) * (2.0 / 3.0);
        self.curve_to(cx1, cy1, cx2, cy2, x, y);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (a, b) = self.place(x1, y1);
        let (c, d) = self.place(x2, y2);
        let (e, f) = self.place(x, y);
        self.cx = x;
        self.cy = y;
        self.ops.push(PathOp::Cubic(a, b, c, d, e, f));
    }

    fn close(&mut self) {
        self.ops.push(PathOp::Close);
    }
}

#[allow(clippy::too_many_arguments)]
fn text_ops(
    out: &mut Vec<Drawn>,
    blobs: &[&[u8]],
    text: &str,
    x: f32,
    y: f32,
    size: f32,
    color: [u8; 4],
    halign: HAlign,
    valign: VAlign,
    vertical: bool,
    map: &Mapper,
) {
    if text.is_empty() || size < 1.0 || color[3] == 0 {
        return;
    }
    let faces: Vec<Face<'_>> = blobs.iter().filter_map(|bytes| Face::parse(bytes, 0).ok()).collect();
    if faces.is_empty() {
        return;
    }
    if vertical && !stack_vertical(text) {
        draw_run(out, &faces, text, x, y, size, color, HAlign::Center, VAlign::Center, true, map);
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
            draw_run(
                out,
                &faces,
                &ch.to_string(),
                x,
                start + index as f32 * size,
                size,
                color,
                HAlign::Center,
                VAlign::Top,
                false,
                map,
            );
        }
        return;
    }
    draw_run(out, &faces, text, x, y, size, color, halign, valign, false, map);
}

#[allow(clippy::too_many_arguments)]
fn draw_run(
    out: &mut Vec<Drawn>,
    faces: &[Face<'_>],
    text: &str,
    x: f32,
    y: f32,
    size: f32,
    color: [u8; 4],
    halign: HAlign,
    valign: VAlign,
    rotate: bool,
    map: &Mapper,
) {
    let metrics = faces.first().unwrap();
    let em = metrics.units_per_em().max(1) as f32;
    let scale = size / em;
    let ascent = metrics.ascender() as f32 * scale;
    let descent = (-metrics.descender()).max(0) as f32 * scale;
    let mut width = 0.0;
    for ch in text.chars() {
        width += glyph_advance(faces, ch, scale, size);
    }
    let pen_x = match halign {
        HAlign::Left => x,
        HAlign::Center => x - width * 0.5,
        HAlign::Right => x - width,
    };
    let baseline = match valign {
        VAlign::Top => y + ascent,
        VAlign::Center => y + (ascent - descent) * 0.5,
        VAlign::Bottom => y,
    };
    let mut pen = if rotate { -width * 0.5 } else { pen_x };
    let base = if rotate { (ascent - descent) * 0.35 } else { baseline };
    for ch in text.chars() {
        let Some((face_index, glyph)) = find_glyph(faces, ch) else {
            pen += size * 0.5;
            continue;
        };
        let mut recorder = Recorder {
            ops: Vec::new(),
            cx: 0.0,
            cy: 0.0,
            scale,
            pen_x: pen,
            baseline: base,
            rotate,
            anchor_x: x,
            anchor_y: y,
            map_scale: map.scale,
            map_ox: map.ox,
            map_oy: map.oy,
            map_scene_h: map.scene_h,
        };
        faces[face_index].outline_glyph(glyph, &mut recorder);
        if !recorder.ops.is_empty() {
            out.push(Drawn {
                ops: recorder.ops,
                fill: Some(color),
                stroke: None,
            });
        }
        pen += glyph_advance(faces, ch, scale, size);
    }
}

fn find_glyph(faces: &[Face<'_>], ch: char) -> Option<(usize, GlyphId)> {
    for (index, face) in faces.iter().enumerate() {
        if let Some(id) = face.glyph_index(ch) {
            if id.0 != 0 {
                return Some((index, id));
            }
        }
    }
    None
}

fn glyph_advance(faces: &[Face<'_>], ch: char, scale: f32, size: f32) -> f32 {
    let Some((index, id)) = find_glyph(faces, ch) else {
        return size * 0.5;
    };
    faces[index]
        .glyph_hor_advance(id)
        .map(|advance| advance as f32 * scale)
        .unwrap_or(size * 0.5)
}

fn emit_pdf(drawn: &[Drawn]) -> String {
    let mut out = String::new();
    for item in drawn {
        write_pdf_item(&mut out, item);
    }
    out
}

fn write_pdf_item(out: &mut String, item: &Drawn) {
    if item.ops.is_empty() {
        return;
    }
    if let Some(color) = item.fill {
        let (r, g, b) = blend(color);
        let _ = writeln!(out, "{r:.4} {g:.4} {b:.4} rg");
    }
    if let Some((color, width, dash)) = &item.stroke {
        let (r, g, b) = blend(*color);
        let _ = writeln!(out, "{r:.4} {g:.4} {b:.4} RG\n{width:.3} w");
        write_pdf_dash(out, *dash, *width);
    } else {
        out.push_str("[] 0 d\n");
    }
    write_pdf_ops(out, &item.ops);
    match (item.fill.is_some(), item.stroke.is_some()) {
        (true, true) => out.push_str("B\n"),
        (true, false) => out.push_str("f\n"),
        (false, true) => out.push_str("S\n"),
        (false, false) => {}
    }
}

fn write_pdf_dash(out: &mut String, dash: Dash, width: f32) {
    let w = width.max(0.4);
    match dash {
        Dash::Solid => out.push_str("[] 0 d\n"),
        Dash::Dash => {
            let _ = writeln!(out, "[{a:.2} {b:.2}] 0 d", a = 4.0 * w, b = 3.0 * w);
        }
        Dash::Dot => {
            let _ = writeln!(out, "[{a:.2} {b:.2}] 0 d", a = w, b = 2.5 * w);
        }
        Dash::DashDot => {
            let _ = writeln!(
                out,
                "[{a:.2} {b:.2} {c:.2} {d:.2}] 0 d",
                a = 4.0 * w,
                b = 2.0 * w,
                c = w,
                d = 2.0 * w
            );
        }
    }
}

fn write_pdf_ops(out: &mut String, ops: &[PathOp]) {
    for op in ops {
        match op {
            PathOp::Move(x, y) => {
                let _ = writeln!(out, "{x:.3} {y:.3} m");
            }
            PathOp::Line(x, y) => {
                let _ = writeln!(out, "{x:.3} {y:.3} l");
            }
            PathOp::Cubic(x1, y1, x2, y2, x, y) => {
                let _ = writeln!(out, "{x1:.3} {y1:.3} {x2:.3} {y2:.3} {x:.3} {y:.3} c");
            }
            PathOp::Close => out.push_str("h\n"),
        }
    }
}

fn emit_ps(drawn: &[Drawn]) -> String {
    let mut out = String::new();
    for item in drawn {
        if item.ops.is_empty() {
            continue;
        }
        if let Some(color) = item.fill {
            let (r, g, b) = blend(color);
            let _ = writeln!(out, "{r:.4} {g:.4} {b:.4} setrgbcolor");
        }
        if let Some((color, width, dash)) = &item.stroke {
            let (r, g, b) = blend(*color);
            let _ = writeln!(out, "{r:.4} {g:.4} {b:.4} setrgbcolor\n{width:.3} setlinewidth");
            write_ps_dash(&mut out, *dash, *width);
        } else {
            out.push_str("[] 0 setdash\n");
        }
        out.push_str("newpath\n");
        for op in &item.ops {
            match op {
                PathOp::Move(x, y) => {
                    let _ = writeln!(out, "{x:.3} {y:.3} moveto");
                }
                PathOp::Line(x, y) => {
                    let _ = writeln!(out, "{x:.3} {y:.3} lineto");
                }
                PathOp::Cubic(x1, y1, x2, y2, x, y) => {
                    let _ = writeln!(out, "{x1:.3} {y1:.3} {x2:.3} {y2:.3} {x:.3} {y:.3} curveto");
                }
                PathOp::Close => out.push_str("closepath\n"),
            }
        }
        match (item.fill.is_some(), item.stroke.is_some()) {
            (true, true) => out.push_str("gsave fill grestore stroke\n"),
            (true, false) => out.push_str("fill\n"),
            (false, true) => out.push_str("stroke\n"),
            (false, false) => {}
        }
    }
    out
}

fn write_ps_dash(out: &mut String, dash: Dash, width: f32) {
    let w = width.max(0.4);
    match dash {
        Dash::Solid => out.push_str("[] 0 setdash\n"),
        Dash::Dash => {
            let _ = writeln!(out, "[{a:.2} {b:.2}] 0 setdash", a = 4.0 * w, b = 3.0 * w);
        }
        Dash::Dot => {
            let _ = writeln!(out, "[{a:.2} {b:.2}] 0 setdash", a = w, b = 2.5 * w);
        }
        Dash::DashDot => {
            let _ = writeln!(
                out,
                "[{a:.2} {b:.2} {c:.2} {d:.2}] 0 setdash",
                a = 4.0 * w,
                b = 2.0 * w,
                c = w,
                d = 2.0 * w
            );
        }
    }
}

fn blend(color: [u8; 4]) -> (f32, f32, f32) {
    let alpha = color[3] as f32 / 255.0;
    let mix = |channel: u8| (channel as f32 * alpha + 255.0 * (1.0 - alpha)) / 255.0;
    (mix(color[0]), mix(color[1]), mix(color[2]))
}

fn assemble_pdf(pages: &[String], page_w: f32, page_h: f32, options: &ExportOptions) -> Vec<u8> {
    let mut objects: Vec<Vec<u8>> = Vec::new();
    objects.push(b"<< /Type /Catalog /Pages 3 0 R >>".to_vec());
    let keywords = format!(
        "dpi={};paper={};orientation={};transparent={};quality={}",
        options.dpi.clamp(36.0, 600.0) as i32,
        options.paper.name(),
        options.orientation.name(),
        if options.transparent { 1 } else { 0 },
        options.quality.clamp(1, 100)
    );
    objects.push(format!("<< /Producer (SciPlot) /Keywords ({keywords}) >>").into_bytes());
    let mut kids = String::from("[");
    for index in 0..pages.len() {
        let page_obj = 4 + index * 2;
        let _ = write!(kids, "{page_obj} 0 R ");
    }
    kids.push(']');
    objects.push(format!("<< /Type /Pages /Kids {kids} /Count {} >>", pages.len()).into_bytes());
    for (index, content) in pages.iter().enumerate() {
        let content_obj = 5 + index * 2;
        objects.push(
            format!(
                "<< /Type /Page /Parent 3 0 R /MediaBox [0 0 {page_w:.0} {page_h:.0}] /Contents {content_obj} 0 R /Resources << >> >>"
            )
            .into_bytes(),
        );
        let mut stream = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
        stream.extend_from_slice(content.as_bytes());
        stream.extend_from_slice(b"endstream");
        objects.push(stream);
    }
    let mut buf = Vec::new();
    buf.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
    let mut offsets = Vec::with_capacity(objects.len());
    for (index, object) in objects.iter().enumerate() {
        offsets.push(buf.len());
        let header = format!("{} 0 obj\n", index + 1);
        buf.extend_from_slice(header.as_bytes());
        buf.extend_from_slice(object);
        buf.extend_from_slice(b"\nendobj\n");
    }
    let xref = buf.len();
    let header = format!("xref\n0 {}\n", objects.len() + 1);
    buf.extend_from_slice(header.as_bytes());
    buf.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets {
        buf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    let trailer = format!(
        "trailer\n<< /Size {} /Root 1 0 R /Info 2 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objects.len() + 1
    );
    buf.extend_from_slice(trailer.as_bytes());
    buf
}
