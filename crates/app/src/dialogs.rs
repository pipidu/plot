use sciplot_analysis::{FitModel, InterpMethod};
use sciplot_core::{
    format_number, parse_initials, Annotation, AnnotationKind, AxisScale, ColumnMode, ColumnType,
    CurveKind, CurveStyle, Dash, DataSource, LegendPos, SurfaceMode, SurfaceSource, Symbol,
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
    if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
        open = false;
        app.pending_after = None;
        app.confirm_style = false;
    }
    let mut stay = true;
    let screen = ctx.screen_rect();
    let pos = egui::pos2((screen.center().x - 240.0).max(24.0), screen.top() + 64.0);
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("sciplot-enter-used"), false));
    let mut frame = egui::Frame::window(&ctx.style());
    frame.inner_margin = egui::Margin::same(16);
    egui::Window::new(title)
        .id(egui::Id::new("sciplot-dialog"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_pos(pos)
        .default_width(480.0)
        .frame(frame)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(12.0, 8.0);
            ui.spacing_mut().button_padding = egui::vec2(12.0, 6.0);
            let panel = ui.visuals().panel_fill;
            let extreme = ui.visuals().extreme_bg_color;
            ui.visuals_mut().extreme_bg_color = extreme.lerp_to_gamma(panel, 0.45);
            stay = ui_of(app, ui, &mut dialog);
        });
    if open && stay {
        app.dialog = Some(dialog);
    } else {
        if matches!(dialog, Dialog::Prefs(_)) {
            sciplot_core::set_language(app.prefs.language);
            ctx.request_repaint();
        }
        app.confirm_style = false;
    }
}

fn action_button(ui: &mut egui::Ui, label: &str) -> bool {
    let clicked = ui
        .add(egui::Button::new(label).min_size(egui::vec2(72.0, 28.0)))
        .clicked();
    let id = egui::Id::new("sciplot-enter-used");
    let popup = ui.memory(|memory| memory.any_popup_open());
    let enter = ui.input(|input| {
        input.key_pressed(egui::Key::Enter)
            && input.modifiers.matches_logically(egui::Modifiers::NONE)
    });
    let used = ui.data(|data| data.get_temp::<bool>(id).unwrap_or(false));
    if enter && !used && !popup {
        ui.data_mut(|data| data.insert_temp(id, true));
        true
    } else {
        clicked
    }
}

fn title_of(dialog: &Dialog) -> String {
    match dialog {
        Dialog::Message { title, .. } => title.clone(),
        Dialog::Unsaved { then } => match then {
            crate::app::AfterSave::Quit => sciplot_core::t("退出 SciPlot").into(),
            _ => sciplot_core::t("未保存的修改").into(),
        },
        Dialog::Help => sciplot_core::t("公式说明").into(),
        Dialog::About => sciplot_core::t("关于").into(),
        Dialog::Update(crate::update::UpdateView::Checking) => sciplot_core::t("检查更新").into(),
        Dialog::Update(crate::update::UpdateView::Current) => sciplot_core::t("检查更新").into(),
        Dialog::Update(crate::update::UpdateView::Newer { .. }) => {
            sciplot_core::t("发现新版本").into()
        }
        Dialog::Update(crate::update::UpdateView::Failed(_)) => sciplot_core::t("检查更新").into(),
        Dialog::Path { action, .. } => match action {
            PathAction::Open => sciplot_core::t("打开项目").into(),
            PathAction::SaveAs => sciplot_core::t("另存为").into(),
            PathAction::ImportCsv => sciplot_core::t("导入 CSV").into(),
            PathAction::ExportCsv => sciplot_core::t("导出 CSV").into(),
            PathAction::ExportPng => sciplot_core::t("导出 PNG").into(),
            PathAction::ExportSvg => sciplot_core::t("导出 SVG").into(),
        },
        Dialog::Formula { .. } => sciplot_core::t("列公式").into(),
        Dialog::MatrixFormula { .. } => sciplot_core::t("矩阵公式").into(),
        Dialog::NewMatrix { .. } => sciplot_core::t("新建矩阵").into(),
        Dialog::ColumnProps { .. } => sciplot_core::t("列类型与角色").into(),
        Dialog::Sort { .. } => sciplot_core::t("排序").into(),
        Dialog::Axis { .. } => sciplot_core::t("坐标轴").into(),
        Dialog::ConfirmDelete { .. } => sciplot_core::t("确认删除").into(),
        Dialog::Legend { .. } => sciplot_core::t("图例").into(),
        Dialog::Title { .. } => sciplot_core::t("标题").into(),
        Dialog::Style { .. } => sciplot_core::t("曲线样式").into(),
        Dialog::Frame { .. } => sciplot_core::t("图层位置").into(),
        Dialog::TextAnn { .. } => sciplot_core::t("文本标注").into(),
        Dialog::ArrowAnn { .. } => sciplot_core::t("箭头或线段").into(),
        Dialog::Intersect { .. } => sciplot_core::t("求交点").into(),
        Dialog::Fit { .. } => sciplot_core::t("拟合").into(),
        Dialog::Range { .. } => sciplot_core::t("分析").into(),
        Dialog::Function { .. } => sciplot_core::t("函数曲线或曲面").into(),
        Dialog::AddCurve { .. } => sciplot_core::t("添加到当前图").into(),
        Dialog::Prefs(_) => sciplot_core::t("默认样式与字体").into(),
        Dialog::ImportInstead { .. } => sciplot_core::t("这不是项目文件").into(),
    }
}

fn ui_of(app: &mut SciPlotApp, ui: &mut egui::Ui, dialog: &mut Dialog) -> bool {
    match dialog {
        Dialog::Message { title, text } => message_dialog(app, ui, title, text),
        Dialog::Help => {
            ui.label(i18n::formula_help());
            button_closes(ui, sciplot_core::t("关闭"))
        }
        Dialog::About => about_dialog(app, ui),
        Dialog::Update(view) => update_dialog(ui, view),
        Dialog::Unsaved { then } => unsaved(app, ui, then),
        Dialog::Path {
            action,
            path,
            extra,
        } => path_dialog(app, ui, *action, path, extra),
        Dialog::Formula {
            table,
            column,
            expr,
            error,
            warning,
        } => formula(app, ui, *table, *column, expr, error, warning),
        Dialog::MatrixFormula { matrix, expr } => matrix_formula(app, ui, *matrix, expr),
        Dialog::NewMatrix { name, rows, cols } => new_matrix(app, ui, name, rows, cols),
        Dialog::ColumnProps { table, column } => column_props(app, ui, *table, *column),
        Dialog::Sort {
            table,
            column,
            ascending,
        } => sort_dialog(app, ui, *table, column, ascending),
        Dialog::Axis {
            graph,
            layer,
            y_axis,
            axis,
            min,
            max,
            step,
            minor,
            error,
        } => axis_dialog(
            app, ui, *graph, *layer, *y_axis, axis, min, max, step, minor, error,
        ),
        Dialog::ConfirmDelete { target, detail } => confirm_delete(app, ui, target, detail),
        Dialog::Legend {
            graph,
            layer,
            legend,
        } => legend_dialog(app, ui, *graph, *layer, legend),
        Dialog::Title {
            graph,
            layer,
            title,
            size,
        } => title_dialog(app, ui, *graph, *layer, title, size),
        Dialog::Style {
            graph,
            layer,
            curve,
            style,
            name,
        } => style_dialog(app, ui, *graph, *layer, *curve, style, name),
        Dialog::Frame {
            graph,
            layer,
            x,
            y,
            w,
            h,
        } => frame_dialog(app, ui, *graph, *layer, x, y, w, h),
        Dialog::TextAnn {
            graph,
            layer,
            x,
            y,
            text,
        } => text_ann(app, ui, *graph, *layer, x, y, text),
        Dialog::ArrowAnn {
            graph,
            layer,
            x1,
            y1,
            x2,
            y2,
            line_only,
        } => arrow_ann(app, ui, *graph, *layer, x1, y1, x2, y2, *line_only),
        Dialog::Intersect {
            graph,
            layer,
            curve_a,
            curve_b,
            kind_a,
            kind_b,
            value_a,
            value_b,
            smooth,
            xmin,
            xmax,
            tol,
        } => intersect_dialog(
            app, ui, *graph, *layer, curve_a, curve_b, kind_a, kind_b, value_a, value_b, smooth,
            xmin, xmax, tol,
        ),
        Dialog::Fit {
            graph,
            layer,
            curve,
            which,
            degree,
            expr,
            initial,
            xmin,
            xmax,
        } => fit_dialog(
            app, ui, *graph, *layer, curve, which, degree, expr, initial, xmin, xmax,
        ),
        Dialog::Range {
            graph,
            layer,
            curve,
            op,
            a,
            b,
            n,
            method,
            window,
            order,
            sg,
            highpass,
        } => range_dialog(
            app, ui, *graph, *layer, curve, *op, a, b, n, method, window, order, sg, highpass,
        ),
        Dialog::Function {
            graph,
            layer,
            expr,
            xmin,
            xmax,
            ymin,
            ymax,
            samples,
            surface,
            mode,
        } => function_dialog(
            app, ui, *graph, *layer, expr, xmin, xmax, ymin, ymax, samples, surface, mode,
        ),
        Dialog::AddCurve { graph, table, kind } => add_curve(app, ui, *graph, table, kind),
        Dialog::Prefs(prefs) => prefs_dialog(app, ui, prefs),
        Dialog::ImportInstead { path } => import_instead(app, ui, path),
    }
}

