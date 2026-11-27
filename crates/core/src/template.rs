//! 图形模板。SciDAVis 的 `.qpt` 是 Qt 二进制，这里不假装能读写它。
//! `.sptpl` 是 SciPlot 自己的 JSON：记下窗口几何、字体、颜色和文字，
//! 打开后是一张没有曲线的空图。曲线样式和刻度不写入模板。

use serde::{Deserialize, Serialize};

use crate::graph::{
    AnnotationKind, AxisScale, FrameRect, Graph, Layer, LayerArrangement, Legend, LegendPos,
};
use crate::GraphId;

pub const TEMPLATE_FORMAT: &str = "sciplot-template";
pub const TEMPLATE_VERSION: u32 = 1;
pub const TEMPLATE_NOTE: &str = "SciPlot graph template (.sptpl). This is not a SciDAVis .qpt binary. It stores window geometry, fonts, colors, and text labels, then opens as an empty graph. Curve styles and tick settings are not stored.";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphTemplate {
    pub name: String,
    pub arrangement: LayerArrangement,
    /// 图窗口的参考宽高，单位是像素。停靠界面不会把它变成独立的系统窗口。
    pub canvas_width: f32,
    pub canvas_height: f32,
    pub layers: Vec<LayerTemplate>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayerTemplate {
    pub title: String,
    pub title_size: f32,
    pub frame: FrameRect,
    pub x_label: String,
    pub y_label: String,
    pub x_label_size: f32,
    pub y_label_size: f32,
    pub x_scale: AxisScale,
    pub y_scale: AxisScale,
    pub x_inverted: bool,
    pub y_inverted: bool,
    pub show_top: bool,
    pub show_right: bool,
    pub top_label: String,
    pub right_label: String,
    pub legend_visible: bool,
    pub legend_position: LegendPos,
    pub labels: Vec<TextLabel>,
}

/// 模板里的文字、箭头和线段。不含曲线，也不含交点标记。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextLabel {
    pub kind: LabelKind,
    pub text: String,
    pub size: f32,
    pub color: [u8; 4],
    pub x: f64,
    pub y: f64,
    pub x2: f64,
    pub y2: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelKind {
    Text,
    Arrow,
    Line,
    TimeStamp,
}

#[derive(Serialize, Deserialize)]
struct TemplateFile {
    format: String,
    version: u32,
    note: String,
    template: GraphTemplate,
}

impl GraphTemplate {
    pub fn from_graph(graph: &Graph, canvas_width: f32, canvas_height: f32) -> Self {
        Self {
            name: graph.name.clone(),
            arrangement: graph.arrangement.clone(),
            canvas_width: canvas_width.max(1.0),
            canvas_height: canvas_height.max(1.0),
            layers: graph.layers.iter().map(LayerTemplate::from_layer).collect(),
        }
    }

    pub fn to_json(&self) -> Result<String, String> {
        let file = TemplateFile {
            format: TEMPLATE_FORMAT.to_string(),
            version: TEMPLATE_VERSION,
            note: TEMPLATE_NOTE.to_string(),
            template: self.clone(),
        };
        serde_json::to_string_pretty(&file).map_err(|err| err.to_string())
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        let file: TemplateFile = serde_json::from_str(text).map_err(|err| {
            crate::trf!(
                "这不是 SciPlot 图形模板（.sptpl）：{err}",
                "This is not a SciPlot graph template (.sptpl): {err}",
                err = err
            )
        })?;
        if file.format != TEMPLATE_FORMAT {
            return Err(crate::tr!(
                "这不是 SciPlot 图形模板。SciDAVis 的 .qpt 是另一种二进制文件，这里不读取。",
                "This is not a SciPlot graph template. A SciDAVis .qpt is a different binary file and is not read here."
            )
            .into());
        }
        if file.version != TEMPLATE_VERSION {
            return Err(crate::trf!(
                "不认识的模板版本 {version}",
                "Unknown template version {version}",
                version = file.version
            ));
        }
        if file.template.layers.is_empty() {
            return Err(crate::tr!("模板里没有图层", "The template has no layers").into());
        }
        Ok(file.template)
    }

    /// 生成一张没有曲线的图。刻度步长、次刻度和刻度格式保持默认，不从模板恢复。
    pub fn into_graph(&self, id: GraphId) -> Graph {
        let mut graph = Graph::new(id, self.name.clone());
        graph.arrangement = self.arrangement.clone();
        graph.layers = self.layers.iter().map(LayerTemplate::to_layer).collect();
        if graph.layers.is_empty() {
            graph.layers.push(Layer::default());
        }
        graph
    }
}

impl LayerTemplate {
    fn from_layer(layer: &Layer) -> Self {
        Self {
            title: layer.title.clone(),
            title_size: layer.title_size,
            frame: layer.frame,
            x_label: layer.x_axis.label.clone(),
            y_label: layer.y_axis.label.clone(),
            x_label_size: layer.x_axis.label_size,
            y_label_size: layer.y_axis.label_size,
            x_scale: layer.x_axis.scale,
            y_scale: layer.y_axis.scale,
            x_inverted: layer.x_axis.inverted,
            y_inverted: layer.y_axis.inverted,
            show_top: layer.show_top,
            show_right: layer.show_right,
            top_label: layer.x_top.label.clone(),
            right_label: layer.y_right.label.clone(),
            legend_visible: layer.legend.visible,
            legend_position: layer.legend.position,
            labels: layer.annotations.iter().filter_map(|annotation| TextLabel::from_kind(&annotation.kind)).collect(),
        }
    }

    fn to_layer(&self) -> Layer {
        let annotations = self
            .labels
            .iter()
            .enumerate()
            .map(|(index, label)| crate::graph::Annotation {
                id: (index as u64) + 1,
                kind: label.to_kind(),
                ..crate::graph::Annotation::default()
            })
            .collect();
        let mut layer = Layer {
            title: self.title.clone(),
            title_size: self.title_size,
            frame: self.frame,
            show_top: self.show_top,
            show_right: self.show_right,
            legend: Legend {
                visible: self.legend_visible,
                position: self.legend_position,
            },
            annotations,
            ..Layer::default()
        };
        layer.x_axis.label = self.x_label.clone();
        layer.y_axis.label = self.y_label.clone();
        layer.x_axis.label_size = self.x_label_size;
        layer.y_axis.label_size = self.y_label_size;
        layer.x_axis.scale = self.x_scale;
        layer.y_axis.scale = self.y_scale;
        layer.x_axis.inverted = self.x_inverted;
        layer.y_axis.inverted = self.y_inverted;
        layer.x_top.label = self.top_label.clone();
        layer.y_right.label = self.right_label.clone();
        layer
    }
}

impl TextLabel {
    fn from_kind(kind: &AnnotationKind) -> Option<Self> {
        Some(match kind {
            AnnotationKind::Text { x, y, text, size, color } => Self {
                kind: LabelKind::Text,
                text: text.clone(),
                size: *size,
                color: *color,
                x: *x,
                y: *y,
                x2: 0.0,
                y2: 0.0,
            },
            AnnotationKind::TimeStamp { x, y, text, size, color } => Self {
                kind: LabelKind::TimeStamp,
                text: text.clone(),
                size: *size,
                color: *color,
                x: *x,
                y: *y,
                x2: 0.0,
                y2: 0.0,
            },
            AnnotationKind::Arrow { x1, y1, x2, y2, color } => Self {
                kind: LabelKind::Arrow,
                text: String::new(),
                size: 0.0,
                color: *color,
                x: *x1,
                y: *y1,
                x2: *x2,
                y2: *y2,
            },
            AnnotationKind::Line { x1, y1, x2, y2, color } => Self {
                kind: LabelKind::Line,
                text: String::new(),
                size: 0.0,
                color: *color,
                x: *x1,
                y: *y1,
                x2: *x2,
                y2: *y2,
            },
            AnnotationKind::Marker { .. } | AnnotationKind::Image { .. } => return None,
        })
    }

    fn to_kind(&self) -> AnnotationKind {
        match self.kind {
            LabelKind::Text => AnnotationKind::Text {
                x: self.x,
                y: self.y,
                text: self.text.clone(),
                size: self.size,
                color: self.color,
            },
            LabelKind::TimeStamp => AnnotationKind::TimeStamp {
                x: self.x,
                y: self.y,
                text: self.text.clone(),
                size: self.size,
                color: self.color,
            },
            LabelKind::Arrow => AnnotationKind::Arrow {
                x1: self.x,
                y1: self.y,
                x2: self.x2,
                y2: self.y2,
                color: self.color,
            },
            LabelKind::Line => AnnotationKind::Line {
                x1: self.x,
                y1: self.y,
                x2: self.x2,
                y2: self.y2,
                color: self.color,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Annotation, AnnotationKind, Curve, CurveKind};

    #[test]
    fn template_roundtrip_keeps_geometry_fonts_colors_and_drops_curves() {
        let mut graph = Graph::new(GraphId(3), "发表图");
        graph.arrangement.rows = 2;
        graph.arrangement.cols = 1;
        graph.arrangement.margin_left = 0.04;
        graph.layers[0].title = "上方".into();
        graph.layers[0].title_size = 22.0;
        graph.layers[0].frame = FrameRect { x: 0.04, y: 0.02, w: 0.9, h: 0.45 };
        graph.layers[0].x_axis.label = "时间".into();
        graph.layers[0].x_axis.label_size = 15.0;
        graph.layers[0].x_axis.scale = AxisScale::Log10;
        graph.layers[0].x_axis.step = Some(2.0);
        graph.layers[0].x_axis.minor_count = Some(5);
        graph.layers[0].x_axis.tick_pattern = "0.00".into();
        graph.layers[0].curves.push(Curve {
            name: "不该留下".into(),
            kind: CurveKind::Line,
            ..Curve::default()
        });
        graph.layers[0].annotations.push(Annotation {
            id: 7,
            kind: AnnotationKind::Text {
                x: 1.0,
                y: 2.0,
                text: "峰值".into(),
                size: 13.0,
                color: [10, 20, 30, 255],
            },
            ..Default::default()
        });
        graph.layers[0].annotations.push(Annotation {
            id: 8,
            kind: AnnotationKind::Marker {
                x: 0.0,
                y: 0.0,
                label: "交点".into(),
                color: [1, 2, 3, 255],
            },
            ..Default::default()
        });
        let template = GraphTemplate::from_graph(&graph, 960.0, 640.0);
        let json = template.to_json().unwrap();
        assert!(json.contains("sciplot-template"));
        assert!(json.contains("not a SciDAVis .qpt"));
        assert!(!json.contains("不该留下"));
        assert!(!json.contains("tick_pattern"));
        assert!(!json.contains("minor_count"));
        assert!(!json.contains("交点"));
        assert!(json.contains("峰值"));
        assert!(json.contains("上方"));
        let back = GraphTemplate::from_json(&json).unwrap();
        let restored = back.into_graph(GraphId(9));
        assert!(restored.layers[0].curves.is_empty());
        assert_eq!(restored.layers[0].title, "上方");
        assert!((restored.layers[0].title_size - 22.0).abs() < 1e-6);
        assert_eq!(restored.layers[0].x_axis.label, "时间");
        assert!((restored.layers[0].x_axis.label_size - 15.0).abs() < 1e-6);
        assert_eq!(restored.layers[0].x_axis.scale, AxisScale::Log10);
        assert!(restored.layers[0].x_axis.step.is_none());
        assert!(restored.layers[0].x_axis.minor_count.is_none());
        assert!(restored.layers[0].x_axis.tick_pattern.is_empty());
        assert!((restored.layers[0].frame.x - 0.04).abs() < 1e-6);
        assert_eq!(restored.arrangement.rows, 2);
        match &restored.layers[0].annotations[0].kind {
            AnnotationKind::Text { text, color, size, .. } => {
                assert_eq!(text, "峰值");
                assert_eq!(*color, [10, 20, 30, 255]);
                assert!((size - 13.0).abs() < 1e-6);
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!((back.canvas_width - 960.0).abs() < 1e-6);
        assert!(GraphTemplate::from_json(r#"{"format":"qpt","version":1,"note":"","template":{}}"#).is_err());
    }
}
