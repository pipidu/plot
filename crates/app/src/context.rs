//! 右键菜单。对象上的操作放在这里，菜单栏只保留项目级命令。

use sciplot_core::{unique_name, ColumnMode, CurveKind, GraphId, MatrixId, NoteId, SurfaceMode, TableId};

use crate::app::{
    clicked_menu, fit_dialog, range_dialog, Dialog, FitWhich, LineChoice, PathAction, PlotHit, RangeOp, SciPlotApp, Tab,
};
use crate::ops;

pub(crate) fn section_tables(app: &mut SciPlotApp, ui: &mut egui::Ui) {
    if clicked_menu(ui, "新建表格") {
        if let Some(id) = app.edit("新建表格", |project| Ok(project.new_table("表格"))) {
            app.open_tab(Tab::Table(id));
        }
    }
    if clicked_menu(ui, "导入 CSV…") {
        crate::app::browse_or_dialog(app, PathAction::ImportCsv, "数据.csv");
    }
}

pub(crate) fn section_matrices(app: &mut SciPlotApp, ui: &mut egui::Ui) {
    if clicked_menu(ui, "新建矩阵…") {
        app.dialog = Some(Dialog::NewMatrix { name: "矩阵".into(), rows: "10".into(), cols: "10".into() });
    }
}

pub(crate) fn section_graphs(app: &mut SciPlotApp, ui: &mut egui::Ui) {
    if clicked_menu(ui, "函数曲线…") {
        app.dialog = Some(function_dialog(None, 0, false));
    }
    if clicked_menu(ui, "函数曲面…") {
        app.dialog = Some(function_dialog(None, 0, true));
    }
}

pub(crate) fn section_notes(app: &mut SciPlotApp, ui: &mut egui::Ui) {
    if clicked_menu(ui, "新建备注") {
        if let Some(id) = app.edit("新建备注", |project| Ok(project.new_note("备注"))) {
            app.open_tab(Tab::Note(id));
        }
    }
}

pub(crate) fn tree_table(app: &mut SciPlotApp, ui: &mut egui::Ui, id: TableId) {
    if clicked_menu(ui, "打开") {
        app.open_tab(Tab::Table(id));
    }
    plot_table_menu(app, ui, id);
    if clicked_menu(ui, "导出 CSV…") {
        app.open_tab(Tab::Table(id));
        app.dialog = Some(Dialog::Path { action: PathAction::ExportCsv, path: "数据.csv".into(), extra: String::new() });
    }
    if clicked_menu(ui, "转为矩阵") {
        if let Some(matrix) = app.edit("表转矩阵", |project| project.matrix_from_table(id)) {
            app.open_tab(Tab::Matrix(matrix));
        }
    }
    if clicked_menu(ui, "相关性") {
        correlation(app, id);
    }
    ui.separator();
    if clicked_menu(ui, "删除表格") {
        app.edit("删除表格", |project| {
            if project.delete_table(id) {
                Ok(())
            } else {
                Err("找不到表格".into())
            }
        });
    }
}

pub(crate) fn tree_matrix(app: &mut SciPlotApp, ui: &mut egui::Ui, id: MatrixId) {
    if clicked_menu(ui, "打开") {
        app.open_tab(Tab::Matrix(id));
    }
    matrix_actions(app, ui, id);
    ui.separator();
    if clicked_menu(ui, "删除矩阵") {
        app.edit("删除矩阵", |project| {
            if project.delete_matrix(id) {
                Ok(())
            } else {
                Err("找不到矩阵".into())
            }
        });
    }
}

pub(crate) fn tree_graph(app: &mut SciPlotApp, ui: &mut egui::Ui, id: GraphId) {
    if clicked_menu(ui, "打开") {
        app.open_tab(Tab::Graph(id));
    }
    export_items(app, ui, id);
    ui.separator();
    if clicked_menu(ui, "删除图形") {
        app.edit("删除图形", |project| {
            if project.delete_graph(id) {
                Ok(())
            } else {
                Err("找不到图形".into())
            }
        });
    }
}

pub(crate) fn tree_note(app: &mut SciPlotApp, ui: &mut egui::Ui, id: NoteId) {
    if clicked_menu(ui, "打开") {
        app.open_tab(Tab::Note(id));
    }
    if clicked_menu(ui, "删除备注") {
        app.edit("删除备注", |project| {
            if project.delete_note(id) {
                Ok(())
            } else {
                Err("找不到备注".into())
            }
        });
    }
}