fn message_dialog(app: &mut SciPlotApp, ui: &mut egui::Ui, title: &str, text: &str) -> bool {
    ui.label(text);
    let mut stay = true;
    ui.horizontal(|ui| {
        if ui.button(sciplot_core::t("复制")).clicked() {
            match arboard::Clipboard::new().and_then(|mut clip| clip.set_text(text.to_string())) {
                Ok(()) => app.status = sciplot_core::t("已复制到剪贴板").into(),
                Err(err) => {
                    app.status = sciplot_core::trf!(
                        "剪贴板不可用：{err}",
                        "Clipboard is unavailable: {err}",
                        err = err
                    )
                }
            }
        }
        if title == sciplot_core::t("统计") && ui.button(sciplot_core::t("写入新表")).clicked()
        {
            if let Some(id) = app.edit(sciplot_core::t("写入统计"), |project| {
                Ok(stats_table(project, text))
            }) {
                app.open_tab(crate::app::Tab::Table(id));
                app.status = sciplot_core::t("统计已写入新表").into();
                stay = false;
            }
        }
        if ui.button(sciplot_core::t("关闭")).clicked() {
            stay = false;
        }
    });
    stay
}

fn stats_table(project: &mut sciplot_core::Project, text: &str) -> sciplot_core::TableId {
    let id = project.new_table(sciplot_core::t("统计"));
    let table = project.table_mut(id).unwrap();
    table.columns.clear();
    table.add_column(sciplot_core::t("项目"), sciplot_core::ColumnMode::X);
    table.add_column(sciplot_core::t("数值"), sciplot_core::ColumnMode::Y);
    for line in text.lines().skip(1) {
        let Some((name, value)) = line.trim().split_once('=') else {
            continue;
        };
        let row = table.nrows();
        table.ensure_rows(row + 1);
        table.columns[0].set_text(row, name.trim());
        table.columns[1].set_text(row, value.trim());
    }
    id
}

fn import_instead(app: &mut SciPlotApp, ui: &mut egui::Ui, path: &str) -> bool {
    ui.label(sciplot_core::t(
        "这个文件是表格，不是 SciPlot 项目（.splot）。",
    ));
    ui.label(sciplot_core::t("可以改成导入 CSV，成为一张新表。"));
    ui.label(path);
    let mut stay = true;
    ui.horizontal(|ui| {
        if action_button(ui, sciplot_core::t("导入为表格")) {
            let path = path.to_string();
            app.perform_path(PathAction::ImportCsv, &path, "");
            stay = false;
        }
        if ui.button(sciplot_core::t("取消")).clicked() {
            stay = false;
        }
    });
    stay
}

fn about_dialog(app: &mut SciPlotApp, ui: &mut egui::Ui) -> bool {
    ui.heading(format!("SciPlot {}", i18n::current_version()));
    ui.label(i18n::about());
    ui.horizontal(|ui| {
        ui.label(sciplot_core::t("仓库"));
        ui.hyperlink_to(i18n::REPO_URL, i18n::REPO_URL);
    });
    let mut stay = true;
    ui.horizontal(|ui| {
        if ui.button(sciplot_core::t("检查更新")).clicked() {
            crate::update::start(app);
            stay = false;
        }
        if ui.button(sciplot_core::t("关闭")).clicked() {
            stay = false;
        }
    });
    stay
}

fn update_dialog(ui: &mut egui::Ui, view: &crate::update::UpdateView) -> bool {
    match view {
        crate::update::UpdateView::Checking => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(sciplot_core::t("正在向 GitHub 查询最新版本…"));
            });
            !ui.button(sciplot_core::t("取消")).clicked()
        }
        crate::update::UpdateView::Current => {
            ui.label(sciplot_core::trf!(
                "当前已是最新版本 {}。",
                "Already up to date ({}).",
                i18n::current_version()
            ));
            button_closes(ui, sciplot_core::t("关闭"))
        }
        crate::update::UpdateView::Newer {
            version,
            page,
            notes,
        } => {
            ui.label(sciplot_core::trf!(
                "发现新版本 {version}，当前是 {}。",
                "Version {version} is available. This copy is {}.",
                i18n::current_version(),
                version = version
            ));
            if !notes.is_empty() {
                egui::ScrollArea::vertical()
                    .max_height(240.0)
                    .show(ui, |ui| {
                        ui.add(egui::Label::new(notes).wrap_mode(egui::TextWrapMode::Wrap));
                    });
            }
            let mut stay = true;
            ui.horizontal(|ui| {
                if ui.button(sciplot_core::t("打开发布页")).clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(page));
                }
                if ui.button(sciplot_core::t("关闭")).clicked() {
                    stay = false;
                }
            });
            stay
        }
        crate::update::UpdateView::Failed(message) => {
            ui.colored_label(egui::Color32::from_rgb(176, 32, 32), message);
            ui.hyperlink_to(sciplot_core::t("打开发布页"), i18n::RELEASES_URL);
            button_closes(ui, sciplot_core::t("关闭"))
        }
    }
}

fn button_closes(ui: &mut egui::Ui, label: &str) -> bool {
    !ui.button(label).clicked()
}

