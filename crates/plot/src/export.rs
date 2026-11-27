//! 导出 PNG、SVG、EPS、PDF 和 SciDAVis 列出的位图格式。
//! 纸张、方向、分辨率、JPEG 质量和透明背景都从这里进文件。

use crate::raster::{to_png_with, to_rgba_with, GlyphFont};
use crate::scene::Scene;
use crate::svg::to_svg;
use crate::vector::{eps_document, pdf_document};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Png,
    Svg,
    Eps,
    Pdf,
    Jpeg,
    Bmp,
    Pbm,
    Pgm,
    Ppm,
    Xbm,
    Xpm,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Png => "png",
            ExportFormat::Svg => "svg",
            ExportFormat::Eps => "eps",
            ExportFormat::Pdf => "pdf",
            ExportFormat::Jpeg => "jpg",
            ExportFormat::Bmp => "bmp",
            ExportFormat::Pbm => "pbm",
            ExportFormat::Pgm => "pgm",
            ExportFormat::Ppm => "ppm",
            ExportFormat::Xbm => "xbm",
            ExportFormat::Xpm => "xpm",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ExportFormat::Png => sciplot_core::tr!("PNG", "PNG"),
            ExportFormat::Svg => sciplot_core::tr!("SVG", "SVG"),
            ExportFormat::Eps => sciplot_core::tr!("EPS", "EPS"),
            ExportFormat::Pdf => sciplot_core::tr!("PDF", "PDF"),
            ExportFormat::Jpeg => sciplot_core::tr!("JPEG", "JPEG"),
            ExportFormat::Bmp => sciplot_core::tr!("BMP", "BMP"),
            ExportFormat::Pbm => sciplot_core::tr!("PBM", "PBM"),
            ExportFormat::Pgm => sciplot_core::tr!("PGM", "PGM"),
            ExportFormat::Ppm => sciplot_core::tr!("PPM", "PPM"),
            ExportFormat::Xbm => sciplot_core::tr!("XBM", "XBM"),
            ExportFormat::Xpm => sciplot_core::tr!("XPM", "XPM"),
        }
    }

    pub fn mime(self) -> &'static str {
        match self {
            ExportFormat::Png => "image/png",
            ExportFormat::Svg => "image/svg+xml",
            ExportFormat::Eps => "application/postscript",
            ExportFormat::Pdf => "application/pdf",
            ExportFormat::Jpeg => "image/jpeg",
            ExportFormat::Bmp => "image/bmp",
            ExportFormat::Pbm => "image/x-portable-bitmap",
            ExportFormat::Pgm => "image/x-portable-graymap",
            ExportFormat::Ppm => "image/x-portable-pixmap",
            ExportFormat::Xbm => "image/x-xbitmap",
            ExportFormat::Xpm => "image/x-xpixmap",
        }
    }

    pub fn is_vector(self) -> bool {
        matches!(self, ExportFormat::Svg | ExportFormat::Eps | ExportFormat::Pdf)
    }

    pub fn supports_transparency(self) -> bool {
        matches!(self, ExportFormat::Png | ExportFormat::Xpm | ExportFormat::Svg)
    }

    pub fn supports_quality(self) -> bool {
        matches!(self, ExportFormat::Jpeg)
    }

    pub fn all() -> &'static [ExportFormat] {
        &[
            ExportFormat::Png,
            ExportFormat::Svg,
            ExportFormat::Eps,
            ExportFormat::Pdf,
            ExportFormat::Jpeg,
            ExportFormat::Bmp,
            ExportFormat::Pbm,
            ExportFormat::Pgm,
            ExportFormat::Ppm,
            ExportFormat::Xbm,
            ExportFormat::Xpm,
        ]
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "png" => Some(Self::Png),
            "svg" => Some(Self::Svg),
            "eps" | "ps" => Some(Self::Eps),
            "pdf" => Some(Self::Pdf),
            "jpeg" | "jpg" => Some(Self::Jpeg),
            "bmp" => Some(Self::Bmp),
            "pbm" => Some(Self::Pbm),
            "pgm" => Some(Self::Pgm),
            "ppm" => Some(Self::Ppm),
            "xbm" => Some(Self::Xbm),
            "xpm" => Some(Self::Xpm),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paper {
    A3,
    A4,
    A5,
    Letter,
    Legal,
}

