use std::sync::Arc;

/// 优先用系统里的微软雅黑或思源黑体，否则用内嵌的 Droid Sans Fallback。
pub fn load_bytes() -> Vec<u8> {
    const EMBEDDED: &[u8] = include_bytes!("../../../assets/fonts/DroidSansFallbackFull.ttf");
    let candidates = [
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\msyh.ttf",
        r"C:\Windows\Fonts\msyhbd.ttc",
        r"C:\Windows\Fonts\simhei.ttf",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            if let Some(font) = sciplot_plot::prepare_font(&bytes) {
                if sciplot_plot::GlyphFont::from_bytes(&font).is_some() {
                    return font;
                }
            }
        }
    }
    sciplot_plot::prepare_font(EMBEDDED).unwrap_or_else(|| EMBEDDED.to_vec())
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
