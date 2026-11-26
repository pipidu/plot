use sciplot_analysis::{FitModel, InterpMethod};
use sciplot_core::{
    format_number, parse_initials, Annotation, AnnotationKind, AxisScale, ColumnMode, ColumnType, CurveKind,
    CurveStyle, Dash, DataSource, LegendPos, SurfaceMode, SurfaceSource, Symbol,
};

use crate::app::{Dialog, FitWhich, LineChoice, PathAction, RangeOp, SciPlotApp};
use crate::i18n;
use crate::ops;

pub fn show(app: &mut SciPlotApp, ctx: &egui::Context) {
    let Some(mut dialog) = app.dialog.take() else {
        return;
    };
    let title = title_of(&dialog);
    let mut open = true;
    let mut stay = true;
    egui::Window::new(title)
        .open(&mut open)
        .resizable(true)
        .default_width(440.0)
        .show(ctx, |ui| {
            stay = ui_of(app, ui, &mut dialog);
        });
    if open && stay {
        app.dialog = Some(dialog);
    }
}

fn title_of(dialog: &Dialog) -> String {
    match dialog {
        Dialog::Message(_) => "结果".into(),
        Dialog::Quit => "退出 SciPlot".into(),
        Dialog::Help => "公式说明".into(),
        Dialog::About => "关于".into(),
        Dialog::Path { action, .. } => match action {
            PathAction::Open => "打开项目".into(),
            PathAction::SaveAs => "另存为".into(),
            PathAction::ImportCsv => "导入 CSV".into(),
            PathAction::ExportCsv => "导出 CSV".into(),
            PathAction::ExportPng => "导出 PNG".into(),
            PathAction::ExportSvg => "导出 SVG".into(),
        },
        Dialog::Formula { .. } => "列公式".into(),
        Dialog::MatrixFormula { .. } => "矩阵公式".into(),
        Dialog::NewMatrix { .. } => "新建矩阵".into(),
        Dialog::ColumnProps { .. } => "列类型与角色".into(),
        Dialog::Sort { .. } => "排序".into(),
        Dialog::Axis { .. } => "坐标轴".into(),
        Dialog::Legend { .. } => "图例".into(),
        Dialog::Title { .. } => "标题".into(),
        Dialog::Style { .. } => "曲线样式".into(),
        Dialog::Frame { .. } => "图层位置".into(),
        Dialog::TextAnn { .. } => "文本标注".into(),
        Dialog::ArrowAnn { .. } => "箭头或线段".into(),
        Dialog::Intersect { .. } => "求交点".into(),
        Dialog::Fit { .. } => "拟合".into(),
        Dialog::Range { .. } => "分析".into(),
        Dialog::Function { .. } => "函数曲线或曲面".into(),
        Dialog::AddCurve { .. } => "添加到当前图".into(),
        Dialog::Prefs(_) => "默认样式与字体".into(),
    }
}

fn ui_of(app: &mut SciPlotApp, ui: &mut egui::Ui, dialog: &mut Dialog) -> bool {
    match dialog {
        Dialog::Message(text) => {
            ui.label(text.as_str());
            button_closes(ui, "关闭")
        }
        Dialog::Help => {
            ui.label(i18n::FORMULA_HELP);
            button_closes(ui, "关闭")
        }
        Dialog::About => {
            ui.label(i18n::ABOUT);
            button_closes(ui, "关闭")
        }
        Dialog::Quit => quit(app, ui),
        Dialog::Path { action, path, extra } => path_dialog(app, ui, *action, path, extra),
        Dialog::Formula { table, column, expr } => formula(app, ui, *table, *column, expr),
        Dialog::MatrixFormula { matrix, expr } => matrix_formula(app, ui, *matrix, expr),
        Dialog::NewMatrix { name, rows, cols } => new_matrix(app, ui, name, rows, cols),
        Dialog::ColumnProps { table, column } => column_props(app, ui, *table, *column),
        Dialog::Sort { table, column, ascending } => sort_dialog(app, ui, *table, column, ascending),
        Dialog::Axis { graph, layer, y_axis, axis, min, max } => axis_dialog(app, ui, *graph, *layer, *y_axis, axis, min, max),
        Dialog::Legend { graph, layer, legend } => legend_dialog(app, ui, *graph, *layer, legend),
        Dialog::Title { graph, layer, title, size } => title_dialog(app, ui, *graph, *layer, title, size),
        Dialog::Style { graph, layer, curve, style, name } => style_dialog(app, ui, *graph, *layer, *curve, style, name),
        Dialog::Frame { graph, layer, x, y, w, h } => frame_dialog(app, ui, *graph, *layer, x, y, w, h),
        Dialog::TextAnn { graph, layer, x, y, text } => text_ann(app, ui, *graph, *layer, x, y, text),
        Dialog::ArrowAnn { graph, layer, x1, y1, x2, y2, line_only } => arrow_ann(app, ui, *graph, *layer, x1, y1, x2, y2, *line_only),
        Dialog::Intersect { graph, layer, curve_a, curve_b, kind_a, kind_b, value_a, value_b, smooth, xmin, xmax, tol } => {
            intersect_dialog(app, ui, *graph, *layer, curve_a, curve_b, kind_a, kind_b, value_a, value_b, smooth, xmin, xmax, tol)
        }
        Dialog::Fit { graph, layer, curve, which, degree, expr, initial } => fit_dialog(app, ui, *graph, *layer, curve, which, degree, expr, initial),
        Dialog::Range { graph, layer, curve, op, a, b, n, method, window, order, sg, highpass } => {
            range_dialog(app, ui, *graph, *layer, curve, *op, a, b, n, method, window, order, sg, highpass)
        }
        Dialog::Function { graph, layer, expr, xmin, xmax, ymin, ymax, samples, surface, mode } => {
            function_dialog(app, ui, *graph, *layer, expr, xmin, xmax, ymin, ymax, samples, surface, mode)
        }
        Dialog::AddCurve { graph, table, kind } => add_curve(app, ui, *graph, table, kind),
        Dialog::Prefs(prefs) => prefs_dialog(app, ui, prefs),
    }
}