impl Paper {
    pub fn name(self) -> &'static str {
        match self {
            Paper::A3 => "A3",
            Paper::A4 => "A4",
            Paper::A5 => "A5",
            Paper::Letter => "Letter",
            Paper::Legal => "Legal",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Paper::A3 => sciplot_core::tr!("A3", "A3"),
            Paper::A4 => sciplot_core::tr!("A4", "A4"),
            Paper::A5 => sciplot_core::tr!("A5", "A5"),
            Paper::Letter => sciplot_core::tr!("信纸", "Letter"),
            Paper::Legal => sciplot_core::tr!("法律用纸", "Legal"),
        }
    }

    pub fn all() -> &'static [Paper] {
        &[Paper::A3, Paper::A4, Paper::A5, Paper::Letter, Paper::Legal]
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "a3" => Some(Self::A3),
            "a4" => Some(Self::A4),
            "a5" => Some(Self::A5),
            "letter" | "信纸" => Some(Self::Letter),
            "legal" | "法律用纸" => Some(Self::Legal),
            _ => None,
        }
    }

    /// 纵向时的宽高，单位是点（1/72 英寸）。
    pub fn portrait_pt(self) -> (f32, f32) {
        match self {
            Paper::A3 => (842.0, 1191.0),
            Paper::A4 => (595.0, 842.0),
            Paper::A5 => (420.0, 595.0),
            Paper::Letter => (612.0, 792.0),
            Paper::Legal => (612.0, 1008.0),
        }
    }

    pub fn size_pt(self, orientation: PageOrientation) -> (f32, f32) {
        let (w, h) = self.portrait_pt();
        match orientation {
            PageOrientation::Portrait => (w, h),
            PageOrientation::Landscape => (h, w),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageOrientation {
    Portrait,
    Landscape,
}

impl PageOrientation {
    pub fn name(self) -> &'static str {
        match self {
            PageOrientation::Portrait => "portrait",
            PageOrientation::Landscape => "landscape",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PageOrientation::Portrait => sciplot_core::tr!("纵向", "Portrait"),
            PageOrientation::Landscape => sciplot_core::tr!("横向", "Landscape"),
        }
    }

    pub fn all() -> &'static [PageOrientation] {
        &[PageOrientation::Portrait, PageOrientation::Landscape]
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "portrait" | "纵向" => Some(Self::Portrait),
            "landscape" | "横向" => Some(Self::Landscape),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExportOptions {
    pub format: ExportFormat,
    pub dpi: f32,
    pub quality: u8,
    pub transparent: bool,
    pub paper: Paper,
    pub orientation: PageOrientation,
    /// 为真时，位图像素按纸张和分辨率来。矢量页始终用纸张尺寸。
    pub fit_paper: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            format: ExportFormat::Png,
            dpi: 144.0,
            quality: 90,
            transparent: false,
            paper: Paper::A4,
            orientation: PageOrientation::Portrait,
            fit_paper: false,
        }
    }
}

impl ExportOptions {
    pub fn clamped(&self) -> Self {
        let mut options = self.clone();
        options.dpi = options.dpi.clamp(36.0, 600.0);
        options.quality = options.quality.clamp(1, 100);
        options
    }
}

pub fn page_points(options: &ExportOptions) -> (f32, f32) {
    options.paper.size_pt(options.orientation)
}

/// 按当前选项该用多大的像素图。不贴纸时沿用原来的 960×640、DPI/96。
pub fn pixel_size(options: &ExportOptions) -> (f32, f32) {
    let options = options.clamped();
    if options.fit_paper || matches!(options.format, ExportFormat::Eps | ExportFormat::Pdf) {
        let (w, h) = page_points(&options);
        ((w / 72.0 * options.dpi).clamp(64.0, 8000.0), (h / 72.0 * options.dpi).clamp(64.0, 8000.0))
    } else {
        let scale = options.dpi / 96.0;
        (960.0 * scale, 640.0 * scale)
    }
}