fn prefs_dialog(app: &mut SciPlotApp, ui: &mut egui::Ui, prefs: &mut crate::prefs::Prefs) -> bool {
    egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
        ui.label(sciplot_core::t("设置保存在本机，不写入项目。之后新建的曲线、标题和坐标轴文字会使用这里的样式。"));
        ui.separator();
        ui.strong(sciplot_core::t("语言"));
        ui.horizontal(|ui| {
            for language in sciplot_core::Language::all() {
                if ui.selectable_value(&mut prefs.language, language, language.label()).changed() {
                    sciplot_core::set_language(prefs.language);
                }
            }
        });
        ui.separator();
        ui.strong(sciplot_core::t("字体"));
        ui.label(crate::fonts::automatic_plot_label());
        ui.label(sciplot_core::t("留空则自动选择。指定文件后，界面和图都改用这个文件。SVG 按 Times New Roman 和宋体的字体名显示。"));
        ui.horizontal_wrapped(|ui| {
            if ui.selectable_label(prefs.font_path.is_empty(), sciplot_core::t("自动")).clicked() {
                prefs.font_path.clear();
            }
            for (name, path) in crate::fonts::known_fonts() {
                if ui.selectable_label(prefs.font_path == path, sciplot_core::t(name)).clicked() {
                    prefs.font_path = path.to_string();
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label(sciplot_core::t("字体文件"));
            ui.add(egui::TextEdit::singleline(&mut prefs.font_path).desired_width(280.0));
            if std::env::var("SCIPLOT_NO_DIALOG").is_err() && ui.button(sciplot_core::t("浏览…")).clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter(sciplot_core::t("字体"), &["ttf", "ttc", "otf"]).pick_file() {
                    prefs.font_path = path.display().to_string();
                }
            }
        });
        ui.separator();
        ui.strong(sciplot_core::t("文字"));
        size_row(ui, sciplot_core::t("标题"), &mut prefs.title_size, 8.0, 72.0);
        size_row(ui, sciplot_core::t("坐标轴标题"), &mut prefs.axis_label_size, 8.0, 48.0);
        size_row(ui, sciplot_core::t("刻度"), &mut prefs.tick_size, 8.0, 36.0);
        ui.separator();
        ui.strong(sciplot_core::t("曲线"));
        size_row(ui, sciplot_core::t("线宽"), &mut prefs.line_width, 0.4, 12.0);
        ui.horizontal_wrapped(|ui| {
            for item in Dash::all() {
                ui.selectable_value(&mut prefs.dash, *item, item.label());
            }
        });
        ui.horizontal_wrapped(|ui| {
            for item in Symbol::all() {
                ui.selectable_value(&mut prefs.symbol, *item, item.label());
            }
        });
        size_row(ui, sciplot_core::t("符号大小"), &mut prefs.symbol_size, 1.0, 24.0);
        let mut bins = prefs.histogram_bins as f32;
        ui.horizontal(|ui| {
            ui.label(sciplot_core::t("直方图组数"));
            if ui.add(egui::DragValue::new(&mut bins).range(2.0..=200.0)).changed() {
                prefs.histogram_bins = bins.round() as usize;
            }
        });
    });
    ui.separator();
    ui.checkbox(
        &mut prefs.use_palette,
        sciplot_core::t("按曲线序号使用调色板"),
    );
    if !prefs.use_palette {
        ui.horizontal(|ui| {
            ui.label(sciplot_core::t("固定颜色"));
            ui.color_edit_button_srgba_unmultiplied(&mut prefs.color);
        });
    }
    ui.label(sciplot_core::t(
        "「用到当前图」会改掉正在看的这张图，和「保存」是两件事。",
    ));
    if app.confirm_style {
        ui.colored_label(
            egui::Color32::from_rgb(140, 90, 0),
            sciplot_core::t("确定把默认样式套到当前图吗？已有曲线的线宽、颜色和字号会被换掉。"),
        );
    }
    let mut stay = true;
    ui.horizontal(|ui| {
        if action_button(ui, sciplot_core::t("保存")) {
            store_prefs(app, prefs, ui.ctx());
            app.confirm_style = false;
            stay = false;
        }
        if ui.button(sciplot_core::t("关闭")).clicked() {
            sciplot_core::set_language(app.prefs.language);
            app.confirm_style = false;
            stay = false;
        }
    });
    if app.confirm_style {
        if action_button(ui, sciplot_core::t("确定套用到当前图")) {
            store_prefs(app, prefs, ui.ctx());
            if let Some(id) = app.active_graph() {
                let stored = app.prefs.clone();
                app.edit(sciplot_core::t("应用默认样式"), |project| {
                    stored.apply_new_graph(project, id);
                    Ok(())
                });
                app.status = sciplot_core::t("已把默认样式用到当前图").into();
            } else {
                app.status = sciplot_core::t("当前没有打开的图形").into();
            }
            app.confirm_style = false;
            stay = false;
        }
    } else if ui.button(sciplot_core::t("用到当前图…")).clicked() {
        app.confirm_style = true;
    }
    stay
}

fn store_prefs(app: &mut SciPlotApp, prefs: &mut crate::prefs::Prefs, ctx: &egui::Context) {
    prefs.clamp();
    app.prefs = prefs.clone();
    let saved = crate::prefs::save(&app.prefs);
    if app.reload_font(ctx) {
        app.status = if saved {
            sciplot_core::t("已保存默认样式和字体").into()
        } else {
            sciplot_core::t("样式已应用，但没能写入设置文件").into()
        };
    } else {
        app.prefs.font_path.clear();
        prefs.font_path.clear();
        crate::prefs::save(&app.prefs);
        app.reload_font(ctx);
        app.status = sciplot_core::t("找不到指定字体，已改用自动字体并保存其余设置").into();
    }
}

fn size_row(ui: &mut egui::Ui, label: &str, value: &mut f32, min: f32, max: f32) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).range(min..=max).speed(0.1));
    });
}

fn unsaved(app: &mut SciPlotApp, ui: &mut egui::Ui, then: &crate::app::AfterSave) -> bool {
    let quitting = matches!(then, crate::app::AfterSave::Quit);
    ui.label(if quitting {
        sciplot_core::t("当前项目有未保存的修改。要先保存再退出吗？")
    } else {
        sciplot_core::t("当前项目有未保存的修改。")
    });
    let mut stay = true;
    ui.horizontal(|ui| {
        let save_label = if quitting {
            sciplot_core::t("保存并退出")
        } else {
            sciplot_core::t("保存")
        };
        let drop_label = if quitting {
            sciplot_core::t("不保存退出")
        } else {
            sciplot_core::t("不保存")
        };
        if action_button(ui, save_label) {
            let saved = app.save_current();
            let asking_path = matches!(app.dialog, Some(Dialog::Path { .. }));
            if saved || asking_path {
                app.pending_after = Some(then.clone());
                stay = false;
            }
        }
        if ui.button(drop_label).clicked() {
            app.dirty = false;
            app.pending_after = Some(then.clone());
            stay = false;
        }
        if ui.button(sciplot_core::t("取消")).clicked() {
            app.pending_after = None;
            stay = false;
        }
    });
    stay
}

fn path_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    action: PathAction,
    path: &mut String,
    extra: &mut String,
) -> bool {
    ui.label(sciplot_core::t("路径"));
    ui.add(egui::TextEdit::singleline(path).desired_width(ui.available_width().max(420.0)));
    if std::env::var("SCIPLOT_NO_DIALOG").is_err() && ui.button(sciplot_core::t("浏览…")).clicked()
    {
        let name = std::path::Path::new(path.as_str())
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        if let Some(picked) = crate::app::native_dialog(app, action, &name) {
            *path = picked.display().to_string();
        }
    }
    if matches!(action, PathAction::ImportCsv | PathAction::ExportCsv) {
        ui.label(if matches!(action, PathAction::ImportCsv) {
            sciplot_core::t("编码。自动识别 UTF-8、带 BOM 或含空字节的 UTF-16，以及 GBK。")
        } else {
            sciplot_core::t("编码。自动和 UTF-8 都会写出带 BOM 的 UTF-8，Excel 能直接打开。")
        });
        ui.horizontal_wrapped(|ui| {
            for item in sciplot_core::CsvEncoding::all() {
                ui.selectable_value(&mut app.prefs.csv_encoding, *item, item.label());
            }
        });
    }
    if matches!(action, PathAction::ExportPng) {
        ui.label(sciplot_core::t("分辨率 DPI"));
        ui.text_edit_singleline(extra);
    }
    let target = crate::app::resolve_user_path(app, path);
    let verb = if matches!(action, PathAction::Open | PathAction::ImportCsv) {
        sciplot_core::t("将读取")
    } else {
        sciplot_core::t("将写入")
    };
    ui.label(format!("{verb}：{}", target.display()));
    let folder = target.is_dir();
    if folder {
        ui.colored_label(
            egui::Color32::from_rgb(140, 90, 0),
            sciplot_core::t("这是文件夹。请在路径末尾写上文件名。"),
        );
    }
    let mut stay = true;
    ui.horizontal(|ui| {
        if action_button(ui, sciplot_core::t("确定")) {
            if folder {
                app.status = sciplot_core::t("这是文件夹。请在路径末尾写上文件名。").into();
            } else {
                app.perform_path(action, path, extra);
                stay = false;
            }
        }
        if ui.button(sciplot_core::t("取消")).clicked() {
            app.status = sciplot_core::t("已取消").into();
            stay = false;
        }
    });
    stay
}

