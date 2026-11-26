use std::f64::consts::PI;

use crate::graph::{CurveKind, SurfaceMode, SurfaceSource};
use crate::util::format_number;
use crate::table::ColumnMode;
use crate::Project;

/// 启动时的示例：三角函数、中文分类柱状图，以及一张三维曲面。
pub fn demo_project() -> Project {
    let mut project = Project::new("示例项目");
    let trig = project.new_table("三角函数");
    {
        let table = project.table_mut(trig).unwrap();
        table.columns.clear();
        let n = 200;
        let mut x = Vec::with_capacity(n);
        let mut ys = Vec::with_capacity(n);
        let mut yc = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f64 / (n as f64 - 1.0) * 2.0 * PI;
            x.push(t);
            ys.push(t.sin());
            yc.push(t.cos());
        }
        table.add_numeric_column("X", ColumnMode::X, &x);
        table.add_numeric_column("sin", ColumnMode::Y, &ys);
        table.add_numeric_column("cos", ColumnMode::Y, &yc);
        table.comment = "0 到 2π 的正弦和余弦，可用来求交点。".to_string();
    }
    let graph = project.plot_table(trig, CurveKind::LineSymbol).unwrap();
    if let Some(graph) = project.graph_mut(graph) {
        graph.name = "正弦与余弦".to_string();
        let layer = &mut graph.layers[0];
        layer.title = "正弦与余弦".to_string();
        layer.x_axis.label = "x（弧度）".to_string();
        layer.y_axis.label = "y".to_string();
    }

    let sales = project.new_table("水果销量");
    {
        let table = project.table_mut(sales).unwrap();
        table.columns.clear();
        table.add_column("水果", ColumnMode::X);
        table.add_column("销量", ColumnMode::Y);
        let names = ["苹果", "香蕉", "橙子", "葡萄", "梨"];
        let values = [12.0, 9.5, 7.0, 11.0, 6.5];
        for (name, value) in names.iter().zip(values) {
            let row = table.nrows();
            table.ensure_rows(row + 1);
            table.columns[0].set_text(row, name);
            table.columns[1].set_text(row, &format_number(value));
        }
    }
    let bars = project.plot_table(sales, CurveKind::Bars).unwrap();
    if let Some(graph) = project.graph_mut(bars) {
        graph.name = "水果销量".to_string();
        let layer = &mut graph.layers[0];
        layer.title = "水果销量".to_string();
        layer.x_axis.label = "水果".to_string();
        layer.y_axis.label = "销量（吨）".to_string();
    }

    let matrix_id = project.new_matrix("波面", 36, 36);
    if let Some(matrix) = project.matrix_mut(matrix_id) {
        matrix.x_min = -PI;
        matrix.x_max = PI;
        matrix.y_min = -PI;
        matrix.y_max = PI;
        matrix.fill_formula("sin(x)*cos(y)").ok();
    }
    project.plot_surface(
        SurfaceSource::Matrix(matrix_id),
        SurfaceMode::Surface,
        "波面",
    );
    project.plot_matrix(matrix_id, CurveKind::Heatmap).ok();

    let note = project.new_note("使用说明");
    if let Some(note) = project.note_mut(note) {
        note.text = "\
SciPlot 是用 Rust 编写的科学绘图程序，界面对齐 SciDAVis。\n\
\n\
· 表格支持列角色（X/Y/误差/标签）、列公式 col(\"列名\")、CSV 导入（UTF-8 与 GBK）。\n\
· 在表格、矩阵或项目树上右键，可以增删行列、绘图和导出。\n\
· 在曲线上右键可以改属性、删除，或做拟合、微积分、FFT 和求交点。\n\
· 求交点可选分段折线或平滑求根，可限制 X 区间，也可与水平线、竖直线求交。\n\
· 在图的空白处右键可改坐标轴、图例和标题。滚轮缩放，左键拖拽平移，双击恢复。\n\
· 「设置」里可以改字体，以及新建曲线的线宽、线型和字号。项目保存为 .splot。\n"
            .to_string();
    }
    project.log_line("已载入示例项目。在「正弦与余弦」上右键曲线，用「分析 → 求交点」计算 sin 与 cos 的交点。");
    project
}