fn button_closes(ui: &mut egui::Ui, label: &str) -> bool {
    !ui.button(label).clicked()
}

fn prefs_dialog(app: &mut SciPlotApp, ui: &mut egui::Ui, prefs: &mut crate::prefs::Prefs) -> bool {
    egui::ScrollArea::vertical().max_height(460.0).show(ui, |ui| {
        ui.label("设置保存在本机，不写入项目。之后新建的曲线、标题和坐标轴文字会使用这里的样式。");
        ui.separator();
        ui.strong("字体");
        ui.label("屏幕和 PNG 使用这个文件。SVG 仍按系统字体名显示。");
        let shown = if prefs.font_path.is_empty() { "自动".to_string() } else { prefs.font_path.clone() };
        egui::ComboBox::from_id_salt("prefs-font").selected_text(shown).show_ui(ui, |ui| {
            ui.selectable_value(&mut prefs.font_path, String::new(), "自动");
            for (name, path) in crate::fonts::known_fonts() {
                ui.selectable_value(&mut prefs.font_path, path.to_string(), name);
            }
        });
        ui.horizontal(|ui| {
            ui.label("字体文件");
            ui.add(egui::TextEdit::singleline(&mut prefs.font_path).desired_width(280.0));
            if std::env::var("SCIPLOT_NO_DIALOG").is_err() && ui.button("浏览…").clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter("字体", &["ttf", "ttc", "otf"]).pick_file() {
                    prefs.font_path = path.display().to_string();
                }
            }
        });
        ui.separator();
        ui.strong("文字");
        size_row(ui, "标题", &mut prefs.title_size, 8.0, 72.0);
        size_row(ui, "坐标轴标题", &mut prefs.axis_label_size, 8.0, 48.0);
        size_row(ui, "刻度", &mut prefs.tick_size, 8.0, 36.0);
        ui.separator();
        ui.strong("曲线");
        size_row(ui, "线宽", &mut prefs.line_width, 0.4, 12.0);
        egui::ComboBox::from_id_salt("prefs-dash").selected_text(prefs.dash.label()).show_ui(ui, |ui| {
            for item in Dash::all() {
                ui.selectable_value(&mut prefs.dash, *item, item.label());
            }
        });
        egui::ComboBox::from_id_salt("prefs-symbol").selected_text(prefs.symbol.label()).show_ui(ui, |ui| {
            for item in Symbol::all() {
                ui.selectable_value(&mut prefs.symbol, *item, item.label());
            }
        });
        size_row(ui, "符号大小", &mut prefs.symbol_size, 1.0, 24.0);
        let mut bins = prefs.histogram_bins as f32;
        ui.horizontal(|ui| {
            ui.label("直方图组数");
            if ui.add(egui::DragValue::new(&mut bins).range(2.0..=200.0)).changed() {
                prefs.histogram_bins = bins.round() as usize;
            }
        });
        ui.checkbox(&mut prefs.use_palette, "按曲线序号使用调色板");
        if !prefs.use_palette {
            ui.horizontal(|ui| {
                ui.label("固定颜色");
                ui.color_edit_button_srgba_unmultiplied(&mut prefs.color);
            });
        }
    });
    ui.separator();
    let mut stay = true;
    ui.horizontal(|ui| {
        if ui.button("保存").clicked() {
            store_prefs(app, prefs, ui.ctx());
            stay = false;
        }
        if ui.button("用到当前图").clicked() {
            store_prefs(app, prefs, ui.ctx());
            if let Some(id) = app.active_graph() {
                let stored = app.prefs.clone();
                app.edit("应用默认样式", |project| {
                    stored.apply_new_graph(project, id);
                    Ok(())
                });
                app.status = "已把默认样式用到当前图".into();
            } else {
                app.status = "当前没有打开的图形".into();
            }
            stay = false;
        }
        if ui.button("关闭").clicked() {
            stay = false;
        }
    });
    stay
}