fn formula(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    table: sciplot_core::TableId,
    column: usize,
    expr: &mut String,
    error: &mut String,
    warning: &mut String,
) -> bool {
    let hint = formula_hint(app, table);
    ui.label(hint);
    if let Some(current) = app
        .project
        .table(table)
        .and_then(|item| item.columns.get(column))
    {
        ui.label(sciplot_core::trf!(
            "写入列：{}（{}）",
            "Write into column: {} ({})",
            current.name,
            current.ty.label()
        ));
    }
    if let Some(table) = app.project.table(table) {
        let listed: Vec<String> = table
            .columns
            .iter()
            .map(|column| format!("{}（{}）", column.name, column.ty.label()))
            .collect();
        if !listed.is_empty() {
            ui.label(sciplot_core::trf!(
                "可用的列：{}",
                "Columns: {}",
                listed.join("、")
            ));
        }
    }
    ui.label(sciplot_core::t("整数除法会截断，需要小数时写成 1.0/2。"));
    ui.text_edit_singleline(expr);
    if !error.is_empty() {
        ui.colored_label(egui::Color32::from_rgb(176, 32, 32), error.as_str());
    }
    if !warning.is_empty() {
        ui.colored_label(egui::Color32::from_rgb(140, 90, 0), warning.as_str());
    }
    let mut stay = true;
    ui.horizontal(|ui| {
        if action_button(ui, sciplot_core::t("计算并写入列")) {
            let formula = expr.clone();
            let outcome = app.edit(sciplot_core::t("列公式"), |project| {
                let table = project.table_mut(table).ok_or(sciplot_core::t("找不到表格"))?;
                sciplot_core::apply_formula(table, column, &formula)
            });
            match outcome {
                Some(evaluated) if evaluated.values.iter().all(|value| value.is_none()) => {
                    error.clear();
                    *warning = if let Some(err) = evaluated.first_error.as_deref() {
                        sciplot_core::explain_error(err)
                    } else {
                        sciplot_core::t("结果全为空：所引用的列可能是文本列，或列名不对。").into()
                    };
                    app.status = warning.clone();
                }
                Some(evaluated) => {
                    error.clear();
                    warning.clear();
                    let written = evaluated.values.iter().filter(|value| value.is_some()).count();
                    if evaluated.warnings > 0 {
                        let detail = evaluated.first_error.as_deref().map(sciplot_core::explain_error).unwrap_or_default();
                        *warning = if detail.is_empty() {
                            sciplot_core::trf!(
                                "已写入 {written} 行，有 {missed} 行没有得到数值。",
                                "Wrote {written} rows. {missed} rows did not produce a number.",
                                written = written,
                                missed = evaluated.warnings
                            )
                        } else {
                            sciplot_core::trf!(
                                "已写入 {written} 行，有 {missed} 行没有得到数值。{detail}",
                                "Wrote {written} rows. {missed} rows did not produce a number. {detail}",
                                written = written,
                                missed = evaluated.warnings,
                                detail = detail
                            )
                        };
                        app.status = warning.clone();
                    } else {
                        app.status = sciplot_core::trf!("已写入 {written} 行。", "Wrote {written} rows.", written = written);
                        stay = false;
                    }
                }
                None => {
                    warning.clear();
                    *error = sciplot_core::explain_error(&app.status);
                    app.status = error.clone();
                }
            }
        }
        if ui.button(sciplot_core::t("取消")).clicked() {
            stay = false;
        }
    });
    stay
}

fn formula_hint(app: &SciPlotApp, table: sciplot_core::TableId) -> String {
    let Some(table) = app.project.table(table) else {
        return sciplot_core::t("例如 col(\"X\")*2").into();
    };
    let numeric = table.columns.iter().find(|column| {
        column.ty == sciplot_core::ColumnType::Numeric
            && column.numbers.iter().any(|value| value.is_some())
    });
    let x_col = table.columns.iter().find(|column| {
        column.mode == sciplot_core::ColumnMode::X && column.ty == sciplot_core::ColumnType::Numeric
    });
    if let Some(column) = x_col.or(numeric) {
        sciplot_core::trf!(
            "例如 col(\"{}\")*2",
            "for example col(\"{}\")*2",
            column.name
        )
    } else if let Some(column) = table.columns.first() {
        sciplot_core::trf!(
            "这一列「{}」是{}，不能直接做乘法。",
            "Column \"{}\" is {}, so it cannot be multiplied directly.",
            column.name,
            column.ty.label()
        )
    } else {
        sciplot_core::t("例如 col(\"X\")*2").into()
    }
}

fn matrix_formula(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    matrix: sciplot_core::MatrixId,
    expr: &mut String,
) -> bool {
    ui.label(sciplot_core::t(
        "用含 x、y 的表达式填充每个单元格，例如 sin(x)*cos(y)。",
    ));
    ui.text_edit_singleline(expr);
    if action_button(ui, sciplot_core::t("填充")) {
        let formula = expr.clone();
        if app
            .edit(sciplot_core::t("矩阵公式"), |project| {
                project
                    .matrix_mut(matrix)
                    .ok_or(sciplot_core::t("找不到矩阵"))?
                    .fill_formula(&formula)
            })
            .is_some()
        {
            app.status = sciplot_core::t("矩阵已按公式填充").into();
            return false;
        }
    }
    !ui.button(sciplot_core::t("取消")).clicked()
}

fn new_matrix(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    name: &mut String,
    rows: &mut String,
    cols: &mut String,
) -> bool {
    ui.label(sciplot_core::t("名称"));
    ui.text_edit_singleline(name);
    ui.label(sciplot_core::t("行数"));
    ui.text_edit_singleline(rows);
    ui.label(sciplot_core::t("列数"));
    ui.text_edit_singleline(cols);
    if action_button(ui, sciplot_core::t("创建")) {
        let name = name.clone();
        let rows = rows.parse::<usize>().unwrap_or(10).clamp(1, 500);
        let cols = cols.parse::<usize>().unwrap_or(10).clamp(1, 500);
        if let Some(id) = app.edit(sciplot_core::t("新建矩阵"), |project| {
            Ok(project.new_matrix(&name, rows, cols))
        }) {
            app.open_tab(crate::app::Tab::Matrix(id));
            app.status = sciplot_core::t("已新建矩阵").into();
            return false;
        }
    }
    !ui.button(sciplot_core::t("取消")).clicked()
}

fn column_props(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    table: sciplot_core::TableId,
    column: usize,
) -> bool {
    let Some(current) = app
        .project
        .table(table)
        .and_then(|t| t.columns.get(column))
        .cloned()
    else {
        ui.label(sciplot_core::t("列不存在"));
        return true;
    };
    ui.label(sciplot_core::trf!(
        "当前：{} / {} / {}",
        "Current: {} / {} / {}",
        current.name,
        current.ty.label(),
        current.mode.label()
    ));
    ui.label(sciplot_core::t(
        "点下面的类型或角色，会立刻改这一列。名称可以在表头左边的表格名称旁改列名。",
    ));
    if ui
        .button(sciplot_core::trf!(
            "设为{}",
            "Set to {}",
            current.ty.label()
        ))
        .clicked()
    {
        apply_column(app, table, column, current.ty, None);
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
    !ui.button(sciplot_core::t("取消")).clicked()
}

fn apply_column(
    app: &mut SciPlotApp,
    table: sciplot_core::TableId,
    column: usize,
    ty: ColumnType,
    mode: Option<ColumnMode>,
) {
    if app
        .edit(sciplot_core::t("列属性"), |project| {
            let column = project
                .table_mut(table)
                .ok_or(sciplot_core::t("找不到表格"))?
                .columns
                .get_mut(column)
                .ok_or(sciplot_core::t("找不到列"))?;
            column.ty = ty;
            if let Some(mode) = mode {
                column.mode = mode;
            }
            Ok(())
        })
        .is_some()
    {
        app.status = sciplot_core::t("已更新列属性").into();
    }
}

fn sort_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    table: sciplot_core::TableId,
    column: &mut usize,
    ascending: &mut bool,
) -> bool {
    let names: Vec<String> = app
        .project
        .table(table)
        .map(|t| t.columns.iter().map(|c| c.name.clone()).collect())
        .unwrap_or_default();
    if names.is_empty() {
        ui.label(sciplot_core::t("表格没有列"));
        return true;
    }
    *column = (*column).min(names.len() - 1);
    egui::ComboBox::from_id_salt("sort-col")
        .selected_text(&names[*column])
        .show_ui(ui, |ui| {
            for (index, name) in names.iter().enumerate() {
                ui.selectable_value(column, index, name);
            }
        });
    ui.radio_value(ascending, true, sciplot_core::t("升序"));
    ui.radio_value(ascending, false, sciplot_core::t("降序"));
    if action_button(ui, sciplot_core::t("排序")) {
        let column = *column;
        let ascending = *ascending;
        if app
            .edit(sciplot_core::t("排序"), |project| {
                project
                    .table_mut(table)
                    .ok_or(sciplot_core::t("找不到表格"))?
                    .sort_by(column, ascending);
                Ok(())
            })
            .is_some()
        {
            app.status = sciplot_core::t("已排序").into();
            return false;
        }
    }
    !ui.button(sciplot_core::t("取消")).clicked()
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
    step: &mut String,
    minor: &mut String,
    error: &mut String,
) -> bool {
    ui.label(if y_axis {
        sciplot_core::t("纵轴")
    } else {
        sciplot_core::t("横轴")
    });
    ui.label(sciplot_core::t("标题"));
    ui.text_edit_singleline(&mut axis.label);
    egui::ComboBox::from_id_salt("axis-scale")
        .selected_text(axis.scale.label())
        .show_ui(ui, |ui| {
            for scale in AxisScale::all() {
                ui.selectable_value(&mut axis.scale, *scale, scale.label());
            }
        });
    ui.checkbox(&mut axis.major_grid, sciplot_core::t("主网格"));
    ui.checkbox(&mut axis.minor_grid, sciplot_core::t("次网格"));
    ui.checkbox(&mut axis.inverted, sciplot_core::t("反向"));
    ui.label(sciplot_core::t("最小值（空为自动）"));
    ui.text_edit_singleline(min);
    ui.label(sciplot_core::t("最大值（空为自动）"));
    ui.text_edit_singleline(max);
    ui.label(sciplot_core::t("主刻度步长（空为自动）"));
    ui.text_edit_singleline(step);
    if axis.scale == AxisScale::Log10 {
        ui.label(sciplot_core::t(
            "对数轴的步长是主刻度间隔的十进制数量级，例如 1 表示每十倍一条。",
        ));
    } else if axis.scale == AxisScale::Reciprocal {
        ui.label(sciplot_core::t("倒数轴的步长作用在 1/x 上。"));
    }
    ui.label(sciplot_core::t("次刻度等分（空为 4）"));
    ui.text_edit_singleline(minor);
    if !error.is_empty() {
        ui.colored_label(egui::Color32::from_rgb(176, 32, 32), error.as_str());
    }
    ui.horizontal(|ui| {
        if ui.button(sciplot_core::t("自动范围")).clicked() {
            min.clear();
            max.clear();
            step.clear();
            minor.clear();
            error.clear();
        }
        if action_button(ui, sciplot_core::t("应用")) {
            match read_axis(axis.scale, min, max, step, minor) {
                Ok(numbers) => {
                    error.clear();
                    axis.min = numbers.min;
                    axis.max = numbers.max;
                    axis.step = numbers.step;
                    axis.minor_count = numbers.minor;
                    let stored = axis.clone();
                    app.edit(sciplot_core::t("坐标轴"), |project| {
                        let layer = project
                            .graph_mut(graph)
                            .ok_or(sciplot_core::t("找不到图形"))?
                            .layers
                            .get_mut(layer)
                            .ok_or(sciplot_core::t("找不到图层"))?;
                        if y_axis {
                            layer.y_axis = stored.clone();
                        } else {
                            layer.x_axis = stored.clone();
                        }
                        Ok(())
                    });
                    clear_axis_view(app, graph, layer, y_axis);
                    app.status = sciplot_core::t("已更新坐标轴").into();
                }
                Err(message) => *error = message,
            }
        }
    });
    !ui.button(sciplot_core::t("取消")).clicked()
}

