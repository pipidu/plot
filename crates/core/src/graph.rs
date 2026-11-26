use serde::{Deserialize, Serialize};

use crate::tr;
use crate::util::{default_true, palette_color};
use crate::{GraphId, MatrixId, TableId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CurveKind {
    #[default]
    Line,
    Scatter,
    LineSymbol,
    Steps,
    Bars,
    HorizontalBars,
    Histogram,
    Area,
    ErrorBars,
    BoxPlot,
    Pie,
    Vector,
    Contour,
    Heatmap,
    Function,
    Parametric,
}

impl CurveKind {
    pub fn label(self) -> &'static str {
        match self {
            CurveKind::Line => tr!("折线", "Line"),
            CurveKind::Scatter => tr!("散点", "Scatter"),
            CurveKind::LineSymbol => tr!("点线", "Line + symbol"),
            CurveKind::Steps => tr!("阶梯", "Steps"),
            CurveKind::Bars => tr!("柱状", "Bars"),
            CurveKind::HorizontalBars => tr!("条形", "Horizontal bars"),
            CurveKind::Histogram => tr!("直方图", "Histogram"),
            CurveKind::Area => tr!("面积", "Area"),
            CurveKind::ErrorBars => tr!("误差棒", "Error bars"),
            CurveKind::BoxPlot => tr!("箱线", "Box plot"),
            CurveKind::Pie => tr!("饼图", "Pie"),
            CurveKind::Vector => tr!("向量场", "Vector field"),
            CurveKind::Contour => tr!("等高线", "Contour"),
            CurveKind::Heatmap => tr!("热力图", "Heat map"),
            CurveKind::Function => tr!("函数", "Function"),
            CurveKind::Parametric => tr!("参数曲线", "Parametric"),
        }
    }

    pub fn uses_grid(self) -> bool {
        matches!(self, CurveKind::Contour | CurveKind::Heatmap)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AxisScale {
    #[default]
    Linear,
    Log10,
    Reciprocal,
}

impl AxisScale {
    pub fn label(self) -> &'static str {
        match self {
            AxisScale::Linear => tr!("线性", "Linear"),
            AxisScale::Log10 => tr!("对数 (log10)", "Log10"),
            AxisScale::Reciprocal => tr!("倒数 (1/x)", "Reciprocal (1/x)"),
        }
    }

    pub fn all() -> &'static [AxisScale] {
        &[AxisScale::Linear, AxisScale::Log10, AxisScale::Reciprocal]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Dash {
    #[default]
    Solid,
    Dash,
    Dot,
    DashDot,
}

impl Dash {
    pub fn label(self) -> &'static str {
        match self {
            Dash::Solid => tr!("实线", "Solid"),
            Dash::Dash => tr!("虚线", "Dashed"),
            Dash::Dot => tr!("点线", "Dotted"),
            Dash::DashDot => tr!("点划线", "Dash-dot"),
        }
    }

    pub fn all() -> &'static [Dash] {
        &[Dash::Solid, Dash::Dash, Dash::Dot, Dash::DashDot]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Symbol {
    None,
    #[default]
    Circle,
    Square,
    Diamond,
    Triangle,
    Cross,
    Plus,
}

impl Symbol {
    pub fn label(self) -> &'static str {
        match self {
            Symbol::None => tr!("无", "None"),
            Symbol::Circle => tr!("圆", "Circle"),
            Symbol::Square => tr!("方", "Square"),
            Symbol::Diamond => tr!("菱形", "Diamond"),
            Symbol::Triangle => tr!("三角", "Triangle"),
            Symbol::Cross => tr!("叉", "Cross"),
            Symbol::Plus => tr!("加号", "Plus"),
        }
    }

    pub fn all() -> &'static [Symbol] {
        &[
            Symbol::None,
            Symbol::Circle,
            Symbol::Square,
            Symbol::Diamond,
            Symbol::Triangle,
            Symbol::Cross,
            Symbol::Plus,
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LegendPos {
    #[default]
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
}

impl LegendPos {
    pub fn label(self) -> &'static str {
        match self {
            LegendPos::TopRight => tr!("右上", "Top right"),
            LegendPos::TopLeft => tr!("左上", "Top left"),
            LegendPos::BottomRight => tr!("右下", "Bottom right"),
            LegendPos::BottomLeft => tr!("左下", "Bottom left"),
        }
    }

    pub fn all() -> &'static [LegendPos] {
        &[
            LegendPos::TopRight,
            LegendPos::TopLeft,
            LegendPos::BottomRight,
            LegendPos::BottomLeft,
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CurveStyle {
    pub color: [u8; 4],
    pub line_width: f32,
    pub dash: Dash,
    pub symbol: Symbol,
    pub symbol_size: f32,
    pub fill: Option<[u8; 4]>,
    pub histogram_bins: usize,
}

impl Default for CurveStyle {
    fn default() -> Self {
        Self::from_index(0)
    }
}

impl CurveStyle {
    pub fn from_index(index: usize) -> Self {
        let color = palette_color(index);
        Self {
            color,
            line_width: 1.6,
            dash: Dash::Solid,
            symbol: Symbol::Circle,
            symbol_size: 5.0,
            fill: Some([color[0], color[1], color[2], 90]),
            histogram_bins: 12,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Axis {
    pub label: String,
    pub scale: AxisScale,
    pub min: Option<f64>,
    pub max: Option<f64>,
    #[serde(default = "default_true")]
    pub major_grid: bool,
    pub minor_grid: bool,
    pub inverted: bool,
    pub label_size: f32,
    pub tick_size: f32,
    /// 主刻度步长。空表示按 1-2-5 自动取。对数轴上表示每隔几个十进制数量级一条主刻度。
    pub step: Option<f64>,
    /// 相邻主刻度之间分成几段。空表示 4 等分。
    pub minor_count: Option<u8>,
}

impl Default for Axis {
    fn default() -> Self {
        Self {
            label: String::new(),
            scale: AxisScale::Linear,
            min: None,
            max: None,
            major_grid: true,
            minor_grid: false,
            inverted: false,
            label_size: 14.0,
            tick_size: 12.0,
            step: None,
            minor_count: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Legend {
    #[serde(default = "default_true")]
    pub visible: bool,
    pub position: LegendPos,
}

impl Default for Legend {
    fn default() -> Self {
        Self {
            visible: true,
            position: LegendPos::TopRight,
        }
    }
}

/// 归一化到整张图上的矩形，原点在左上，宽高为占图幅的比例。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FrameRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Default for FrameRect {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DataSource {
    Columns {
        table: TableId,
        x: usize,
        y: usize,
        x_err: Option<usize>,
        y_err: Option<usize>,
        dx: Option<usize>,
        dy: Option<usize>,
    },
    Matrix {
        matrix: MatrixId,
    },
    Function {
        expr: String,
        x_min: f64,
        x_max: f64,
        samples: usize,
    },
    Parametric {
        x_expr: String,
        y_expr: String,
        t_min: f64,
        t_max: f64,
        samples: usize,
    },
    Points {
        x: Vec<f64>,
        y: Vec<f64>,
    },
}

impl Default for DataSource {
    fn default() -> Self {
        DataSource::Points {
            x: Vec::new(),
            y: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Curve {
    pub name: String,
    pub kind: CurveKind,
    pub style: CurveStyle,
    pub source: DataSource,
    #[serde(default = "default_true")]
    pub visible: bool,
}

impl Default for Curve {
    fn default() -> Self {
        Self {
            name: crate::locale::t("曲线").to_string(),
            kind: CurveKind::Line,
            style: CurveStyle::default(),
            source: DataSource::default(),
            visible: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AnnotationKind {
    Text {
        x: f64,
        y: f64,
        text: String,
        size: f32,
        color: [u8; 4],
    },
    Arrow {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        color: [u8; 4],
    },
    Line {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        color: [u8; 4],
    },
    Marker {
        x: f64,
        y: f64,
        label: String,
        color: [u8; 4],
    },
}

impl Default for AnnotationKind {
    fn default() -> Self {
        AnnotationKind::Text {
            x: 0.0,
            y: 0.0,
            text: String::new(),
            size: 13.0,
            color: [20, 20, 20, 255],
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Annotation {
    pub id: u64,
    pub kind: AnnotationKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layer {
    pub title: String,
    pub title_size: f32,
    pub frame: FrameRect,
    pub x_axis: Axis,
    pub y_axis: Axis,
    pub legend: Legend,
    pub curves: Vec<Curve>,
    pub annotations: Vec<Annotation>,
}

impl Default for Layer {
    fn default() -> Self {
        Self {
            title: String::new(),
            title_size: 16.0,
            frame: FrameRect::default(),
            x_axis: Axis {
                label: "x".to_string(),
                ..Axis::default()
            },
            y_axis: Axis {
                label: "y".to_string(),
                ..Axis::default()
            },
            legend: Legend::default(),
            curves: Vec::new(),
            annotations: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SurfaceMode {
    #[default]
    Surface,
    Wireframe,
    Scatter,
}

impl SurfaceMode {
    pub fn label(self) -> &'static str {
        match self {
            SurfaceMode::Surface => tr!("曲面", "Surface"),
            SurfaceMode::Wireframe => tr!("线框", "Wireframe"),
            SurfaceMode::Scatter => tr!("散点", "Scatter"),
        }
    }

    pub fn all() -> &'static [SurfaceMode] {
        &[SurfaceMode::Surface, SurfaceMode::Wireframe, SurfaceMode::Scatter]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SurfaceSource {
    Matrix(MatrixId),
    Function {
        expr: String,
        x_min: f64,
        x_max: f64,
        y_min: f64,
        y_max: f64,
        n: usize,
    },
}

impl Default for SurfaceSource {
    fn default() -> Self {
        SurfaceSource::Function {
            expr: "sin(x)*cos(y)".to_string(),
            x_min: -std::f64::consts::PI,
            x_max: std::f64::consts::PI,
            y_min: -std::f64::consts::PI,
            y_max: std::f64::consts::PI,
            n: 40,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SurfaceSpec {
    pub source: SurfaceSource,
    pub mode: SurfaceMode,
    pub azimuth: f32,
    pub elevation: f32,
    pub zoom: f32,
    #[serde(default = "default_true")]
    pub perspective: bool,
    pub title: String,
}

impl Default for SurfaceSpec {
    fn default() -> Self {
        Self {
            source: SurfaceSource::default(),
            mode: SurfaceMode::Surface,
            azimuth: -55.0,
            elevation: 32.0,
            zoom: 1.0,
            perspective: true,
            title: crate::locale::t("三维图").to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Graph {
    pub id: GraphId,
    pub name: String,
    pub layers: Vec<Layer>,
    pub surface: Option<SurfaceSpec>,
}

impl Default for Graph {
    fn default() -> Self {
        Self {
            id: GraphId(0),
            name: crate::locale::t("图").to_string(),
            layers: vec![Layer::default()],
            surface: None,
        }
    }
}

impl Graph {
    pub fn new(id: GraphId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            ..Self::default()
        }
    }

    pub fn reflow_layers(&mut self) {
        let n = self.layers.len().max(1) as f32;
        for (i, layer) in self.layers.iter_mut().enumerate() {
            layer.frame = FrameRect {
                x: 0.0,
                y: i as f32 / n,
                w: 1.0,
                h: 1.0 / n,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reflow_splits_vertically() {
        let mut graph = Graph::new(GraphId(1), "g");
        graph.layers.push(Layer::default());
        graph.reflow_layers();
        assert!((graph.layers[0].frame.h - 0.5).abs() < 1e-6);
        assert!((graph.layers[1].frame.y - 0.5).abs() < 1e-6);
    }
}