fn store_prefs(app: &mut SciPlotApp, prefs: &mut crate::prefs::Prefs, ctx: &egui::Context) {
    prefs.clamp();
    app.prefs = prefs.clone();
    let saved = crate::prefs::save(&app.prefs);
    if app.reload_font(ctx) {
        app.status = if saved { "已保存默认样式和字体".into() } else { "样式已应用，但没能写入设置文件".into() };
    } else {
        app.prefs.font_path.clear();
        prefs.font_path.clear();
        crate::prefs::save(&app.prefs);
        app.reload_font(ctx);
        app.status = "找不到指定字体，已改用自动字体并保存其余设置".into();
    }
}

fn size_row(ui: &mut egui::Ui, label: &str, value: &mut f32, min: f32, max: f32) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).range(min..=max).speed(0.1));
    });
}

fn quit(app: &mut SciPlotApp, ui: &mut egui::Ui) -> bool {
    ui.label("当前项目有未保存的修改。");
    let mut stay = true;
    ui.horizontal(|ui| {
        if ui.button("保存并退出").clicked() {
            if app.save_current() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            } else if matches!(app.dialog, Some(Dialog::Path { .. })) {
                stay = false;
            }
        }
        if ui.button("不保存退出").clicked() {
            app.dirty = false;
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            stay = false;
        }
        if ui.button("取消").clicked() {
            stay = false;
        }
    });
    stay
}

fn path_dialog(app: &mut SciPlotApp, ui: &mut egui::Ui, action: PathAction, path: &mut String, extra: &mut String) -> bool {
    ui.label("路径");
    ui.text_edit_singleline(path);
    if matches!(action, PathAction::ExportPng) {
        ui.label("分辨率 DPI");
        ui.text_edit_singleline(extra);
    }
    if ui.button("确定").clicked() {
        app.perform_path(action, path, extra);
        return false;
    }
    true
}

fn formula(app: &mut SciPlotApp, ui: &mut egui::Ui, table: sciplot_core::TableId, column: usize, expr: &mut String) -> bool {
    ui.label("例如 sin(col(\"X\"))*2 。整数除法会截断，小数请写 1.0/2。");
    ui.text_edit_singleline(expr);
    if ui.button("计算并写入列").clicked() {
        let formula = expr.clone();
        if app.edit("列公式", |project| {
            let table = project.table_mut(table).ok_or("找不到表格")?;
            sciplot_core::apply_formula(table, column, &formula).map(|_| ())
        }).is_some() {
            app.status = "列公式已计算".into();
            return false;
        }
    }
    true
}

fn matrix_formula(app: &mut SciPlotApp, ui: &mut egui::Ui, matrix: sciplot_core::MatrixId, expr: &mut String) -> bool {
    ui.label("用含 x、y 的表达式填充每个单元格，例如 sin(x)*cos(y)。");
    ui.text_edit_singleline(expr);
    if ui.button("填充").clicked() {
        let formula = expr.clone();
        if app.edit("矩阵公式", |project| {
            project.matrix_mut(matrix).ok_or("找不到矩阵")?.fill_formula(&formula)
        }).is_some() {
            app.status = "矩阵已按公式填充".into();
            return false;
        }
    }
    true
}

fn new_matrix(app: &mut SciPlotApp, ui: &mut egui::Ui, name: &mut String, rows: &mut String, cols: &mut String) -> bool {
    ui.label("名称");
    ui.text_edit_singleline(name);
    ui.label("行数");
    ui.text_edit_singleline(rows);
    ui.label("列数");
    ui.text_edit_singleline(cols);
    if ui.button("创建").clicked() {
        let name = name.clone();
        let rows = rows.parse::<usize>().unwrap_or(10).clamp(1, 500);
        let cols = cols.parse::<usize>().unwrap_or(10).clamp(1, 500);
        if let Some(id) = app.edit("新建矩阵", |project| Ok(project.new_matrix(&name, rows, cols))) {
            app.open_tab(crate::app::Tab::Matrix(id));
            app.status = "已新建矩阵".into();
            return false;
        }
    }
    true
}

fn column_props(app: &mut SciPlotApp, ui: &mut egui::Ui, table: sciplot_core::TableId, column: usize) -> bool {
    let Some(current) = app.project.table(table).and_then(|t| t.columns.get(column)).cloned() else {
        ui.label("列不存在");
        return true;
    };
    ui.label(format!("当前：{} / {} / {}", current.name, current.ty.label(), current.mode.label()));
    ui.label("改名、类型和角色后点应用。名称留在表格里直接改也可以。");
    if ui.button("应用为数值列").clicked() {
        apply_column(app, table, column, ColumnType::Numeric, None);
    }
    ui.horizontal(|ui| {
        for ty in ColumnType::all() {
            if ui.button(ty.label()).clicked() {
                apply_column(app, table, column, *ty, None);
            }
        }
    });
    ui.horizontal(|ui| {
        for mode in ColumnMode::all() {
            if ui.button(mode.label()).clicked() {
                apply_column(app, table, column, current.ty, Some(*mode));
            }
        }
    });
    true
}

fn apply_column(app: &mut SciPlotApp, table: sciplot_core::TableId, column: usize, ty: ColumnType, mode: Option<ColumnMode>) {
    if app.edit("列属性", |project| {
        let column = project.table_mut(table).ok_or("找不到表格")?.columns.get_mut(column).ok_or("找不到列")?;
        column.ty = ty;
        if let Some(mode) = mode {
            column.mode = mode;
        }
        Ok(())
    }).is_some() {
        app.status = "已更新列属性".into();
    }
}