pub(crate) fn table_column(app: &mut SciPlotApp, ui: &mut egui::Ui, table: TableId, column: usize, row: Option<usize>) {
    app.selected_column = Some((table, column));
    let name = app.project.table(table).and_then(|item| item.columns.get(column)).map(|col| col.name.clone()).unwrap_or_else(|| "列".into());
    ui.label(name);
    ui.separator();
    if clicked_menu(ui, "在左侧插入列") {
        insert_column(app, table, column);
    }
    if clicked_menu(ui, "在右侧插入列") {
        insert_column(app, table, column + 1);
    }
    if clicked_menu(ui, "删除此列") {
        app.edit("删除列", |project| {
            if project.table_mut(table).ok_or("找不到表格")?.remove_column(column) {
                Ok(())
            } else {
                Err("没有这一列".into())
            }
        });
        if app.selected_column == Some((table, column)) {
            app.selected_column = None;
        }
    }
    if let Some(row) = row {
        ui.separator();
        row_items(app, ui, table, row);
    }
    ui.separator();
    if clicked_menu(ui, "列公式…") {
        let expr = app.project.table(table).and_then(|item| item.columns.get(column)).map(|col| col.formula.clone()).unwrap_or_default();
        app.dialog = Some(Dialog::Formula { table, column, expr });
    }
    if clicked_menu(ui, "重算公式") {
        recompute(app, table);
    }
    if clicked_menu(ui, "排序…") {
        app.dialog = Some(Dialog::Sort { table, column, ascending: true });
    }
    if clicked_menu(ui, "统计") {
        match ops::column_stats(&app.project, table, column) {
            Ok(text) => app.dialog = Some(Dialog::Message(text)),
            Err(err) => app.status = err,
        }
    }
    if clicked_menu(ui, "列类型与角色…") {
        app.dialog = Some(Dialog::ColumnProps { table, column });
    }
    plot_table_menu(app, ui, table);
    if clicked_menu(ui, "导出 CSV…") {
        app.open_tab(Tab::Table(table));
        app.dialog = Some(Dialog::Path { action: PathAction::ExportCsv, path: "数据.csv".into(), extra: String::new() });
    }
    if clicked_menu(ui, "导入 CSV…") {
        crate::app::browse_or_dialog(app, PathAction::ImportCsv, "数据.csv");
    }
}

pub(crate) fn table_row(app: &mut SciPlotApp, ui: &mut egui::Ui, table: TableId, row: usize) {
    row_items(app, ui, table, row);
}

pub(crate) fn matrix_menu(app: &mut SciPlotApp, ui: &mut egui::Ui, id: MatrixId) {
    let name = app.project.matrix(id).map(|matrix| matrix.name.clone()).unwrap_or_else(|| "矩阵".into());
    ui.label(name);
    ui.separator();
    matrix_actions(app, ui, id);
}

pub(crate) fn plot_menu(app: &mut SciPlotApp, ui: &mut egui::Ui) {
    let Some(hit) = app.plot_hit.clone() else {
        ui.label("在图上右键");
        return;
    };
    match hit {
        PlotHit::Curve { graph, layer, curve } => curve_menu(app, ui, graph, layer, curve),
        PlotHit::Annotation { graph, layer, id } => annotation_menu(app, ui, graph, layer, id),
        PlotHit::Layer { graph, layer } => layer_menu(app, ui, graph, layer),
        PlotHit::Surface { graph } => surface_menu(app, ui, graph),
    }
}

fn row_items(app: &mut SciPlotApp, ui: &mut egui::Ui, table: TableId, row: usize) {
    if clicked_menu(ui, "在上方插入行") {
        app.edit("插入行", |project| {
            project.table_mut(table).ok_or("找不到表格")?.insert_row(row);
            Ok(())
        });
    }
    if clicked_menu(ui, "在下方插入行") {
        app.edit("插入行", |project| {
            project.table_mut(table).ok_or("找不到表格")?.insert_row(row + 1);
            Ok(())
        });
    }
    if clicked_menu(ui, "删除此行") {
        let n = app.project.table(table).map(|item| item.nrows()).unwrap_or(0);
        if row >= n {
            app.status = "这一行还没有数据".into();
        } else {
            app.edit("删除行", |project| {
                project.table_mut(table).ok_or("找不到表格")?.remove_row(row);
                Ok(())
            });
        }
    }
}

