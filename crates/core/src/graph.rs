use serde::{Deserialize, Serialize};

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
            CurveKind::Line => "折线",
            CurveKind::Scatter => "散点",
            CurveKind::LineSymbol => "点线",
            CurveKind::Steps => "阶梯",
            CurveKind::Bars => "柱状",
            CurveKind::HorizontalBars => "条形",
            CurveKind::Histogram => "直方图",
            CurveKind::Area => "面积",
            CurveKind::ErrorBars => "误差棒",
            CurveKind::BoxPlot => "箱线",
            CurveKind::Pie => "饼图",
            CurveKind::Vector => "向量场",
            CurveKind::Contour => "等高线",
            CurveKind::Heatmap => "热力图",
            CurveKind::Function => "函数",
            CurveKind::Parametric => "参数曲线",
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
            AxisScale::Linear => "线性",
            AxisScale::Log10 => "对数 (log10)",
            AxisScale::Reciprocal => "倒数 (1/x)",
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
            Dash::Solid => "实线",
            Dash::Dash => "虚线",
            Dash::Dot => "点线",
            Dash::DashDot => "点划线",
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
            Symbol::None => "无",
            Symbol::Circle => "圆",
            Symbol::Square => "方",
            Symbol::Diamond => "菱形",
            Symbol::Triangle => "三角",
            Symbol::Cross => "叉",
            Symbol::Plus => "加号",
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
            LegendPos::TopRight => "右上",
            LegendPos::TopLeft => "左上",
            LegendPos::BottomRight => "右下",
            LegendPos::BottomLeft => "左下",
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
            name: "曲线".to_string(),
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
            SurfaceMode::Surface => "曲面",
            SurfaceMode::Wireframe => "线框",
            SurfaceMode::Scatter => "散点",
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
            title: "三维图".to_string(),
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
            name: "图".to_string(),
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