struct AxisNumbers {
    min: Option<f64>,
    max: Option<f64>,
    step: Option<f64>,
    minor: Option<u8>,
}

fn read_axis(
    scale: AxisScale,
    min: &str,
    max: &str,
    step: &str,
    minor: &str,
) -> Result<AxisNumbers, String> {
    let lo = parse_required(min, sciplot_core::t("最小值"))?;
    let hi = parse_required(max, sciplot_core::t("最大值"))?;
    let tick = parse_required(step, sciplot_core::t("主刻度步长"))?;
    let parts = parse_minor(minor)?;
    if let (Some(lo), Some(hi)) = (lo, hi) {
        if lo >= hi {
            return Err(sciplot_core::t("最小值必须小于最大值").into());
        }
    }
    if scale == AxisScale::Log10 {
        if lo.is_some_and(|value| value <= 0.0) || hi.is_some_and(|value| value <= 0.0) {
            return Err(sciplot_core::t("对数坐标的最小值和最大值都必须大于 0").into());
        }
        if tick.is_some_and(|value| value < 1.0) {
            return Err(sciplot_core::t("对数轴步长至少为 1，表示每隔几个十进制数量级").into());
        }
    }
    if scale == AxisScale::Reciprocal {
        if let (Some(lo), Some(hi)) = (lo, hi) {
            if lo < 0.0 && hi > 0.0 {
                return Err(sciplot_core::t("倒数坐标的最小值和最大值不能在 0 的两侧").into());
            }
            if lo == 0.0 || hi == 0.0 {
                return Err(sciplot_core::t("倒数坐标的范围不能包含 0").into());
            }
        }
    }
    if tick.is_some_and(|value| value <= 0.0) {
        return Err(sciplot_core::t("主刻度步长必须大于 0").into());
    }
    Ok(AxisNumbers {
        min: lo,
        max: hi,
        step: tick,
        minor: parts,
    })
}

fn parse_required(text: &str, label: &str) -> Result<Option<f64>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    text.parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .map(Some)
        .ok_or_else(|| {
            sciplot_core::trf!(
                "{label}不是有效数字",
                "{label} is not a number",
                label = label
            )
        })
}

fn parse_minor(text: &str) -> Result<Option<u8>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let value: u32 = text
        .parse()
        .map_err(|_| sciplot_core::t("次刻度等分必须是整数").to_string())?;
    if !(2..=20).contains(&value) {
        return Err(sciplot_core::t("次刻度等分请填 2 到 20").into());
    }
    Ok(Some(value as u8))
}

fn clear_axis_view(app: &mut SciPlotApp, graph: sciplot_core::GraphId, layer: usize, y_axis: bool) {
    let Some(views) = app.views.get_mut(&graph) else {
        return;
    };
    let Some(view) = views.get_mut(layer) else {
        return;
    };
    if y_axis {
        view.y_min = None;
        view.y_max = None;
    } else {
        view.x_min = None;
        view.x_max = None;
    }
}

fn confirm_delete(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    target: &crate::app::DeleteTarget,
    detail: &str,
) -> bool {
    ui.label(detail);
    ui.label(sciplot_core::t("确定要删除吗？"));
    let mut close = false;
    ui.horizontal(|ui| {
        if ui.button(sciplot_core::t("删除")).clicked() {
            let target = target.clone();
            crate::app::commit_delete_target(app, &target);
            close = true;
        }
        if ui.button(sciplot_core::t("取消")).clicked() {
            close = true;
        }
    });
    !close
}

fn legend_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    graph: sciplot_core::GraphId,
    layer: usize,
    legend: &mut sciplot_core::Legend,
) -> bool {
    ui.checkbox(&mut legend.visible, sciplot_core::t("显示图例"));
    egui::ComboBox::from_id_salt("legend-pos")
        .selected_text(legend.position.label())
        .show_ui(ui, |ui| {
            for pos in LegendPos::all() {
                ui.selectable_value(&mut legend.position, *pos, pos.label());
            }
        });
    if action_button(ui, sciplot_core::t("应用")) {
        let stored = legend.clone();
        app.edit(sciplot_core::t("图例"), |project| {
            project
                .graph_mut(graph)
                .ok_or(sciplot_core::t("找不到图形"))?
                .layers
                .get_mut(layer)
                .ok_or(sciplot_core::t("找不到图层"))?
                .legend = stored.clone();
            Ok(())
        });
        app.status = sciplot_core::t("已更新图例").into();
    }
    !ui.button(sciplot_core::t("取消")).clicked()
}