fn insert_column(app: &mut SciPlotApp, table: TableId, index: usize) {
    let names = app.project.table(table).map(|item| item.columns.iter().map(|col| col.name.clone()).collect::<Vec<_>>()).unwrap_or_default();
    let name = unique_name(&names, "C");
    app.edit("插入列", |project| {
        project.table_mut(table).ok_or("找不到表格")?.insert_column(index, name, ColumnMode::Y);
        Ok(())
    });
}

fn recompute(app: &mut SciPlotApp, id: TableId) {
    let formulas: Vec<(usize, String)> = app
        .project
        .table(id)
        .map(|table| table.columns.iter().enumerate().filter(|(_, col)| !col.formula.is_empty()).map(|(index, col)| (index, col.formula.clone())).collect())
        .unwrap_or_default();
    if app
        .edit("重算公式", |project| {
            let table = project.table_mut(id).ok_or("找不到表格")?;
            for (index, formula) in &formulas {
                sciplot_core::apply_formula(table, *index, formula)?;
            }
            Ok(())
        })
        .is_some()
    {
        app.status = "已重算列公式".into();
    }
}

fn correlation(app: &mut SciPlotApp, table: TableId) {
    match ops::correlation_report(&app.project, table) {
        Ok(text) => {
            app.project.log_line(&text);
            app.dialog = Some(Dialog::Message(text));
        }
        Err(err) => app.status = err,
    }
}

fn plot_table_menu(app: &mut SciPlotApp, ui: &mut egui::Ui, table: TableId) {
    ui.menu_button("绘制为", |ui| {
        for kind in [
            CurveKind::Line,
            CurveKind::Scatter,
            CurveKind::LineSymbol,
            CurveKind::Steps,
            CurveKind::Bars,
            CurveKind::HorizontalBars,
            CurveKind::Histogram,
            CurveKind::Area,
            CurveKind::ErrorBars,
            CurveKind::BoxPlot,
            CurveKind::Pie,
            CurveKind::Vector,
        ] {
            if clicked_menu(ui, kind.label()) {
                app.plot_table_as(table, kind);
            }
        }
    });
}

fn matrix_actions(app: &mut SciPlotApp, ui: &mut egui::Ui, id: MatrixId) {
    if clicked_menu(ui, "公式填充…") {
        let expr = app.project.matrix(id).map(|matrix| matrix.formula.clone()).unwrap_or_default();
        app.dialog = Some(Dialog::MatrixFormula { matrix: id, expr });
    }
    if clicked_menu(ui, "转置") {
        if let Some(created) = app.edit("转置", |project| ops::transpose_into_new(project, id)) {
            app.open_tab(Tab::Matrix(created));
        }
    }
    if clicked_menu(ui, "求逆") {
        if let Some(created) = app.edit("求逆", |project| ops::invert_into_new(project, id)) {
            app.open_tab(Tab::Matrix(created));
        }
    }
    if clicked_menu(ui, "转为表格") {
        if let Some(table) = app.edit("矩阵转表", |project| project.table_from_matrix(id)) {
            app.open_tab(Tab::Table(table));
        }
    }
    ui.menu_button("绘制为", |ui| {
        if clicked_menu(ui, "热力图") {
            app.plot_matrix_as(id, CurveKind::Heatmap);
        }
        if clicked_menu(ui, "等高线") {
            app.plot_matrix_as(id, CurveKind::Contour);
        }
        if clicked_menu(ui, "三维曲面") {
            app.plot_surface_as(id, SurfaceMode::Surface);
        }
        if clicked_menu(ui, "三维线框") {
            app.plot_surface_as(id, SurfaceMode::Wireframe);
        }
        if clicked_menu(ui, "三维散点") {
            app.plot_surface_as(id, SurfaceMode::Scatter);
        }
    });
}