fn sort_dialog(app: &mut SciPlotApp, ui: &mut egui::Ui, table: sciplot_core::TableId, column: &mut usize, ascending: &mut bool) -> bool {
    let names: Vec<String> = app.project.table(table).map(|t| t.columns.iter().map(|c| c.name.clone()).collect()).unwrap_or_default();
    if names.is_empty() {
        ui.label("表格没有列");
        return true;
    }
    *column = (*column).min(names.len() - 1);
    egui::ComboBox::from_id_salt("sort-col").selected_text(&names[*column]).show_ui(ui, |ui| {
        for (index, name) in names.iter().enumerate() {
            ui.selectable_value(column, index, name);
        }
    });
    ui.checkbox(ascending, "升序");
    if ui.button("排序").clicked() {
        let column = *column;
        let ascending = *ascending;
        if app.edit("排序", |project| {
            project.table_mut(table).ok_or("找不到表格")?.sort_by(column, ascending);
            Ok(())
        }).is_some() {
            app.status = "已排序".into();
            return false;
        }
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn axis_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    graph: sciplot_core::GraphId,
    layer: usize,
    y_axis: bool,
    axis: &mut sciplot_core::Axis,
    min: &mut String,
    max: &mut String,
) -> bool {
    ui.label(if y_axis { "纵轴" } else { "横轴" });
    ui.label("标题");
    ui.text_edit_singleline(&mut axis.label);
    egui::ComboBox::from_id_salt("axis-scale").selected_text(axis.scale.label()).show_ui(ui, |ui| {
        for scale in AxisScale::all() {
            ui.selectable_value(&mut axis.scale, *scale, scale.label());
        }
    });
    ui.checkbox(&mut axis.major_grid, "主网格");
    ui.checkbox(&mut axis.minor_grid, "次网格");
    ui.checkbox(&mut axis.inverted, "反向");
    ui.label("最小值（空为自动）");
    ui.text_edit_singleline(min);
    ui.label("最大值（空为自动）");
    ui.text_edit_singleline(max);
    if ui.button("应用").clicked() {
        axis.min = parse_opt(min);
        axis.max = parse_opt(max);
        let stored = axis.clone();
        app.edit("坐标轴", |project| {
            let layer = project.graph_mut(graph).ok_or("找不到图形")?.layers.get_mut(layer).ok_or("找不到图层")?;
            if y_axis {
                layer.y_axis = stored.clone();
            } else {
                layer.x_axis = stored.clone();
            }
            Ok(())
        });
        app.status = "已更新坐标轴".into();
    }
    true
}

fn legend_dialog(app: &mut SciPlotApp, ui: &mut egui::Ui, graph: sciplot_core::GraphId, layer: usize, legend: &mut sciplot_core::Legend) -> bool {
    ui.checkbox(&mut legend.visible, "显示图例");
    egui::ComboBox::from_id_salt("legend-pos").selected_text(legend.position.label()).show_ui(ui, |ui| {
        for pos in LegendPos::all() {
            ui.selectable_value(&mut legend.position, *pos, pos.label());
        }
    });
    if ui.button("应用").clicked() {
        let stored = legend.clone();
        app.edit("图例", |project| {
            project.graph_mut(graph).ok_or("找不到图形")?.layers.get_mut(layer).ok_or("找不到图层")?.legend = stored.clone();
            Ok(())
        });
        app.status = "已更新图例".into();
    }
    true
}

fn title_dialog(app: &mut SciPlotApp, ui: &mut egui::Ui, graph: sciplot_core::GraphId, layer: usize, title: &mut String, size: &mut String) -> bool {
    ui.label("标题");
    ui.text_edit_singleline(title);
    ui.label("字号");
    ui.text_edit_singleline(size);
    if ui.button("应用").clicked() {
        let title = title.clone();
        let size = size.parse::<f32>().unwrap_or(16.0).clamp(8.0, 72.0);
        app.edit("标题", |project| {
            let layer = project.graph_mut(graph).ok_or("找不到图形")?.layers.get_mut(layer).ok_or("找不到图层")?;
            layer.title = title.clone();
            layer.title_size = size;
            Ok(())
        });
        app.status = "已更新标题".into();
    }
    true
}

fn style_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    graph: sciplot_core::GraphId,
    layer: usize,
    curve: usize,
    style: &mut CurveStyle,
    name: &mut String,
) -> bool {
    ui.label("名称");
    ui.text_edit_singleline(name);
    egui::ComboBox::from_id_salt("dash").selected_text(style.dash.label()).show_ui(ui, |ui| {
        for dash in Dash::all() {
            ui.selectable_value(&mut style.dash, *dash, dash.label());
        }
    });
    egui::ComboBox::from_id_salt("symbol").selected_text(style.symbol.label()).show_ui(ui, |ui| {
        for symbol in Symbol::all() {
            ui.selectable_value(&mut style.symbol, *symbol, symbol.label());
        }
    });
    ui.add(egui::Slider::new(&mut style.line_width, 0.4..=8.0).text("线宽"));
    ui.add(egui::Slider::new(&mut style.symbol_size, 2.0..=18.0).text("符号"));
    ui.add(egui::Slider::new(&mut style.histogram_bins, 2..=80).text("直方图箱数"));
    color_sliders(ui, &mut style.color);
    if ui.button("应用").clicked() {
        let style = style.clone();
        let name = name.clone();
        app.edit("曲线样式", |project| {
            let curve = project.graph_mut(graph).ok_or("找不到图形")?.layers.get_mut(layer).ok_or("找不到图层")?.curves.get_mut(curve).ok_or("找不到曲线")?;
            curve.style = style.clone();
            curve.name = name.clone();
            Ok(())
        });
        app.status = "已更新曲线样式".into();
    }
    true
}

