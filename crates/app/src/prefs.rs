//! 本机界面设置，不写进 `.splot`。存在用户主目录的 `.sciplot_prefs.json`。

use std::cell::Cell;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sciplot_core::{default_true, palette_color, CurveStyle, Dash, GraphId, Language, Project, Symbol};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// 空字符串表示按系统字体自动选择。
    pub font_path: String,
    pub title_size: f32,
    pub axis_label_size: f32,
    pub tick_size: f32,
    pub line_width: f32,
    pub dash: Dash,
    pub symbol: Symbol,
    pub symbol_size: f32,
    pub histogram_bins: usize,
    /// 为真时每条新曲线按序号取调色板，为假时用 `color`。
    pub use_palette: bool,
    pub color: [u8; 4],
    /// 上次成功打开或保存文件的目录。空表示还没有。
    #[serde(default)]
    pub last_dir: String,
    /// 导入导出 CSV 时用的编码。自动导入会识别 UTF-8、UTF-16 和 GBK。
    #[serde(default)]
    pub csv_encoding: sciplot_core::CsvEncoding,
    /// 界面语言。缺省是简体中文。
    #[serde(default)]
    pub language: Language,
    /// 窗口打开时在 127.0.0.1 上提供给本机 AI 的 API。
    #[serde(default = "default_true")]
    pub api_enabled: bool,
    /// 本机 API 端口。被占用时会顺延尝试后面的端口。
    #[serde(default = "default_api_port")]
    pub api_port: u16,
}

fn default_api_port() -> u16 {
    8765
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            font_path: String::new(),
            title_size: 16.0,
            axis_label_size: 14.0,
            tick_size: 12.0,
            line_width: 1.6,
            dash: Dash::Solid,
            symbol: Symbol::Circle,
            symbol_size: 5.0,
            histogram_bins: 12,
            use_palette: true,
            color: palette_color(0),
            last_dir: String::new(),
            csv_encoding: sciplot_core::CsvEncoding::Auto,
            language: Language::Zh,
            api_enabled: true,
            api_port: default_api_port(),
        }
    }
}

impl Prefs {
    pub fn clamp(&mut self) {
        self.font_path = self.font_path.trim().to_string();
        self.title_size = self.title_size.clamp(8.0, 72.0);
        self.axis_label_size = self.axis_label_size.clamp(8.0, 48.0);
        self.tick_size = self.tick_size.clamp(8.0, 36.0);
        self.line_width = self.line_width.clamp(0.4, 12.0);
        self.symbol_size = self.symbol_size.clamp(1.0, 24.0);
        self.histogram_bins = self.histogram_bins.clamp(2, 200);
        if self.api_port < 1024 {
            self.api_port = default_api_port();
        }
    }

    pub fn font_request(&self) -> Option<&str> {
        let path = self.font_path.trim();
        if path.is_empty() {
            None
        } else {
            Some(path)
        }
    }

    pub fn paint_curve(&self, style: &mut CurveStyle, index: usize) {
        let color = if self.use_palette { palette_color(index) } else { self.color };
        style.line_width = self.line_width;
        style.dash = self.dash;
        style.symbol = self.symbol;
        style.symbol_size = self.symbol_size;
        style.histogram_bins = self.histogram_bins.max(1);
        style.color = color;
        style.fill = Some([color[0], color[1], color[2], 90]);
    }

    /// 新建图形：文字字号和全部曲线都换成当前默认。
    pub fn apply_new_graph(&self, project: &mut Project, id: GraphId) {
        let Some(graph) = project.graph_mut(id) else {
            return;
        };
        for layer in &mut graph.layers {
            layer.title_size = self.title_size;
            layer.x_axis.label_size = self.axis_label_size;
            layer.y_axis.label_size = self.axis_label_size;
            layer.x_axis.tick_size = self.tick_size;
            layer.y_axis.tick_size = self.tick_size;
            for (index, curve) in layer.curves.iter_mut().enumerate() {
                self.paint_curve(&mut curve.style, index);
            }
        }
    }

    /// 只改从 `from` 起新加进去的曲线，不动已有曲线和字号。
    pub fn apply_curves(&self, project: &mut Project, id: GraphId, layer: usize, from: usize) {
        let Some(layer) = project.graph_mut(id).and_then(|graph| graph.layers.get_mut(layer)) else {
            return;
        };
        for (index, curve) in layer.curves.iter_mut().enumerate().skip(from) {
            self.paint_curve(&mut curve.style, index);
        }
    }
}

thread_local! {
    static PREFS_HELD: Cell<bool> = const { Cell::new(false) };
}

static PREFS_IO: Mutex<()> = Mutex::new(());

struct PrefsHold;

impl Drop for PrefsHold {
    fn drop(&mut self) {
        PREFS_HELD.with(|held| held.set(false));
    }
}

/// 读写本机设置文件时串起来，测试才能在改完之后把原来的文件放回去。
pub(crate) fn with_prefs_io<T>(body: impl FnOnce() -> T) -> T {
    if PREFS_HELD.with(|held| held.get()) {
        return body();
    }
    let _guard = PREFS_IO.lock().unwrap_or_else(|err| err.into_inner());
    PREFS_HELD.with(|held| held.set(true));
    let _hold = PrefsHold;
    body()
}

pub fn load() -> Prefs {
    with_prefs_io(load_unlocked)
}

