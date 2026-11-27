use serde::{Deserialize, Serialize};

use crate::datetime::AxisClock;
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
    /// 从每个点垂直落到基线（线性轴是 y = 0）。
    DropLines,
    /// 过数据点的自然三次样条。这是绘图样式，不是插值分析。
    Spline,
    /// 先垂直再水平。和 `Steps`（先水平再垂直）组成阶梯图。
    HorizontalSteps,
    /// 同一层上的多条直方图按箱叠高。
    StackedHistogram,
    /// 极坐标 r = f(θ)，画成笛卡尔折线。
    Polar,
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
            CurveKind::DropLines => tr!("垂直下落线", "Vertical drop lines"),
            CurveKind::Spline => tr!("样条", "Spline"),
            CurveKind::HorizontalSteps => tr!("水平阶梯", "Horizontal steps"),
            CurveKind::StackedHistogram => tr!("堆叠直方图", "Stacked histogram"),
            CurveKind::Polar => tr!("极坐标", "Polar"),
        }
    }

    pub fn uses_grid(self) -> bool {
        matches!(self, CurveKind::Contour | CurveKind::Heatmap)
    }

    /// 表格右键「绘制为」里列出的类型。函数、极坐标和矩阵图另有入口。
    pub fn table_kinds() -> &'static [CurveKind] {
        &[
            CurveKind::Line,
            CurveKind::Scatter,
            CurveKind::LineSymbol,
            CurveKind::Steps,
            CurveKind::HorizontalSteps,
            CurveKind::DropLines,
            CurveKind::Spline,
            CurveKind::Bars,
            CurveKind::HorizontalBars,
            CurveKind::Histogram,
            CurveKind::StackedHistogram,
            CurveKind::Area,
            CurveKind::ErrorBars,
            CurveKind::BoxPlot,
            CurveKind::Pie,
            CurveKind::Vector,
        ]
    }
}

/// 阶梯拐角放在区间的哪一侧。`Right` 是原来的「先水平再垂直」。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StepAnchor {
    Left,
    Center,
    #[default]
    Right,
}

impl StepAnchor {
    pub fn label(self) -> &'static str {
        match self {
            StepAnchor::Left => tr!("靠左", "Left"),
            StepAnchor::Center => tr!("居中", "Center"),
            StepAnchor::Right => tr!("靠右", "Right"),
        }
    }

    pub fn all() -> &'static [StepAnchor] {
        &[StepAnchor::Left, StepAnchor::Center, StepAnchor::Right]
    }
}

/// 饼图扇区的填法。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FillPattern {
    #[default]
    Solid,
    Horizontal,
    Vertical,
    Diagonal,
    Cross,
    Dots,
}

impl FillPattern {
    pub fn label(self) -> &'static str {
        match self {
            FillPattern::Solid => tr!("实心", "Solid"),
            FillPattern::Horizontal => tr!("横线", "Horizontal"),
            FillPattern::Vertical => tr!("竖线", "Vertical"),
            FillPattern::Diagonal => tr!("斜线", "Diagonal"),
            FillPattern::Cross => tr!("十字", "Cross"),
            FillPattern::Dots => tr!("点", "Dots"),
        }
    }

    pub fn all() -> &'static [FillPattern] {
        &[
            FillPattern::Solid,
            FillPattern::Horizontal,
            FillPattern::Vertical,
            FillPattern::Diagonal,
            FillPattern::Cross,
            FillPattern::Dots,
        ]
    }
}

/// 箱线的须画到哪里。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WhiskerMode {
    /// 1.5 倍四分位距，外面的点画成离群值。
    #[default]
    Iqr15,
    /// 须画到最小和最大。
    MinMax,
    /// 须画到给定百分位和它的补数，例如 10 与 90。
    Percentile,
}

impl WhiskerMode {
    pub fn label(self) -> &'static str {
        match self {
            WhiskerMode::Iqr15 => tr!("1.5 倍四分位距", "1.5 IQR"),
            WhiskerMode::MinMax => tr!("最小到最大", "Min to max"),
            WhiskerMode::Percentile => tr!("百分位", "Percentile"),
        }
    }

    pub fn all() -> &'static [WhiskerMode] {
        &[WhiskerMode::Iqr15, WhiskerMode::MinMax, WhiskerMode::Percentile]
    }
}

/// 向量的第三、第四列怎么解释。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VectorMode {
    /// dx、dy。
    #[default]
    DxDy,
    /// 终点的 X、Y。
    EndPoint,
    /// 角度（弧度）和模长。
    AngleMagnitude,
}

impl VectorMode {
    pub fn label(self) -> &'static str {
        match self {
            VectorMode::DxDy => tr!("dx、dy", "dx, dy"),
            VectorMode::EndPoint => tr!("终点 XY", "End point XY"),
            VectorMode::AngleMagnitude => tr!("角度和模长", "Angle and magnitude"),
        }
    }

    pub fn all() -> &'static [VectorMode] {
        &[VectorMode::DxDy, VectorMode::EndPoint, VectorMode::AngleMagnitude]
    }
}

