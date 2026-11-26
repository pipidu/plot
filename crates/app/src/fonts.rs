use std::sync::Arc;

use sciplot_plot::PLOT_FONT_FAMILY;

const EMBEDDED: &[u8] = include_bytes!("../../../assets/fonts/DroidSansFallbackFull.ttf");

const UI_FONTS: &[(&str, &str)] = &[
    ("微软雅黑", r"C:\Windows\Fonts\msyh.ttc"),
    ("微软雅黑", r"C:\Windows\Fonts\msyh.ttf"),
    ("微软雅黑粗体", r"C:\Windows\Fonts\msyhbd.ttc"),
    ("黑体", r"C:\Windows\Fonts\simhei.ttf"),
    ("文泉驿微米黑", "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc"),
    ("思源黑体", "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"),
    ("思源黑体", "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc"),
    ("Droid Sans Fallback", "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf"),
];

const LATIN_FONTS: &[(&str, &str)] = &[
    ("Times New Roman", r"C:\Windows\Fonts\times.ttf"),
    ("Times New Roman", r"C:\Windows\Fonts\times.TTF"),
    ("Liberation Serif", "/usr/share/fonts/truetype/liberation/LiberationSerif-Regular.ttf"),
    ("Liberation Serif", "/usr/share/fonts/truetype/liberation2/LiberationSerif-Regular.ttf"),
    ("Nimbus Roman", "/usr/share/fonts/opentype/urw-base35/NimbusRoman-Regular.otf"),
    ("TeX Gyre Termes", "/usr/share/fonts/opentype/texgyre/texgyretermes-regular.otf"),
    ("FreeSerif", "/usr/share/fonts/truetype/freefont/FreeSerif.ttf"),
];

const CJK_SERIF: &[(&str, &str)] = &[
    ("宋体", r"C:\Windows\Fonts\simsun.ttc"),
    ("宋体", r"C:\Windows\Fonts\simsun.ttf"),
    ("宋体", r"C:\Windows\Fonts\simsunb.ttf"),
    ("Noto Serif CJK", "/usr/share/fonts/opentype/noto/NotoSerifCJK-Regular.ttc"),
    ("Noto Serif CJK", "/usr/share/fonts/truetype/noto/NotoSerifCJK-Regular.ttc"),
    ("思源宋体", "/usr/share/fonts/opentype/source-han-serif/SourceHanSerifSC-Regular.otf"),
    ("文鼎明体", "/usr/share/fonts/truetype/arphic/uming.ttc"),
    ("文鼎明体", "/usr/share/fonts/truetype/arphic-uming/uming.ttc"),
];

pub struct LoadedFonts {
    pub ui: Vec<u8>,
    pub latin: Vec<u8>,
    pub cjk: Vec<u8>,
    pub ok: bool,
}

/// 本机上确实存在、并且能被绘图引擎读入的字体。
pub fn known_fonts() -> Vec<(&'static str, &'static str)> {
    let mut found = Vec::new();
    for list in [UI_FONTS, LATIN_FONTS, CJK_SERIF] {
        for item in list {
            if read_prepared(item.1).is_some() && !found.iter().any(|(_, path)| *path == item.1) {
                found.push(*item);
            }
        }
    }
    found
}

pub fn load(path: Option<&str>) -> LoadedFonts {
    if let Some(path) = path.map(str::trim).filter(|path| !path.is_empty()) {
        if let Some(font) = read_prepared(path) {
            return LoadedFonts { ui: font.clone(), latin: font.clone(), cjk: font, ok: true };
        }
        let fonts = automatic();
        return LoadedFonts { ok: false, ..fonts };
    }
    let mut fonts = automatic();
    fonts.ok = true;
    fonts
}

pub fn plot_glyphs(fonts: &LoadedFonts) -> Option<sciplot_plot::GlyphFont> {
    let mut faces = vec![fonts.latin.clone()];
    if fonts.cjk != fonts.latin {
        faces.push(fonts.cjk.clone());
    }
    sciplot_plot::GlyphFont::from_faces(&faces)
}

/// 设置页上说明当前自动选中的图字体。
pub fn automatic_plot_label() -> String {
    let latin = sciplot_core::t(resolved_name(LATIN_FONTS).unwrap_or("Droid Sans Fallback"));
    let cjk = sciplot_core::t(resolved_name(CJK_SERIF).unwrap_or("Droid Sans Fallback"));
    sciplot_core::trf!(
        "图的默认字体：英文 {0}，中文 {1}。界面仍用黑体。",
        "Plot fonts: {0} for English, {1} for Chinese. The interface stays in a sans font.",
        latin,
        cjk
    )
}

fn automatic() -> LoadedFonts {
    let ui = first_font(UI_FONTS).unwrap_or_else(embedded);
    let latin = first_font(LATIN_FONTS).unwrap_or_else(|| ui.clone());
    let cjk = first_font(CJK_SERIF).unwrap_or_else(embedded);
    LoadedFonts { ui, latin, cjk, ok: true }
}

fn resolved_name(list: &[(&'static str, &str)]) -> Option<&'static str> {
    list.iter().find(|(_, path)| read_prepared(path).is_some()).map(|(name, _)| *name)
}

fn first_font(list: &[(&str, &str)]) -> Option<Vec<u8>> {
    list.iter().find_map(|(_, path)| read_prepared(path))
}

fn embedded() -> Vec<u8> {
    sciplot_plot::prepare_font(EMBEDDED).unwrap_or_else(|| EMBEDDED.to_vec())
}

fn read_prepared(path: &str) -> Option<Vec<u8>> {
    let bytes = std::fs::read(path).ok()?;
    let font = sciplot_plot::prepare_font(&bytes)?;
    sciplot_plot::GlyphFont::from_bytes(&font)?;
    Some(font)
}

pub fn install(ctx: &egui::Context, fonts: &LoadedFonts) {
    let mut defs = egui::FontDefinitions::default();
    insert(&mut defs, "sciplot-ui", &fonts.ui);
    let latin_name = if fonts.latin == fonts.ui {
        "sciplot-ui"
    } else {
        insert(&mut defs, "sciplot-latin", &fonts.latin);
        "sciplot-latin"
    };
    let cjk_name = if fonts.cjk == fonts.latin {
        latin_name
    } else if fonts.cjk == fonts.ui {
        "sciplot-ui"
    } else {
        insert(&mut defs, "sciplot-cjk", &fonts.cjk);
        "sciplot-cjk"
    };
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        defs.families.entry(family).or_default().insert(0, "sciplot-ui".to_owned());
    }
    let mut plot = vec![latin_name.to_owned()];
    if cjk_name != latin_name {
        plot.push(cjk_name.to_owned());
    }
    defs.families.insert(egui::FontFamily::Name(PLOT_FONT_FAMILY.into()), plot);
    ctx.set_fonts(defs);
}

fn insert(defs: &mut egui::FontDefinitions, name: &str, bytes: &[u8]) {
    defs.font_data.insert(name.to_owned(), Arc::new(egui::FontData::from_owned(bytes.to_vec())));
}