fn color_sliders(ui: &mut egui::Ui, color: &mut [u8; 4]) {
    let mut r = color[0] as f32;
    let mut g = color[1] as f32;
    let mut b = color[2] as f32;
    ui.add(egui::Slider::new(&mut r, 0.0..=255.0).text("红"));
    ui.add(egui::Slider::new(&mut g, 0.0..=255.0).text("绿"));
    ui.add(egui::Slider::new(&mut b, 0.0..=255.0).text("蓝"));
    color[0] = r.round() as u8;
    color[1] = g.round() as u8;
    color[2] = b.round() as u8;
}

#[allow(clippy::too_many_arguments)]
fn frame_dialog(app: &mut SciPlotApp, ui: &mut egui::Ui, graph: sciplot_core::GraphId, layer: usize, x: &mut String, y: &mut String, w: &mut String, h: &mut String) -> bool {
    ui.label("相对整张图的位置，左上角为原点，宽高是比例。");
    labeled(ui, "x", x);
    labeled(ui, "y", y);
    labeled(ui, "宽", w);
    labeled(ui, "高", h);
    if ui.button("应用").clicked() {
        let rect = sciplot_core::FrameRect {
            x: x.parse().unwrap_or(0.0),
            y: y.parse().unwrap_or(0.0),
            w: w.parse().unwrap_or(1.0),
            h: h.parse().unwrap_or(1.0),
        };
        app.edit("图层位置", |project| {
            project.graph_mut(graph).ok_or("找不到图形")?.layers.get_mut(layer).ok_or("找不到图层")?.frame = rect;
            Ok(())
        });
        app.status = "已更新图层位置".into();
    }
    true
}