fn title_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    graph: sciplot_core::GraphId,
    layer: usize,
    title: &mut String,
    size: &mut String,
) -> bool {
    ui.label(sciplot_core::t("标题"));
    ui.text_edit_singleline(title);
    ui.label(sciplot_core::t("字号"));
    ui.text_edit_singleline(size);
    if action_button(ui, sciplot_core::t("应用")) {
        let title = title.clone();
        let size = size.parse::<f32>().unwrap_or(16.0).clamp(8.0, 72.0);
        app.edit(sciplot_core::t("标题"), |project| {
            let layer = project
                .graph_mut(graph)
                .ok_or(sciplot_core::t("找不到图形"))?
                .layers
                .get_mut(layer)
                .ok_or(sciplot_core::t("找不到图层"))?;
            layer.title = title.clone();
            layer.title_size = size;
            Ok(())
        });
        app.status = sciplot_core::t("已更新标题").into();
    }
    !ui.button(sciplot_core::t("取消")).clicked()
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
    ui.label(sciplot_core::t("名称"));
    ui.text_edit_singleline(name);
    egui::ComboBox::from_id_salt("dash")
        .selected_text(style.dash.label())
        .show_ui(ui, |ui| {
            for dash in Dash::all() {
                ui.selectable_value(&mut style.dash, *dash, dash.label());
            }
        });
    egui::ComboBox::from_id_salt("symbol")
        .selected_text(style.symbol.label())
        .show_ui(ui, |ui| {
            for symbol in Symbol::all() {
                ui.selectable_value(&mut style.symbol, *symbol, symbol.label());
            }
        });
    ui.add(egui::Slider::new(&mut style.line_width, 0.4..=8.0).text(sciplot_core::t("线宽")));
    ui.add(egui::Slider::new(&mut style.symbol_size, 2.0..=18.0).text(sciplot_core::t("符号")));
    let kind = app
        .project
        .graph(graph)
        .and_then(|item| item.layers.get(layer))
        .and_then(|item| item.curves.get(curve))
        .map(|item| item.kind);
    if kind == Some(CurveKind::Histogram) {
        ui.add(
            egui::Slider::new(&mut style.histogram_bins, 2..=80)
                .text(sciplot_core::t("直方图箱数")),
        );
    }
    color_sliders(ui, &mut style.color);
    if action_button(ui, sciplot_core::t("应用")) {
        let style = style.clone();
        let name = name.clone();
        app.edit(sciplot_core::t("曲线样式"), |project| {
            let curve = project
                .graph_mut(graph)
                .ok_or(sciplot_core::t("找不到图形"))?
                .layers
                .get_mut(layer)
                .ok_or(sciplot_core::t("找不到图层"))?
                .curves
                .get_mut(curve)
                .ok_or(sciplot_core::t("找不到曲线"))?;
            curve.style = style.clone();
            curve.name = name.clone();
            Ok(())
        });
        app.status = sciplot_core::t("已更新曲线样式").into();
    }
    !ui.button(sciplot_core::t("取消")).clicked()
}

fn color_sliders(ui: &mut egui::Ui, color: &mut [u8; 4]) {
    let mut r = color[0] as f32;
    let mut g = color[1] as f32;
    let mut b = color[2] as f32;
    ui.add(egui::Slider::new(&mut r, 0.0..=255.0).text(sciplot_core::t("红")));
    ui.add(egui::Slider::new(&mut g, 0.0..=255.0).text(sciplot_core::t("绿")));
    ui.add(egui::Slider::new(&mut b, 0.0..=255.0).text(sciplot_core::t("蓝")));
    color[0] = r.round() as u8;
    color[1] = g.round() as u8;
    color[2] = b.round() as u8;
}

#[allow(clippy::too_many_arguments)]
fn frame_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    graph: sciplot_core::GraphId,
    layer: usize,
    x: &mut String,
    y: &mut String,
    w: &mut String,
    h: &mut String,
) -> bool {
    ui.label(sciplot_core::t(
        "相对整张图的位置，左上角为原点，宽高是比例。",
    ));
    labeled(ui, "x", x);
    labeled(ui, "y", y);
    labeled(ui, sciplot_core::t("宽"), w);
    labeled(ui, sciplot_core::t("高"), h);
    if action_button(ui, sciplot_core::t("应用")) {
        let rect = sciplot_core::FrameRect {
            x: x.parse().unwrap_or(0.0),
            y: y.parse().unwrap_or(0.0),
            w: w.parse().unwrap_or(1.0),
            h: h.parse().unwrap_or(1.0),
        };
        app.edit(sciplot_core::t("图层位置"), |project| {
            project
                .graph_mut(graph)
                .ok_or(sciplot_core::t("找不到图形"))?
                .layers
                .get_mut(layer)
                .ok_or(sciplot_core::t("找不到图层"))?
                .frame = rect;
            Ok(())
        });
        app.status = sciplot_core::t("已更新图层位置").into();
    }
    !ui.button(sciplot_core::t("取消")).clicked()
}

fn text_ann(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    graph: sciplot_core::GraphId,
    layer: usize,
    x: &mut String,
    y: &mut String,
    text: &mut String,
) -> bool {
    labeled(ui, "x", x);
    labeled(ui, "y", y);
    ui.label(sciplot_core::t("文字"));
    ui.text_edit_singleline(text);
    if action_button(ui, sciplot_core::t("添加")) {
        let (Some(x), Some(y)) = (parse_opt(x), parse_opt(y)) else {
            app.status = sciplot_core::t("请输入数值坐标").into();
            return true;
        };
        let text = text.clone();
        app.edit(sciplot_core::t("文本标注"), |project| {
            push_annotation(
                project,
                graph,
                layer,
                AnnotationKind::Text {
                    x,
                    y,
                    text: text.clone(),
                    size: 14.0,
                    color: [20, 20, 20, 255],
                },
            )
        });
        app.status = sciplot_core::t("已添加文本").into();
        return false;
    }
    !ui.button(sciplot_core::t("取消")).clicked()
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
    if action_button(ui, sciplot_core::t("添加")) {
        let nums = [x1, y1, x2, y2].map(|text| parse_opt(text));
        if nums.iter().any(|v| v.is_none()) {
            app.status = sciplot_core::t("请输入数值坐标").into();
            return true;
        }
        let [x1, y1, x2, y2] = [
            nums[0].unwrap(),
            nums[1].unwrap(),
            nums[2].unwrap(),
            nums[3].unwrap(),
        ];
        let color = [180, 40, 40, 255];
        let kind = if line_only {
            AnnotationKind::Line {
                x1,
                y1,
                x2,
                y2,
                color,
            }
        } else {
            AnnotationKind::Arrow {
                x1,
                y1,
                x2,
                y2,
                color,
            }
        };
        app.edit(sciplot_core::t("标注"), |project| {
            push_annotation(project, graph, layer, kind.clone())
        });
        app.status = sciplot_core::t("已添加标注").into();
        return false;
    }
    !ui.button(sciplot_core::t("取消")).clicked()
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
    ui.label(sciplot_core::t("曲线 A"));
    choice_row(ui, "a", kind_a, curve_a, &names, value_a);
    ui.label(sciplot_core::t("曲线 B"));
    choice_row(ui, "b", kind_b, curve_b, &names, value_b);
    ui.checkbox(
        smooth,
        sciplot_core::t("平滑求根（样条或函数，否则按折线精确求交）"),
    );
    labeled_hint(
        ui,
        sciplot_core::t("X 最小（可空）"),
        xmin,
        sciplot_core::t("全部"),
    );
    labeled_hint(
        ui,
        sciplot_core::t("X 最大（可空）"),
        xmax,
        sciplot_core::t("全部"),
    );
    if !xmin.trim().is_empty() || !xmax.trim().is_empty() {
        ui.colored_label(
            egui::Color32::from_rgb(140, 90, 0),
            sciplot_core::t("将只在上面的 X 区间里求交。区间外的交点不会出现。"),
        );
        if ui.button(sciplot_core::t("改用全部范围")).clicked() {
            xmin.clear();
            xmax.clear();
        }
    }
    labeled(ui, sciplot_core::t("容差"), tol);
    if let Some(text) = constant_outside(*kind_a, value_a, *kind_b, value_b, xmin, xmax) {
        ui.colored_label(egui::Color32::from_rgb(140, 90, 0), text);
    }
    if action_button(ui, sciplot_core::t("计算")) {
        let built_a = build_input(app, graph, layer, *kind_a, *curve_a, value_a);
        let built_b = build_input(app, graph, layer, *kind_b, *curve_b, value_b);
        match (built_a, built_b) {
            (Ok(a), Ok(b)) => {
                let options = sciplot_analysis::IntersectOptions {
                    mode: if *smooth {
                        sciplot_analysis::IntersectMode::Smooth
                    } else {
                        sciplot_analysis::IntersectMode::Piecewise
                    },
                    x_min: parse_opt(xmin),
                    x_max: parse_opt(xmax),
                    tolerance: parse_opt(tol).unwrap_or(1e-8),
                };
                let name = sciplot_core::trf!("交点_{}_{}", "Intersect_{}_{}", a.name(), b.name());
                if let Some(count) = app.edit(sciplot_core::t("求交点"), |project| {
                    ops::apply_intersection(
                        project,
                        graph,
                        layer,
                        &name,
                        &ops_intersect(&a, &b, &options),
                    )
                }) {
                    if count == 0 {
                        app.status =
                            sciplot_core::t("没有找到交点，没有新建表格，也没有在图上标点").into();
                        return true;
                    }
                    app.status = sciplot_core::trf!("求得 {count} 个交点，已标在图上，并写入新表格", "Found {count} intersections, marked on the graph and written to a new table", count = count);
                    return false;
                }
            }
            (Err(err), _) | (_, Err(err)) => app.status = err,
        }
    }
    !ui.button(sciplot_core::t("取消")).clicked()
}