fn curve_menu(app: &mut SciPlotApp, ui: &mut egui::Ui, graph: GraphId, layer: usize, curve: usize) {
    let (name, visible) = app
        .project
        .graph(graph)
        .and_then(|item| item.layers.get(layer))
        .and_then(|item| item.curves.get(curve))
        .map(|item| (item.name.clone(), item.visible))
        .unwrap_or_else(|| ("曲线".into(), true));
    ui.label(&name);
    ui.separator();
    if clicked_menu(ui, "属性…") {
        open_style(app, graph, layer, curve);
    }
    if clicked_menu(ui, if visible { "隐藏" } else { "显示" }) {
        app.edit("曲线可见性", |project| {
            let curve = project.graph_mut(graph).and_then(|item| item.layers.get_mut(layer)).and_then(|item| item.curves.get_mut(curve)).ok_or("找不到曲线")?;
            curve.visible = !curve.visible;
            Ok(())
        });
    }
    if clicked_menu(ui, "删除曲线") {
        app.edit("删除曲线", |project| {
            let layer = project.graph_mut(graph).and_then(|item| item.layers.get_mut(layer)).ok_or("找不到图层")?;
            if curve >= layer.curves.len() {
                return Err("找不到曲线".into());
            }
            layer.curves.remove(curve);
            Ok(())
        });
    }
    ui.separator();
    analysis_menu(app, ui, graph, layer, curve);
    ui.separator();
    layer_format(app, ui, graph, layer);
}

fn annotation_menu(app: &mut SciPlotApp, ui: &mut egui::Ui, graph: GraphId, layer: usize, id: u64) {
    ui.label("标注");
    if clicked_menu(ui, "删除") {
        app.selected_ann = Some((graph, layer, id));
        app.edit("删除标注", |project| {
            let layer = project.graph_mut(graph).and_then(|item| item.layers.get_mut(layer)).ok_or("找不到图层")?;
            layer.annotations.retain(|annotation| annotation.id != id);
            Ok(())
        });
        app.selected_ann = None;
    }
    layer_format(app, ui, graph, layer);
}

fn layer_menu(app: &mut SciPlotApp, ui: &mut egui::Ui, graph: GraphId, layer: usize) {
    ui.label("图层");
    ui.separator();
    layer_format(app, ui, graph, layer);
    ui.separator();
    if clicked_menu(ui, "添加曲线…") {
        let table = app.project.tables.first().map(|table| table.id).unwrap_or(TableId(0));
        app.dialog = Some(Dialog::AddCurve { graph, table, kind: CurveKind::Line });
    }
    if clicked_menu(ui, "函数曲线…") {
        app.dialog = Some(function_dialog(Some(graph), layer, false));
    }
    if clicked_menu(ui, "函数曲面…") {
        app.dialog = Some(function_dialog(None, 0, true));
    }
    if clicked_menu(ui, "新建图层") {
        app.edit("新建图层", |project| {
            let graph = project.graph_mut(graph).ok_or("找不到图形")?;
            graph.layers.push(sciplot_core::Layer::default());
            graph.reflow_layers();
            Ok(())
        });
    }
    if clicked_menu(ui, "删除图层") {
        app.edit("删除图层", |project| {
            let graph = project.graph_mut(graph).ok_or("找不到图形")?;
            if graph.layers.len() <= 1 {
                return Err("至少保留一个图层".into());
            }
            let index = layer.min(graph.layers.len() - 1);
            graph.layers.remove(index);
            graph.reflow_layers();
            Ok(())
        });
    }
    let curve = app.project.graph(graph).and_then(|item| item.layers.get(layer)).map(|item| item.curves.len().saturating_sub(1)).unwrap_or(0);
    if app.project.graph(graph).and_then(|item| item.layers.get(layer)).is_some_and(|item| !item.curves.is_empty()) {
        ui.separator();
        analysis_menu(app, ui, graph, layer, curve.min(app.curve));
    }
    ui.separator();
    view_items(app, ui, graph);
    export_items(app, ui, graph);
}

fn surface_menu(app: &mut SciPlotApp, ui: &mut egui::Ui, graph: GraphId) {
    ui.label("三维图");
    ui.separator();
    for mode in [SurfaceMode::Surface, SurfaceMode::Wireframe, SurfaceMode::Scatter] {
        if clicked_menu(ui, mode.label()) {
            app.edit("三维显示", |project| {
                project.graph_mut(graph).and_then(|item| item.surface.as_mut()).ok_or("不是三维图")?.mode = mode;
                Ok(())
            });
        }
    }
    ui.separator();
    export_items(app, ui, graph);
}

