//! SciPlot 绘图引擎。先生成与后端无关的图元，再交给 egui、SVG 或 PNG。

mod color;
mod contour;
mod interact;
mod paint;
mod raster;
mod render;
mod scale;
mod scene;
mod surface;
mod svg;

pub use interact::{data_to_screen, nearest_curve, nearest_point, pan_limits, screen_to_data, zoom_limits, CurveHit, NearestPoint};
pub use paint::{paint, PLOT_FONT_FAMILY};
pub use raster::{prepare_font, to_png, to_rgba, GlyphFont};
pub use render::render;
pub use scene::{LayerFrame, LayerView, LegendHit, Prim, Scene};
pub use svg::{to_svg, SVG_FONT};

#[cfg(test)]
mod tests {
    use super::*;
    use sciplot_core::{demo_project, AxisScale, CurveKind, Graph, Layer, Project, SurfaceMode, SurfaceSource};

    #[test]
    fn demo_svg_contains_chinese_axis_title() {
        let project = demo_project();
        let graph = project.graphs.iter().find(|graph| graph.name == sciplot_core::t("正弦与余弦")).unwrap();
        let scene = render(&project, graph, &[], 900.0, 560.0, 1.0);
        let svg = to_svg(&scene);
        assert!(svg.contains("正弦与余弦"), "{svg}");
        assert!(svg.contains("x（弧度）"));
        assert!(svg.contains(SVG_FONT));
        assert!(svg.contains("<polyline"));
        assert!(!scene.layers.is_empty());
    }

    #[test]
    fn log_axis_without_positive_data_stays_visible() {
        let mut project = Project::new("t");
        let table = project.new_table("t");
        {
            let table = project.table_mut(table).unwrap();
            table.columns[0].set_numbers(&[-3.0, -2.0, -1.0]);
            table.columns[1].set_numbers(&[-30.0, -20.0, -10.0]);
        }
        let graph_id = project.plot_table(table, CurveKind::Line).unwrap();
        project.graph_mut(graph_id).unwrap().layers[0].y_axis.scale = AxisScale::Log10;
        let graph = project.graph(graph_id).unwrap().clone();
        let scene = render(&project, &graph, &[], 800.0, 600.0, 1.0);
        let frame = &scene.layers[0];
        assert!(frame.y_min > 0.0 && frame.y_max > frame.y_min, "{} {}", frame.y_min, frame.y_max);
        assert!(frame.y_max < 1.0e6, "{} {}", frame.y_min, frame.y_max);
        assert!(scene.notes.iter().any(|note| note.contains("正数")));
        assert!(scene.text_content().contains("正数"));
    }

    #[test]
    fn axis_min_alone_keeps_the_data_max() {
        let mut project = Project::new("t");
        let table = project.new_table("t");
        {
            let table = project.table_mut(table).unwrap();
            table.columns[0].set_numbers(&[0.0, 5.0, 10.0]);
            table.columns[1].set_numbers(&[1.0, 2.0, 3.0]);
        }
        let graph_id = project.plot_table(table, CurveKind::Line).unwrap();
        project.graph_mut(graph_id).unwrap().layers[0].x_axis.min = Some(-2.0);
        let graph = project.graph(graph_id).unwrap().clone();
        let scene = render(&project, &graph, &[], 800.0, 600.0, 1.0);
        let frame = &scene.layers[0];
        assert!((frame.x_min - (-2.0)).abs() < 1e-9, "{}", frame.x_min);
        assert!(frame.x_max > 10.0, "{}", frame.x_max);
    }

    #[test]
    fn png_starts_with_signature_and_draws_glyphs() {
        let project = demo_project();
        let graph = project.graphs.iter().find(|graph| graph.name == sciplot_core::t("正弦与余弦")).unwrap();
        let scene = render(&project, graph, &[], 640.0, 400.0, 1.0);
        let bytes = include_bytes!("../../../assets/fonts/DroidSansFallbackFull.ttf");
        let font = GlyphFont::from_bytes(bytes).expect("内嵌字体应能加载");
        let png = to_png(&scene, Some(&font)).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert!(png.len() > 2000);
    }

