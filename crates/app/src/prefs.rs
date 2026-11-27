//! 本机界面设置，不写进 `.splot`。存在用户主目录的 `.sciplot_prefs.json`。

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

pub fn load() -> Prefs {
    let Ok(text) = std::fs::read_to_string(path()) else {
        return Prefs::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save(prefs: &Prefs) -> bool {
    let Ok(text) = serde_json::to_string_pretty(prefs) else {
        return false;
    };
    std::fs::write(path(), text).is_ok()
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
    home_dir().join(".sciplot_prefs.json")
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
}