fn layer_format(app: &mut SciPlotApp, ui: &mut egui::Ui, graph: GraphId, layer: usize) {
    if clicked_menu(ui, "横轴…") {
        open_axis(app, graph, layer, false);
    }
    if clicked_menu(ui, "纵轴…") {
        open_axis(app, graph, layer, true);
    }
    if clicked_menu(ui, "图例…") {
        if let Some(legend) = app.project.graph(graph).and_then(|item| item.layers.get(layer)).map(|item| item.legend.clone()) {
            app.dialog = Some(Dialog::Legend { graph, layer, legend });
        }
    }
    if clicked_menu(ui, "标题…") {
        if let Some(item) = app.project.graph(graph).and_then(|item| item.layers.get(layer)) {
            app.dialog = Some(Dialog::Title { graph, layer, title: item.title.clone(), size: format!("{}", item.title_size) });
        }
    }
    if clicked_menu(ui, "图层位置…") {
        if let Some(frame) = app.project.graph(graph).and_then(|item| item.layers.get(layer)).map(|item| item.frame) {
            app.dialog = Some(Dialog::Frame {
                graph,
                layer,
                x: frame.x.to_string(),
                y: frame.y.to_string(),
                w: frame.w.to_string(),
                h: frame.h.to_string(),
            });
        }
    }
    if clicked_menu(ui, "文本标注…") {
        app.dialog = Some(Dialog::TextAnn { graph, layer, x: "0".into(), y: "0".into(), text: "标注".into() });
    }
    if clicked_menu(ui, "箭头…") {
        app.dialog = Some(Dialog::ArrowAnn { graph, layer, x1: "0".into(), y1: "0".into(), x2: "1".into(), y2: "1".into(), line_only: false });
    }
    if clicked_menu(ui, "线段…") {
        app.dialog = Some(Dialog::ArrowAnn { graph, layer, x1: "0".into(), y1: "0".into(), x2: "1".into(), y2: "1".into(), line_only: true });
    }
}

fn analysis_menu(app: &mut SciPlotApp, ui: &mut egui::Ui, graph: GraphId, layer: usize, curve: usize) {
    ui.menu_button("分析", |ui| {
        if clicked_menu(ui, "线性拟合…") {
            app.dialog = Some(fit_dialog(graph, layer, curve, FitWhich::Linear));
        }
        if clicked_menu(ui, "多项式拟合…") {
            app.dialog = Some(fit_dialog(graph, layer, curve, FitWhich::Poly));
        }
        if clicked_menu(ui, "指数拟合…") {
            app.dialog = Some(fit_dialog(graph, layer, curve, FitWhich::Exp));
        }
        if clicked_menu(ui, "高斯拟合…") {
            app.dialog = Some(fit_dialog(graph, layer, curve, FitWhich::Gauss));
        }
        if clicked_menu(ui, "洛伦兹拟合…") {
            app.dialog = Some(fit_dialog(graph, layer, curve, FitWhich::Lorentz));
        }
        if clicked_menu(ui, "自定义拟合…") {
            app.dialog = Some(fit_dialog(graph, layer, curve, FitWhich::Custom));
        }
        ui.separator();
        if clicked_menu(ui, "微分") {
            let prefs = app.prefs.clone();
            if let Some((table, gid)) = app.edit("微分", |project| {
                let table = ops::derivative_table(project, graph, layer, curve)?;
                let gid = project.plot_table(table, CurveKind::Line)?;
                prefs.apply_new_graph(project, gid);
                Ok((table, gid))
            }) {
                app.open_tab(Tab::Table(table));
                app.open_tab(Tab::Graph(gid));
            }
        }
        if clicked_menu(ui, "积分…") {
            app.dialog = Some(range_dialog(graph, layer, curve, RangeOp::Integral));
        }
        if clicked_menu(ui, "插值…") {
            app.dialog = Some(range_dialog(graph, layer, curve, RangeOp::Interp));
        }
        if clicked_menu(ui, "平滑…") {
            app.dialog = Some(range_dialog(graph, layer, curve, RangeOp::Smooth));
        }
        if clicked_menu(ui, "FFT") {
            let prefs = app.prefs.clone();
            if let Some((table, gid)) = app.edit("FFT", |project| {
                let table = ops::fft_table(project, graph, layer, curve)?;
                let gid = project.plot_table(table, CurveKind::Line)?;
                prefs.apply_new_graph(project, gid);
                Ok((table, gid))
            }) {
                app.open_tab(Tab::Table(table));
                app.open_tab(Tab::Graph(gid));
            }
        }
        if clicked_menu(ui, "FFT 滤波…") {
            app.dialog = Some(range_dialog(graph, layer, curve, RangeOp::Filter));
        }
        ui.separator();
        if clicked_menu(ui, "求交点…") {
            open_intersect(app, graph, layer, curve);
        }
    });
}