pub fn export_scene(scene: &Scene, font: Option<&GlyphFont>, faces: &[&[u8]], options: &ExportOptions) -> Result<Vec<u8>, String> {
    let options = options.clamped();
    match options.format {
        ExportFormat::Png => to_png_with(scene, font, options.transparent && options.format.supports_transparency()),
        ExportFormat::Svg => Ok(to_svg(scene).into_bytes()),
        ExportFormat::Eps => Ok(eps_document(scene, faces, &options)),
        ExportFormat::Pdf => Ok(pdf_document(std::slice::from_ref(scene), faces, &options)),
        ExportFormat::Jpeg => {
            let (w, h, rgba) = to_rgba_with(scene, font, false)?;
            encode_jpeg(&rgba, w, h, options.quality)
        }
        ExportFormat::Bmp => {
            let (w, h, rgba) = to_rgba_with(scene, font, false)?;
            Ok(encode_bmp(&rgba, w, h))
        }
        ExportFormat::Pbm => {
            let (w, h, rgba) = to_rgba_with(scene, font, false)?;
            Ok(encode_pbm(&rgba, w, h))
        }
        ExportFormat::Pgm => {
            let (w, h, rgba) = to_rgba_with(scene, font, false)?;
            Ok(encode_pgm(&rgba, w, h))
        }
        ExportFormat::Ppm => {
            let (w, h, rgba) = to_rgba_with(scene, font, false)?;
            Ok(encode_ppm(&rgba, w, h))
        }
        ExportFormat::Xbm => {
            let (w, h, rgba) = to_rgba_with(scene, font, false)?;
            Ok(encode_xbm(&rgba, w, h))
        }
        ExportFormat::Xpm => {
            let (w, h, rgba) = to_rgba_with(scene, font, options.transparent)?;
            Ok(encode_xpm(&rgba, w, h, options.transparent))
        }
    }
}

/// 打印。没有系统打印机时写成多页 PDF，一图一页。
pub fn print_pdf(scenes: &[Scene], faces: &[&[u8]], options: &ExportOptions) -> Result<Vec<u8>, String> {
    if scenes.is_empty() {
        return Err(sciplot_core::t("没有可打印的图形").into());
    }
    let mut options = options.clamped();
    options.format = ExportFormat::Pdf;
    Ok(pdf_document(scenes, faces, &options))
}

fn encode_jpeg(rgba: &[u8], width: u32, height: u32, quality: u8) -> Result<Vec<u8>, String> {
    let rgb = flatten_rgb(rgba);
    let mut bytes = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, quality);
    encoder
        .encode(&rgb, width, height, image::ExtendedColorType::Rgb8)
        .map_err(|err| err.to_string())?;
    Ok(bytes)
}

fn flatten_rgb(rgba: &[u8]) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(rgba.len() / 4 * 3);
    for pixel in rgba.chunks_exact(4) {
        let alpha = pixel[3] as u16;
        let mix = |channel: u8| -> u8 { ((channel as u16 * alpha + 255 * (255 - alpha)) / 255) as u8 };
        rgb.push(mix(pixel[0]));
        rgb.push(mix(pixel[1]));
        rgb.push(mix(pixel[2]));
    }
    rgb
}

fn encode_bmp(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let row_stride = (width * 3).div_ceil(4) * 4;
    let pixel_bytes = row_stride * height;
    let offset = 54u32;
    let file_size = offset + pixel_bytes;
    let mut out = vec![0u8; file_size as usize];
    out[0] = b'B';
    out[1] = b'M';
    out[2..6].copy_from_slice(&file_size.to_le_bytes());
    out[10..14].copy_from_slice(&offset.to_le_bytes());
    out[14..18].copy_from_slice(&40u32.to_le_bytes());
    out[18..22].copy_from_slice(&width.to_le_bytes());
    out[22..26].copy_from_slice(&height.to_le_bytes());
    out[26..28].copy_from_slice(&1u16.to_le_bytes());
    out[28..30].copy_from_slice(&24u16.to_le_bytes());
    out[34..38].copy_from_slice(&pixel_bytes.to_le_bytes());
    for y in 0..height {
        let src_y = height - 1 - y;
        let row = &mut out[offset as usize + y as usize * row_stride as usize..];
        for x in 0..width {
            let i = ((src_y * width + x) * 4) as usize;
            let (r, g, b, a) = (rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]);
            let mix = |channel: u8| -> u8 { ((channel as u16 * a as u16 + 255 * (255 - a as u16)) / 255) as u8 };
            let dest = (x * 3) as usize;
            row[dest] = mix(b);
            row[dest + 1] = mix(g);
            row[dest + 2] = mix(r);
        }
    }
    out
}

