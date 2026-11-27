//! 右键菜单。对象上的操作放在这里，菜单栏只保留项目级命令。

use sciplot_core::{
    unique_name, ColumnMode, CurveKind, GraphId, MatrixId, NoteId, SurfaceMode, TableId,
};

use crate::app::{
    clicked_menu, fit_dialog, range_dialog, Dialog, FitWhich, LineChoice, PathAction, PlotHit,
    RangeOp, SciPlotApp, Tab,
};
use crate::ops;

pub(crate) fn section_tables(app: &mut SciPlotApp, ui: &mut egui::Ui) {
    if clicked_menu(ui, sciplot_core::t("新建表格")) {
        if let Some(id) = app.edit(sciplot_core::t("新建表格"), |project| {
            Ok(project.new_table(sciplot_core::t("表格")))
        }) {
            app.open_tab(Tab::Table(id));
        }
    }
    if clicked_menu(ui, sciplot_core::t("导入 CSV…")) {
        crate::app::open_import(app);
    }
}

pub(crate) fn section_matrices(app: &mut SciPlotApp, ui: &mut egui::Ui) {
    if clicked_menu(ui, sciplot_core::t("新建矩阵…")) {
        app.dialog = Some(Dialog::NewMatrix {
            name: sciplot_core::t("矩阵").into(),
            rows: "10".into(),
            cols: "10".into(),
        });
    }
}

pub(crate) fn section_graphs(app: &mut SciPlotApp, ui: &mut egui::Ui) {
    if clicked_menu(ui, sciplot_core::t("函数曲线…")) {
        app.dialog = Some(function_dialog(None, 0, false));
    }
    if clicked_menu(ui, sciplot_core::t("函数曲面…")) {
        app.dialog = Some(function_dialog(None, 0, true));
    }
}

pub(crate) fn section_notes(app: &mut SciPlotApp, ui: &mut egui::Ui) {
    if clicked_menu(ui, sciplot_core::t("新建备注")) {
        if let Some(id) = app.edit(sciplot_core::t("新建备注"), |project| {
            Ok(project.new_note(sciplot_core::t("备注")))
        }) {
            app.open_tab(Tab::Note(id));
        }
    }
}

pub(crate) fn tree_table(app: &mut SciPlotApp, ui: &mut egui::Ui, id: TableId) {
    if clicked_menu(ui, sciplot_core::t("打开")) {
        app.open_tab(Tab::Table(id));
    }
    plot_table_menu(app, ui, id);
    if clicked_menu(ui, sciplot_core::t("导出 CSV…")) {
        app.open_tab(Tab::Table(id));
        crate::app::start_file(app, PathAction::ExportCsv, "");
    }
    if clicked_menu(ui, sciplot_core::t("转为矩阵")) {
        if let Some(matrix) = app.edit(sciplot_core::t("表转矩阵"), |project| {
            project.matrix_from_table(id)
        }) {
            app.open_tab(Tab::Matrix(matrix));
        }
    }
    if clicked_menu(ui, sciplot_core::t("相关性")) {
        correlation(app, id);
    }
    ui.separator();
    if clicked_menu(ui, sciplot_core::t("删除表格")) {
        crate::app::ask_delete_table(app, id);
    }
}

pub(crate) fn tree_matrix(app: &mut SciPlotApp, ui: &mut egui::Ui, id: MatrixId) {
    if clicked_menu(ui, sciplot_core::t("打开")) {
        app.open_tab(Tab::Matrix(id));
    }
    matrix_actions(app, ui, id);
    ui.separator();
    if clicked_menu(ui, sciplot_core::t("删除矩阵")) {
        crate::app::ask_delete_matrix(app, id);
    }
}

pub(crate) fn tree_graph(app: &mut SciPlotApp, ui: &mut egui::Ui, id: GraphId) {
    if clicked_menu(ui, sciplot_core::t("打开")) {
        app.open_tab(Tab::Graph(id));
    }
    if app.project.graph(id).is_some_and(|graph| {
        graph.surface.is_none() && graph.layers.iter().any(|layer| !layer.curves.is_empty())
    }) {
        let layer = 0;
        curve_visibility_menu(app, ui, id, layer);
    }
    export_items(app, ui, id);
    ui.separator();
    if clicked_menu(ui, sciplot_core::t("删除图形")) {
        app.edit(sciplot_core::t("删除图形"), |project| {
            if project.delete_graph(id) {
                Ok(())
            } else {
                Err(sciplot_core::t("找不到图形").into())
            }
        });
    }
}

pub(crate) fn tree_note(app: &mut SciPlotApp, ui: &mut egui::Ui, id: NoteId) {
    if clicked_menu(ui, sciplot_core::t("打开")) {
        app.open_tab(Tab::Note(id));
    }
    if clicked_menu(ui, sciplot_core::t("删除备注")) {
        app.edit(sciplot_core::t("删除备注"), |project| {
            if project.delete_note(id) {
                Ok(())
            } else {
                Err(sciplot_core::t("找不到备注").into())
            }
        });
    }
}