    #[test]
    fn log_axis_maps_decade_to_the_middle() {
        let mut project = Project::new("log");
        let table = project.new_table("t");
        {
            let table = project.table_mut(table).unwrap();
            table.columns.clear();
            table.add_numeric_column("X", sciplot_core::ColumnMode::X, &[1.0, 10.0, 100.0]);
            table.add_numeric_column("Y", sciplot_core::ColumnMode::Y, &[1.0, 2.0, 3.0]);
        }
        let id = project.plot_table(table, CurveKind::Line).unwrap();
        {
            let graph = project.graph_mut(id).unwrap();
            graph.layers[0].x_axis.scale = AxisScale::Log10;
            graph.layers[0].x_axis.min = Some(1.0);
            graph.layers[0].x_axis.max = Some(100.0);
        }
        let graph = project.graph(id).unwrap();
        let scene = render(&project, graph, &[], 800.0, 500.0, 1.0);
        let frame = &scene.layers[0];
        let (x, _) = screen_to_data(frame, frame.plot.x + frame.plot.w * 0.5, frame.plot.y + frame.plot.h * 0.5).unwrap();
        assert!((x - 10.0).abs() < 0.2, "x={x}");
    }

    #[test]
    fn surface_emits_triangles() {
        let project = demo_project();
        let graph = project.graphs.iter().find(|graph| graph.surface.is_some()).unwrap();
        let scene = render(&project, graph, &[], 700.0, 520.0, 1.0);
        let triangles = scene.primitives.iter().filter(|prim| matches!(prim, Prim::Triangle { .. })).count();
        assert!(triangles > 100, "triangles={triangles}");
        assert!(scene.text_content().contains("波面"));
    }

    #[test]
    fn function_surface_also_renders() {
        let mut project = Project::new("fn");
        project.plot_surface(
            SurfaceSource::Function {
                expr: "sin(x)*cos(y)".into(),
                x_min: -3.0,
                x_max: 3.0,
                y_min: -3.0,
                y_max: 3.0,
                n: 12,
            },
            SurfaceMode::Wireframe,
            sciplot_core::t("函数曲面"),
        );
        let graph = &project.graphs[0];
        let scene = render(&project, graph, &[], 600.0, 400.0, 1.0);
        assert!(scene.primitives.iter().any(|prim| matches!(prim, Prim::Polyline { .. })));
    }

    #[test]
    fn bars_keep_chinese_categories() {
        let project = demo_project();
        let graph = project.graphs.iter().find(|graph| graph.name == sciplot_core::t("水果销量")).unwrap();
        let scene = render(&project, graph, &[], 800.0, 480.0, 1.0);
        let text = scene.text_content();
        assert!(text.contains("苹果"));
        assert!(text.contains("香蕉"));
        assert!(scene.primitives.iter().any(|prim| matches!(prim, Prim::Rect { fill: Some(_), .. })));
    }

    #[test]
    fn hidden_curve_stays_in_the_legend() {
        let mut project = demo_project();
        let graph = project.graphs.iter_mut().find(|graph| graph.name == sciplot_core::t("正弦与余弦")).unwrap();
        graph.layers[0].curves[1].visible = false;
        let graph = project.graphs.iter().find(|graph| graph.name == sciplot_core::t("正弦与余弦")).unwrap();
        let scene = render(&project, graph, &[], 900.0, 560.0, 1.0);
        let text = scene.text_content();
        assert!(text.contains("cos（已隐藏）"), "{text}");
        assert!(text.contains("sin"));
        assert!(scene.legend_hits.iter().any(|hit| hit.curve == 1));
    }

    #[test]
    fn latin_y_axis_title_is_one_rotated_line() {
        let project = Project::new("p");
        let mut graph = Graph::new(sciplot_core::GraphId(1), "g");
        graph.layers = vec![Layer::default()];
        graph.layers[0].y_axis.label = "Sales (tonnes)".into();
        let scene = render(&project, &graph, &[], 800.0, 480.0, 1.0);
        let svg = to_svg(&scene);
        assert!(svg.contains("rotate(-90"), "{svg}");
        assert!(svg.contains("Sales (tonnes)"), "{svg}");
        assert!(!svg.contains(">S</text>"), "{svg}");
    }

    #[test]
    fn empty_layer_still_has_a_frame() {
        let project = Project::new("empty");
        let mut graph = Graph::new(sciplot_core::GraphId(1), sciplot_core::t("空"));
        graph.layers = vec![Layer::default()];
        let scene = render(&project, &graph, &[], 400.0, 300.0, 1.0);
        assert_eq!(scene.layers.len(), 1);
        assert!(scene.layers[0].plot.w > 10.0);
    }
}
