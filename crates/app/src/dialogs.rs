use sciplot_analysis::{FitAlgorithm, FitModel, FitOptions, FftBand, FftShape, InterpMethod};
use sciplot_core::{
    format_number, parse_initials, Annotation, AnnotationKind, AxisScale, ColumnMode, ColumnType,
    CurveAxes, CurveKind, CurveStyle, Dash, DataSource, ErrorSource, FillPattern, LegendPos, StepAnchor,
    SurfaceMode, SurfaceSource, Symbol, VectorMode, WhiskerMode, XAxisSide, YAxisSide,
};

use crate::app::{
    AxisEdge, Dialog, FilterKind, FitWhich, FunctionForm, LineChoice, PathAction, RangeOp, SciPlotApp, SignalKind,
    WeightChoice,
};
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
            PathAction::Append => sciplot_core::t("追加项目…").into(),
            PathAction::SaveAs => sciplot_core::t("另存为").into(),
            PathAction::SaveCompressed => sciplot_core::t("另存为压缩项目…").into(),
            PathAction::ExportSciprj => sciplot_core::t("导出 SciDAVis 项目…").into(),
            PathAction::ImportCsv => sciplot_core::t("导入 CSV").into(),
            PathAction::ImportImage => sciplot_core::tr!("导入图像", "Import image").into(),
            PathAction::ExportCsv => sciplot_core::t("导出 CSV").into(),
            PathAction::ExportPng => sciplot_core::t("导出 PNG").into(),
            PathAction::ExportSvg => sciplot_core::t("导出 SVG").into(),
        },
        Dialog::Formula { .. } => sciplot_core::t("列公式").into(),
        Dialog::MatrixFormula { .. } => sciplot_core::t("矩阵公式").into(),
        Dialog::NewMatrix { .. } => sciplot_core::t("新建矩阵").into(),
        Dialog::ColumnProps { .. } => sciplot_core::t("列类型与角色").into(),
        Dialog::RenameColumn { .. } => sciplot_core::t("重命名列").into(),
        Dialog::Sort { .. } => sciplot_core::t("排序").into(),
        Dialog::Stats { .. } => sciplot_core::t("统计").into(),
        Dialog::Normalize { .. } => sciplot_core::t("归一化").into(),
        Dialog::CellFormula { .. } => sciplot_core::t("单元格公式").into(),
        Dialog::ImportAscii { .. } => sciplot_core::t("导入 ASCII").into(),
        Dialog::Axis { .. } => sciplot_core::t("坐标轴").into(),
        Dialog::Wizard { .. } => sciplot_core::tr!("绘图向导", "Plot wizard").into(),
        Dialog::Errors { .. } => sciplot_core::tr!("误差棒", "Error bars").into(),
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
        Dialog::Fft { inverse, .. } => {
            if *inverse {
                sciplot_core::tr!("逆 FFT", "Inverse FFT").into()
            } else {
                sciplot_core::tr!("FFT", "FFT").into()
            }
        }
        Dialog::Signal { .. } => sciplot_core::tr!("信号处理", "Signal").into(),
        Dialog::SurfaceSettings { .. } => sciplot_core::tr!("三维设置", "3D settings").into(),
        Dialog::ArrangeLayers { .. } => sciplot_core::tr!("排列图层", "Arrange layers").into(),
        Dialog::InsertLayer { .. } => sciplot_core::tr!("插入其他图形", "Insert another graph").into(),
        Dialog::ExportGraph { all, .. } => {
            if *all {
                sciplot_core::tr!("导出全部图形", "Export all graphs").into()
            } else {
                sciplot_core::tr!("导出图形", "Export graph").into()
            }
        }
        Dialog::Print { all, .. } => {
            if *all {
                sciplot_core::tr!("打印全部图形", "Print all plots").into()
            } else {
                sciplot_core::tr!("打印", "Print").into()
            }
        }
        Dialog::TemplateFile { save, .. } => {
            if *save {
                sciplot_core::tr!("保存为模板", "Save as template").into()
            } else {
                sciplot_core::tr!("打开模板", "Open template").into()
            }
        }
        Dialog::StyleFile { save, .. } => {
            if *save {
                sciplot_core::tr!("导出默认样式", "Export default style").into()
            } else {
                sciplot_core::tr!("打开默认样式", "Open default style").into()
            }
        }
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
        Dialog::ColumnProps {
            table,
            column,
            format,
            places,
            time_format,
        } => column_props(app, ui, *table, *column, format, places, time_format),
        Dialog::RenameColumn {
            table,
            column,
            name,
            error,
        } => rename_column(app, ui, *table, *column, name, error),
        Dialog::Sort {
            table,
            column,
            ascending,
            mode,
            picked,
        } => sort_dialog(app, ui, *table, column, ascending, mode, picked),
        Dialog::Stats { table, picked } => stats_dialog(app, ui, *table, picked),
        Dialog::Normalize { table, picked, whole } => normalize_dialog(app, ui, *table, picked, whole),
        Dialog::CellFormula {
            table,
            column,
            row,
            expr,
            error,
        } => cell_formula(app, ui, *table, *column, *row, expr, error),
        Dialog::ImportAscii {
            encoding,
            delimiter,
            header,
            decimal,
            skip,
            layout,
            paths,
            error,
        } => import_ascii(
            app, ui, encoding, delimiter, header, decimal, skip, layout, paths, error,
        ),
        Dialog::Axis {
            graph,
            layer,
            edge,
            axis,
            min,
            max,
            step,
            minor,
            error,
        } => axis_dialog(
            app, ui, *graph, *layer, *edge, axis, min, max, step, minor, error,
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
            axes,
            vector_mode,
        } => style_dialog(app, ui, *graph, *layer, *curve, style, name, axes, vector_mode),
        Dialog::Wizard {
            step,
            create_new,
            new_name,
            new_cols,
            new_rows,
            table,
            x,
            y,
            kind,
            vector_mode,
            third,
            fourth,
        } => wizard_dialog(
            app,
            ui,
            step,
            create_new,
            new_name,
            new_cols,
            new_rows,
            table,
            x,
            y,
            kind,
            vector_mode,
            third,
            fourth,
        ),
        Dialog::Errors {
            graph,
            layer,
            curve,
            spec,
            x_col,
            y_col,
        } => errors_dialog(app, ui, *graph, *layer, *curve, spec, x_col, y_col),
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
            weight,
            weight_column,
            algorithm,
            tolerance,
            max_iter,
            peaks,
            centers,
            gaussian,
            plugin,
        } => fit_dialog(
            app, ui, *graph, *layer, curve, which, degree, expr, initial, xmin, xmax, weight, weight_column,
            algorithm, tolerance, max_iter, peaks, centers, gaussian, plugin,
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
            filter,
        } => range_dialog(
            app, ui, *graph, *layer, curve, *op, a, b, n, method, window, order, sg, filter,
        ),
        Dialog::Fft {
            graph,
            layer,
            curve,
            inverse,
            normalize,
            shift,
            imag,
            dt,
        } => fft_dialog(app, ui, *graph, *layer, curve, inverse, normalize, shift, imag, dt),
        Dialog::Signal {
            table,
            kind,
            auto,
            column_a,
            column_b,
        } => signal_dialog(app, ui, *table, *kind, auto, column_a, column_b),
        Dialog::SurfaceSettings {
            graph,
            mode,
            point_style,
        } => surface_settings_dialog(app, ui, *graph, mode, point_style),
        Dialog::Function {
            graph,
            layer,
            form,
            expr,
            y_expr,
            xmin,
            xmax,
            ymin,
            ymax,
            samples,
            surface,
            mode,
        } => function_dialog(
            app, ui, *graph, *layer, form, expr, y_expr, xmin, xmax, ymin, ymax, samples, surface, mode,
        ),
        Dialog::AddCurve { graph, table, kind } => add_curve(app, ui, *graph, table, kind),
        Dialog::Prefs(prefs) => prefs_dialog(app, ui, prefs),
        Dialog::ImportInstead { path } => import_instead(app, ui, path),
        Dialog::ArrangeLayers {
            graph,
            rows,
            cols,
            align,
            margin_left,
            margin_right,
            margin_top,
            margin_bottom,
            gap_x,
            gap_y,
            auto_rearrange,
        } => arrange_layers_dialog(
            app,
            ui,
            *graph,
            rows,
            cols,
            align,
            margin_left,
            margin_right,
            margin_top,
            margin_bottom,
            gap_x,
            gap_y,
            auto_rearrange,
        ),
        Dialog::InsertLayer { graph, source } => insert_layer_dialog(app, ui, *graph, source),
        Dialog::ExportGraph {
            all,
            format,
            path,
            dpi,
            quality,
            transparent,
            paper,
            orientation,
            fit_paper,
        } => export_graph_dialog(
            app,
            ui,
            *all,
            format,
            path,
            dpi,
            quality,
            transparent,
            paper,
            orientation,
            fit_paper,
        ),
        Dialog::Print {
            all,
            path,
            dpi,
            paper,
            orientation,
        } => print_dialog(app, ui, *all, path, dpi, paper, orientation),
        Dialog::TemplateFile { save, graph, path } => template_file_dialog(app, ui, *save, *graph, path),
        Dialog::StyleFile { save, path } => style_file_dialog(app, ui, *save, path),
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
                if ui.selectable_value(&mut prefs.language, *language, language.label()).changed() {
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
    ui.separator();
    ui.strong(sciplot_core::t("本机 API"));
    ui.checkbox(
        &mut prefs.api_enabled,
        sciplot_core::t("允许本机程序通过 API 画图"),
    );
    ui.horizontal(|ui| {
        ui.label(sciplot_core::t("端口"));
        ui.add(egui::DragValue::new(&mut prefs.api_port).range(1024..=65535));
    });
    ui.label(sciplot_core::t(
        "只监听 127.0.0.1。令牌写在主目录的 .sciplot_api.json，退出程序时删除。",
    ));
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
    if matches!(action, PathAction::ImportImage) {
        ui.label(sciplot_core::tr!(
            "支持 PNG、JPEG、GIF、BMP。从文件菜单导入时写成强度矩阵；从图上插入时嵌在当前图里。",
            "PNG, JPEG, GIF, and BMP. The File menu stores an intensity matrix. Inserting on a graph embeds the picture."
        ));
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
    let verb = if matches!(
        action,
        PathAction::Open | PathAction::Append | PathAction::ImportCsv | PathAction::ImportImage
    ) {
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
    ui.label(sciplot_core::t("除法得到小数：1/2 等于 0.5，并且 1 等于 1.0。"));
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
                let others: Vec<sciplot_core::Table> = project
                    .tables
                    .iter()
                    .filter(|item| item.id != table)
                    .cloned()
                    .collect();
                let table = project.table_mut(table).ok_or(sciplot_core::t("找不到表格"))?;
                sciplot_core::apply_formula_with(table, column, &formula, &others)
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
    format: &mut sciplot_core::NumericFormat,
    places: &mut u8,
    time_format: &mut String,
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
        "点下面的类型或角色，会立刻改这一列。数值格式和时间格式用下面的按钮应用。列名用表头右键「重命名此列…」，或双击表头。",
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
    ui.separator();
    ui.label(sciplot_core::t("数值格式"));
    egui::ComboBox::from_id_salt("numeric-format")
        .selected_text(format.label())
        .show_ui(ui, |ui| {
            for kind in sciplot_core::NumericFormat::all() {
                ui.selectable_value(format, *kind, kind.label());
            }
        });
    ui.label(sciplot_core::t("小数位数"));
    ui.add(egui::DragValue::new(places).range(0..=12));
    ui.label(sciplot_core::t("时间格式"));
    ui.text_edit_singleline(time_format);
    ui.label(sciplot_core::t(
        "日期格式：YYYY 年，MM 月，DD 日，HH 时，mm 分，ss 秒。留空则按范围自动缩短。",
    ));
    if ui.button(sciplot_core::t("应用格式")).clicked() {
        let format = *format;
        let places = (*places).clamp(0, 12);
        let time_format = time_format.trim().to_string();
        if app
            .edit(sciplot_core::t("列属性"), |project| {
                let column = project
                    .table_mut(table)
                    .ok_or(sciplot_core::t("找不到表格"))?
                    .columns
                    .get_mut(column)
                    .ok_or(sciplot_core::t("找不到列"))?;
                column.numeric_format = format;
                column.decimal_places = places;
                column.time_format = time_format;
                column.refresh_display();
                Ok(())
            })
            .is_some()
        {
            app.status = sciplot_core::t("已更新显示格式").into();
        }
    }
    !ui.button(sciplot_core::t("取消")).clicked()
}

fn rename_column(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    table: sciplot_core::TableId,
    column: usize,
    name: &mut String,
    error: &mut String,
) -> bool {
    ui.label(sciplot_core::t("名称"));
    ui.text_edit_singleline(name);
    if !error.is_empty() {
        ui.colored_label(egui::Color32::from_rgb(176, 32, 32), error.as_str());
    }
    let mut stay = true;
    ui.horizontal(|ui| {
        if action_button(ui, sciplot_core::t("确定")) {
            let new_name = name.clone();
            let current = app
                .project
                .table(table)
                .and_then(|item| item.columns.get(column))
                .map(|col| col.name.clone());
            if current.as_deref() == Some(new_name.as_str()) {
                stay = false;
            } else if app
                .edit(sciplot_core::t("重命名列"), |project| {
                    project.rename_table_column(table, column, &new_name)
                })
                .is_some()
            {
                error.clear();
                app.status = sciplot_core::t("已重命名列").into();
                stay = false;
            } else {
                *error = app.status.clone();
            }
        }
        if ui.button(sciplot_core::t("取消")).clicked() {
            stay = false;
        }
    });
    stay
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
            if column.ty != ty {
                column.set_type(ty);
            }
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
    mode: &mut sciplot_core::SortMode,
    picked: &mut Vec<bool>,
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
    ui.label(sciplot_core::t("排序方式"));
    for kind in sciplot_core::SortMode::all() {
        ui.radio_value(mode, *kind, kind.label());
    }
    if *mode != sciplot_core::SortMode::Whole {
        ui.label(sciplot_core::t("所选列"));
        if picked.len() != names.len() {
            picked.resize(names.len(), false);
        }
        for (index, name) in names.iter().enumerate() {
            if let Some(flag) = picked.get_mut(index) {
                ui.checkbox(flag, name);
            }
        }
    }
    if action_button(ui, sciplot_core::t("排序")) {
        let column = *column;
        let ascending = *ascending;
        let mode = *mode;
        let selected: Vec<usize> = picked
            .iter()
            .enumerate()
            .filter(|(_, flag)| **flag)
            .map(|(index, _)| index)
            .collect();
        if mode != sciplot_core::SortMode::Whole && selected.is_empty() {
            app.status = sciplot_core::t("没有选中要排序的列").into();
            return true;
        }
        if app
            .edit(sciplot_core::t("排序"), |project| {
                project
                    .table_mut(table)
                    .ok_or(sciplot_core::t("找不到表格"))?
                    .sort_columns(&selected, column, ascending, mode);
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
    edge: AxisEdge,
    axis: &mut sciplot_core::Axis,
    min: &mut String,
    max: &mut String,
    step: &mut String,
    minor: &mut String,
    error: &mut String,
) -> bool {
    ui.label(edge.label());
    if matches!(edge, AxisEdge::Top | AxisEdge::Right) {
        ui.label(sciplot_core::tr!(
            "上轴和右轴不跟随鼠标平移和滚轮缩放，范围用这里的最小、最大值或自动范围。",
            "The top and right axes do not follow mouse pan or wheel zoom. Set their range here, or leave it automatic."
        ));
    }
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
    ui.label(sciplot_core::t("刻度文字格式（空为自动）"));
    egui::ComboBox::from_id_salt("tick-kind")
        .selected_text(axis.tick_kind.label())
        .show_ui(ui, |ui| {
            for kind in sciplot_core::TickKind::all() {
                ui.selectable_value(&mut axis.tick_kind, *kind, kind.label());
            }
        });
    ui.text_edit_singleline(&mut axis.tick_pattern);
    ui.label(sciplot_core::t(
        "日期格式：YYYY 年，MM 月，DD 日，HH 时，mm 分，ss 秒。留空则按范围自动缩短。",
    ));
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
                        match edge {
                            AxisEdge::Bottom => layer.x_axis = stored.clone(),
                            AxisEdge::Left => layer.y_axis = stored.clone(),
                            AxisEdge::Top => {
                                layer.x_top = stored.clone();
                                layer.show_top = true;
                            }
                            AxisEdge::Right => {
                                layer.y_right = stored.clone();
                                layer.show_right = true;
                            }
                        }
                        Ok(())
                    });
                    clear_axis_view(app, graph, layer, edge);
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

fn clear_axis_view(app: &mut SciPlotApp, graph: sciplot_core::GraphId, layer: usize, edge: AxisEdge) {
    let Some(views) = app.views.get_mut(&graph) else {
        return;
    };
    let Some(view) = views.get_mut(layer) else {
        return;
    };
    match edge {
        AxisEdge::Left => {
            view.y_min = None;
            view.y_max = None;
        }
        AxisEdge::Bottom => {
            view.x_min = None;
            view.x_max = None;
        }
        AxisEdge::Top | AxisEdge::Right => {}
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

#[allow(clippy::too_many_arguments)]
fn style_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    graph: sciplot_core::GraphId,
    layer: usize,
    curve: usize,
    style: &mut CurveStyle,
    name: &mut String,
    axes: &mut CurveAxes,
    vector_mode: &mut VectorMode,
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
    if matches!(kind, Some(CurveKind::Histogram | CurveKind::StackedHistogram)) {
        ui.add(
            egui::Slider::new(&mut style.histogram_bins, 2..=80)
                .text(sciplot_core::t("直方图箱数")),
        );
    }
    if matches!(kind, Some(CurveKind::Steps | CurveKind::HorizontalSteps)) {
        ui.label(sciplot_core::tr!("阶梯拐角", "Step corner"));
        egui::ComboBox::from_id_salt("step-anchor")
            .selected_text(style.step_anchor.label())
            .show_ui(ui, |ui| {
                for item in StepAnchor::all() {
                    ui.selectable_value(&mut style.step_anchor, *item, item.label());
                }
            });
        ui.checkbox(&mut style.show_symbols, sciplot_core::tr!("在数据点上画符号", "Show symbols on the points"));
    }
    if kind == Some(CurveKind::Spline) {
        ui.checkbox(&mut style.show_symbols, sciplot_core::tr!("在数据点上画符号", "Show symbols on the points"));
    }
    if kind == Some(CurveKind::Pie) {
        ui.add(egui::Slider::new(&mut style.pie_start_deg, -180.0..=180.0).text(sciplot_core::tr!("起始角", "Start angle")));
        ui.add(egui::Slider::new(&mut style.pie_size, 0.2..=1.0).text(sciplot_core::tr!("半径", "Radius")));
        egui::ComboBox::from_id_salt("pie-pattern")
            .selected_text(style.pie_pattern.label())
            .show_ui(ui, |ui| {
                for item in FillPattern::all() {
                    ui.selectable_value(&mut style.pie_pattern, *item, item.label());
                }
            });
    }
    if kind == Some(CurveKind::BoxPlot) {
        egui::ComboBox::from_id_salt("whisker")
            .selected_text(style.whisker.label())
            .show_ui(ui, |ui| {
                for item in WhiskerMode::all() {
                    ui.selectable_value(&mut style.whisker, *item, item.label());
                }
            });
        if style.whisker == WhiskerMode::Percentile {
            ui.add(
                egui::Slider::new(&mut style.whisker_percentile, 0.0..=49.0)
                    .text(sciplot_core::tr!("下百分位", "Lower percentile")),
            );
        }
        ui.checkbox(&mut style.box_mean, sciplot_core::tr!("画出均值", "Show the mean"));
        ui.checkbox(&mut style.box_outliers, sciplot_core::tr!("画出离群点", "Show outliers"));
    }
    if kind == Some(CurveKind::Vector) {
        egui::ComboBox::from_id_salt("vector-mode")
            .selected_text(vector_mode.label())
            .show_ui(ui, |ui| {
                for item in VectorMode::all() {
                    ui.selectable_value(vector_mode, *item, item.label());
                }
            });
    }
    ui.label(sciplot_core::tr!("横轴", "X axis"));
    egui::ComboBox::from_id_salt("curve-x-side")
        .selected_text(axes.x.label())
        .show_ui(ui, |ui| {
            for item in XAxisSide::all() {
                ui.selectable_value(&mut axes.x, *item, item.label());
            }
        });
    ui.label(sciplot_core::tr!("纵轴", "Y axis"));
    egui::ComboBox::from_id_salt("curve-y-side")
        .selected_text(axes.y.label())
        .show_ui(ui, |ui| {
            for item in YAxisSide::all() {
                ui.selectable_value(&mut axes.y, *item, item.label());
            }
        });
    color_sliders(ui, &mut style.color);
    if action_button(ui, sciplot_core::t("应用")) {
        let style = style.clone();
        let name = name.clone();
        let axes = *axes;
        let vector_mode = *vector_mode;
        app.edit(sciplot_core::t("曲线样式"), |project| {
            let layer = project
                .graph_mut(graph)
                .ok_or(sciplot_core::t("找不到图形"))?
                .layers
                .get_mut(layer)
                .ok_or(sciplot_core::t("找不到图层"))?;
            if axes.x == XAxisSide::Top {
                layer.show_top = true;
            }
            if axes.y == YAxisSide::Right {
                layer.show_right = true;
            }
            let curve = layer.curves.get_mut(curve).ok_or(sciplot_core::t("找不到曲线"))?;
            curve.style = style.clone();
            curve.name = name.clone();
            curve.axes = axes;
            curve.vector_mode = vector_mode;
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
        let x_top = app.ann_x_top;
        let y_right = app.ann_y_right;
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
                x_top,
                y_right,
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
        let x_top = app.ann_x_top;
        let y_right = app.ann_y_right;
        app.edit(sciplot_core::t("标注"), |project| {
            push_annotation(project, graph, layer, kind.clone(), x_top, y_right)
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
    weight: &mut WeightChoice,
    weight_column: &mut String,
    algorithm: &mut FitAlgorithm,
    tolerance: &mut String,
    max_iter: &mut String,
    peaks: &mut String,
    centers: &mut String,
    gaussian: &mut bool,
    plugin: &mut usize,
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
            for item in FitWhich::all() {
                ui.selectable_value(which, *item, item.label());
            }
        });
    if *which == FitWhich::Poly {
        labeled(ui, sciplot_core::t("次数"), degree);
    }
    if *which == FitWhich::Custom {
        labeled(ui, sciplot_core::t("表达式，例如 a*exp(-b*x)"), expr);
        labeled(ui, sciplot_core::t("初值，例如 a=1, b=0.5"), initial);
    }
    if *which == FitWhich::MultiPeak {
        labeled(ui, sciplot_core::tr!("峰的个数（1–8）", "Number of peaks (1–8)"), peaks);
        labeled(ui, sciplot_core::tr!("峰位，逗号分隔，可空", "Peak centers, comma separated, optional"), centers);
        ui.checkbox(gaussian, sciplot_core::tr!("高斯峰（否则洛伦兹）", "Gaussian peaks (otherwise Lorentzian)"));
    }
    if *which == FitWhich::Plugin {
        if app.fit_plugins.is_empty() {
            ui.label(sciplot_core::tr!(
                "把 name / expr / initial 写成文本，放进主目录或程序旁边的 fitPlugins。不是 SciDAVis 的 .fdf，也不是 Python。",
                "Put a text file with name, expr, and initial into fitPlugins next to the program or in the home directory. This is not a SciDAVis .fdf or Python plugin."
            ));
        } else {
            let names: Vec<String> = app.fit_plugins.iter().map(|plugin| plugin.name.clone()).collect();
            *plugin = (*plugin).min(names.len() - 1);
            egui::ComboBox::from_id_salt("fit-plugin")
                .selected_text(&names[*plugin])
                .show_ui(ui, |ui| {
                    for (index, name) in names.iter().enumerate() {
                        ui.selectable_value(plugin, index, name);
                    }
                });
        }
    }
    egui::ComboBox::from_id_salt("fit-weight")
        .selected_text(weight.label())
        .show_ui(ui, |ui| {
            for item in WeightChoice::all() {
                ui.selectable_value(weight, *item, item.label());
            }
        });
    if matches!(*weight, WeightChoice::Dataset | WeightChoice::Direct) {
        labeled(ui, sciplot_core::tr!("权重列名", "Weight column"), weight_column);
    }
    if !matches!(*which, FitWhich::Linear | FitWhich::Poly) {
        egui::ComboBox::from_id_salt("fit-algorithm")
            .selected_text(algorithm.label())
            .show_ui(ui, |ui| {
                for item in [FitAlgorithm::ScaledLm, FitAlgorithm::UnscaledLm, FitAlgorithm::NelderMead] {
                    ui.selectable_value(algorithm, item, item.label());
                }
            });
        labeled(ui, sciplot_core::tr!("容差", "Tolerance"), tolerance);
        labeled(ui, sciplot_core::tr!("最大迭代", "Max iterations"), max_iter);
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
    let pick_peaks = if *which == FitWhich::MultiPeak {
        action_button(ui, sciplot_core::tr!("在图上点选峰位", "Click peaks on the graph"))
    } else {
        false
    };
    if action_button(ui, sciplot_core::t("拟合")) || pick_peaks {
        let model = match which {
            FitWhich::Linear => Ok(FitModel::Linear),
            FitWhich::Poly => degree
                .parse::<usize>()
                .map(|d| FitModel::Polynomial(d.clamp(1, 8)))
                .map_err(|_| sciplot_core::t("次数无效").to_string()),
            FitWhich::Exp => Ok(FitModel::Exponential),
            FitWhich::ExpDecay1 => Ok(FitModel::ExpDecay1),
            FitWhich::ExpDecay2 => Ok(FitModel::ExpDecay2),
            FitWhich::ExpDecay3 => Ok(FitModel::ExpDecay3),
            FitWhich::ExpGrowth => Ok(FitModel::ExpGrowth),
            FitWhich::Boltzmann => Ok(FitModel::Boltzmann),
            FitWhich::Gauss => Ok(FitModel::Gaussian),
            FitWhich::Lorentz => Ok(FitModel::Lorentzian),
            FitWhich::MultiPeak => parse_centers(centers).map(|typed| FitModel::MultiPeak {
                gaussian: *gaussian,
                centers: typed,
            }),
            FitWhich::Custom => parse_initials(initial).map(|initial| FitModel::Custom {
                expr: expr.clone(),
                initial,
            }),
            FitWhich::Plugin => app.fit_plugins.get(*plugin).map(|item| FitModel::Custom {
                expr: item.expr.clone(),
                initial: item.initial.clone(),
            }).ok_or_else(|| {
                sciplot_core::tr!("还没有可用的拟合插件", "There is no fit plugin to use").to_string()
            }),
        };
        let mut options = FitOptions {
            weighting: weight.to_weighting(),
            algorithm: *algorithm,
            tolerance: tolerance.parse().unwrap_or(1e-12),
            max_iterations: max_iter.parse::<usize>().unwrap_or(80).max(1),
            plugin_name: if *which == FitWhich::Plugin {
                app.fit_plugins.get(*plugin).map(|item| item.name.clone())
            } else {
                None
            },
            ..FitOptions::default()
        };
        if matches!(*weight, WeightChoice::Dataset | WeightChoice::Direct) {
            match ops::weight_column(&app.project, graph, layer, *curve, weight_column.trim()) {
                Ok(column) => options.column = column,
                Err(err) => {
                    app.status = err;
                    return true;
                }
            }
        }
        match model {
            Ok(model) => {
                let curve = *curve;
                let x_min = parse_opt(xmin);
                let x_max = parse_opt(xmax);
                if pick_peaks {
                    let need = peaks.parse::<usize>().unwrap_or(2).clamp(1, 8);
                    let FitModel::MultiPeak { centers, .. } = &model else {
                        return true;
                    };
                    if centers.len() >= need {
                        app.status = sciplot_core::tr!(
                            "手填的峰位已经够了，直接点拟合",
                            "Enough centers are already typed. Use Fit."
                        )
                        .into();
                        return true;
                    }
                    app.peak_pick = Some(crate::app::PeakPick {
                        graph,
                        layer,
                        curve,
                        gaussian: *gaussian,
                        need,
                        centers: centers.clone(),
                        options,
                        x_min,
                        x_max,
                    });
                    app.status = sciplot_core::trf!(
                        "在图上点选峰位（{got}/{need}）。Esc 取消。",
                        "Click peak positions on the graph ({got}/{need}). Esc cancels.",
                        got = centers.len(),
                        need = need
                    );
                    return false;
                }
                if matches!(model, FitModel::MultiPeak { ref centers, .. } if centers.is_empty()) {
                    app.status = sciplot_core::tr!(
                        "请填写峰位，或在图上点选",
                        "Type the peak centers, or click them on the graph"
                    )
                    .into();
                    return true;
                }
                let title = sciplot_core::trf!("{}拟合结果", "{} fit", which.label());
                let before = app
                    .project
                    .graph(graph)
                    .and_then(|item| item.layers.get(layer))
                    .map(|item| item.curves.len())
                    .unwrap_or(0);
                let prefs = app.prefs.clone();
                if let Some(report) = app.edit(sciplot_core::t("拟合"), |project| {
                    let report = ops::fit_on_graph(
                        project, graph, layer, curve, &model, x_min, x_max, &options,
                    )?;
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

fn parse_centers(text: &str) -> Result<Vec<f64>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let mut centers = Vec::new();
    for part in text.split(|ch: char| ch == ',' || ch == '，' || ch.is_whitespace()) {
        if part.is_empty() {
            continue;
        }
        let value = part.parse::<f64>().map_err(|_| {
            sciplot_core::trf!("峰位无效：{part}", "Invalid peak center: {part}", part = part)
        })?;
        if !value.is_finite() {
            return Err(sciplot_core::tr!("峰位必须是有限数值", "Peak centers must be finite").into());
        }
        centers.push(value);
    }
    if centers.len() > 8 {
        return Err(sciplot_core::tr!("最多 8 个峰", "At most 8 peaks").into());
    }
    Ok(centers)
}

#[allow(clippy::too_many_arguments)]
fn fft_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    graph: sciplot_core::GraphId,
    layer: usize,
    curve: &mut usize,
    inverse: &mut bool,
    normalize: &mut bool,
    shift: &mut bool,
    imag: &mut String,
    dt: &mut String,
) -> bool {
    let names = curve_names(app, graph, layer);
    if !names.is_empty() {
        *curve = (*curve).min(names.len() - 1);
        egui::ComboBox::from_id_salt("fft-curve")
            .selected_text(&names[*curve])
            .show_ui(ui, |ui| {
                for (index, name) in names.iter().enumerate() {
                    ui.selectable_value(curve, index, name);
                }
            });
    }
    ui.checkbox(inverse, sciplot_core::tr!("逆变换", "Inverse"));
    ui.checkbox(normalize, sciplot_core::tr!("振幅归一化（除以点数）", "Normalize amplitude (divide by the length)"));
    ui.checkbox(shift, sciplot_core::tr!("把零频移到中间", "Shift zero frequency to the center"));
    labeled(ui, sciplot_core::tr!("虚部列名，可空", "Imaginary column, optional"), imag);
    labeled_hint(
        ui,
        sciplot_core::tr!("采样间隔，可空", "Sample spacing, optional"),
        dt,
        sciplot_core::tr!("用 X 的间隔", "Use the spacing of X"),
    );
    ui.label(sciplot_core::tr!(
        "正向结果写实部、虚部、振幅和相位。逆变换要读这对实部/虚部，原来的单边 FFT 表不能直接拿来逆变换。逆变换如果输入已经是频率，请填写原来的采样间隔。",
        "Forward FFT writes real, imaginary, amplitude, and phase. Inverse FFT reads that pair. The older one-sided FFT table cannot be inverted directly. If the inverse input is already frequency, enter the original sample spacing."
    ));
    if action_button(ui, sciplot_core::t("计算")) {
        let curve = *curve;
        let inverse = *inverse;
        let shape = FftShape {
            normalize: *normalize,
            shift: *shift,
        };
        let imag_name = imag.trim().to_string();
        let dt = parse_opt(dt);
        let prefs = app.prefs.clone();
        if let Some(table) = app.edit(if inverse { sciplot_core::tr!("逆 FFT", "Inverse FFT") } else { "FFT" }, |project| {
            ops::fft_complex_table(
                project,
                graph,
                layer,
                curve,
                inverse,
                shape,
                Some(imag_name.as_str()).filter(|name| !name.is_empty()),
                dt,
            )
        }) {
            app.open_tab(crate::app::Tab::Table(table));
            if let Some(gid) = app.edit(sciplot_core::t("绘制结果"), |project| {
                let gid = project.plot_table(table, CurveKind::Line)?;
                prefs.apply_new_graph(project, gid);
                Ok(gid)
            }) {
                app.open_tab(crate::app::Tab::Graph(gid));
            }
            return false;
        }
    }
    !ui.button(sciplot_core::t("取消")).clicked()
}

fn signal_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    table: sciplot_core::TableId,
    kind: SignalKind,
    auto: &mut bool,
    column_a: &mut usize,
    column_b: &mut usize,
) -> bool {
    let names = app
        .project
        .table(table)
        .map(|item| item.columns.iter().map(|column| column.name.clone()).collect::<Vec<_>>())
        .unwrap_or_default();
    if names.len() < 2 && !(*auto && kind == SignalKind::Correlate) {
        ui.label(sciplot_core::tr!("至少需要两列数值", "At least two numeric columns are needed"));
    }
    if !names.is_empty() {
        *column_a = (*column_a).min(names.len() - 1);
        egui::ComboBox::from_id_salt("signal-a")
            .selected_text(&names[*column_a])
            .show_ui(ui, |ui| {
                for (index, name) in names.iter().enumerate() {
                    ui.selectable_value(column_a, index, name);
                }
            });
    }
    if kind == SignalKind::Correlate {
        ui.checkbox(auto, sciplot_core::tr!("自相关（只用第一列）", "Autocorrelation (first column only)"));
    }
    if (kind != SignalKind::Correlate || !*auto) && names.len() >= 2 {
            *column_b = (*column_b).min(names.len() - 1);
            egui::ComboBox::from_id_salt("signal-b")
                .selected_text(&names[*column_b])
                .show_ui(ui, |ui| {
                    for (index, name) in names.iter().enumerate() {
                        ui.selectable_value(column_b, index, name);
                    }
                });
    }
    ui.label(match kind {
        SignalKind::Convolve => sciplot_core::tr!(
            "线性卷积，结果长度是两列长度之和减 1。",
            "Linear convolution. The result length is the sum of the lengths minus 1."
        ),
        SignalKind::Deconvolve => sciplot_core::tr!(
            "用第二列做核做多项式除法。核的第一个点不能是 0。",
            "Polynomial division, using the second column as the kernel. Its first sample cannot be 0."
        ),
        SignalKind::Correlate => sciplot_core::tr!(
            "信号的协方差，横轴是滞后样本。这不是列与列的 Pearson 相关。",
            "Covariance of the signals. The horizontal axis is the lag in samples. This is not the Pearson correlation of columns."
        ),
    });
    if action_button(ui, sciplot_core::t("计算")) {
        let kind_key = if kind == SignalKind::Correlate && *auto {
            "autocorrelate"
        } else {
            kind.key()
        };
        let column_b = if kind_key == "autocorrelate" { None } else { Some(*column_b) };
        let prefs = app.prefs.clone();
        if let Some(created) = app.edit(kind.label(), |project| {
            ops::signal_table(project, table, kind_key, *column_a, column_b)
        }) {
            app.open_tab(crate::app::Tab::Table(created));
            if let Some(gid) = app.edit(sciplot_core::t("绘制结果"), |project| {
                let gid = project.plot_table(created, CurveKind::Line)?;
                prefs.apply_new_graph(project, gid);
                Ok(gid)
            }) {
                app.open_tab(crate::app::Tab::Graph(gid));
            }
            return false;
        }
    }
    !ui.button(sciplot_core::t("取消")).clicked()
}

fn surface_settings_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    graph: sciplot_core::GraphId,
    mode: &mut SurfaceMode,
    point_style: &mut sciplot_core::SurfacePointStyle,
) -> bool {
    egui::ComboBox::from_id_salt("surface-mode")
        .selected_text(mode.label())
        .show_ui(ui, |ui| {
            for item in SurfaceMode::all() {
                ui.selectable_value(mode, *item, item.label());
            }
        });
    egui::ComboBox::from_id_salt("surface-point")
        .selected_text(point_style.label())
        .show_ui(ui, |ui| {
            for item in sciplot_core::SurfacePointStyle::all() {
                ui.selectable_value(point_style, *item, item.label());
            }
        });
    ui.label(sciplot_core::tr!(
        "隐藏线用画家算法挡住后面的线。十字、锥体和竖线画在屏幕上，不是带光照的三维实体。",
        "Hidden lines use the painter's algorithm. Cross, cone, and bar markers are drawn in screen space, not as lit 3D solids."
    ));
    if action_button(ui, sciplot_core::t("应用")) {
        let mode = *mode;
        let point_style = *point_style;
        if app
            .edit(sciplot_core::tr!("三维设置", "3D settings"), |project| {
                let spec = project
                    .graph_mut(graph)
                    .and_then(|item| item.surface.as_mut())
                    .ok_or(sciplot_core::t("不是三维图"))?;
                spec.mode = mode;
                spec.point_style = point_style;
                Ok(())
            })
            .is_some()
        {
            return false;
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
    filter: &mut FilterKind,
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
            egui::ComboBox::from_id_salt("fft-band")
                .selected_text(filter.label())
                .show_ui(ui, |ui| {
                    for item in FilterKind::all() {
                        ui.selectable_value(filter, *item, item.label());
                    }
                });
            let band = matches!(*filter, FilterKind::BandPass | FilterKind::BandStop);
            labeled(
                ui,
                if band {
                    sciplot_core::tr!("下限频率", "Lower frequency")
                } else {
                    sciplot_core::t("截止频率")
                },
                a,
            );
            if band {
                labeled(ui, sciplot_core::tr!("上限频率", "Upper frequency"), b);
            }
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
                let Some(low) = parse_opt(a) else {
                    app.status = sciplot_core::t("请输入截止频率").into();
                    return true;
                };
                let band = match *filter {
                    FilterKind::LowPass => FftBand::LowPass { cutoff: low },
                    FilterKind::HighPass => FftBand::HighPass { cutoff: low },
                    FilterKind::BandPass | FilterKind::BandStop => {
                        let Some(high) = parse_opt(b) else {
                            app.status = sciplot_core::tr!("请输入上限频率", "Enter the upper frequency").into();
                            return true;
                        };
                        if *filter == FilterKind::BandPass {
                            FftBand::BandPass { low, high }
                        } else {
                            FftBand::BandStop { low, high }
                        }
                    }
                };
                app.edit(sciplot_core::t("滤波"), |project| {
                    ops::filter_table(project, graph, layer, curve, band).map(|_| ())
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
fn wizard_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    step: &mut usize,
    create_new: &mut bool,
    new_name: &mut String,
    new_cols: &mut String,
    new_rows: &mut String,
    table: &mut sciplot_core::TableId,
    x: &mut usize,
    y: &mut Vec<bool>,
    kind: &mut CurveKind,
    vector_mode: &mut VectorMode,
    third: &mut usize,
    fourth: &mut usize,
) -> bool {
    ui.label(match *step {
        0 => sciplot_core::tr!("第 1 步：选择或新建数据", "Step 1: choose or create data"),
        1 => sciplot_core::tr!("第 2 步：图型和列", "Step 2: plot type and columns"),
        _ => sciplot_core::tr!("第 3 步：确认并绘图", "Step 3: confirm and plot"),
    });
    if *step == 0 {
        ui.checkbox(create_new, sciplot_core::tr!("新建表格", "Create a new table"));
        if *create_new {
            labeled(ui, sciplot_core::t("名称"), new_name);
            labeled(ui, sciplot_core::tr!("列数", "Columns"), new_cols);
            labeled(ui, sciplot_core::tr!("行数", "Rows"), new_rows);
        } else if app.project.tables.is_empty() {
            ui.label(sciplot_core::tr!("还没有表格，请新建一张。", "There is no table yet. Create one."));
            *create_new = true;
        } else {
            let names: Vec<(sciplot_core::TableId, String)> = app.project.tables.iter().map(|item| (item.id, item.name.clone())).collect();
            let selected = names.iter().find(|(id, _)| *id == *table).map(|(_, name)| name.clone()).unwrap_or_else(|| names[0].1.clone());
            if !names.iter().any(|(id, _)| *id == *table) {
                *table = names[0].0;
            }
            ui.label(sciplot_core::t("表格"));
            egui::ComboBox::from_id_salt("wizard-table")
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    for (id, name) in &names {
                        ui.selectable_value(table, *id, name);
                    }
                });
        }
        if action_button(ui, sciplot_core::tr!("下一步", "Next")) {
            if *create_new {
                let cols = new_cols.parse::<usize>().unwrap_or(0).clamp(2, 32);
                let rows = new_rows.parse::<usize>().unwrap_or(0).clamp(1, 1_000_000);
                let name = new_name.clone();
                if let Some(id) = app.edit(sciplot_core::tr!("绘图向导", "Plot wizard"), |project| {
                    prepare_wizard_table(project, &name, cols, rows)
                }) {
                    *table = id;
                    sync_wizard_columns(app, *table, x, y, third, fourth);
                    *step = 1;
                }
            } else if app.project.table(*table).is_some() {
                sync_wizard_columns(app, *table, x, y, third, fourth);
                *step = 1;
            } else {
                app.status = sciplot_core::t("找不到表格").into();
            }
        }
        return !ui.button(sciplot_core::t("取消")).clicked();
    }
    let columns: Vec<String> = app
        .project
        .table(*table)
        .map(|item| item.columns.iter().map(|column| column.name.clone()).collect())
        .unwrap_or_default();
    if columns.is_empty() {
        ui.label(sciplot_core::t("表格没有列"));
        return button_closes(ui, sciplot_core::t("关闭"));
    }
    while y.len() < columns.len() {
        y.push(false);
    }
    *x = (*x).min(columns.len() - 1);
    *third = (*third).min(columns.len() - 1);
    *fourth = (*fourth).min(columns.len() - 1);
    egui::ComboBox::from_id_salt("wizard-kind")
        .selected_text(kind.label())
        .show_ui(ui, |ui| {
            for item in CurveKind::table_kinds() {
                ui.selectable_value(kind, *item, item.label());
            }
        });
    column_combo(ui, "wizard-x", sciplot_core::tr!("X 列", "X column"), x, &columns);
    if *kind == CurveKind::Vector {
        egui::ComboBox::from_id_salt("wizard-vector")
            .selected_text(vector_mode.label())
            .show_ui(ui, |ui| {
                for item in VectorMode::all() {
                    ui.selectable_value(vector_mode, *item, item.label());
                }
            });
        let mut y_index = y.iter().position(|on| *on).unwrap_or(0).min(columns.len() - 1);
        column_combo(ui, "wizard-y", sciplot_core::tr!("起点 Y", "Start Y"), &mut y_index, &columns);
        y.fill(false);
        y[y_index] = true;
        let (third_label, fourth_label) = match *vector_mode {
            VectorMode::EndPoint => (
                sciplot_core::tr!("终点 X", "End X"),
                sciplot_core::tr!("终点 Y", "End Y"),
            ),
            VectorMode::AngleMagnitude => (
                sciplot_core::tr!("角度（弧度）", "Angle (radians)"),
                sciplot_core::tr!("模长", "Magnitude"),
            ),
            VectorMode::DxDy => (sciplot_core::tr!("dx", "dx"), sciplot_core::tr!("dy", "dy")),
        };
        column_combo(ui, "wizard-3", third_label, third, &columns);
        column_combo(ui, "wizard-4", fourth_label, fourth, &columns);
    } else {
        ui.label(sciplot_core::tr!("Y 列", "Y columns"));
        for (index, name) in columns.iter().enumerate() {
            if index == *x {
                continue;
            }
            ui.checkbox(&mut y[index], name);
        }
    }
    if *step == 1 {
        if action_button(ui, sciplot_core::tr!("下一步", "Next")) {
            *step = 2;
        }
        if ui.button(sciplot_core::tr!("上一步", "Back")).clicked() {
            *step = 0;
        }
        return !ui.button(sciplot_core::t("取消")).clicked();
    }
    let table_name = app.project.table(*table).map(|item| item.name.clone()).unwrap_or_default();
    ui.label(sciplot_core::trf!("表格：{}", "Table: {}", table_name));
    ui.label(sciplot_core::trf!("图型：{}", "Plot: {}", kind.label()));
    if action_button(ui, sciplot_core::t("绘制")) {
        let picked: Vec<usize> = if *kind == CurveKind::Vector {
            let y_index = y.iter().position(|on| *on).unwrap_or(0);
            vec![y_index, *third, *fourth]
        } else {
            y.iter().enumerate().filter(|(_, on)| **on).map(|(index, _)| index).collect()
        };
        let kind = *kind;
        let mode = *vector_mode;
        let x = *x;
        let prefs = app.prefs.clone();
        let table_id = *table;
        if let Some(id) = app.edit(sciplot_core::tr!("绘图向导", "Plot wizard"), |project| {
            let id = project.plot_picked(table_id, kind, mode, x, &picked)?;
            prefs.apply_new_graph(project, id);
            Ok(id)
        }) {
            app.open_tab(crate::app::Tab::Graph(id));
            app.status = sciplot_core::tr!("已按向导绘图", "Plotted from the wizard").into();
            return false;
        }
    }
    if ui.button(sciplot_core::tr!("上一步", "Back")).clicked() {
        *step = 1;
        return true;
    }
    !ui.button(sciplot_core::t("取消")).clicked()
}

pub(crate) fn prepare_wizard_table(
    project: &mut sciplot_core::Project,
    name: &str,
    cols: usize,
    rows: usize,
) -> Result<sciplot_core::TableId, String> {
    let cols = cols.clamp(2, 32);
    let rows = rows.clamp(1, 1_000_000);
    let id = project.new_table(name);
    let table = project.table_mut(id).ok_or(sciplot_core::t("找不到表格"))?;
    while table.columns.len() < cols {
        let index = table.columns.len();
        let mode = if index == 0 {
            sciplot_core::ColumnMode::X
        } else {
            sciplot_core::ColumnMode::Y
        };
        table.add_column(sciplot_core::column_letter(index), mode);
    }
    while table.columns.len() > cols {
        table.columns.pop();
    }
    table.ensure_rows(rows);
    Ok(id)
}

fn sync_wizard_columns(
    app: &SciPlotApp,
    table: sciplot_core::TableId,
    x: &mut usize,
    y: &mut Vec<bool>,
    third: &mut usize,
    fourth: &mut usize,
) {
    let Some(table) = app.project.table(table) else {
        return;
    };
    let columns = &table.columns;
    if columns.is_empty() {
        return;
    }
    *x = columns
        .iter()
        .position(|column| column.mode == sciplot_core::ColumnMode::X)
        .unwrap_or(0)
        .min(columns.len() - 1);
    y.resize(columns.len(), false);
    for (index, column) in columns.iter().enumerate() {
        y[index] = index != *x && (column.mode == sciplot_core::ColumnMode::Y || column.mode == sciplot_core::ColumnMode::Z);
    }
    if !y.iter().any(|on| *on) {
        if let Some(index) = (0..columns.len()).find(|index| *index != *x) {
            y[index] = true;
        }
    }
    *third = (*third).min(columns.len() - 1);
    *fourth = (*fourth).min(columns.len() - 1);
}

fn column_combo(ui: &mut egui::Ui, salt: &str, label: &str, index: &mut usize, columns: &[String]) {
    let selected = columns.get(*index).cloned().unwrap_or_default();
    ui.label(label);
    egui::ComboBox::from_id_salt(salt)
        .selected_text(selected)
        .show_ui(ui, |ui| {
            for (item, name) in columns.iter().enumerate() {
                ui.selectable_value(index, item, name);
            }
        });
}

#[allow(clippy::too_many_arguments)]
fn errors_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    graph: sciplot_core::GraphId,
    layer: usize,
    curve: usize,
    spec: &mut sciplot_core::ErrorSpec,
    x_col: &mut String,
    y_col: &mut String,
) -> bool {
    ui.label(sciplot_core::tr!(
        "加到当前曲线上。百分比按数值的绝对值。标准差和标准误用这一列里有限的点。",
        "Drawn on the current curve. Percent uses the absolute value. Standard deviation and standard error use the finite points in that column."
    ));
    error_source_row(ui, "err-x", sciplot_core::tr!("X 误差", "X error"), &mut spec.x, &mut spec.x_percent, x_col);
    error_source_row(ui, "err-y", sciplot_core::tr!("Y 误差", "Y error"), &mut spec.y, &mut spec.y_percent, y_col);
    if action_button(ui, sciplot_core::t("应用")) {
        let spec = spec.clone();
        let x_col = x_col.clone();
        let y_col = y_col.clone();
        if app
            .edit(sciplot_core::tr!("误差棒", "Error bars"), |project| {
                let names = {
                    let source = project
                        .graph(graph)
                        .and_then(|item| item.layers.get(layer))
                        .and_then(|item| item.curves.get(curve))
                        .map(|item| item.source.clone())
                        .ok_or(sciplot_core::t("找不到曲线"))?;
                    match source {
                        DataSource::Columns { table, .. } => project
                            .table(table)
                            .map(|item| item.columns.iter().map(|column| column.name.clone()).collect())
                            .unwrap_or_default(),
                        _ => Vec::new(),
                    }
                };
                let from_table = project
                    .graph(graph)
                    .and_then(|item| item.layers.get(layer))
                    .and_then(|item| item.curves.get(curve))
                    .is_some_and(|item| matches!(item.source, DataSource::Columns { .. }));
                if !from_table && (spec.x == ErrorSource::Column || spec.y == ErrorSource::Column) {
                    return Err(sciplot_core::tr!(
                        "这条曲线没有数据列，不能从列读取误差",
                        "This curve has no table columns to read errors from"
                    )
                    .into());
                }
                let x_index = if spec.x == ErrorSource::Column {
                    Some(find_named(&names, &x_col)?)
                } else {
                    None
                };
                let y_index = if spec.y == ErrorSource::Column {
                    Some(find_named(&names, &y_col)?)
                } else {
                    None
                };
                let curve = project
                    .graph_mut(graph)
                    .ok_or(sciplot_core::t("找不到图形"))?
                    .layers
                    .get_mut(layer)
                    .ok_or(sciplot_core::t("找不到图层"))?
                    .curves
                    .get_mut(curve)
                    .ok_or(sciplot_core::t("找不到曲线"))?;
                curve.error = spec.clone();
                if let DataSource::Columns { x_err, y_err, .. } = &mut curve.source {
                    if spec.x == ErrorSource::Column || spec.x == ErrorSource::None {
                        *x_err = x_index;
                    }
                    if spec.y == ErrorSource::Column || spec.y == ErrorSource::None {
                        *y_err = y_index;
                    }
                }
                Ok(())
            })
            .is_some()
        {
            app.status = sciplot_core::tr!("已更新误差棒", "Updated the error bars").into();
            return false;
        }
    }
    !ui.button(sciplot_core::t("取消")).clicked()
}

fn error_source_row(ui: &mut egui::Ui, salt: &str, label: &str, source: &mut ErrorSource, percent: &mut f64, column: &mut String) {
    ui.label(label);
    egui::ComboBox::from_id_salt(salt)
        .selected_text(source.label())
        .show_ui(ui, |ui| {
            for item in ErrorSource::all() {
                ui.selectable_value(source, *item, item.label());
            }
        });
    if *source == ErrorSource::Percent {
        ui.add(egui::Slider::new(percent, 0.0..=100.0).text(sciplot_core::tr!("百分比", "Percent")));
    }
    if *source == ErrorSource::Column {
        ui.text_edit_singleline(column);
    }
}

fn find_named(names: &[String], wanted: &str) -> Result<usize, String> {
    let wanted = wanted.trim();
    names
        .iter()
        .position(|name| name == wanted)
        .ok_or_else(|| {
            sciplot_core::trf!("找不到列「{wanted}」", "No column named \"{wanted}\"", wanted = wanted)
        })
}

#[allow(clippy::too_many_arguments)]
fn function_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    graph: Option<sciplot_core::GraphId>,
    layer: usize,
    form: &mut FunctionForm,
    expr: &mut String,
    y_expr: &mut String,
    xmin: &mut String,
    xmax: &mut String,
    ymin: &mut String,
    ymax: &mut String,
    samples: &mut String,
    surface: &mut bool,
    mode: &mut SurfaceMode,
) -> bool {
    ui.checkbox(surface, sciplot_core::tr!("画成三维曲面", "Draw a 3D surface"));
    if !*surface {
        egui::ComboBox::from_id_salt("fn-form")
            .selected_text(form.label())
            .show_ui(ui, |ui| {
                for item in FunctionForm::all() {
                    ui.selectable_value(form, *item, item.label());
                }
            });
    }
    let expr_label = if *surface {
        sciplot_core::t("表达式")
    } else {
        match *form {
            FunctionForm::Cartesian => sciplot_core::tr!("y = f(x)", "y = f(x)"),
            FunctionForm::Parametric => sciplot_core::tr!("x(t)", "x(t)"),
            FunctionForm::Polar => sciplot_core::tr!("r(θ)，θ 用弧度", "r(theta), theta in radians"),
        }
    };
    labeled(ui, expr_label, expr);
    if !*surface && *form == FunctionForm::Parametric {
        labeled(ui, sciplot_core::tr!("y(t)", "y(t)"), y_expr);
    }
    let (min_label, max_label) = if *surface || *form == FunctionForm::Cartesian {
        (sciplot_core::t("x 最小"), sciplot_core::t("x 最大"))
    } else if *form == FunctionForm::Polar {
        (
            sciplot_core::tr!("θ 最小", "theta min"),
            sciplot_core::tr!("θ 最大", "theta max"),
        )
    } else {
        (sciplot_core::tr!("t 最小", "t min"), sciplot_core::tr!("t 最大", "t max"))
    };
    labeled(ui, min_label, xmin);
    labeled(ui, max_label, xmax);
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
            let form = *form;
            let y_expr = y_expr.clone();
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
                let (name, kind, source) = match form {
                    FunctionForm::Parametric => {
                        sciplot_core::CompiledExpr::compile(&y_expr)?;
                        (
                            format!("({expr}, {y_expr})"),
                            CurveKind::Parametric,
                            DataSource::Parametric {
                                x_expr: expr.clone(),
                                y_expr: y_expr.clone(),
                                t_min: x0,
                                t_max: x1,
                                samples,
                            },
                        )
                    }
                    FunctionForm::Polar => (
                        expr.clone(),
                        CurveKind::Polar,
                        DataSource::Polar {
                            expr: expr.clone(),
                            theta_min: x0,
                            theta_max: x1,
                            samples,
                        },
                    ),
                    FunctionForm::Cartesian => (
                        expr.clone(),
                        CurveKind::Function,
                        DataSource::Function {
                            expr: expr.clone(),
                            x_min: x0,
                            x_max: x1,
                            samples,
                        },
                    ),
                };
                sciplot_core::CompiledExpr::compile(match &source {
                    DataSource::Parametric { x_expr, .. } => x_expr,
                    DataSource::Polar { expr, .. } | DataSource::Function { expr, .. } => expr,
                    _ => &expr,
                })?;
                layer_ref.curves.push(sciplot_core::Curve {
                    name,
                    kind,
                    style,
                    source,
                    visible: true,
                    ..sciplot_core::Curve::default()
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
    CurveKind::table_kinds()
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

pub(crate) fn push_annotation(
    project: &mut sciplot_core::Project,
    graph: sciplot_core::GraphId,
    layer: usize,
    kind: AnnotationKind,
    x_top: bool,
    y_right: bool,
) -> Result<(), String> {
    let id = project.alloc_annotation_id();
    let layer = project
        .graph_mut(graph)
        .ok_or(sciplot_core::t("找不到图形"))?
        .layers
        .get_mut(layer)
        .ok_or(sciplot_core::t("找不到图层"))?;
    layer.annotations.push(Annotation {
        id,
        kind,
        x_top,
        y_right,
    });
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

fn column_checks(ui: &mut egui::Ui, names: &[String], picked: &mut [bool]) {
    for (index, name) in names.iter().enumerate() {
        if let Some(flag) = picked.get_mut(index) {
            ui.checkbox(flag, name);
        }
    }
}

fn picked_indexes(picked: &[bool]) -> Vec<usize> {
    picked
        .iter()
        .enumerate()
        .filter(|(_, flag)| **flag)
        .map(|(index, _)| index)
        .collect()
}

fn stats_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    table: sciplot_core::TableId,
    picked: &mut Vec<bool>,
) -> bool {
    let names: Vec<String> = app
        .project
        .table(table)
        .map(|item| item.columns.iter().map(|column| column.name.clone()).collect())
        .unwrap_or_default();
    if picked.len() != names.len() {
        picked.resize(names.len(), false);
    }
    ui.label(sciplot_core::t("所选列"));
    column_checks(ui, &names, picked);
    let indexes = picked_indexes(picked);
    ui.horizontal(|ui| {
        if ui.button(sciplot_core::t("列统计表…")).clicked() {
            if let Some(id) = app.edit(sciplot_core::t("列统计表…"), |project| {
                ops::column_stats_table(project, table, &indexes)
            }) {
                app.open_tab(crate::app::Tab::Table(id));
                app.status = sciplot_core::t("已写入列统计").into();
            }
        }
        if ui.button(sciplot_core::t("行统计…")).clicked() {
            if let Some(id) = app.edit(sciplot_core::t("行统计…"), |project| {
                ops::row_stats_table(project, table, &indexes)
            }) {
                app.open_tab(crate::app::Tab::Table(id));
                app.status = sciplot_core::t("已写入行统计").into();
            }
        }
    });
    !ui.button(sciplot_core::t("关闭")).clicked()
}

fn normalize_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    table: sciplot_core::TableId,
    picked: &mut Vec<bool>,
    whole: &mut bool,
) -> bool {
    let names: Vec<String> = app
        .project
        .table(table)
        .map(|item| item.columns.iter().map(|column| column.name.clone()).collect())
        .unwrap_or_default();
    if picked.len() != names.len() {
        picked.resize(names.len(), false);
    }
    ui.label(sciplot_core::t("所选列"));
    column_checks(ui, &names, picked);
    ui.radio_value(whole, false, sciplot_core::t("按列缩放到 [0, 1]"));
    ui.radio_value(whole, true, sciplot_core::t("整表缩放到 [0, 1]"));
    if action_button(ui, sciplot_core::t("归一化")) {
        let indexes = picked_indexes(picked);
        if indexes.is_empty() {
            app.status = sciplot_core::t("没有选中要归一化的列").into();
            return true;
        }
        let mode = if *whole {
            sciplot_core::NormalizeMode::Table
        } else {
            sciplot_core::NormalizeMode::Columns
        };
        if app
            .edit(sciplot_core::t("归一化"), |project| {
                project
                    .table_mut(table)
                    .ok_or(sciplot_core::t("找不到表格"))?
                    .normalize_unit(&indexes, mode)?;
                Ok(())
            })
            .is_some()
        {
            app.status = sciplot_core::t("已归一化").into();
            return false;
        }
    }
    !ui.button(sciplot_core::t("取消")).clicked()
}

fn cell_formula(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    table: sciplot_core::TableId,
    column: usize,
    row: usize,
    expr: &mut String,
    error: &mut String,
) -> bool {
    ui.label(sciplot_core::t(
        "这一格的公式。以 = 开头，或在公式编辑模式里直接写。可以用 tablecol(\"表\",\"列\") 引用别的表。",
    ));
    ui.text_edit_singleline(expr);
    if !error.is_empty() {
        ui.colored_label(egui::Color32::from_rgb(176, 32, 32), error.as_str());
    }
    if action_button(ui, sciplot_core::t("计算这一格")) {
        let formula = expr.clone();
        let outcome = app.edit(sciplot_core::t("单元格公式"), |project| {
            let others: Vec<sciplot_core::Table> = project
                .tables
                .iter()
                .filter(|item| item.id != table)
                .cloned()
                .collect();
            let current = project.table_mut(table).ok_or(sciplot_core::t("找不到表格"))?;
            if column >= current.columns.len() {
                return Err(sciplot_core::t("找不到列").into());
            }
            current.columns[column].set_cell_formula_raw(row, &formula);
            sciplot_core::commit_cell(&mut *current, column, row, true, &others)
        });
        match outcome {
            Some(()) => {
                error.clear();
                app.status = sciplot_core::t("已写入单元格").into();
                return false;
            }
            None => *error = app.status.clone(),
        }
    }
    !ui.button(sciplot_core::t("取消")).clicked()
}

#[allow(clippy::too_many_arguments)]
fn import_ascii(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    encoding: &mut sciplot_core::CsvEncoding,
    delimiter: &mut sciplot_core::CsvDelimiter,
    header: &mut sciplot_core::CsvHeader,
    decimal: &mut sciplot_core::DecimalMark,
    skip: &mut String,
    layout: &mut sciplot_core::CsvLayout,
    paths: &mut String,
    error: &mut String,
) -> bool {
    ui.label(sciplot_core::t("编码"));
    egui::ComboBox::from_id_salt("csv-encoding")
        .selected_text(encoding.label())
        .show_ui(ui, |ui| {
            for kind in sciplot_core::CsvEncoding::all() {
                ui.selectable_value(encoding, *kind, kind.label());
            }
        });
    ui.label(sciplot_core::t("分隔符"));
    egui::ComboBox::from_id_salt("csv-delimiter")
        .selected_text(delimiter.label())
        .show_ui(ui, |ui| {
            for kind in sciplot_core::CsvDelimiter::all() {
                ui.selectable_value(delimiter, *kind, kind.label());
            }
        });
    ui.label(sciplot_core::t("表头"));
    egui::ComboBox::from_id_salt("csv-header")
        .selected_text(header.label())
        .show_ui(ui, |ui| {
            for kind in sciplot_core::CsvHeader::all() {
                ui.selectable_value(header, *kind, kind.label());
            }
        });
    ui.label(sciplot_core::t("小数点"));
    egui::ComboBox::from_id_salt("csv-decimal")
        .selected_text(decimal.label())
        .show_ui(ui, |ui| {
            for kind in sciplot_core::DecimalMark::all() {
                ui.selectable_value(decimal, *kind, kind.label());
            }
        });
    ui.label(sciplot_core::t("跳过的行数"));
    ui.text_edit_singleline(skip);
    ui.label(sciplot_core::t("多个文件怎么放"));
    for kind in sciplot_core::CsvLayout::all() {
        ui.radio_value(layout, *kind, kind.label());
    }
    ui.label(sciplot_core::t("文件路径。每行一个，也可以用 | 分开。留空则选择文件。"));
    ui.text_edit_multiline(paths);
    if !error.is_empty() {
        ui.colored_label(egui::Color32::from_rgb(176, 32, 32), error.as_str());
    }
    if action_button(ui, sciplot_core::t("导入")) {
        let skip_rows = skip.trim().parse::<usize>().unwrap_or(0);
        let options = sciplot_core::CsvImportOptions {
            encoding: *encoding,
            delimiter: *delimiter,
            skip_rows,
            header: *header,
            decimal: *decimal,
        };
        let layout = *layout;
        let mut files = split_import_paths(paths);
        if files.is_empty() {
            if std::env::var("SCIPLOT_NO_DIALOG").is_ok() {
                *error = sciplot_core::t("请填写要导入的文件，每行一个路径。").into();
                return true;
            }
            match rfd::FileDialog::new()
                .set_directory(crate::app::default_dir(app))
                .add_filter("CSV", &["csv", "txt", "tsv", "dat"])
                .pick_files()
            {
                Some(picked) => files = picked,
                None => return true,
            }
        }
        match read_import_files(&files) {
            Ok(bytes) => match app.import_prepared(bytes, options, layout) {
                Ok(()) => return false,
                Err(err) => *error = err,
            },
            Err(err) => *error = err,
        }
    }
    !ui.button(sciplot_core::t("取消")).clicked()
}

fn split_import_paths(text: &str) -> Vec<std::path::PathBuf> {
    text.split(['\n', '|'])
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(std::path::PathBuf::from)
        .collect()
}

fn read_import_files(paths: &[std::path::PathBuf]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut files = Vec::new();
    for path in paths {
        let bytes = std::fs::read(path).map_err(|err| format!("{}: {err}", path.display()))?;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("import")
            .to_string();
        files.push((name, bytes));
    }
    if files.is_empty() {
        return Err(sciplot_core::t("没有要导入的文件").into());
    }
    Ok(files)
}

fn replace_known_extension(path: &str, ext: &str) -> String {
    let known = [
        "png", "svg", "eps", "pdf", "jpg", "jpeg", "bmp", "pbm", "pgm", "ppm", "xbm", "xpm", "sptpl", "spstyle",
    ];
    let file = std::path::Path::new(path);
    let Some(current) = file.extension().and_then(|item| item.to_str()) else {
        return path.to_string();
    };
    if known.iter().any(|item| item.eq_ignore_ascii_case(current)) {
        file.with_extension(ext).display().to_string()
    } else {
        path.to_string()
    }
}

fn graphs_for(app: &SciPlotApp, all: bool) -> Vec<sciplot_core::GraphId> {
    if all {
        app.project.graphs.iter().map(|graph| graph.id).collect()
    } else {
        app.active_graph().into_iter().collect()
    }
}

#[allow(clippy::too_many_arguments)]
fn arrange_layers_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    graph: sciplot_core::GraphId,
    rows: &mut String,
    cols: &mut String,
    align: &mut sciplot_core::LayerAlign,
    margin_left: &mut String,
    margin_right: &mut String,
    margin_top: &mut String,
    margin_bottom: &mut String,
    gap_x: &mut String,
    gap_y: &mut String,
    auto_rearrange: &mut bool,
) -> bool {
    ui.label(sciplot_core::tr!(
        "按行和列把图层铺到图上。边距和间距是整张图的百分比。删掉一层时，勾选下面的选项才会自动重排。",
        "Lay the layers out in rows and columns. Margins and gaps are percentages of the whole graph. After a layer is deleted, layers are rearranged only when the option below is on."
    ));
    labeled(ui, sciplot_core::tr!("行", "Rows"), rows);
    labeled(ui, sciplot_core::tr!("列", "Columns"), cols);
    egui::ComboBox::from_id_salt("layer-align")
        .selected_text(align.label())
        .show_ui(ui, |ui| {
            for item in sciplot_core::LayerAlign::all() {
                ui.selectable_value(align, *item, item.label());
            }
        });
    labeled(ui, sciplot_core::tr!("左边距 %", "Left margin %"), margin_left);
    labeled(ui, sciplot_core::tr!("右边距 %", "Right margin %"), margin_right);
    labeled(ui, sciplot_core::tr!("上边距 %", "Top margin %"), margin_top);
    labeled(ui, sciplot_core::tr!("下边距 %", "Bottom margin %"), margin_bottom);
    labeled(ui, sciplot_core::tr!("水平间距 %", "Horizontal gap %"), gap_x);
    labeled(ui, sciplot_core::tr!("垂直间距 %", "Vertical gap %"), gap_y);
    ui.checkbox(
        auto_rearrange,
        sciplot_core::tr!("删除图层后自动重排", "Rearrange automatically after a layer is deleted"),
    );
    let mut stay = true;
    ui.horizontal(|ui| {
        if action_button(ui, sciplot_core::t("应用")) {
            let arrangement = crate::app::arrangement_from_fields(
                rows,
                cols,
                *align,
                margin_left,
                margin_right,
                margin_top,
                margin_bottom,
                gap_x,
                gap_y,
                *auto_rearrange,
            );
            app.arrange_graph(graph, arrangement);
            stay = false;
        }
        if ui.button(sciplot_core::t("关闭")).clicked() {
            stay = false;
        }
    });
    stay
}

fn insert_layer_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    graph: sciplot_core::GraphId,
    source: &mut sciplot_core::GraphId,
) -> bool {
    let others: Vec<(sciplot_core::GraphId, String)> = app
        .project
        .graphs
        .iter()
        .filter(|item| item.id != graph)
        .map(|item| (item.id, item.name.clone()))
        .collect();
    if others.is_empty() {
        ui.label(sciplot_core::tr!(
            "项目里没有另一张图可以插入。",
            "There is no other graph to insert."
        ));
        return button_closes(ui, sciplot_core::t("关闭"));
    }
    if !others.iter().any(|(id, _)| *id == *source) {
        *source = others[0].0;
    }
    let selected = others
        .iter()
        .find(|(id, _)| *id == *source)
        .map(|(_, name)| name.clone())
        .unwrap_or_default();
    ui.label(sciplot_core::tr!(
        "把选中的图的每个图层追加到当前图，再按当前网格重排。",
        "Append every layer of the selected graph, then rearrange with the current grid."
    ));
    egui::ComboBox::from_id_salt("insert-graph")
        .selected_text(selected)
        .show_ui(ui, |ui| {
            for (id, name) in &others {
                ui.selectable_value(source, *id, name);
            }
        });
    let mut stay = true;
    ui.horizontal(|ui| {
        if action_button(ui, sciplot_core::tr!("插入", "Insert")) {
            app.insert_graph_as_layers(graph, *source);
            stay = false;
        }
        if ui.button(sciplot_core::t("取消")).clicked() {
            stay = false;
        }
    });
    stay
}

#[allow(clippy::too_many_arguments)]
fn export_graph_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    all: bool,
    format: &mut sciplot_plot::ExportFormat,
    path: &mut String,
    dpi: &mut String,
    quality: &mut String,
    transparent: &mut bool,
    paper: &mut sciplot_plot::Paper,
    orientation: &mut sciplot_plot::PageOrientation,
    fit_paper: &mut bool,
) -> bool {
    ui.label(if all {
        sciplot_core::tr!(
            "把项目里每张图写到这个文件夹，一张图一个文件。",
            "Write every graph in the project into this folder, one file each."
        )
    } else {
        sciplot_core::tr!("导出当前图。", "Export the current graph.")
    });
    egui::ComboBox::from_id_salt("export-format")
        .selected_text(format.label())
        .show_ui(ui, |ui| {
            for item in sciplot_plot::ExportFormat::all() {
                if ui.selectable_value(format, *item, item.label()).clicked() && !all {
                    *path = replace_known_extension(path, item.extension());
                }
            }
        });
    labeled(ui, sciplot_core::t("路径"), path);
    labeled(ui, sciplot_core::tr!("分辨率 (DPI)", "Resolution (DPI)"), dpi);
    if format.supports_quality() {
        labeled(ui, sciplot_core::tr!("质量 (1–100)", "Quality (1–100)"), quality);
    }
    if format.supports_transparency() {
        ui.checkbox(transparent, sciplot_core::tr!("透明背景", "Transparent background"));
    }
    egui::ComboBox::from_id_salt("export-paper")
        .selected_text(paper.label())
        .show_ui(ui, |ui| {
            for item in sciplot_plot::Paper::all() {
                ui.selectable_value(paper, *item, item.label());
            }
        });
    egui::ComboBox::from_id_salt("export-orient")
        .selected_text(orientation.label())
        .show_ui(ui, |ui| {
            for item in sciplot_plot::PageOrientation::all() {
                ui.selectable_value(orientation, *item, item.label());
            }
        });
    ui.checkbox(
        fit_paper,
        sciplot_core::tr!("按纸张尺寸出图", "Fit to paper size"),
    );
    ui.label(sciplot_core::tr!(
        "EPS 和 PDF 始终按纸张和方向排版。透明背景用于 PNG、XPM 和 SVG。JPEG 使用上面的质量。",
        "EPS and PDF always use the paper size and orientation. Transparency applies to PNG, XPM, and SVG. JPEG uses the quality above."
    ));
    let mut stay = true;
    ui.horizontal(|ui| {
        if action_button(ui, sciplot_core::tr!("导出", "Export")) {
            let options = crate::app::options_from_fields(
                *format,
                dpi,
                quality,
                *transparent,
                *paper,
                *orientation,
                *fit_paper,
            );
            let ids = graphs_for(app, all);
            match app.export_graphs_to(&ids, std::path::Path::new(path), &options, all) {
                Ok(_) => stay = false,
                Err(err) => app.status = err,
            }
        }
        if ui.button(sciplot_core::t("取消")).clicked() {
            stay = false;
        }
    });
    stay
}

fn print_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    all: bool,
    path: &mut String,
    dpi: &mut String,
    paper: &mut sciplot_plot::Paper,
    orientation: &mut sciplot_plot::PageOrientation,
) -> bool {
    ui.label(sciplot_core::tr!(
        "打印写成 PDF。没有系统打印机时，这个文件就是打印结果。",
        "Print writes a PDF. Without a system printer, that file is the printout."
    ));
    ui.label(if all {
        sciplot_core::tr!("打印项目里的全部图形，每张一页。", "Print every graph, one page each.")
    } else {
        sciplot_core::tr!("打印当前图形。", "Print the current graph.")
    });
    labeled(ui, sciplot_core::t("路径"), path);
    labeled(ui, sciplot_core::tr!("分辨率 (DPI)", "Resolution (DPI)"), dpi);
    egui::ComboBox::from_id_salt("print-paper")
        .selected_text(paper.label())
        .show_ui(ui, |ui| {
            for item in sciplot_plot::Paper::all() {
                ui.selectable_value(paper, *item, item.label());
            }
        });
    egui::ComboBox::from_id_salt("print-orient")
        .selected_text(orientation.label())
        .show_ui(ui, |ui| {
            for item in sciplot_plot::PageOrientation::all() {
                ui.selectable_value(orientation, *item, item.label());
            }
        });
    let mut stay = true;
    ui.horizontal(|ui| {
        if action_button(ui, sciplot_core::tr!("打印", "Print")) {
            let options = crate::app::options_from_fields(
                sciplot_plot::ExportFormat::Pdf,
                dpi,
                "90",
                false,
                *paper,
                *orientation,
                true,
            );
            let ids = graphs_for(app, all);
            match app.print_graphs_to(&ids, std::path::Path::new(path), &options) {
                Ok(()) => stay = false,
                Err(err) => app.status = err,
            }
        }
        if ui.button(sciplot_core::t("取消")).clicked() {
            stay = false;
        }
    });
    stay
}

fn template_file_dialog(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    save: bool,
    graph: sciplot_core::GraphId,
    path: &mut String,
) -> bool {
    ui.label(sciplot_core::tr!(
        "SciPlot 模板是 .sptpl。它记下画布大小、字体、颜色和文字标注，打开后是一张空图，再用「添加曲线」填数据。曲线样式和刻度不在模板里。这不是 SciDAVis 的 .qpt。",
        "A SciPlot template is a .sptpl. It stores canvas size, fonts, colors, and text labels, then opens as an empty graph to fill with Add curve. Curve styles and ticks are not stored. This is not a SciDAVis .qpt."
    ));
    labeled(ui, sciplot_core::t("路径"), path);
    let mut stay = true;
    ui.horizontal(|ui| {
        let label = if save {
            sciplot_core::tr!("保存", "Save")
        } else {
            sciplot_core::tr!("打开", "Open")
        };
        if action_button(ui, label) {
            let file = std::path::Path::new(path);
            let result = if save {
                app.save_graph_template(graph, file).map(|_| ())
            } else {
                app.open_graph_template(file).map(|_| ())
            };
            match result {
                Ok(()) => stay = false,
                Err(err) => app.status = err,
            }
        }
        if ui.button(sciplot_core::t("取消")).clicked() {
            stay = false;
        }
    });
    stay
}

fn style_file_dialog(app: &mut SciPlotApp, ui: &mut egui::Ui, save: bool, path: &mut String) -> bool {
    ui.label(sciplot_core::tr!(
        "默认样式是 .spstyle，只含字体文件、字号、线型、符号、直方图箱数和颜色。之后新建的图和曲线会用它。它和图形模板不是同一种文件。",
        "A default style is a .spstyle. It stores the font file, sizes, line, symbol, histogram bins, and colors used by new graphs and new curves. It is not a graph template."
    ));
    labeled(ui, sciplot_core::t("路径"), path);
    let mut stay = true;
    ui.horizontal(|ui| {
        let label = if save {
            sciplot_core::tr!("保存", "Save")
        } else {
            sciplot_core::tr!("打开", "Open")
        };
        if action_button(ui, label) {
            let file = std::path::Path::new(path);
            let result = if save {
                app.save_style_file(file)
            } else {
                app.load_style_file(file)
            };
            match result {
                Ok(()) => {
                    if save {
                        app.status = sciplot_core::tr!(
                            "已导出默认样式。这不是图形模板。",
                            "Exported the default style. This is not a graph template."
                        )
                        .into();
                    } else if !app.reload_font(ui.ctx()) {
                        app.prefs.font_path.clear();
                        let _ = crate::prefs::save(&app.prefs);
                        app.reload_font(ui.ctx());
                        app.status = sciplot_core::t("找不到指定字体，已改用自动字体并保存其余设置").into();
                    }
                    stay = false;
                }
                Err(err) => app.status = err,
            }
        }
        if ui.button(sciplot_core::t("取消")).clicked() {
            stay = false;
        }
    });
    stay
}
