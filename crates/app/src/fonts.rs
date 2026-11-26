use std::sync::Arc;

const EMBEDDED: &[u8] = include_bytes!("../../../assets/fonts/DroidSansFallbackFull.ttf");

const CANDIDATES: &[(&str, &str)] = &[
    ("微软雅黑", r"C:\Windows\Fonts\msyh.ttc"),
    ("微软雅黑", r"C:\Windows\Fonts\msyh.ttf"),
    ("微软雅黑粗体", r"C:\Windows\Fonts\msyhbd.ttc"),
    ("黑体", r"C:\Windows\Fonts\simhei.ttf"),
    ("文泉驿微米黑", "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc"),
    ("思源黑体", "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"),
    ("思源黑体", "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc"),
    ("Droid Sans Fallback", "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf"),
];

/// 本机上确实存在、并且能被绘图引擎读入的字体。
pub fn known_fonts() -> Vec<(&'static str, &'static str)> {
    CANDIDATES
        .iter()
        .copied()
        .filter(|(_, path)| read_prepared(path).is_some())
        .collect()
}

/// 优先用系统里的微软雅黑或思源黑体，否则用内嵌的 Droid Sans Fallback。
pub fn load_bytes() -> Vec<u8> {
    for (_, path) in CANDIDATES {
        if let Some(font) = read_prepared(path) {
            return font;
        }
    }
    sciplot_plot::prepare_font(EMBEDDED).unwrap_or_else(|| EMBEDDED.to_vec())
}

/// `path` 为空时与 [`load_bytes`] 相同。指定文件读不出来时退回自动字体，第二个值是 `false`。
pub fn load_chosen(path: Option<&str>) -> (Vec<u8>, bool) {
    if let Some(path) = path.map(str::trim).filter(|path| !path.is_empty()) {
        if let Some(font) = read_prepared(path) {
            return (font, true);
        }
        return (load_bytes(), false);
    }
    (load_bytes(), true)
}

fn read_prepared(path: &str) -> Option<Vec<u8>> {
    let bytes = std::fs::read(path).ok()?;
    let font = sciplot_plot::prepare_font(&bytes)?;
    sciplot_plot::GlyphFont::from_bytes(&font)?;
    Some(font)
}

pub fn install(ctx: &egui::Context, bytes: &[u8]) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "sciplot-cjk".to_owned(),
        Arc::new(egui::FontData::from_owned(bytes.to_vec())),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(family).or_default().insert(0, "sciplot-cjk".to_owned());
    }
    ctx.set_fonts(fonts);
}