fn load_unlocked() -> Prefs {
    let Ok(text) = std::fs::read_to_string(path()) else {
        return Prefs::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub const STYLE_FORMAT: &str = "sciplot-style";
pub const STYLE_VERSION: u32 = 1;
pub const STYLE_NOTE: &str = "SciPlot default style (.spstyle). This file shares the local defaults for new graphs and new curves: font file, sizes, line, symbol, histogram bins, and colors. It is not a graph template and does not store a plot. Language, the local API, and the last folder stay on this machine.";

/// 可以拷给别人的默认样式。不含语言、本机 API 和最近目录。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SharedStyle {
    pub font_path: String,
    pub title_size: f32,
    pub axis_label_size: f32,
    pub tick_size: f32,
    pub line_width: f32,
    pub dash: Dash,
    pub symbol: Symbol,
    pub symbol_size: f32,
    pub histogram_bins: usize,
    pub use_palette: bool,
    pub color: [u8; 4],
}

#[derive(Serialize, Deserialize)]
struct StyleFile {
    format: String,
    version: u32,
    note: String,
    style: SharedStyle,
}

impl Prefs {
    pub fn shared_style(&self) -> SharedStyle {
        SharedStyle {
            font_path: self.font_path.clone(),
            title_size: self.title_size,
            axis_label_size: self.axis_label_size,
            tick_size: self.tick_size,
            line_width: self.line_width,
            dash: self.dash,
            symbol: self.symbol,
            symbol_size: self.symbol_size,
            histogram_bins: self.histogram_bins,
            use_palette: self.use_palette,
            color: self.color,
        }
    }

    pub fn apply_shared_style(&mut self, style: SharedStyle) {
        self.font_path = style.font_path;
        self.title_size = style.title_size;
        self.axis_label_size = style.axis_label_size;
        self.tick_size = style.tick_size;
        self.line_width = style.line_width;
        self.dash = style.dash;
        self.symbol = style.symbol;
        self.symbol_size = style.symbol_size;
        self.histogram_bins = style.histogram_bins;
        self.use_palette = style.use_palette;
        self.color = style.color;
        self.clamp();
    }

    pub fn style_json(&self) -> Result<String, String> {
        let file = StyleFile {
            format: STYLE_FORMAT.to_string(),
            version: STYLE_VERSION,
            note: STYLE_NOTE.to_string(),
            style: self.shared_style(),
        };
        serde_json::to_string_pretty(&file).map_err(|err| err.to_string())
    }

    pub fn apply_style_json(&mut self, text: &str) -> Result<(), String> {
        let file: StyleFile = serde_json::from_str(text).map_err(|err| {
            sciplot_core::trf!(
                "这不是 SciPlot 默认样式（.spstyle）：{err}",
                "This is not a SciPlot default style (.spstyle): {err}",
                err = err
            )
        })?;
        if file.format != STYLE_FORMAT {
            return Err(sciplot_core::tr!(
                "这不是默认样式文件。图形模板是另一种文件。",
                "This is not a default-style file. A graph template is a different file."
            )
            .into());
        }
        if file.version != STYLE_VERSION {
            return Err(sciplot_core::trf!(
                "不认识的样式版本 {version}",
                "Unknown style version {version}",
                version = file.version
            ));
        }
        let port = self.api_port;
        let api_enabled = self.api_enabled;
        let language = self.language;
        let last_dir = self.last_dir.clone();
        let csv_encoding = self.csv_encoding;
        self.apply_shared_style(file.style);
        self.api_port = port;
        self.api_enabled = api_enabled;
        self.language = language;
        self.last_dir = last_dir;
        self.csv_encoding = csv_encoding;
        Ok(())
    }
}

pub fn save(prefs: &Prefs) -> bool {
    with_prefs_io(|| {
        let Ok(text) = serde_json::to_string_pretty(prefs) else {
            return false;
        };
        std::fs::write(path(), text).is_ok()
    })
}

pub(crate) fn prefs_path() -> std::path::PathBuf {
    home_dir().join(".sciplot_prefs.json")
}

pub fn home_dir() -> std::path::PathBuf {
    if let Some(dir) = std::env::var_os("SCIPLOT_HOME").filter(|dir| !dir.is_empty()) {
        return std::path::PathBuf::from(dir);
    }
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

fn path() -> std::path::PathBuf {
    prefs_path()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_keeps_defaults() {
        let prefs = Prefs::default();
        let text = serde_json::to_string(&prefs).unwrap();
        let back: Prefs = serde_json::from_str(&text).unwrap();
        assert_eq!(back, prefs);
        let mut style = CurveStyle::from_index(1);
        back.paint_curve(&mut style, 1);
        assert_eq!(style.color, palette_color(1));
        assert!((style.line_width - 1.6).abs() < 1e-6);
    }

    #[test]
    fn shared_style_roundtrip_does_not_touch_api_or_language() {
        let prefs = Prefs {
            api_port: 9999,
            language: Language::En,
            last_dir: "keep".into(),
            title_size: 21.0,
            use_palette: false,
            color: [1, 2, 3, 255],
            dash: Dash::Dot,
            ..Prefs::default()
        };
        let json = prefs.style_json().unwrap();
        assert!(json.contains("sciplot-style"));
        assert!(json.contains("not a graph template"));
        assert!(!json.contains("9999"));
        let mut other = Prefs { api_port: 1234, language: Language::Zh, ..Prefs::default() };
        other.apply_style_json(&json).unwrap();
        assert!((other.title_size - 21.0).abs() < 1e-6);
        assert!(!other.use_palette);
        assert_eq!(other.color, [1, 2, 3, 255]);
        assert_eq!(other.dash, Dash::Dot);
        assert_eq!(other.api_port, 1234);
        assert_eq!(other.language, Language::Zh);
        assert!(other.apply_style_json(r#"{"format":"sciplot-template","version":1,"note":"","style":{}}"#).is_err());
    }
}