fn luma(pixel: &[u8]) -> u8 {
    let alpha = pixel[3] as u16;
    let mix = |channel: u8| (channel as u16 * alpha + 255 * (255 - alpha)) / 255;
    ((mix(pixel[0]) * 3 + mix(pixel[1]) * 6 + mix(pixel[2])) / 10) as u8
}

fn encode_pbm(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut out = format!("P4\n{width} {height}\n").into_bytes();
    for y in 0..height as usize {
        let mut byte = 0u8;
        let mut bit = 0;
        for x in 0..width as usize {
            let pixel = &rgba[((y * width as usize + x) * 4)..];
            let black = luma(pixel) < 128;
            byte = (byte << 1) | u8::from(black);
            bit += 1;
            if bit == 8 {
                out.push(byte);
                byte = 0;
                bit = 0;
            }
        }
        if bit != 0 {
            byte <<= 8 - bit;
            out.push(byte);
        }
    }
    out
}

fn encode_pgm(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut out = format!("P5\n{width} {height}\n255\n").into_bytes();
    for pixel in rgba.chunks_exact(4) {
        out.push(luma(pixel));
    }
    out
}

fn encode_ppm(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut out = format!("P6\n{width} {height}\n255\n").into_bytes();
    out.extend(flatten_rgb(rgba));
    out
}

fn encode_xbm(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut out = format!("#define sciplot_width {width}\n#define sciplot_height {height}\nstatic unsigned char sciplot_bits[] = {{\n").into_bytes();
    let mut row = Vec::new();
    for y in 0..height as usize {
        row.clear();
        let mut byte = 0u8;
        let mut bit = 0;
        for x in 0..width as usize {
            let pixel = &rgba[((y * width as usize + x) * 4)..];
            if luma(pixel) < 128 {
                byte |= 1 << bit;
            }
            bit += 1;
            if bit == 8 {
                row.push(byte);
                byte = 0;
                bit = 0;
            }
        }
        if bit != 0 {
            row.push(byte);
        }
        for (index, byte) in row.iter().enumerate() {
            let comma = if y + 1 == height as usize && index + 1 == row.len() { "" } else { "," };
            out.extend(format!("0x{byte:02x}{comma}").into_bytes());
        }
        out.push(b'\n');
    }
    out.extend(b"};\n");
    out
}

fn encode_xpm(rgba: &[u8], width: u32, height: u32, transparent: bool) -> Vec<u8> {
    let mut colors: Vec<[u8; 4]> = Vec::new();
    let mut index_of = vec![0u16; (width * height) as usize];
    for (pixel_index, pixel) in rgba.chunks_exact(4).enumerate() {
        let key = if transparent && pixel[3] < 128 {
            [0, 0, 0, 0]
        } else {
            [pixel[0] & 0xF0, pixel[1] & 0xF0, pixel[2] & 0xF0, 255]
        };
        let index = if let Some(found) = colors.iter().position(|color| *color == key) {
            found
        } else {
            colors.push(key);
            colors.len() - 1
        };
        index_of[pixel_index] = index as u16;
    }
    if colors.is_empty() {
        colors.push([255, 255, 255, 255]);
    }
    let mut out = format!(
        "/* XPM */\nstatic char *sciplot[] = {{\n\"{width} {height} {} 2\",\n",
        colors.len()
    );
    for (index, color) in colors.iter().enumerate() {
        let token = xpm_token(index);
        if color[3] == 0 {
            out.push_str(&format!("\"{token} c None\",\n"));
        } else {
            out.push_str(&format!("\"{token} c #{:02X}{:02X}{:02X}\",\n", color[0], color[1], color[2]));
        }
    }
    for y in 0..height as usize {
        out.push('"');
        for x in 0..width as usize {
            out.push_str(xpm_token(index_of[y * width as usize + x] as usize));
        }
        if y + 1 == height as usize {
            out.push_str("\"\n}};\n");
        } else {
            out.push_str("\",\n");
        }
    }
    out.into_bytes()
}