fn constant_outside(
    kind_a: LineChoice,
    value_a: &str,
    kind_b: LineChoice,
    value_b: &str,
    xmin: &str,
    xmax: &str,
) -> Option<String> {
    let lo = parse_opt(xmin)?;
    let hi = parse_opt(xmax)?;
    let (lo, hi) = (lo.min(hi), lo.max(hi));
    let mut notes = Vec::new();
    if kind_a == LineChoice::Vertical {
        if let Some(value) = parse_opt(value_a) {
            if value < lo || value > hi {
                notes.push(sciplot_core::trf!(
                    "竖直线 x={} 在区间 [{}, {}] 外面",
                    "Vertical line x={} is outside [{}, {}]",
                    format_number(value),
                    format_number(lo),
                    format_number(hi)
                ));
            }
        }
    }
    if kind_b == LineChoice::Vertical {
        if let Some(value) = parse_opt(value_b) {
            if value < lo || value > hi {
                notes.push(sciplot_core::trf!(
                    "竖直线 x={} 在区间 [{}, {}] 外面",
                    "Vertical line x={} is outside [{}, {}]",
                    format_number(value),
                    format_number(lo),
                    format_number(hi)
                ));
            }
        }
    }
    if notes.is_empty() {
        None
    } else {
        Some(notes.join("。"))
    }
}

fn ops_intersect(
    a: &sciplot_analysis::CurveInput,
    b: &sciplot_analysis::CurveInput,
    options: &sciplot_analysis::IntersectOptions,
) -> sciplot_analysis::IntersectOutput {
    sciplot_analysis::intersect(a, b, options)
}

