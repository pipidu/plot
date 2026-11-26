use sciplot_core::Dash;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VAlign {
    Top,
    Center,
    Bottom,
}

#[derive(Clone, Debug)]
pub enum Prim {
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: [u8; 4],
        width: f32,
        dash: Dash,
    },
    Polyline {
        pts: Vec<(f32, f32)>,
        color: [u8; 4],
        width: f32,
        dash: Dash,
    },
    Polygon {
        pts: Vec<(f32, f32)>,
        fill: [u8; 4],
    },
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        fill: Option<[u8; 4]>,
        stroke: Option<([u8; 4], f32)>,
    },
    Circle {
        x: f32,
        y: f32,
        r: f32,
        fill: Option<[u8; 4]>,
        stroke: Option<([u8; 4], f32)>,
    },
    Triangle {
        a: (f32, f32),
        b: (f32, f32),
        c: (f32, f32),
        fill: [u8; 4],
    },
    Text {
        x: f32,
        y: f32,
        text: String,
        size: f32,
        color: [u8; 4],
        halign: HAlign,
        valign: VAlign,
        vertical: bool,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct PxRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl PxRect {
    pub fn contains(self, px: f32, py: f32) -> bool {
        px >= self.x && py >= self.y && px <= self.x + self.w && py <= self.y + self.h
    }

    pub fn right(self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(self) -> f32 {
        self.y + self.h
    }
}

/// 用户缩放/平移覆盖坐标轴范围。`None` 表示沿用坐标轴设置或自动范围。
#[derive(Clone, Copy, Debug, Default)]
pub struct LayerView {
    pub x_min: Option<f64>,
    pub x_max: Option<f64>,
    pub y_min: Option<f64>,
    pub y_max: Option<f64>,
}

/// 一个图层在屏幕上的绘图区，以及已经解析好的数据范围。交互用它做命中和缩放。
#[derive(Clone, Debug)]
pub struct LayerFrame {
    pub plot: PxRect,
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
    pub x_scale: sciplot_core::AxisScale,
    pub y_scale: sciplot_core::AxisScale,
    pub x_inverted: bool,
    pub y_inverted: bool,
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub width: f32,
    pub height: f32,
    pub primitives: Vec<Prim>,
    pub layers: Vec<LayerFrame>,
    pub notes: Vec<String>,
}

impl Scene {
    pub fn text_content(&self) -> String {
        let mut out = String::new();
        for prim in &self.primitives {
            if let Prim::Text { text, .. } = prim {
                out.push_str(text);
                out.push('\n');
            }
        }
        out
    }
}

pub fn dash_intervals(dash: Dash, width: f32) -> Option<Vec<f32>> {
    let w = width.max(1.0);
    match dash {
        Dash::Solid => None,
        Dash::Dash => Some(vec![4.0 * w, 3.0 * w]),
        Dash::Dot => Some(vec![w, 2.5 * w]),
        Dash::DashDot => Some(vec![4.0 * w, 2.0 * w, w, 2.0 * w]),
    }
}