fn xpm_token(index: usize) -> &'static str {
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    // 62 * 62 tokens, leaked once via a process-lifetime table built on first use.
    use std::sync::OnceLock;
    static TABLE: OnceLock<Vec<String>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut tokens = Vec::with_capacity(CHARS.len() * CHARS.len());
        for a in CHARS {
            for b in CHARS {
                tokens.push(format!("{}{}", *a as char, *b as char));
            }
        }
        tokens
    });
    table.get(index).map(String::as_str).unwrap_or("aa")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render;
    use sciplot_core::{demo_project, Curve, CurveKind, DataSource, Graph, GraphId};

    fn line_scene() -> Scene {
        let project = sciplot_core::Project::new("p");
        let mut graph = Graph::new(GraphId(1), "g");
        graph.layers[0].title = "Line".into();
        graph.layers[0].curves.push(Curve {
            name: "y".into(),
            kind: CurveKind::Line,
            source: DataSource::Points {
                x: vec![0.0, 1.0, 2.0],
                y: vec![0.0, 1.0, 0.0],
            },
            ..Curve::default()
        });
        render(&project, &graph, &[], 120.0, 80.0, 1.0)
    }

    fn font() -> Vec<u8> {
        include_bytes!("../../../assets/fonts/DroidSansFallbackFull.ttf").to_vec()
    }

    #[test]
    fn paper_orientation_and_pixel_size() {
        let mut options = ExportOptions {
            paper: Paper::Letter,
            orientation: PageOrientation::Landscape,
            dpi: 72.0,
            fit_paper: true,
            ..ExportOptions::default()
        };
        assert_eq!(page_points(&options), (792.0, 612.0));
        assert_eq!(pixel_size(&options), (792.0, 612.0));
        options.fit_paper = false;
        options.format = ExportFormat::Png;
        options.dpi = 96.0;
        assert_eq!(pixel_size(&options), (960.0, 640.0));
        options.dpi = 144.0;
        assert!((pixel_size(&options).0 - 1440.0).abs() < 1e-3);
    }

    #[test]
    fn bitmap_headers_eps_pdf_and_transparency() {
        let scene = line_scene();
        let bytes = font();
        let face = bytes.as_slice();
        let glyphs = GlyphFont::from_bytes(&bytes);
        let mut options = ExportOptions {
            format: ExportFormat::Png,
            dpi: 72.0,
            quality: 40,
            transparent: true,
            paper: Paper::A4,
            orientation: PageOrientation::Portrait,
            fit_paper: false,
        };
        let png = export_scene(&scene, glyphs.as_ref(), &[face], &options).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let decoder = png::Decoder::new(std::io::Cursor::new(&png));
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size()];
        reader.next_frame(&mut buf).unwrap();
        assert_eq!(buf[3], 0, "transparent PNG corner stays clear");

        options.transparent = false;
        options.format = ExportFormat::Jpeg;
        options.quality = 20;
        let low = export_scene(&scene, glyphs.as_ref(), &[face], &options).unwrap();
        options.quality = 95;
        let high = export_scene(&scene, glyphs.as_ref(), &[face], &options).unwrap();
        assert_eq!(&low[..3], &[0xFF, 0xD8, 0xFF]);
        assert_eq!(&high[..3], &[0xFF, 0xD8, 0xFF]);
        assert_ne!(low.len(), high.len());

        options.format = ExportFormat::Bmp;
        let bmp = export_scene(&scene, glyphs.as_ref(), &[face], &options).unwrap();
        assert_eq!(&bmp[..2], b"BM");

        options.format = ExportFormat::Pbm;
        let pbm = export_scene(&scene, glyphs.as_ref(), &[face], &options).unwrap();
        assert!(pbm.starts_with(b"P4\n"));
        options.format = ExportFormat::Pgm;
        assert!(export_scene(&scene, glyphs.as_ref(), &[face], &options).unwrap().starts_with(b"P5\n"));
        options.format = ExportFormat::Ppm;
        assert!(export_scene(&scene, glyphs.as_ref(), &[face], &options).unwrap().starts_with(b"P6\n"));

        options.format = ExportFormat::Xbm;
        let xbm = String::from_utf8(export_scene(&scene, glyphs.as_ref(), &[face], &options).unwrap()).unwrap();
        assert!(xbm.contains("#define sciplot_width"));
        assert!(xbm.contains("sciplot_bits"));

        options.format = ExportFormat::Xpm;
        options.transparent = true;
        let xpm = String::from_utf8(export_scene(&scene, glyphs.as_ref(), &[face], &options).unwrap()).unwrap();
        assert!(xpm.contains("/* XPM */"));
        assert!(xpm.contains("c None"), "{xpm}");

        options.format = ExportFormat::Pdf;
        options.paper = Paper::Letter;
        options.orientation = PageOrientation::Landscape;
        options.transparent = true;
        options.dpi = 144.0;
        let pdf = String::from_utf8_lossy(&export_scene(&scene, glyphs.as_ref(), &[face], &options).unwrap()).into_owned();
        assert!(pdf.starts_with("%PDF-1.4"));
        assert!(pdf.contains("/MediaBox [0 0 792 612]"), "{pdf}");
        assert!(pdf.contains("dpi=144"));
        assert!(pdf.contains("paper=Letter"));
        assert!(pdf.contains("orientation=landscape"));
        assert!(pdf.contains("transparent=1"));
        assert!(pdf.contains(" l\n") || pdf.contains(" l\r"));

        options.format = ExportFormat::Eps;
        options.paper = Paper::A4;
        options.orientation = PageOrientation::Portrait;
        options.transparent = false;
        let eps = String::from_utf8(export_scene(&scene, glyphs.as_ref(), &[face], &options).unwrap()).unwrap();
        assert!(eps.starts_with("%!PS-Adobe-3.0 EPSF-3.0"));
        assert!(eps.contains("%%BoundingBox: 0 0 595 842"), "{eps}");
        assert!(eps.contains("%%Orientation: Portrait"));
        assert!(eps.contains("%%DocumentPaperSizes: A4"));
        assert!(eps.contains("%%SciPlotResolution: 144"));
        assert!(eps.contains("lineto") || eps.contains("moveto"));

        options.format = ExportFormat::Svg;
        let svg = String::from_utf8(export_scene(&scene, glyphs.as_ref(), &[face], &options).unwrap()).unwrap();
        assert!(svg.contains("<svg"));
    }

    #[test]
    fn print_all_is_a_multi_page_pdf() {
        let scene = line_scene();
        let bytes = font();
        let options = ExportOptions {
            paper: Paper::A5,
            orientation: PageOrientation::Portrait,
            ..ExportOptions::default()
        };
        let pdf = print_pdf(&[scene.clone(), scene], &[bytes.as_slice()], &options).unwrap();
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.starts_with("%PDF-1.4"));
        assert!(text.contains("/Count 2"));
        assert!(text.contains("/MediaBox [0 0 420 595]"));
        assert!(print_pdf(&[], &[], &options).is_err());
    }

    #[test]
    fn chinese_title_becomes_vector_outlines() {
        let project = demo_project();
        let graph = project.graphs.iter().find(|graph| graph.name == sciplot_core::t("正弦与余弦")).unwrap();
        let scene = render(&project, graph, &[], 320.0, 200.0, 1.0);
        let bytes = font();
        let options = ExportOptions { format: ExportFormat::Pdf, ..ExportOptions::default() };
        let with_font = pdf_document(std::slice::from_ref(&scene), &[bytes.as_slice()], &options);
        let without = pdf_document(std::slice::from_ref(&scene), &[], &options);
        assert!(with_font.len() > without.len() + 500, "{} vs {}", with_font.len(), without.len());
        let text = String::from_utf8_lossy(&with_font);
        assert!(text.contains(" c\n"), "glyph curves should be in the PDF");
    }
}