fn choice_row(
    ui: &mut egui::Ui,
    salt: &str,
    kind: &mut LineChoice,
    curve: &mut usize,
    names: &[String],
    value: &mut String,
) {
    egui::ComboBox::from_id_salt(format!("kind-{salt}"))
        .selected_text(kind.label())
        .show_ui(ui, |ui| {
            for item in [
                LineChoice::Curve,
                LineChoice::Horizontal,
                LineChoice::Vertical,
            ] {
                ui.selectable_value(kind, item, item.label());
            }
        });
    if *kind == LineChoice::Curve {
        if names.is_empty() {
            ui.label(sciplot_core::t("这一层没有曲线"));
        } else {
            *curve = (*curve).min(names.len() - 1);
            egui::ComboBox::from_id_salt(format!("curve-{salt}"))
                .selected_text(&names[*curve])
                .show_ui(ui, |ui| {
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
    xmin: &mut String,
    xmax: &mut String,
) -> bool {
    let names = curve_names(app, graph, layer);
    if names.is_empty() {
        ui.label(sciplot_core::t("没有可拟合的曲线"));
        return true;
    }
    *curve = (*curve).min(names.len() - 1);
    egui::ComboBox::from_id_salt("fit-curve")
        .selected_text(&names[*curve])
        .show_ui(ui, |ui| {
            for (index, name) in names.iter().enumerate() {
                ui.selectable_value(curve, index, name);
            }
        });
    egui::ComboBox::from_id_salt("fit-model")
        .selected_text(which.label())
        .show_ui(ui, |ui| {
            for item in [
                FitWhich::Linear,
                FitWhich::Poly,
                FitWhich::Exp,
                FitWhich::Gauss,
                FitWhich::Lorentz,
                FitWhich::Custom,
            ] {
                ui.selectable_value(which, item, item.label());
            }
        });
    if *which == FitWhich::Poly {
        labeled(ui, sciplot_core::t("次数"), degree);
    }
    if *which == FitWhich::Custom {
        labeled(ui, sciplot_core::t("表达式，例如 a*exp(-b*x)"), expr);
        labeled(ui, sciplot_core::t("初值，例如 a=1, b=0.5"), initial);
    }
    ui.label(sciplot_core::t(
        "X 区间留空表示用全部点。图上 Shift 拖出的区间会填在这里。",
    ));
    labeled_hint(
        ui,
        sciplot_core::t("X 最小（可空）"),
        xmin,
        sciplot_core::t("全部"),
    );
    labeled_hint(
        ui,
        sciplot_core::t("X 最大（可空）"),
        xmax,
        sciplot_core::t("全部"),
    );
    if action_button(ui, sciplot_core::t("拟合")) {
        let model = match which {
            FitWhich::Linear => Ok(FitModel::Linear),
            FitWhich::Poly => degree
                .parse::<usize>()
                .map(|d| FitModel::Polynomial(d.clamp(1, 8)))
                .map_err(|_| sciplot_core::t("次数无效").to_string()),
            FitWhich::Exp => Ok(FitModel::Exponential),
            FitWhich::Gauss => Ok(FitModel::Gaussian),
            FitWhich::Lorentz => Ok(FitModel::Lorentzian),
            FitWhich::Custom => parse_initials(initial).map(|initial| FitModel::Custom {
                expr: expr.clone(),
                initial,
            }),
        };
        match model {
            Ok(model) => {
                let curve = *curve;
                let x_min = parse_opt(xmin);
                let x_max = parse_opt(xmax);
                let title = sciplot_core::trf!("{}拟合结果", "{} fit", which.label());
                let before = app
                    .project
                    .graph(graph)
                    .and_then(|item| item.layers.get(layer))
                    .map(|item| item.curves.len())
                    .unwrap_or(0);
                let prefs = app.prefs.clone();
                if let Some(report) = app.edit(sciplot_core::t("拟合"), |project| {
                    let report =
                        ops::fit_on_graph(project, graph, layer, curve, &model, x_min, x_max)?;
                    prefs.apply_curves(project, graph, layer, before);
                    Ok(report)
                }) {
                    app.dialog = Some(Dialog::Message {
                        title,
                        text: report,
                    });
                    return false;
                }
            }
            Err(err) => app.status = err,
        }
    }
    !ui.button(sciplot_core::t("取消")).clicked()
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
        egui::ComboBox::from_id_salt("range-curve")
            .selected_text(&names[*curve])
            .show_ui(ui, |ui| {
                for (index, name) in names.iter().enumerate() {
                    ui.selectable_value(curve, index, name);
                }
            });
    }
    match op {
        RangeOp::Integral => {
            labeled(ui, sciplot_core::t("下限"), a);
            labeled(ui, sciplot_core::t("上限"), b);
        }
        RangeOp::Interp => {
            egui::ComboBox::from_id_salt("interp")
                .selected_text(method.label())
                .show_ui(ui, |ui| {
                    for item in [
                        InterpMethod::Linear,
                        InterpMethod::CubicSpline,
                        InterpMethod::Akima,
                    ] {
                        ui.selectable_value(method, item, item.label());
                    }
                });
            labeled(ui, sciplot_core::t("点数"), n);
        }
        RangeOp::Smooth => {
            ui.checkbox(sg, sciplot_core::t("Savitzky-Golay（否则移动平均）"));
            labeled(ui, sciplot_core::t("窗口（奇数）"), window);
            labeled(ui, sciplot_core::t("多项式次数"), order);
        }
        RangeOp::Filter => {
            labeled(ui, sciplot_core::t("截止频率"), a);
            ui.checkbox(highpass, sciplot_core::t("高通"));
        }
    }
    if action_button(ui, sciplot_core::t("计算")) {
        let curve = *curve;
        let done = match op {
            RangeOp::Integral => {
                let (Some(xmin), Some(xmax)) = (parse_opt(a), parse_opt(b)) else {
                    app.status = sciplot_core::t("请输入积分区间").into();
                    return true;
                };
                app.edit(sciplot_core::t("积分"), |project| {
                    ops::integral_report(project, graph, layer, curve, xmin, xmax).map(|_| ())
                })
            }
            RangeOp::Interp => {
                let method = *method;
                let n = n.parse::<usize>().unwrap_or(200);
                app.edit(sciplot_core::t("插值"), |project| {
                    ops::interpolate_table(project, graph, layer, curve, method, n).map(|_| ())
                })
            }
            RangeOp::Smooth => {
                let window = window.parse::<usize>().unwrap_or(5);
                let order = order.parse::<usize>().unwrap_or(2);
                let sg = *sg;
                app.edit(sciplot_core::t("平滑"), |project| {
                    ops::smooth_table(project, graph, layer, curve, sg, window, order).map(|_| ())
                })
            }
            RangeOp::Filter => {
                let Some(cutoff) = parse_opt(a) else {
                    app.status = sciplot_core::t("请输入截止频率").into();
                    return true;
                };
                let highpass = *highpass;
                app.edit(sciplot_core::t("滤波"), |project| {
                    ops::filter_table(project, graph, layer, curve, cutoff, highpass).map(|_| ())
                })
            }
        };
        if done.is_some() {
            if let Some(table) = app.project.tables.last().map(|t| t.id) {
                app.open_tab(crate::app::Tab::Table(table));
                let prefs = app.prefs.clone();
                if let Some(gid) = app.edit(sciplot_core::t("绘制结果"), |project| {
                    let gid = project.plot_table(table, CurveKind::Line)?;
                    prefs.apply_new_graph(project, gid);
                    Ok(gid)
                }) {
                    app.open_tab(crate::app::Tab::Graph(gid));
                }
            }
            app.status = sciplot_core::t("计算完成，结果在新表格和新图里").into();
            return false;
        }
    }
    !ui.button(sciplot_core::t("取消")).clicked()
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
    ui.checkbox(surface, sciplot_core::t("画成三维曲面（否则是 y=f(x)）"));
    labeled(ui, sciplot_core::t("表达式"), expr);
    labeled(ui, sciplot_core::t("x 最小"), xmin);
    labeled(ui, sciplot_core::t("x 最大"), xmax);
    if *surface {
        labeled(ui, sciplot_core::t("y 最小"), ymin);
        labeled(ui, sciplot_core::t("y 最大"), ymax);
        egui::ComboBox::from_id_salt("surf-mode")
            .selected_text(mode.label())
            .show_ui(ui, |ui| {
                for item in SurfaceMode::all() {
                    ui.selectable_value(mode, *item, item.label());
                }
            });
    } else {
        labeled(ui, sciplot_core::t("采样点数"), samples);
    }
    if action_button(ui, sciplot_core::t("绘制")) {
        let expr = expr.clone();
        let x0 = parse_opt(xmin).unwrap_or(0.0);
        let x1 = parse_opt(xmax).unwrap_or(1.0);
        if *surface {
            let y0 = parse_opt(ymin).unwrap_or(-std::f64::consts::PI);
            let y1 = parse_opt(ymax).unwrap_or(std::f64::consts::PI);
            let mode = *mode;
            let prefs = app.prefs.clone();
            if let Some(id) = app.edit(sciplot_core::t("函数曲面"), |project| {
                let id = project.plot_surface(
                    SurfaceSource::Function {
                        expr: expr.clone(),
                        x_min: x0,
                        x_max: x1,
                        y_min: y0,
                        y_max: y1,
                        n: 40,
                    },
                    mode,
                    sciplot_core::t("函数曲面"),
                );
                prefs.apply_new_graph(project, id);
                Ok(id)
            }) {
                app.open_tab(crate::app::Tab::Graph(id));
                app.status = sciplot_core::t("已新建函数曲面").into();
                return false;
            }
        } else {
            let samples = samples.parse::<usize>().unwrap_or(200).clamp(8, 2000);
            let kind = CurveKind::Function;
            let prefs = app.prefs.clone();
            let before = graph.and_then(|id| {
                app.project
                    .graph(id)
                    .and_then(|item| item.layers.get(layer))
                    .map(|item| item.curves.len())
            });
            if let Some(id) = app.edit(sciplot_core::t("函数曲线"), |project| {
                let graph_id = match graph {
                    Some(id) => id,
                    None => project.new_graph(sciplot_core::t("函数曲线")),
                };
                let layer_ref = project
                    .graph_mut(graph_id)
                    .ok_or(sciplot_core::t("找不到图形"))?
                    .layers
                    .get_mut(layer)
                    .ok_or(sciplot_core::t("找不到图层"))?;
                let style = CurveStyle::from_index(layer_ref.curves.len());
                layer_ref.curves.push(sciplot_core::Curve {
                    name: expr.clone(),
                    kind,
                    style,
                    source: DataSource::Function {
                        expr: expr.clone(),
                        x_min: x0,
                        x_max: x1,
                        samples,
                    },
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
                app.status = sciplot_core::t("已添加函数曲线").into();
                return false;
            }
        }
    }
    !ui.button(sciplot_core::t("取消")).clicked()
}

fn add_curve(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    graph: sciplot_core::GraphId,
    table: &mut sciplot_core::TableId,
    kind: &mut CurveKind,
) -> bool {
    let tables: Vec<(sciplot_core::TableId, String)> = app
        .project
        .tables
        .iter()
        .map(|t| (t.id, t.name.clone()))
        .collect();
    if tables.is_empty() {
        ui.label(sciplot_core::t("没有表格"));
        return true;
    }
    if !tables.iter().any(|(id, _)| id == table) {
        *table = tables[0].0;
    }
    let selected = tables
        .iter()
        .find(|(id, _)| id == table)
        .map(|(_, name)| name.clone())
        .unwrap_or_default();
    egui::ComboBox::from_id_salt("add-table")
        .selected_text(selected)
        .show_ui(ui, |ui| {
            for (id, name) in &tables {
                ui.selectable_value(table, *id, name);
            }
        });
    egui::ComboBox::from_id_salt("add-kind")
        .selected_text(kind.label())
        .show_ui(ui, |ui| {
            for item in plot_kinds() {
                ui.selectable_value(kind, *item, item.label());
            }
        });
    if action_button(ui, sciplot_core::t("添加")) {
        let table = *table;
        let kind = *kind;
        let before = app
            .project
            .graph(graph)
            .and_then(|item| item.layers.first())
            .map(|layer| layer.curves.len())
            .unwrap_or(0);
        let prefs = app.prefs.clone();
        if app
            .edit(sciplot_core::t("添加曲线"), |project| {
                project.add_curves(graph, 0, table, kind)?;
                prefs.apply_curves(project, graph, 0, before);
                Ok(())
            })
            .is_some()
        {
            app.status = sciplot_core::t("已把曲线加到当前图").into();
            return false;
        }
    }
    !ui.button(sciplot_core::t("取消")).clicked()
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
    app.project
        .graph(graph)
        .and_then(|g| g.layers.get(layer))
        .map(|layer| layer.curves.iter().map(|c| c.name.clone()).collect())
        .unwrap_or_default()
}

fn build_input(
    app: &SciPlotApp,
    graph: sciplot_core::GraphId,
    layer: usize,
    kind: LineChoice,
    curve: usize,
    value: &str,
) -> Result<sciplot_analysis::CurveInput, String> {
    match kind {
        LineChoice::Horizontal => parse_opt(value)
            .map(ops::horizontal_line)
            .ok_or_else(|| sciplot_core::t("请输入水平线的 y 值").into()),
        LineChoice::Vertical => parse_opt(value)
            .map(ops::vertical_line)
            .ok_or_else(|| sciplot_core::t("请输入竖直线的 x 值").into()),
        LineChoice::Curve => {
            let curve = app
                .project
                .graph(graph)
                .and_then(|g| g.layers.get(layer))
                .and_then(|layer| layer.curves.get(curve))
                .ok_or(sciplot_core::t("找不到曲线"))?;
            ops::curve_input(&app.project, curve)
        }
    }
}

fn push_annotation(
    project: &mut sciplot_core::Project,
    graph: sciplot_core::GraphId,
    layer: usize,
    kind: AnnotationKind,
) -> Result<(), String> {
    let id = project.alloc_annotation_id();
    let layer = project
        .graph_mut(graph)
        .ok_or(sciplot_core::t("找不到图形"))?
        .layers
        .get_mut(layer)
        .ok_or(sciplot_core::t("找不到图层"))?;
    layer.annotations.push(Annotation { id, kind });
    Ok(())
}

fn labeled(ui: &mut egui::Ui, label: &str, text: &mut String) {
    ui.label(label);
    ui.add(egui::TextEdit::singleline(text).desired_width(220.0));
}

fn labeled_hint(ui: &mut egui::Ui, label: &str, text: &mut String, hint: &str) {
    ui.label(label);
    ui.add(
        egui::TextEdit::singleline(text)
            .hint_text(hint)
            .desired_width(220.0),
    );
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