fn text_ann(app: &mut SciPlotApp, ui: &mut egui::Ui, graph: sciplot_core::GraphId, layer: usize, x: &mut String, y: &mut String, text: &mut String) -> bool {
    labeled(ui, "x", x);
    labeled(ui, "y", y);
    ui.label("文字");
    ui.text_edit_singleline(text);
    if ui.button("添加").clicked() {
        let (Some(x), Some(y)) = (parse_opt(x), parse_opt(y)) else {
            app.status = "请输入数值坐标".into();
            return true;
        };
        let text = text.clone();
        app.edit("文本标注", |project| push_annotation(project, graph, layer, AnnotationKind::Text { x, y, text: text.clone(), size: 14.0, color: [20, 20, 20, 255] }));
        app.status = "已添加文本".into();
        return false;
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn arrow_ann(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    graph: sciplot_core::GraphId,
    layer: usize,
    x1: &mut String,
    y1: &mut String,
    x2: &mut String,
    y2: &mut String,
    line_only: bool,
) -> bool {
    labeled(ui, "x1", x1);
    labeled(ui, "y1", y1);
    labeled(ui, "x2", x2);
    labeled(ui, "y2", y2);
    if ui.button("添加").clicked() {
        let nums = [x1, y1, x2, y2].map(|text| parse_opt(text));
        if nums.iter().any(|v| v.is_none()) {
            app.status = "请输入数值坐标".into();
            return true;
        }
        let [x1, y1, x2, y2] = [nums[0].unwrap(), nums[1].unwrap(), nums[2].unwrap(), nums[3].unwrap()];
        let color = [180, 40, 40, 255];
        let kind = if line_only {
            AnnotationKind::Line { x1, y1, x2, y2, color }
        } else {
            AnnotationKind::Arrow { x1, y1, x2, y2, color }
        };
        app.edit("标注", |project| push_annotation(project, graph, layer, kind.clone()));
        app.status = "已添加标注".into();
        return false;
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn intersect_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    graph: sciplot_core::GraphId,
    layer: usize,
    curve_a: &mut usize,
    curve_b: &mut usize,
    kind_a: &mut LineChoice,
    kind_b: &mut LineChoice,
    value_a: &mut String,
    value_b: &mut String,
    smooth: &mut bool,
    xmin: &mut String,
    xmax: &mut String,
    tol: &mut String,
) -> bool {
    let names = curve_names(app, graph, layer);
    ui.label("曲线 A");
    choice_row(ui, "a", kind_a, curve_a, &names, value_a);
    ui.label("曲线 B");
    choice_row(ui, "b", kind_b, curve_b, &names, value_b);
    ui.checkbox(smooth, "平滑求根（样条或函数，否则按折线精确求交）");
    labeled(ui, "X 最小（可空）", xmin);
    labeled(ui, "X 最大（可空）", xmax);
    labeled(ui, "容差", tol);
    if ui.button("计算").clicked() {
        let built_a = build_input(app, graph, layer, *kind_a, *curve_a, value_a);
        let built_b = build_input(app, graph, layer, *kind_b, *curve_b, value_b);
        match (built_a, built_b) {
            (Ok(a), Ok(b)) => {
                let options = sciplot_analysis::IntersectOptions {
                    mode: if *smooth { sciplot_analysis::IntersectMode::Smooth } else { sciplot_analysis::IntersectMode::Piecewise },
                    x_min: parse_opt(xmin),
                    x_max: parse_opt(xmax),
                    tolerance: parse_opt(tol).unwrap_or(1e-8),
                };
                let name = format!("交点_{}_{}", a.name(), b.name());
                if app.edit("求交点", |project| ops::apply_intersection(project, graph, layer, &name, &ops_intersect(&a, &b, &options)).map(|_| ())).is_some() {
                    app.status = "交点已标在图上，并写入新表格".into();
                    return false;
                }
            }
            (Err(err), _) | (_, Err(err)) => app.status = err,
        }
    }
    true
}

fn ops_intersect(a: &sciplot_analysis::CurveInput, b: &sciplot_analysis::CurveInput, options: &sciplot_analysis::IntersectOptions) -> sciplot_analysis::IntersectOutput {
    sciplot_analysis::intersect(a, b, options)
}

fn choice_row(ui: &mut egui::Ui, salt: &str, kind: &mut LineChoice, curve: &mut usize, names: &[String], value: &mut String) {
    egui::ComboBox::from_id_salt(format!("kind-{salt}")).selected_text(kind.label()).show_ui(ui, |ui| {
        for item in [LineChoice::Curve, LineChoice::Horizontal, LineChoice::Vertical] {
            ui.selectable_value(kind, item, item.label());
        }
    });
    if *kind == LineChoice::Curve {
        if names.is_empty() {
            ui.label("这一层没有曲线");
        } else {
            *curve = (*curve).min(names.len() - 1);
            egui::ComboBox::from_id_salt(format!("curve-{salt}")).selected_text(&names[*curve]).show_ui(ui, |ui| {
                for (index, name) in names.iter().enumerate() {
                    ui.selectable_value(curve, index, name);
                }
            });
        }
    } else {
        ui.text_edit_singleline(value);
    }
}

#[allow(clippy::too_many_arguments)]
fn fit_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    graph: sciplot_core::GraphId,
    layer: usize,
    curve: &mut usize,
    which: &mut FitWhich,
    degree: &mut String,
    expr: &mut String,
    initial: &mut String,
) -> bool {
    let names = curve_names(app, graph, layer);
    if names.is_empty() {
        ui.label("没有可拟合的曲线");
        return true;
    }
    *curve = (*curve).min(names.len() - 1);
    egui::ComboBox::from_id_salt("fit-curve").selected_text(&names[*curve]).show_ui(ui, |ui| {
        for (index, name) in names.iter().enumerate() {
            ui.selectable_value(curve, index, name);
        }
    });
    egui::ComboBox::from_id_salt("fit-model").selected_text(which.label()).show_ui(ui, |ui| {
        for item in [FitWhich::Linear, FitWhich::Poly, FitWhich::Exp, FitWhich::Gauss, FitWhich::Lorentz, FitWhich::Custom] {
            ui.selectable_value(which, item, item.label());
        }
    });
    if *which == FitWhich::Poly {
        labeled(ui, "次数", degree);
    }
    if *which == FitWhich::Custom {
        labeled(ui, "表达式，例如 a*exp(-b*x)", expr);
        labeled(ui, "初值，例如 a=1, b=0.5", initial);
    }
    if ui.button("拟合").clicked() {
        let model = match which {
            FitWhich::Linear => Ok(FitModel::Linear),
            FitWhich::Poly => degree.parse::<usize>().map(|d| FitModel::Polynomial(d.clamp(1, 8))).map_err(|_| "次数无效".to_string()),
            FitWhich::Exp => Ok(FitModel::Exponential),
            FitWhich::Gauss => Ok(FitModel::Gaussian),
            FitWhich::Lorentz => Ok(FitModel::Lorentzian),
            FitWhich::Custom => parse_initials(initial).map(|initial| FitModel::Custom { expr: expr.clone(), initial }),
        };
        match model {
            Ok(model) => {
                let curve = *curve;
                let before = app.project.graph(graph).and_then(|item| item.layers.get(layer)).map(|item| item.curves.len()).unwrap_or(0);
                let prefs = app.prefs.clone();
                if let Some(report) = app.edit("拟合", |project| {
                    let report = ops::fit_on_graph(project, graph, layer, curve, &model)?;
                    prefs.apply_curves(project, graph, layer, before);
                    Ok(report)
                }) {
                    app.dialog = Some(Dialog::Message(report));
                    return false;
                }
            }
            Err(err) => app.status = err,
        }
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn range_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    graph: sciplot_core::GraphId,
    layer: usize,
    curve: &mut usize,
    op: RangeOp,
    a: &mut String,
    b: &mut String,
    n: &mut String,
    method: &mut InterpMethod,
    window: &mut String,
    order: &mut String,
    sg: &mut bool,
    highpass: &mut bool,
) -> bool {
    let names = curve_names(app, graph, layer);
    if !names.is_empty() {
        *curve = (*curve).min(names.len() - 1);
        egui::ComboBox::from_id_salt("range-curve").selected_text(&names[*curve]).show_ui(ui, |ui| {
            for (index, name) in names.iter().enumerate() {
                ui.selectable_value(curve, index, name);
            }
        });
    }
    match op {
        RangeOp::Integral => {
            labeled(ui, "下限", a);
            labeled(ui, "上限", b);
        }
        RangeOp::Interp => {
            egui::ComboBox::from_id_salt("interp").selected_text(method.label()).show_ui(ui, |ui| {
                for item in [InterpMethod::Linear, InterpMethod::CubicSpline, InterpMethod::Akima] {
                    ui.selectable_value(method, item, item.label());
                }
            });
            labeled(ui, "点数", n);
        }
        RangeOp::Smooth => {
            ui.checkbox(sg, "Savitzky-Golay（否则移动平均）");
            labeled(ui, "窗口（奇数）", window);
            labeled(ui, "多项式次数", order);
        }
        RangeOp::Filter => {
            labeled(ui, "截止频率", a);
            ui.checkbox(highpass, "高通");
        }
    }
    if ui.button("计算").clicked() {
        let curve = *curve;
        let done = match op {
            RangeOp::Integral => {
                let (Some(xmin), Some(xmax)) = (parse_opt(a), parse_opt(b)) else {
                    app.status = "请输入积分区间".into();
                    return true;
                };
                app.edit("积分", |project| ops::integral_report(project, graph, layer, curve, xmin, xmax).map(|_| ()))
            }
            RangeOp::Interp => {
                let method = *method;
                let n = n.parse::<usize>().unwrap_or(200);
                app.edit("插值", |project| ops::interpolate_table(project, graph, layer, curve, method, n).map(|_| ()))
            }
            RangeOp::Smooth => {
                let window = window.parse::<usize>().unwrap_or(5);
                let order = order.parse::<usize>().unwrap_or(2);
                let sg = *sg;
                app.edit("平滑", |project| ops::smooth_table(project, graph, layer, curve, sg, window, order).map(|_| ()))
            }
            RangeOp::Filter => {
                let Some(cutoff) = parse_opt(a) else {
                    app.status = "请输入截止频率".into();
                    return true;
                };
                let highpass = *highpass;
                app.edit("滤波", |project| ops::filter_table(project, graph, layer, curve, cutoff, highpass).map(|_| ()))
            }
        };
        if done.is_some() {
            if let Some(table) = app.project.tables.last().map(|t| t.id) {
                app.open_tab(crate::app::Tab::Table(table));
                let prefs = app.prefs.clone();
                if let Some(gid) = app.edit("绘制结果", |project| {
                    let gid = project.plot_table(table, CurveKind::Line)?;
                    prefs.apply_new_graph(project, gid);
                    Ok(gid)
                }) {
                    app.open_tab(crate::app::Tab::Graph(gid));
                }
            }
            app.status = "计算完成，结果在新表格和新图里".into();
            return false;
        }
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn function_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    graph: Option<sciplot_core::GraphId>,
    layer: usize,
    expr: &mut String,
    xmin: &mut String,
    xmax: &mut String,
    ymin: &mut String,
    ymax: &mut String,
    samples: &mut String,
    surface: &mut bool,
    mode: &mut SurfaceMode,
) -> bool {
    ui.checkbox(surface, "画成三维曲面（否则是 y=f(x)）");
    labeled(ui, "表达式", expr);
    labeled(ui, "x 最小", xmin);
    labeled(ui, "x 最大", xmax);
    if *surface {
        labeled(ui, "y 最小", ymin);
        labeled(ui, "y 最大", ymax);
        egui::ComboBox::from_id_salt("surf-mode").selected_text(mode.label()).show_ui(ui, |ui| {
            for item in SurfaceMode::all() {
                ui.selectable_value(mode, *item, item.label());
            }
        });
    } else {
        labeled(ui, "采样点数", samples);
    }
    if ui.button("绘制").clicked() {
        let expr = expr.clone();
        let x0 = parse_opt(xmin).unwrap_or(0.0);
        let x1 = parse_opt(xmax).unwrap_or(1.0);
        if *surface {
            let y0 = parse_opt(ymin).unwrap_or(-std::f64::consts::PI);
            let y1 = parse_opt(ymax).unwrap_or(std::f64::consts::PI);
            let mode = *mode;
            let prefs = app.prefs.clone();
            if let Some(id) = app.edit("函数曲面", |project| {
                let id = project.plot_surface(
                    SurfaceSource::Function { expr: expr.clone(), x_min: x0, x_max: x1, y_min: y0, y_max: y1, n: 40 },
                    mode,
                    "函数曲面",
                );
                prefs.apply_new_graph(project, id);
                Ok(id)
            }) {
                app.open_tab(crate::app::Tab::Graph(id));
                app.status = "已新建函数曲面".into();
                return false;
            }
        } else {
            let samples = samples.parse::<usize>().unwrap_or(200).clamp(8, 2000);
            let kind = CurveKind::Function;
            let prefs = app.prefs.clone();
            let before = graph.and_then(|id| app.project.graph(id).and_then(|item| item.layers.get(layer)).map(|item| item.curves.len()));
            if let Some(id) = app.edit("函数曲线", |project| {
                let graph_id = match graph {
                    Some(id) => id,
                    None => project.new_graph("函数曲线"),
                };
                let layer_ref = project.graph_mut(graph_id).ok_or("找不到图形")?.layers.get_mut(layer).ok_or("找不到图层")?;
                let style = CurveStyle::from_index(layer_ref.curves.len());
                layer_ref.curves.push(sciplot_core::Curve {
                    name: expr.clone(),
                    kind,
                    style,
                    source: DataSource::Function { expr: expr.clone(), x_min: x0, x_max: x1, samples },
                    visible: true,
                });
                if let Some(start) = before {
                    prefs.apply_curves(project, graph_id, layer, start);
                } else {
                    prefs.apply_new_graph(project, graph_id);
                }
                Ok(graph_id)
            }) {
                app.open_tab(crate::app::Tab::Graph(id));
                app.status = "已添加函数曲线".into();
                return false;
            }
        }
    }
    true
}

fn add_curve(app: &mut SciPlotApp, ui: &mut egui::Ui, graph: sciplot_core::GraphId, table: &mut sciplot_core::TableId, kind: &mut CurveKind) -> bool {
    let tables: Vec<(sciplot_core::TableId, String)> = app.project.tables.iter().map(|t| (t.id, t.name.clone())).collect();
    if tables.is_empty() {
        ui.label("没有表格");
        return true;
    }
    if !tables.iter().any(|(id, _)| id == table) {
        *table = tables[0].0;
    }
    let selected = tables.iter().find(|(id, _)| id == table).map(|(_, name)| name.clone()).unwrap_or_default();
    egui::ComboBox::from_id_salt("add-table").selected_text(selected).show_ui(ui, |ui| {
        for (id, name) in &tables {
            ui.selectable_value(table, *id, name);
        }
    });
    egui::ComboBox::from_id_salt("add-kind").selected_text(kind.label()).show_ui(ui, |ui| {
        for item in plot_kinds() {
            ui.selectable_value(kind, *item, item.label());
        }
    });
    if ui.button("添加").clicked() {
        let table = *table;
        let kind = *kind;
        let before = app.project.graph(graph).and_then(|item| item.layers.first()).map(|layer| layer.curves.len()).unwrap_or(0);
        let prefs = app.prefs.clone();
        if app
            .edit("添加曲线", |project| {
                project.add_curves(graph, 0, table, kind)?;
                prefs.apply_curves(project, graph, 0, before);
                Ok(())
            })
            .is_some()
        {
            app.status = "已把曲线加到当前图".into();
            return false;
        }
    }
    true
}

fn plot_kinds() -> &'static [CurveKind] {
    &[
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
    ]
}

fn curve_names(app: &SciPlotApp, graph: sciplot_core::GraphId, layer: usize) -> Vec<String> {
    app.project.graph(graph).and_then(|g| g.layers.get(layer)).map(|layer| layer.curves.iter().map(|c| c.name.clone()).collect()).unwrap_or_default()
}

fn build_input(app: &SciPlotApp, graph: sciplot_core::GraphId, layer: usize, kind: LineChoice, curve: usize, value: &str) -> Result<sciplot_analysis::CurveInput, String> {
    match kind {
        LineChoice::Horizontal => parse_opt(value).map(ops::horizontal_line).ok_or_else(|| "请输入水平线的 y 值".into()),
        LineChoice::Vertical => parse_opt(value).map(ops::vertical_line).ok_or_else(|| "请输入竖直线的 x 值".into()),
        LineChoice::Curve => {
            let curve = app.project.graph(graph).and_then(|g| g.layers.get(layer)).and_then(|layer| layer.curves.get(curve)).ok_or("找不到曲线")?;
            ops::curve_input(&app.project, curve)
        }
    }
}

fn push_annotation(project: &mut sciplot_core::Project, graph: sciplot_core::GraphId, layer: usize, kind: AnnotationKind) -> Result<(), String> {
    let id = project.alloc_annotation_id();
    let layer = project.graph_mut(graph).ok_or("找不到图形")?.layers.get_mut(layer).ok_or("找不到图层")?;
    layer.annotations.push(Annotation { id, kind });
    Ok(())
}

fn labeled(ui: &mut egui::Ui, label: &str, text: &mut String) {
    ui.label(label);
    ui.text_edit_singleline(text);
}

fn parse_opt(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() {
        None
    } else {
        text.parse::<f64>().ok().filter(|v| v.is_finite())
    }
}

pub fn number_text(value: Option<f64>) -> String {
    value.map(format_number).unwrap_or_default()
}