/// 误差从哪来。`None` 时如果数据源里已经有误差列，仍会画出那些列。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ErrorSource {
    #[default]
    None,
    Column,
    Percent,
    StandardDeviation,
    StandardError,
}

impl ErrorSource {
    pub fn label(self) -> &'static str {
        match self {
            ErrorSource::None => tr!("无", "None"),
            ErrorSource::Column => tr!("数据列", "Column"),
            ErrorSource::Percent => tr!("百分比", "Percent"),
            ErrorSource::StandardDeviation => tr!("标准差", "Standard deviation"),
            ErrorSource::StandardError => tr!("标准误", "Standard error"),
        }
    }

    pub fn all() -> &'static [ErrorSource] {
        &[
            ErrorSource::None,
            ErrorSource::Column,
            ErrorSource::Percent,
            ErrorSource::StandardDeviation,
            ErrorSource::StandardError,
        ]
    }
}

fn default_percent() -> f64 {
    5.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ErrorSpec {
    pub x: ErrorSource,
    pub y: ErrorSource,
    #[serde(default = "default_percent")]
    pub x_percent: f64,
    #[serde(default = "default_percent")]
    pub y_percent: f64,
}

impl Default for ErrorSpec {
    fn default() -> Self {
        Self {
            x: ErrorSource::None,
            y: ErrorSource::None,
            x_percent: 5.0,
            y_percent: 5.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum XAxisSide {
    #[default]
    Bottom,
    Top,
}

impl XAxisSide {
    pub fn label(self) -> &'static str {
        match self {
            XAxisSide::Bottom => tr!("下轴", "Bottom"),
            XAxisSide::Top => tr!("上轴", "Top"),
        }
    }

    pub fn all() -> &'static [XAxisSide] {
        &[XAxisSide::Bottom, XAxisSide::Top]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum YAxisSide {
    #[default]
    Left,
    Right,
}

impl YAxisSide {
    pub fn label(self) -> &'static str {
        match self {
            YAxisSide::Left => tr!("左轴", "Left"),
            YAxisSide::Right => tr!("右轴", "Right"),
        }
    }

    pub fn all() -> &'static [YAxisSide] {
        &[YAxisSide::Left, YAxisSide::Right]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CurveAxes {
    pub x: XAxisSide,
    pub y: YAxisSide,
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
    /// 阶梯拐角。只对阶梯和水平阶梯有用。
    pub step_anchor: StepAnchor,
    /// 阶梯和样条是否在数据点上画符号。默认不画，避免改掉已有阶梯图。
    pub show_symbols: bool,
    /// 扇区起始角，度。-90 表示从正上方开始，和原来的饼图一致。
    #[serde(default = "default_pie_start")]
    pub pie_start_deg: f32,
    /// 半径占绘图区短边一半的比例。0.68 时和原来的饼图一样大。
    #[serde(default = "default_pie_size")]
    pub pie_size: f32,
    pub pie_pattern: FillPattern,
    pub whisker: WhiskerMode,
    /// 百分位须的下百分位。上百分位是 100 减去它。
    #[serde(default = "default_whisker_percentile")]
    pub whisker_percentile: f64,
    pub box_mean: bool,
    #[serde(default = "default_true")]
    pub box_outliers: bool,
}

fn default_pie_start() -> f32 {
    -90.0
}

fn default_pie_size() -> f32 {
    0.68
}

fn default_whisker_percentile() -> f64 {
    10.0
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
            step_anchor: StepAnchor::Right,
            show_symbols: false,
            pie_start_deg: default_pie_start(),
            pie_size: default_pie_size(),
            pie_pattern: FillPattern::Solid,
            whisker: WhiskerMode::Iqr15,
            whisker_percentile: default_whisker_percentile(),
            box_mean: false,
            box_outliers: true,
        }
    }
}

/// 刻度写成普通数字，还是日期、时刻、日期时间。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TickKind {
    #[default]
    Number,
    Date,
    Time,
    DateTime,
}

impl TickKind {
    pub fn label(self) -> &'static str {
        match self {
            TickKind::Number => tr!("数字", "Number"),
            TickKind::Date => tr!("日期", "Date"),
            TickKind::Time => tr!("时刻", "Time"),
            TickKind::DateTime => tr!("日期时间", "Date and time"),
        }
    }

    pub fn all() -> &'static [TickKind] {
        &[TickKind::Number, TickKind::Date, TickKind::Time, TickKind::DateTime]
    }

    pub fn clock(self) -> AxisClock {
        match self {
            TickKind::Number => AxisClock::Number,
            TickKind::Date => AxisClock::Date,
            TickKind::Time => AxisClock::Time,
            TickKind::DateTime => AxisClock::DateTime,
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
    pub tick_kind: TickKind,
    /// 刻度文字格式。空字符串按范围自动选短格式。写法同列的日期格式。
    pub tick_pattern: String,
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
            tick_kind: TickKind::Number,
            tick_pattern: String::new(),
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
    /// r = f(θ)。θ 用弧度，表达式里可以用 `theta`、`t` 或 `x`。
    Polar {
        expr: String,
        theta_min: f64,
        theta_max: f64,
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
    pub axes: CurveAxes,
    pub vector_mode: VectorMode,
    pub error: ErrorSpec,
}

impl Default for Curve {
    fn default() -> Self {
        Self {
            name: crate::locale::t("曲线").to_string(),
            kind: CurveKind::Line,
            style: CurveStyle::default(),
            source: DataSource::default(),
            visible: true,
            axes: CurveAxes::default(),
            vector_mode: VectorMode::DxDy,
            error: ErrorSpec::default(),
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
    /// 图上的图片。`x`,`y` 是左下角的数据坐标（下轴、左轴），`w`,`h` 是数据单位里的宽高。
    Image {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        /// PNG 的 base64。插入时由程序写成 PNG，方便 SVG 直接嵌进去。
        png_base64: String,
    },
    /// 插入当时的时间文字。不会在每次重画时改掉。
    TimeStamp {
        x: f64,
        y: f64,
        text: String,
        size: f32,
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
    /// 上轴。`show_top` 为真，或有曲线绑到上轴时才画。
    pub x_top: Axis,
    /// 右轴。
    pub y_right: Axis,
    pub show_top: bool,
    pub show_right: bool,
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
            x_top: Axis {
                label: String::new(),
                ..Axis::default()
            },
            y_right: Axis {
                label: String::new(),
                ..Axis::default()
            },
            show_top: false,
            show_right: false,
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
    /// 填色多边形，不加光照。
    Polygons,
    Wireframe,
    /// 用画家算法挡住被挡住的网格线。
    HiddenLines,
    /// 填色多边形上再画网格。
    WireSurface,
    Scatter,
}

impl SurfaceMode {
    pub fn label(self) -> &'static str {
        match self {
            SurfaceMode::Surface => tr!("曲面", "Surface"),
            SurfaceMode::Polygons => tr!("多边形", "Polygons"),
            SurfaceMode::Wireframe => tr!("线框", "Wireframe"),
            SurfaceMode::HiddenLines => tr!("隐藏线", "Hidden lines"),
            SurfaceMode::WireSurface => tr!("网格曲面", "Wire surface"),
            SurfaceMode::Scatter => tr!("散点", "Scatter"),
        }
    }

    pub fn all() -> &'static [SurfaceMode] {
        &[
            SurfaceMode::Surface,
            SurfaceMode::Polygons,
            SurfaceMode::Wireframe,
            SurfaceMode::HiddenLines,
            SurfaceMode::WireSurface,
            SurfaceMode::Scatter,
        ]
    }
}

/// 散点模式下的点样式。圆点是原来的样子。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SurfacePointStyle {
    #[default]
    Dot,
    Cross,
    Cone,
    Bar,
}

impl SurfacePointStyle {
    pub fn label(self) -> &'static str {
        match self {
            SurfacePointStyle::Dot => tr!("圆点", "Dots"),
            SurfacePointStyle::Cross => tr!("十字", "Cross"),
            SurfacePointStyle::Cone => tr!("锥体", "Cone"),
            SurfacePointStyle::Bar => tr!("竖线", "Vertical bar"),
        }
    }

    pub fn all() -> &'static [SurfacePointStyle] {
        &[
            SurfacePointStyle::Dot,
            SurfacePointStyle::Cross,
            SurfacePointStyle::Cone,
            SurfacePointStyle::Bar,
        ]
    }
}

/// 表格里选中一列之后的三维图。色带、柱和轨迹都只认这一列。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TablePlot3D {
    Ribbon,
    Bars,
    Trajectory,
}

impl TablePlot3D {
    pub fn label(self) -> &'static str {
        match self {
            TablePlot3D::Ribbon => tr!("三维色带", "3D ribbons"),
            TablePlot3D::Bars => tr!("三维柱", "3D bars"),
            TablePlot3D::Trajectory => tr!("三维轨迹", "3D trajectory"),
        }
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
    /// 表格中选中的一列。`column` 是 Z。
    Table {
        table: TableId,
        column: usize,
        kind: TablePlot3D,
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
    pub point_style: SurfacePointStyle,
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
            point_style: SurfacePointStyle::Dot,
        }
    }
}

impl SurfaceSpec {
    pub fn camera_is_default(&self) -> bool {
        (self.azimuth + 55.0).abs() < 0.05
            && (self.elevation - 32.0).abs() < 0.05
            && (self.zoom - 1.0).abs() < 0.01
    }

    pub fn restore_camera(&mut self) {
        self.azimuth = -55.0;
        self.elevation = 32.0;
        self.zoom = 1.0;
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