fn view_items(app: &mut SciPlotApp, ui: &mut egui::Ui, graph: GraphId) {
    if clicked_menu(ui, "数据读取器") {
        app.reader = !app.reader;
        app.status = if app.reader { "数据读取器已打开".into() } else { "数据读取器已关闭".into() };
    }
    if clicked_menu(ui, "清除缩放") {
        app.views.remove(&graph);
        app.status = "已恢复自动范围".into();
    }
}

fn export_items(app: &mut SciPlotApp, ui: &mut egui::Ui, graph: GraphId) {
    if clicked_menu(ui, "导出 PNG…") {
        app.open_tab(Tab::Graph(graph));
        app.dialog = Some(Dialog::Path { action: PathAction::ExportPng, path: "图形.png".into(), extra: "144".into() });
    }
    if clicked_menu(ui, "导出 SVG…") {
        app.open_tab(Tab::Graph(graph));
        app.dialog = Some(Dialog::Path { action: PathAction::ExportSvg, path: "图形.svg".into(), extra: String::new() });
    }
    if clicked_menu(ui, "复制 PNG") {
        app.open_tab(Tab::Graph(graph));
        app.copy_png();
    }
    if clicked_menu(ui, "复制 SVG") {
        app.open_tab(Tab::Graph(graph));
        app.copy_svg();
    }
}

fn open_style(app: &mut SciPlotApp, graph: GraphId, layer: usize, curve: usize) {
    if let Some(item) = app.project.graph(graph).and_then(|item| item.layers.get(layer)).and_then(|item| item.curves.get(curve)) {
        app.curve = curve;
        app.dialog = Some(Dialog::Style { graph, layer, curve, style: item.style.clone(), name: item.name.clone() });
    } else {
        app.status = "这一层没有曲线".into();
    }
}

fn open_axis(app: &mut SciPlotApp, graph: GraphId, layer: usize, y_axis: bool) {
    let axis = app.project.graph(graph).and_then(|item| item.layers.get(layer)).map(|item| if y_axis { item.y_axis.clone() } else { item.x_axis.clone() });
    if let Some(axis) = axis {
        app.dialog = Some(Dialog::Axis {
            graph,
            layer,
            y_axis,
            min: crate::dialogs::number_text(axis.min),
            max: crate::dialogs::number_text(axis.max),
            axis,
        });
    }
}

fn open_intersect(app: &mut SciPlotApp, graph: GraphId, layer: usize, curve: usize) {
    let count = app.project.graph(graph).and_then(|item| item.layers.get(layer)).map(|item| item.curves.len()).unwrap_or(0);
    let curve_b = if count > 1 && curve == 0 { 1 } else { 0 };
    let (xmin, xmax) = app
        .xsel
        .as_ref()
        .filter(|sel| sel.graph == graph)
        .map(|sel| (crate::dialogs::number_text(Some(sel.x0.min(sel.x1))), crate::dialogs::number_text(Some(sel.x0.max(sel.x1)))))
        .unwrap_or_default();
    app.curve = curve;
    app.dialog = Some(Dialog::Intersect {
        graph,
        layer,
        curve_a: curve,
        curve_b,
        kind_a: LineChoice::Curve,
        kind_b: LineChoice::Curve,
        value_a: "0".into(),
        value_b: "0".into(),
        smooth: false,
        xmin,
        xmax,
        tol: "1e-8".into(),
    });
}

fn function_dialog(graph: Option<GraphId>, layer: usize, surface: bool) -> Dialog {
    if surface {
        Dialog::Function {
            graph: None,
            layer: 0,
            expr: "sin(x)*cos(y)".into(),
            xmin: "-3.14".into(),
            xmax: "3.14".into(),
            ymin: "-3.14".into(),
            ymax: "3.14".into(),
            samples: "40".into(),
            surface: true,
            mode: SurfaceMode::Surface,
        }
    } else {
        Dialog::Function {
            graph,
            layer,
            expr: "sin(x)".into(),
            xmin: "0".into(),
            xmax: "6.2832".into(),
            ymin: "-3.14".into(),
            ymax: "3.14".into(),
            samples: "200".into(),
            surface: false,
            mode: SurfaceMode::Surface,
        }
    }
}