pub(crate) fn open_rename_column(app: &mut SciPlotApp, table: TableId, column: usize) {
    let name = app
        .project
        .table(table)
        .and_then(|item| item.columns.get(column))
        .map(|col| col.name.clone())
        .unwrap_or_default();
    app.dialog = Some(Dialog::RenameColumn {
        table,
        column,
        name,
        error: String::new(),
    });
}

pub(crate) fn table_column(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    table: TableId,
    column: usize,
    row: Option<usize>,
) {
    app.selected_column = Some((table, column));
    let name = app
        .project
        .table(table)
        .and_then(|item| item.columns.get(column))
        .map(|col| col.name.clone())
        .unwrap_or_else(|| sciplot_core::t("列").into());
    ui.label(name);
    ui.separator();
    if clicked_menu(ui, sciplot_core::t("重命名此列…")) {
        open_rename_column(app, table, column);
    }
    if clicked_menu(ui, sciplot_core::t("在左侧插入列")) {
        insert_column(app, table, column);
    }
    if clicked_menu(ui, sciplot_core::t("在右侧插入列")) {
        insert_column(app, table, column + 1);
    }
    if clicked_menu(ui, sciplot_core::t("删除此列")) {
        app.edit(sciplot_core::t("删除列"), |project| {
            project.remove_table_column(table, column)
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
    if clicked_menu(ui, sciplot_core::t("列公式…")) {
        let expr = app
            .project
            .table(table)
            .and_then(|item| item.columns.get(column))
            .map(|col| col.formula.clone())
            .unwrap_or_default();
        app.dialog = Some(Dialog::Formula {
            table,
            column,
            expr,
            error: String::new(),
            warning: String::new(),
        });
    }
    if clicked_menu(ui, sciplot_core::t("重算公式")) {
        recompute(app, table);
    }
    if let Some(row) = row {
        if clicked_menu(ui, sciplot_core::t("单元格公式…")) {
            let expr = app
                .project
                .table(table)
                .and_then(|item| item.columns.get(column))
                .map(|col| {
                    let cell = col.cell_formula(row);
                    if cell.is_empty() {
                        col.formula.clone()
                    } else {
                        cell.to_string()
                    }
                })
                .unwrap_or_default();
            app.dialog = Some(Dialog::CellFormula {
                table,
                column,
                row,
                expr,
                error: String::new(),
            });
        }
    }
    let formula_label = if app.formula_mode {
        sciplot_core::t("关闭公式编辑")
    } else {
        sciplot_core::t("公式编辑模式")
    };
    if clicked_menu(ui, formula_label) {
        app.formula_mode = !app.formula_mode;
    }
    if clicked_menu(ui, sciplot_core::t("用行号填充"))
        && app
            .edit(sciplot_core::t("用行号填充"), |project| {
                project
                    .table_mut(table)
                    .ok_or(sciplot_core::t("找不到表格"))?
                    .fill_row_numbers(column)
            })
            .is_some()
    {
        app.status = sciplot_core::t("已填充行号").into();
    }
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(1);
    if clicked_menu(ui, sciplot_core::t("用随机数填充"))
        && app
            .edit(sciplot_core::t("用随机数填充"), |project| {
                project
                    .table_mut(table)
                    .ok_or(sciplot_core::t("找不到表格"))?
                    .fill_random(column, seed)
            })
            .is_some()
    {
        app.status = sciplot_core::t("已填充随机数").into();
    }
    ui.separator();
    if clicked_menu(ui, sciplot_core::t("复制单元格")) {
        if app.cell_anchor.is_none() {
            app.selected_column = Some((table, column));
            app.cell_anchor = None;
            app.cell_focus = None;
        }
        app.copy_cells();
    }
    if clicked_menu(ui, sciplot_core::t("剪切单元格")) {
        app.cut_cells();
    }
    if clicked_menu(ui, sciplot_core::t("粘贴单元格")) {
        if app.cell_anchor.is_none() {
            let row = row.unwrap_or(0);
            app.cell_anchor = Some((table, column, row));
            app.cell_focus = Some((table, column, row));
        }
        app.paste_cells();
    }
    ui.separator();
    if clicked_menu(ui, sciplot_core::t("排序…")) {
        let n = app.project.table(table).map(|item| item.columns.len()).unwrap_or(0);
        let mut picked = vec![false; n];
        if column < n {
            picked[column] = true;
        }
        app.dialog = Some(Dialog::Sort {
            table,
            column,
            ascending: true,
            mode: sciplot_core::SortMode::Whole,
            picked,
        });
    }
    if clicked_menu(ui, sciplot_core::t("统计")) {
        match ops::column_stats(&app.project, table, column) {
            Ok(text) => {
                app.dialog = Some(Dialog::Message {
                    title: sciplot_core::t("统计").into(),
                    text,
                })
            }
            Err(err) => app.status = err,
        }
    }
    if clicked_menu(ui, sciplot_core::t("列统计表…")) || clicked_menu(ui, sciplot_core::t("行统计…")) {
        app.dialog = Some(Dialog::Stats {
            table,
            picked: column_flags(app, table, column),
        });
    }
    if clicked_menu(ui, sciplot_core::t("归一化…")) {
        app.dialog = Some(Dialog::Normalize {
            table,
            picked: column_flags(app, table, column),
            whole: false,
        });
    }
    if clicked_menu(ui, sciplot_core::t("列类型与角色…")) {
        let (format, places, time_format) = app
            .project
            .table(table)
            .and_then(|item| item.columns.get(column))
            .map(|column| (column.numeric_format, column.decimal_places, column.time_format.clone()))
            .unwrap_or((sciplot_core::NumericFormat::General, 6, String::new()));
        app.dialog = Some(Dialog::ColumnProps {
            table,
            column,
            format,
            places,
            time_format,
        });
    }
    plot_table_menu(app, ui, table);
    if clicked_menu(ui, sciplot_core::t("导出 CSV…")) {
        app.open_tab(Tab::Table(table));
        crate::app::start_file(app, PathAction::ExportCsv, "");
    }
    if clicked_menu(ui, sciplot_core::t("导入 CSV…")) {
        crate::app::open_import(app);
    }
}

fn column_flags(app: &SciPlotApp, table: TableId, column: usize) -> Vec<bool> {
    let n = app.project.table(table).map(|item| item.columns.len()).unwrap_or(0);
    let mut picked = vec![false; n];
    if column < n {
        picked[column] = true;
    }
    picked
}

pub(crate) fn table_row(app: &mut SciPlotApp, ui: &mut egui::Ui, table: TableId, row: usize) {
    row_items(app, ui, table, row);
}

pub(crate) fn matrix_menu(app: &mut SciPlotApp, ui: &mut egui::Ui, id: MatrixId) {
    let name = app
        .project
        .matrix(id)
        .map(|matrix| matrix.name.clone())
        .unwrap_or_else(|| sciplot_core::t("矩阵").into());
    ui.label(name);
    ui.separator();
    matrix_actions(app, ui, id);
}

pub(crate) fn plot_menu(app: &mut SciPlotApp, ui: &mut egui::Ui) {
    let Some(hit) = app.plot_hit.clone() else {
        ui.label(sciplot_core::t("在图上右键"));
        return;
    };
    match hit {
        PlotHit::Curve {
            graph,
            layer,
            curve,
        } => curve_menu(app, ui, graph, layer, curve),
        PlotHit::Annotation { graph, layer, id } => annotation_menu(app, ui, graph, layer, id),
        PlotHit::Layer { graph, layer } => layer_menu(app, ui, graph, layer),
        PlotHit::Surface { graph } => surface_menu(app, ui, graph),
    }
}

fn row_items(app: &mut SciPlotApp, ui: &mut egui::Ui, table: TableId, row: usize) {
    if clicked_menu(ui, sciplot_core::t("在上方插入行")) {
        app.edit(sciplot_core::t("插入行"), |project| {
            project
                .table_mut(table)
                .ok_or(sciplot_core::t("找不到表格"))?
                .insert_row(row);
            Ok(())
        });
    }
    if clicked_menu(ui, sciplot_core::t("在下方插入行")) {
        app.edit(sciplot_core::t("插入行"), |project| {
            project
                .table_mut(table)
                .ok_or(sciplot_core::t("找不到表格"))?
                .insert_row(row + 1);
            Ok(())
        });
    }
    if clicked_menu(ui, sciplot_core::t("删除此行")) {
        let n = app
            .project
            .table(table)
            .map(|item| item.nrows())
            .unwrap_or(0);
        if row >= n {
            app.status = sciplot_core::t("这一行还没有数据").into();
        } else {
            app.edit(sciplot_core::t("删除行"), |project| {
                project
                    .table_mut(table)
                    .ok_or(sciplot_core::t("找不到表格"))?
                    .remove_row(row);
                Ok(())
            });
        }
    }
}

fn insert_column(app: &mut SciPlotApp, table: TableId, index: usize) {
    let names = app
        .project
        .table(table)
        .map(|item| {
            item.columns
                .iter()
                .map(|col| col.name.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let name = unique_name(&names, "C");
    app.edit(sciplot_core::t("插入列"), |project| {
        project.insert_table_column(table, index, name, ColumnMode::None)
    });
}

fn recompute(app: &mut SciPlotApp, id: TableId) {
    let report = app.edit(sciplot_core::t("重算公式"), |project| {
        let others: Vec<sciplot_core::Table> = project
            .tables
            .iter()
            .filter(|table| table.id != id)
            .cloned()
            .collect();
        let table = project.table_mut(id).ok_or(sciplot_core::t("找不到表格"))?;
        let report = sciplot_core::recalculate_table(table, &others);
        if let Some(err) = report.first_error.clone() {
            project.log_line(&err);
        }
        Ok(report)
    });
    if let Some(report) = report {
        app.status = match report.first_error {
            Some(err) => err,
            None => sciplot_core::trf!(
                "已重算公式，写入 {n} 个单元格。",
                "Recalculated formulas and wrote {n} cells.",
                n = report.written
            ),
        };
    }
}

fn correlation(app: &mut SciPlotApp, table: TableId) {
    match ops::correlation_report(&app.project, table) {
        Ok(text) => {
            app.project.log_line(&text);
            app.dialog = Some(Dialog::Message {
                title: sciplot_core::t("相关性").into(),
                text,
            });
        }
        Err(err) => app.status = err,
    }
}

fn plot_table_menu(app: &mut SciPlotApp, ui: &mut egui::Ui, table: TableId) {
    if clicked_menu(ui, sciplot_core::tr!("绘图向导…", "Plot wizard…")) {
        app.dialog = Some(open_wizard(app, table));
    }
    ui.menu_button(sciplot_core::t("绘制为"), |ui| {
        for kind in CurveKind::table_kinds() {
            if *kind == CurveKind::Vector {
                continue;
            }
            if clicked_menu(ui, kind.label()) {
                app.plot_table_as(table, *kind);
            }
        }
        if clicked_menu(ui, sciplot_core::tr!("向量 dx、dy", "Vector dx, dy")) {
            app.plot_table_as_mode(table, CurveKind::Vector, sciplot_core::VectorMode::DxDy);
        }
        if clicked_menu(ui, sciplot_core::tr!("向量终点 XY", "Vector end XY")) {
            app.plot_table_as_mode(table, CurveKind::Vector, sciplot_core::VectorMode::EndPoint);
        }
        if clicked_menu(ui, sciplot_core::tr!("向量角度和模长", "Vector angle and magnitude")) {
            app.plot_table_as_mode(table, CurveKind::Vector, sciplot_core::VectorMode::AngleMagnitude);
        }
    });
}

fn open_wizard(app: &SciPlotApp, table: TableId) -> Dialog {
    let columns = app.project.table(table).map(|item| item.columns.clone()).unwrap_or_default();
    let x = columns.iter().position(|column| column.mode == ColumnMode::X).unwrap_or(0);
    let mut y = vec![false; columns.len()];
    for (index, column) in columns.iter().enumerate() {
        if index != x && (column.mode == ColumnMode::Y || column.mode == ColumnMode::Z) {
            y[index] = true;
        }
    }
    if !y.iter().any(|on| *on) {
        if let Some(index) = columns.iter().enumerate().position(|(index, _)| index != x) {
            y[index] = true;
        }
    }
    let numeric: Vec<usize> = columns
        .iter()
        .enumerate()
        .filter(|(index, column)| *index != x && column.numbers.iter().any(|value| value.is_some()))
        .map(|(index, _)| index)
        .collect();
    let third = numeric.first().copied().unwrap_or(0);
    let fourth = numeric.get(1).copied().unwrap_or(third);
    Dialog::Wizard {
        table,
        x,
        y,
        kind: CurveKind::Line,
        vector_mode: sciplot_core::VectorMode::DxDy,
        third,
        fourth,
    }
}

fn matrix_actions(app: &mut SciPlotApp, ui: &mut egui::Ui, id: MatrixId) {
    if clicked_menu(ui, sciplot_core::t("公式填充…")) {
        let expr = app
            .project
            .matrix(id)
            .map(|matrix| matrix.formula.clone())
            .unwrap_or_default();
        app.dialog = Some(Dialog::MatrixFormula { matrix: id, expr });
    }
    if clicked_menu(ui, sciplot_core::t("转置")) {
        if let Some(created) = app.edit(sciplot_core::t("转置"), |project| {
            ops::transpose_into_new(project, id)
        }) {
            app.open_tab(Tab::Matrix(created));
        }
    }
    if clicked_menu(ui, sciplot_core::t("求逆")) {
        if let Some(created) = app.edit(sciplot_core::t("求逆"), |project| {
            ops::invert_into_new(project, id)
        }) {
            app.open_tab(Tab::Matrix(created));
        } else {
            let text = app.status.clone();
            app.dialog = Some(Dialog::Message {
                title: sciplot_core::t("求逆失败").into(),
                text,
            });
        }
    }
    if clicked_menu(ui, sciplot_core::t("转为表格")) {
        if let Some(table) = app.edit(sciplot_core::t("矩阵转表"), |project| {
            project.table_from_matrix(id)
        }) {
            app.open_tab(Tab::Table(table));
        }
    }
    ui.menu_button(sciplot_core::t("绘制为"), |ui| {
        if clicked_menu(ui, sciplot_core::t("热力图")) {
            app.plot_matrix_as(id, CurveKind::Heatmap);
        }
        if clicked_menu(ui, sciplot_core::t("等高线")) {
            app.plot_matrix_as(id, CurveKind::Contour);
        }
        if clicked_menu(ui, sciplot_core::t("三维曲面")) {
            app.plot_surface_as(id, SurfaceMode::Surface);
        }
        if clicked_menu(ui, sciplot_core::t("三维线框")) {
            app.plot_surface_as(id, SurfaceMode::Wireframe);
        }
        if clicked_menu(ui, sciplot_core::t("三维散点")) {
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
        .unwrap_or_else(|| (sciplot_core::t("曲线").into(), true));
    ui.label(&name);
    ui.separator();
    if clicked_menu(ui, sciplot_core::t("属性…")) {
        open_style(app, graph, layer, curve);
    }
    if clicked_menu(ui, sciplot_core::tr!("误差棒…", "Error bars…")) {
        open_errors(app, graph, layer, curve);
    }
    if clicked_menu(
        ui,
        if visible {
            sciplot_core::t("隐藏")
        } else {
            sciplot_core::t("显示")
        },
    ) {
        app.edit(sciplot_core::t("曲线可见性"), |project| {
            let curve = project
                .graph_mut(graph)
                .and_then(|item| item.layers.get_mut(layer))
                .and_then(|item| item.curves.get_mut(curve))
                .ok_or(sciplot_core::t("找不到曲线"))?;
            curve.visible = !curve.visible;
            Ok(())
        });
    }
    if clicked_menu(ui, sciplot_core::t("删除曲线")) {
        app.edit(sciplot_core::t("删除曲线"), |project| {
            let layer = project
                .graph_mut(graph)
                .and_then(|item| item.layers.get_mut(layer))
                .ok_or(sciplot_core::t("找不到图层"))?;
            if curve >= layer.curves.len() {
                return Err(sciplot_core::t("找不到曲线").into());
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
    ui.label(sciplot_core::t("标注"));
    if clicked_menu(ui, sciplot_core::t("删除")) {
        app.selected_ann = Some((graph, layer, id));
        app.edit(sciplot_core::t("删除标注"), |project| {
            let layer = project
                .graph_mut(graph)
                .and_then(|item| item.layers.get_mut(layer))
                .ok_or(sciplot_core::t("找不到图层"))?;
            layer.annotations.retain(|annotation| annotation.id != id);
            Ok(())
        });
        app.selected_ann = None;
    }
    layer_format(app, ui, graph, layer);
}

fn layer_menu(app: &mut SciPlotApp, ui: &mut egui::Ui, graph: GraphId, layer: usize) {
    ui.label(sciplot_core::t("图层"));
    ui.separator();
    curve_visibility_menu(app, ui, graph, layer);
    layer_format(app, ui, graph, layer);
    ui.separator();
    if clicked_menu(ui, sciplot_core::t("添加曲线…")) {
        let table = app
            .project
            .tables
            .first()
            .map(|table| table.id)
            .unwrap_or(TableId(0));
        app.dialog = Some(Dialog::AddCurve {
            graph,
            table,
            kind: CurveKind::Line,
        });
    }
    if clicked_menu(ui, sciplot_core::t("函数曲线…")) {
        app.dialog = Some(function_dialog(Some(graph), layer, false));
    }
    if clicked_menu(ui, sciplot_core::t("函数曲面…")) {
        app.dialog = Some(function_dialog(None, 0, true));
    }
    if clicked_menu(ui, sciplot_core::t("新建图层")) {
        app.edit(sciplot_core::t("新建图层"), |project| {
            let graph = project
                .graph_mut(graph)
                .ok_or(sciplot_core::t("找不到图形"))?;
            graph.layers.push(sciplot_core::Layer::default());
            graph.reflow_layers();
            Ok(())
        });
    }
    if clicked_menu(ui, sciplot_core::t("删除图层")) {
        app.edit(sciplot_core::t("删除图层"), |project| {
            let graph = project
                .graph_mut(graph)
                .ok_or(sciplot_core::t("找不到图形"))?;
            if graph.layers.len() <= 1 {
                return Err(sciplot_core::t("至少保留一个图层").into());
            }
            let index = layer.min(graph.layers.len() - 1);
            graph.layers.remove(index);
            graph.reflow_layers();
            Ok(())
        });
    }
    let curve = app
        .project
        .graph(graph)
        .and_then(|item| item.layers.get(layer))
        .map(|item| item.curves.len().saturating_sub(1))
        .unwrap_or(0);
    if app
        .project
        .graph(graph)
        .and_then(|item| item.layers.get(layer))
        .is_some_and(|item| !item.curves.is_empty())
    {
        ui.separator();
        analysis_menu(app, ui, graph, layer, curve.min(app.curve));
    }
    ui.separator();
    view_items(app, ui, graph);
    export_items(app, ui, graph);
}

fn surface_menu(app: &mut SciPlotApp, ui: &mut egui::Ui, graph: GraphId) {
    ui.label(sciplot_core::t("三维图"));
    ui.separator();
    let current = app
        .project
        .graph(graph)
        .and_then(|item| item.surface.as_ref())
        .map(|spec| spec.mode);
    for mode in [
        SurfaceMode::Surface,
        SurfaceMode::Wireframe,
        SurfaceMode::Scatter,
    ] {
        let label = if Some(mode) == current {
            format!("✓ {}", mode.label())
        } else {
            mode.label().to_string()
        };
        if clicked_menu(ui, &label) {
            app.edit(sciplot_core::t("三维显示"), |project| {
                project
                    .graph_mut(graph)
                    .and_then(|item| item.surface.as_mut())
                    .ok_or(sciplot_core::t("不是三维图"))?
                    .mode = mode;
                Ok(())
            });
        }
    }
    ui.separator();
    export_items(app, ui, graph);
}

fn curve_visibility_menu(app: &mut SciPlotApp, ui: &mut egui::Ui, graph: GraphId, layer: usize) {
    let curves: Vec<(usize, String, bool)> = app
        .project
        .graph(graph)
        .and_then(|item| item.layers.get(layer))
        .map(|item| {
            item.curves
                .iter()
                .enumerate()
                .map(|(index, curve)| (index, curve.name.clone(), curve.visible))
                .collect()
        })
        .unwrap_or_default();
    if curves.is_empty() {
        return;
    }
    if curves.iter().any(|(_, _, visible)| !*visible)
        && clicked_menu(ui, sciplot_core::t("显示全部曲线"))
    {
        app.edit(sciplot_core::t("曲线可见性"), |project| {
            let layer = project
                .graph_mut(graph)
                .and_then(|item| item.layers.get_mut(layer))
                .ok_or(sciplot_core::t("找不到图层"))?;
            for curve in &mut layer.curves {
                curve.visible = true;
            }
            Ok(())
        });
        app.status = sciplot_core::t("已显示全部曲线").into();
    }
    ui.menu_button(sciplot_core::t("曲线可见性"), |ui| {
        for (index, name, visible) in curves {
            let label = if visible {
                sciplot_core::trf!("隐藏 {name}", "Hide {name}", name = name)
            } else {
                sciplot_core::trf!("显示 {name}", "Show {name}", name = name)
            };
            if clicked_menu(ui, &label) {
                app.edit(sciplot_core::t("曲线可见性"), |project| {
                    let curve = project
                        .graph_mut(graph)
                        .and_then(|item| item.layers.get_mut(layer))
                        .and_then(|item| item.curves.get_mut(index))
                        .ok_or(sciplot_core::t("找不到曲线"))?;
                    curve.visible = !visible;
                    Ok(())
                });
            }
        }
    });
    ui.separator();
}

fn layer_format(app: &mut SciPlotApp, ui: &mut egui::Ui, graph: GraphId, layer: usize) {
    if clicked_menu(ui, sciplot_core::t("横轴…")) {
        open_axis(app, graph, layer, crate::app::AxisEdge::Bottom);
    }
    if clicked_menu(ui, sciplot_core::t("纵轴…")) {
        open_axis(app, graph, layer, crate::app::AxisEdge::Left);
    }
    if clicked_menu(ui, sciplot_core::tr!("上轴…", "Top axis…")) {
        open_axis(app, graph, layer, crate::app::AxisEdge::Top);
    }
    if clicked_menu(ui, sciplot_core::tr!("右轴…", "Right axis…")) {
        open_axis(app, graph, layer, crate::app::AxisEdge::Right);
    }
    if clicked_menu(ui, sciplot_core::t("图例…")) {
        if let Some(legend) = app
            .project
            .graph(graph)
            .and_then(|item| item.layers.get(layer))
            .map(|item| item.legend.clone())
        {
            app.dialog = Some(Dialog::Legend {
                graph,
                layer,
                legend,
            });
        }
    }
    if clicked_menu(ui, sciplot_core::t("标题…")) {
        if let Some(item) = app
            .project
            .graph(graph)
            .and_then(|item| item.layers.get(layer))
        {
            app.dialog = Some(Dialog::Title {
                graph,
                layer,
                title: item.title.clone(),
                size: format!("{}", item.title_size),
            });
        }
    }
    if clicked_menu(ui, sciplot_core::t("图层位置…")) {
        if let Some(frame) = app
            .project
            .graph(graph)
            .and_then(|item| item.layers.get(layer))
            .map(|item| item.frame)
        {
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
    if clicked_menu(ui, sciplot_core::t("文本标注…")) {
        app.dialog = Some(Dialog::TextAnn {
            graph,
            layer,
            x: "0".into(),
            y: "0".into(),
            text: sciplot_core::t("标注").into(),
        });
    }
    if clicked_menu(ui, sciplot_core::t("箭头…")) {
        app.dialog = Some(Dialog::ArrowAnn {
            graph,
            layer,
            x1: "0".into(),
            y1: "0".into(),
            x2: "1".into(),
            y2: "1".into(),
            line_only: false,
        });
    }
    if clicked_menu(ui, sciplot_core::t("线段…")) {
        app.dialog = Some(Dialog::ArrowAnn {
            graph,
            layer,
            x1: "0".into(),
            y1: "0".into(),
            x2: "1".into(),
            y2: "1".into(),
            line_only: true,
        });
    }
    if clicked_menu(ui, sciplot_core::tr!("插入图像…", "Insert image…")) {
        app.start_image_insert(graph, layer);
    }
    if clicked_menu(ui, sciplot_core::tr!("时间戳", "Time stamp")) {
        app.add_timestamp(graph, layer);
    }
}

fn analysis_menu(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    graph: GraphId,
    layer: usize,
    curve: usize,
) {
    ui.menu_button(sciplot_core::t("分析"), |ui| {
        if clicked_menu(ui, sciplot_core::t("线性拟合…")) {
            app.dialog = Some(open_fit(app, graph, layer, curve, FitWhich::Linear));
        }
        if clicked_menu(ui, sciplot_core::t("多项式拟合…")) {
            app.dialog = Some(open_fit(app, graph, layer, curve, FitWhich::Poly));
        }
        if clicked_menu(ui, sciplot_core::t("指数拟合…")) {
            app.dialog = Some(open_fit(app, graph, layer, curve, FitWhich::Exp));
        }
        if clicked_menu(ui, sciplot_core::t("高斯拟合…")) {
            app.dialog = Some(open_fit(app, graph, layer, curve, FitWhich::Gauss));
        }
        if clicked_menu(ui, sciplot_core::t("洛伦兹拟合…")) {
            app.dialog = Some(open_fit(app, graph, layer, curve, FitWhich::Lorentz));
        }
        if clicked_menu(ui, sciplot_core::t("自定义拟合…")) {
            app.dialog = Some(open_fit(app, graph, layer, curve, FitWhich::Custom));
        }
        ui.separator();
        if clicked_menu(ui, sciplot_core::t("微分")) {
            let prefs = app.prefs.clone();
            if let Some((table, gid)) = app.edit(sciplot_core::t("微分"), |project| {
                let table = ops::derivative_table(project, graph, layer, curve)?;
                let gid = project.plot_table(table, CurveKind::Line)?;
                prefs.apply_new_graph(project, gid);
                Ok((table, gid))
            }) {
                app.open_tab(Tab::Table(table));
                app.open_tab(Tab::Graph(gid));
            }
        }
        if clicked_menu(ui, sciplot_core::t("积分…")) {
            app.dialog = Some(range_dialog(graph, layer, curve, RangeOp::Integral));
        }
        if clicked_menu(ui, sciplot_core::t("插值…")) {
            app.dialog = Some(range_dialog(graph, layer, curve, RangeOp::Interp));
        }
        if clicked_menu(ui, sciplot_core::t("平滑…")) {
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
        if clicked_menu(ui, sciplot_core::t("FFT 滤波…")) {
            app.dialog = Some(range_dialog(graph, layer, curve, RangeOp::Filter));
        }
        ui.separator();
        if clicked_menu(ui, sciplot_core::t("求交点…")) {
            open_intersect(app, graph, layer, curve);
        }
    });
}

fn view_items(app: &mut SciPlotApp, ui: &mut egui::Ui, graph: GraphId) {
    if clicked_menu(ui, sciplot_core::t("数据读取器")) {
        app.reader = !app.reader;
        app.status = if app.reader {
            sciplot_core::t("数据读取器已打开").into()
        } else {
            sciplot_core::t("数据读取器已关闭").into()
        };
    }
    if clicked_menu(ui, sciplot_core::t("清除缩放")) {
        crate::app::reset_graph_view(app, graph);
    }
}

fn export_items(app: &mut SciPlotApp, ui: &mut egui::Ui, graph: GraphId) {
    if clicked_menu(ui, sciplot_core::t("导出 PNG…")) {
        app.open_tab(Tab::Graph(graph));
        crate::app::start_file(app, PathAction::ExportPng, "144");
    }
    if clicked_menu(ui, sciplot_core::t("导出 SVG…")) {
        app.open_tab(Tab::Graph(graph));
        crate::app::start_file(app, PathAction::ExportSvg, "");
    }
    if clicked_menu(ui, sciplot_core::t("复制 PNG")) {
        app.open_tab(Tab::Graph(graph));
        app.copy_png();
    }
    if clicked_menu(ui, sciplot_core::t("复制 SVG")) {
        app.open_tab(Tab::Graph(graph));
        app.copy_svg();
    }
}

fn open_style(app: &mut SciPlotApp, graph: GraphId, layer: usize, curve: usize) {
    if let Some(item) = app
        .project
        .graph(graph)
        .and_then(|item| item.layers.get(layer))
        .and_then(|item| item.curves.get(curve))
    {
        app.curve = curve;
        app.dialog = Some(Dialog::Style {
            graph,
            layer,
            curve,
            style: item.style.clone(),
            name: item.name.clone(),
            axes: item.axes,
            vector_mode: item.vector_mode,
        });
    } else {
        app.status = sciplot_core::t("这一层没有曲线").into();
    }
}

fn open_errors(app: &mut SciPlotApp, graph: GraphId, layer: usize, curve: usize) {
    let Some(item) = app
        .project
        .graph(graph)
        .and_then(|item| item.layers.get(layer))
        .and_then(|item| item.curves.get(curve))
    else {
        return;
    };
    let (x_col, y_col) = match &item.source {
        sciplot_core::DataSource::Columns { table, x_err, y_err, .. } => {
            let names = app.project.table(*table).map(|table| {
                table.columns.iter().map(|column| column.name.clone()).collect::<Vec<_>>()
            });
            let pick = |index: Option<usize>| {
                index
                    .and_then(|index| names.as_ref().and_then(|names| names.get(index)).cloned())
                    .unwrap_or_default()
            };
            (pick(*x_err), pick(*y_err))
        }
        _ => (String::new(), String::new()),
    };
    app.dialog = Some(Dialog::Errors {
        graph,
        layer,
        curve,
        spec: item.error.clone(),
        x_col,
        y_col,
    });
}

fn open_axis(app: &mut SciPlotApp, graph: GraphId, layer: usize, edge: crate::app::AxisEdge) {
    let axis = app
        .project
        .graph(graph)
        .and_then(|item| item.layers.get(layer))
        .map(|item| match edge {
            crate::app::AxisEdge::Bottom => item.x_axis.clone(),
            crate::app::AxisEdge::Left => item.y_axis.clone(),
            crate::app::AxisEdge::Top => item.x_top.clone(),
            crate::app::AxisEdge::Right => item.y_right.clone(),
        });
    if let Some(axis) = axis {
        app.dialog = Some(Dialog::Axis {
            graph,
            layer,
            edge,
            min: crate::dialogs::number_text(axis.min),
            max: crate::dialogs::number_text(axis.max),
            step: crate::dialogs::number_text(axis.step),
            minor: axis
                .minor_count
                .map(|count| count.to_string())
                .unwrap_or_default(),
            error: String::new(),
            axis,
        });
    }
}

fn x_is_strictly_increasing(app: &SciPlotApp, graph: GraphId, layer: usize, curve: usize) -> bool {
    let Some(curve) = app
        .project
        .graph(graph)
        .and_then(|item| item.layers.get(layer))
        .and_then(|item| item.curves.get(curve))
    else {
        return false;
    };
    let Ok(series) = sciplot_core::resolve_curve(&app.project, curve) else {
        return false;
    };
    let xs: Vec<f64> = series
        .x
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect();
    xs.len() >= 2 && xs.windows(2).all(|pair| pair[1] > pair[0])
}

fn open_fit(
    app: &SciPlotApp,
    graph: GraphId,
    layer: usize,
    curve: usize,
    which: FitWhich,
) -> Dialog {
    let (xmin, xmax) = selection_text(app, graph);
    fit_dialog(graph, layer, curve, which, xmin, xmax)
}

fn selection_text(app: &SciPlotApp, graph: GraphId) -> (String, String) {
    app.xsel
        .as_ref()
        .filter(|sel| sel.graph == graph)
        .map(|sel| {
            (
                crate::dialogs::number_text(Some(sel.x0.min(sel.x1))),
                crate::dialogs::number_text(Some(sel.x0.max(sel.x1))),
            )
        })
        .unwrap_or_default()
}

fn open_intersect(app: &mut SciPlotApp, graph: GraphId, layer: usize, curve: usize) {
    let count = app
        .project
        .graph(graph)
        .and_then(|item| item.layers.get(layer))
        .map(|item| item.curves.len())
        .unwrap_or(0);
    let curve_b = if count > 1 && curve == 0 { 1 } else { 0 };
    let (xmin, xmax) = selection_text(app, graph);
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
        smooth: x_is_strictly_increasing(app, graph, layer, curve),
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
            form: crate::app::FunctionForm::Cartesian,
            expr: "sin(x)*cos(y)".into(),
            y_expr: "cos(t)".into(),
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
            form: crate::app::FunctionForm::Cartesian,
            expr: "sin(x)".into(),
            y_expr: "cos(t)".into(),
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
