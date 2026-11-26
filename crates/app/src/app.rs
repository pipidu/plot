use std::collections::HashMap;
use std::path::{Path, PathBuf};

use egui_dock::{DockArea, DockState, TabViewer};
use egui_extras::{Column, TableBuilder};
use sciplot_core::{
    demo_project, export_csv, import_csv, AnnotationKind, ColumnMode, CurveKind, Graph, GraphId, History, Layer,
    MatrixId, NoteId, Project, SurfaceMode, SurfaceSource, TableId,
};
use sciplot_plot::{
    data_to_screen, nearest_point, pan_limits, render, screen_to_data, to_png, to_rgba, to_svg, zoom_limits,
    GlyphFont, LayerFrame, LayerView, Scene,
};

use crate::dialogs::{self, number_text};
use crate::fonts;
use crate::i18n;
use crate::ops;

#[derive(Clone, PartialEq, Eq)]
pub enum Tab {
    Table(TableId),
    Matrix(MatrixId),
    Graph(GraphId),
    Note(NoteId),
}

impl Tab {
    fn key(&self) -> String {
        match self {
            Tab::Table(id) => format!("table-{}", id.0),
            Tab::Matrix(id) => format!("matrix-{}", id.0),
            Tab::Graph(id) => format!("graph-{}", id.0),
            Tab::Note(id) => format!("note-{}", id.0),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LineChoice {
    Curve,
    Horizontal,
    Vertical,
}

impl LineChoice {
    pub fn label(self) -> &'static str {
        match self {
            LineChoice::Curve => "曲线",
            LineChoice::Horizontal => "水平线 y=c",
            LineChoice::Vertical => "竖直线 x=c",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FitWhich {
    Linear,
    Poly,
    Exp,
    Gauss,
    Lorentz,
    Custom,
}

impl FitWhich {
    pub fn label(self) -> &'static str {
        match self {
            FitWhich::Linear => "线性",
            FitWhich::Poly => "多项式",
            FitWhich::Exp => "指数",
            FitWhich::Gauss => "高斯",
            FitWhich::Lorentz => "洛伦兹",
            FitWhich::Custom => "自定义",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RangeOp {
    Integral,
    Interp,
    Smooth,
    Filter,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PathAction {
    Open,
    SaveAs,
    ImportCsv,
    ExportCsv,
    ExportPng,
    ExportSvg,
}

#[derive(Clone)]
pub enum Dialog {
    Message(String),
    Quit,
    Help,
    About,
    Path { action: PathAction, path: String, extra: String },
    Formula { table: TableId, column: usize, expr: String },
    MatrixFormula { matrix: MatrixId, expr: String },
    NewMatrix { name: String, rows: String, cols: String },
    ColumnProps { table: TableId, column: usize },
    Sort { table: TableId, column: usize, ascending: bool },
    Axis { graph: GraphId, layer: usize, y_axis: bool, axis: sciplot_core::Axis, min: String, max: String },
    Legend { graph: GraphId, layer: usize, legend: sciplot_core::Legend },
    Title { graph: GraphId, layer: usize, title: String, size: String },
    Style { graph: GraphId, layer: usize, curve: usize, style: sciplot_core::CurveStyle, name: String },
    Frame { graph: GraphId, layer: usize, x: String, y: String, w: String, h: String },
    TextAnn { graph: GraphId, layer: usize, x: String, y: String, text: String },
    ArrowAnn { graph: GraphId, layer: usize, x1: String, y1: String, x2: String, y2: String, line_only: bool },
    Intersect {
        graph: GraphId,
        layer: usize,
        curve_a: usize,
        curve_b: usize,
        kind_a: LineChoice,
        kind_b: LineChoice,
        value_a: String,
        value_b: String,
        smooth: bool,
        xmin: String,
        xmax: String,
        tol: String,
    },
    Fit { graph: GraphId, layer: usize, curve: usize, which: FitWhich, degree: String, expr: String, initial: String },
    Range {
        graph: GraphId,
        layer: usize,
        curve: usize,
        op: RangeOp,
        a: String,
        b: String,
        n: String,
        method: sciplot_analysis::InterpMethod,
        window: String,
        order: String,
        sg: bool,
        highpass: bool,
    },
    Function {
        graph: Option<GraphId>,
        layer: usize,
        expr: String,
        xmin: String,
        xmax: String,
        ymin: String,
        ymax: String,
        samples: String,
        surface: bool,
        mode: SurfaceMode,
    },
    AddCurve { graph: GraphId, table: TableId, kind: CurveKind },
}

struct XSel {
    graph: GraphId,
    layer: usize,
    x0: f64,
    x1: f64,
}

struct AnnDrag {
    graph: GraphId,
    layer: usize,
    id: u64,
    origin: AnnotationKind,
    start_x: f64,
    start_y: f64,
}

pub struct SciPlotApp {
    pub project: Project,
    history: History,
    dock: DockState<Tab>,
    pub dialog: Option<Dialog>,
    pub status: String,
    path: Option<PathBuf>,
    pub dirty: bool,
    recent: Vec<PathBuf>,
    focus: Option<Tab>,
    views: HashMap<GraphId, Vec<LayerView>>,
    show_tree: bool,
    show_log: bool,
    reader: bool,
    xsel: Option<XSel>,
    ann_drag: Option<AnnDrag>,
    selected_ann: Option<(GraphId, usize, u64)>,
    selected_column: Option<(TableId, usize)>,
    active_layer: usize,
    curve: usize,
    pushed_edit: bool,
    font: Vec<u8>,
    glyphs: Option<sciplot_plot::GlyphFont>,
    frames: u32,
}

pub fn run() -> eframe::Result<()> {
    let viewport = egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]).with_title(i18n::APP_TITLE);
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        i18n::APP_TITLE,
        options,
        Box::new(|cc| {
            let app = SciPlotApp::demo();
            fonts::install(&cc.egui_ctx, &app.font);
            Ok(Box::new(app))
        }),
    )
}

impl SciPlotApp {
    fn demo() -> Self {
        let mut project = demo_project();
        ops::enrich_demo(&mut project);
        let mut app = Self::empty(project);
        app.rebuild_dock(true);
        app.status = "已载入示例。正弦与余弦的交点已标在图上。".into();
        app
    }

    fn empty(project: Project) -> Self {
        let mut dock = DockState::new(Vec::<Tab>::new());
        chinese_dock(&mut dock);
        Self {
            project,
            history: History::new(),
            dock,
            dialog: None,
            status: "就绪".into(),
            path: None,
            dirty: false,
            recent: load_recent(),
            focus: None,
            views: HashMap::new(),
            show_tree: true,
            show_log: true,
            reader: false,
            xsel: None,
            ann_drag: None,
            selected_ann: None,
            selected_column: None,
            active_layer: 0,
            curve: 0,
            pushed_edit: false,
            font: fonts::load_bytes(),
            glyphs: None,
            frames: 0,
        }
    }

    pub fn edit<T>(&mut self, label: &str, f: impl FnOnce(&mut Project) -> Result<T, String>) -> Option<T> {
        let before = self.project.clone();
        match f(&mut self.project) {
            Ok(value) => {
                self.history.push(label, &before);
                self.dirty = true;
                Some(value)
            }
            Err(err) => {
                self.project = before;
                self.status = err;
                None
            }
        }
    }

    pub fn open_tab(&mut self, tab: Tab) {
        if let Some(found) = self.dock.find_tab(&tab) {
            self.dock.set_active_tab(found);
            self.dock.set_focused_node_and_surface((found.0, found.1));
        } else {
            self.dock.push_to_focused_leaf(tab.clone());
        }
        self.focus = Some(tab);
    }

    fn rebuild_dock(&mut self, demo: bool) {
        self.dock = DockState::new(Vec::<Tab>::new());
        chinese_dock(&mut self.dock);
        if demo {
            if let Some(table) = self.project.tables.iter().find(|t| t.name == "三角函数").map(|t| t.id) {
                self.open_tab(Tab::Table(table));
            }
            if let Some(graph) = self.project.graphs.iter().find(|g| g.name == "正弦与余弦").map(|g| g.id) {
                self.open_tab(Tab::Graph(graph));
            }
            return;
        }
        let tabs: Vec<Tab> = self
            .project
            .tables
            .iter()
            .map(|t| Tab::Table(t.id))
            .chain(self.project.matrices.iter().map(|m| Tab::Matrix(m.id)))
            .chain(self.project.graphs.iter().map(|g| Tab::Graph(g.id)))
            .chain(self.project.notes.iter().map(|n| Tab::Note(n.id)))
            .collect();
        for tab in tabs {
            self.open_tab(tab);
        }
    }

    fn active_table(&self) -> Option<TableId> {
        match self.focus {
            Some(Tab::Table(id)) => Some(id),
            _ => self.selected_column.map(|(id, _)| id).or_else(|| self.project.tables.first().map(|t| t.id)),
        }
    }

    fn active_matrix(&self) -> Option<MatrixId> {
        match self.focus {
            Some(Tab::Matrix(id)) => Some(id),
            _ => self.project.matrices.first().map(|m| m.id),
        }
    }

    fn active_graph(&self) -> Option<GraphId> {
        match self.focus {
            Some(Tab::Graph(id)) => Some(id),
            _ => self.project.graphs.iter().find(|g| g.surface.is_none()).map(|g| g.id),
        }
    }

    fn layer_index(&self, graph: GraphId) -> usize {
        let n = self.project.graph(graph).map(|g| g.layers.len()).unwrap_or(1);
        self.active_layer.min(n.saturating_sub(1))
    }

    pub fn save_current(&mut self) -> bool {
        if let Some(path) = self.path.clone() {
            self.write_path(&path)
        } else {
            self.status = "还没有文件名，请用「另存为」".into();
            self.dialog = Some(Dialog::Path {
                action: PathAction::SaveAs,
                path: format!("{}.splot", self.project.name),
                extra: String::new(),
            });
            false
        }
    }

    fn write_path(&mut self, path: &Path) -> bool {
        match self.project.to_json().and_then(|text| std::fs::write(path, text).map_err(|err| err.to_string())) {
            Ok(()) => {
                self.path = Some(path.to_path_buf());
                self.dirty = false;
                remember(&mut self.recent, path);
                self.status = format!("已保存 {}", path.display());
                true
            }
            Err(err) => {
                self.status = err;
                false
            }
        }
    }

    pub fn perform_path(&mut self, action: PathAction, path: &str, extra: &str) {
        let path = PathBuf::from(path.trim());
        if path.as_os_str().is_empty() {
            self.status = "路径是空的".into();
            return;
        }
        match action {
            PathAction::Open => self.load_path(&path),
            PathAction::SaveAs => {
                self.write_path(&path);
            }
            PathAction::ImportCsv => {
                match std::fs::read(&path).map_err(|err| err.to_string()).and_then(|bytes| import_csv(&bytes)) {
                    Ok(table) => {
                        if let Some(id) = self.edit("导入 CSV", move |project| Ok(project.insert_table(table))) {
                            self.open_tab(Tab::Table(id));
                            self.status = format!("已导入 {}", path.display());
                        }
                    }
                    Err(err) => self.status = err,
                }
            }
            PathAction::ExportCsv => {
                let Some(id) = self.active_table() else {
                    self.status = "没有可导出的表格".into();
                    return;
                };
                let Some(table) = self.project.table(id) else { return };
                let bytes = export_csv(table);
                match std::fs::write(&path, bytes) {
                    Ok(()) => self.status = format!("已导出 {}", path.display()),
                    Err(err) => self.status = err.to_string(),
                }
            }
            PathAction::ExportPng | PathAction::ExportSvg => self.export_graph(action, &path, extra),
        }
    }

    fn load_path(&mut self, path: &Path) {
        match std::fs::read_to_string(path).map_err(|err| err.to_string()).and_then(|text| Project::from_json(&text)) {
            Ok(project) => {
                self.project = project;
                self.history = History::new();
                self.views.clear();
                self.path = Some(path.to_path_buf());
                self.dirty = false;
                remember(&mut self.recent, path);
                self.rebuild_dock(false);
                self.status = format!("已打开 {}", path.display());
            }
            Err(err) => self.status = err,
        }
    }

    fn export_graph(&mut self, action: PathAction, path: &Path, extra: &str) {
        let Some(scene) = self.scene_of_active(extra) else {
            self.status = "没有可导出的图形".into();
            return;
        };
        let result = match action {
            PathAction::ExportSvg => std::fs::write(path, to_svg(&scene)).map_err(|err| err.to_string()),
            _ => {
                let font = GlyphFont::from_bytes(&self.font);
                to_png(&scene, font.as_ref()).and_then(|bytes| std::fs::write(path, bytes).map_err(|err| err.to_string()))
            }
        };
        self.status = match result {
            Ok(()) => format!("已导出 {}", path.display()),
            Err(err) => err,
        };
    }

    fn scene_of_active(&self, dpi_text: &str) -> Option<Scene> {
        let id = self.active_graph()?;
        let graph = self.project.graph(id)?.clone();
        let views = self.views.get(&id).cloned().unwrap_or_default();
        let dpi = dpi_text.trim().parse::<f32>().unwrap_or(144.0).clamp(72.0, 600.0);
        let scale = dpi / 96.0;
        Some(render(&self.project, &graph, &views, 960.0 * scale, 640.0 * scale, scale))
    }

    fn copy_png(&mut self) {
        let Some(scene) = self.scene_of_active("144") else {
            self.status = "没有可复制的图形".into();
            return;
        };
        let font = GlyphFont::from_bytes(&self.font);
        match to_rgba(&scene, font.as_ref()) {
            Ok((w, h, bytes)) => match arboard::Clipboard::new().and_then(|mut clip| {
                clip.set_image(arboard::ImageData {
                    width: w as usize,
                    height: h as usize,
                    bytes: std::borrow::Cow::Owned(bytes),
                })
            }) {
                Ok(()) => self.status = "已复制 PNG 到剪贴板".into(),
                Err(err) => self.status = format!("剪贴板不可用：{err}"),
            },
            Err(err) => self.status = err,
        }
    }

    fn copy_svg(&mut self) {
        let Some(scene) = self.scene_of_active("96") else {
            self.status = "没有可复制的图形".into();
            return;
        };
        match arboard::Clipboard::new().and_then(|mut clip| clip.set_text(to_svg(&scene))) {
            Ok(()) => self.status = "已复制 SVG 到剪贴板".into(),
            Err(err) => self.status = format!("剪贴板不可用：{err}"),
        }
    }

    fn undo(&mut self) {
        if let Some((label, project)) = self.history.undo(&self.project) {
            self.project = project;
            self.dirty = true;
            self.status = format!("已撤销：{label}");
        }
    }

    fn redo(&mut self) {
        if let Some((label, project)) = self.history.redo(&self.project) {
            self.project = project;
            self.dirty = true;
            self.status = format!("已重做：{label}");
        }
    }

    fn plot_kind(&mut self, kind: CurveKind) {
        if matches!(kind, CurveKind::Heatmap | CurveKind::Contour) {
            self.plot_matrix(kind);
            return;
        }
        let Some(table) = self.active_table() else {
            self.status = "先打开或选中一张表格".into();
            return;
        };
        if let Some(id) = self.edit("绘图", |project| project.plot_table(table, kind)) {
            self.open_tab(Tab::Graph(id));
            self.status = format!("已新建{}", kind.label());
        }
    }

    fn plot_matrix(&mut self, kind: CurveKind) {
        let Some(matrix) = self.active_matrix() else {
            self.status = "热力图和等高线需要一个矩阵".into();
            return;
        };
        if let Some(id) = self.edit("绘图", |project| project.plot_matrix(matrix, kind)) {
            self.open_tab(Tab::Graph(id));
            self.status = format!("已新建{}", kind.label());
        }
    }

    fn plot_surface(&mut self, mode: SurfaceMode) {
        let Some(matrix) = self.active_matrix() else {
            self.status = "先选中一个矩阵，或用函数曲面".into();
            return;
        };
        let title = self.project.matrix(matrix).map(|m| m.name.clone()).unwrap_or_else(|| "三维图".into());
        if let Some(id) = self.edit("三维图", |project| Ok(project.plot_surface(SurfaceSource::Matrix(matrix), mode, &title))) {
            self.open_tab(Tab::Graph(id));
            self.status = format!("已新建三维{}", mode.label());
        }
    }

    fn require_graph(&mut self) -> Option<(GraphId, usize)> {
        let graph = self.active_graph()?;
        let layer = self.layer_index(graph);
        Some((graph, layer))
    }

    fn tab_title(&self, tab: &Tab) -> String {
        match tab {
            Tab::Table(id) => self.project.table(*id).map(|t| t.name.clone()).unwrap_or_else(|| "表格".into()),
            Tab::Matrix(id) => self.project.matrix(*id).map(|m| m.name.clone()).unwrap_or_else(|| "矩阵".into()),
            Tab::Graph(id) => self.project.graph(*id).map(|g| g.name.clone()).unwrap_or_else(|| "图".into()),
            Tab::Note(id) => self.project.notes.iter().find(|n| n.id == *id).map(|n| n.name.clone()).unwrap_or_else(|| "备注".into()),
        }
    }
}

impl eframe::App for SciPlotApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frames += 1;
        if let Ok(limit) = std::env::var("SCIPLOT_EXIT_FRAMES") {
            if self.frames > limit.parse::<u32>().unwrap_or(40) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.dirty && !matches!(self.dialog, Some(Dialog::Quit)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.dialog = Some(Dialog::Quit);
        }
        shortcuts(self, ctx);
        egui::TopBottomPanel::top("menu").show(ctx, |ui| menus(self, ui));
        if self.show_tree {
            egui::SidePanel::left("tree").resizable(true).default_width(220.0).show(ctx, |ui| tree(self, ui));
        }
        egui::TopBottomPanel::bottom("status").exact_height(24.0).show(ctx, |ui| {
            ui.label(&self.status);
        });
        if self.show_log {
            egui::TopBottomPanel::bottom("log").resizable(true).default_height(120.0).show(ctx, |ui| log_view(self, ui));
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut dock = std::mem::replace(&mut self.dock, DockState::new(Vec::new()));
            DockArea::new(&mut dock).show_inside(ui, &mut Viewer { app: self });
            self.dock = dock;
        });
        dialogs::show(self, ctx);
    }
}

fn shortcuts(app: &mut SciPlotApp, ctx: &egui::Context) {
    let cmd = egui::Modifiers::COMMAND;
    if ctx.input_mut(|i| i.consume_key(cmd, egui::Key::N)) {
        new_project(app);
    }
    if ctx.input_mut(|i| i.consume_key(cmd, egui::Key::O)) {
        browse_open(app);
    }
    if ctx.input_mut(|i| i.consume_key(cmd, egui::Key::S)) {
        app.save_current();
    }
    if ctx.input_mut(|i| i.consume_key(cmd, egui::Key::Z)) {
        app.undo();
    }
    if ctx.input_mut(|i| i.consume_key(cmd, egui::Key::Y)) {
        app.redo();
    }
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Delete)) {
        delete_selected_annotation(app);
    }
}

fn menus(app: &mut SciPlotApp, ui: &mut egui::Ui) {
    egui::menu::bar(ui, |ui| {
        ui.menu_button(i18n::FILE, |ui| file_menu(app, ui));
        ui.menu_button(i18n::EDIT, |ui| edit_menu(app, ui));
        ui.menu_button(i18n::TABLE, |ui| table_menu(app, ui));
        ui.menu_button(i18n::MATRIX, |ui| matrix_menu(app, ui));
        ui.menu_button(i18n::PLOT, |ui| plot_menu(app, ui));
        ui.menu_button(i18n::ANALYSIS, |ui| analysis_menu(app, ui));
        ui.menu_button(i18n::FORMAT, |ui| format_menu(app, ui));
        ui.menu_button(i18n::WINDOW, |ui| window_menu(app, ui));
        ui.menu_button(i18n::HELP, |ui| {
            if clicked(ui, "公式说明", "") {
                app.dialog = Some(Dialog::Help);
            }
            if clicked(ui, "关于", "") {
                app.dialog = Some(Dialog::About);
            }
        });
    });
}

fn clicked(ui: &mut egui::Ui, label: &str, shortcut: &str) -> bool {
    let mut button = egui::Button::new(label);
    if !shortcut.is_empty() {
        button = button.shortcut_text(shortcut);
    }
    let hit = ui.add(button).clicked();
    if hit {
        ui.close_menu();
    }
    hit
}

fn file_menu(app: &mut SciPlotApp, ui: &mut egui::Ui) {
    if clicked(ui, "新建项目", "Ctrl+N") {
        new_project(app);
    }
    if clicked(ui, "打开…", "Ctrl+O") {
        browse_open(app);
    }
    if clicked(ui, "保存", "Ctrl+S") {
        app.save_current();
    }
    if clicked(ui, "另存为…", "") {
        app.dialog = Some(Dialog::Path { action: PathAction::SaveAs, path: format!("{}.splot", app.project.name), extra: String::new() });
    }
    ui.separator();
    if clicked(ui, "导入 CSV…", "") {
        browse_or_dialog(app, PathAction::ImportCsv, "数据.csv");
    }
    if clicked(ui, "导出 CSV…", "") {
        app.dialog = Some(Dialog::Path { action: PathAction::ExportCsv, path: "数据.csv".into(), extra: String::new() });
    }
    if clicked(ui, "导出 PNG…", "") {
        app.dialog = Some(Dialog::Path { action: PathAction::ExportPng, path: "图形.png".into(), extra: "144".into() });
    }
    if clicked(ui, "导出 SVG…", "") {
        app.dialog = Some(Dialog::Path { action: PathAction::ExportSvg, path: "图形.svg".into(), extra: String::new() });
    }
    if clicked(ui, "复制 PNG", "") {
        app.copy_png();
    }
    if clicked(ui, "复制 SVG", "") {
        app.copy_svg();
    }
    ui.separator();
    ui.menu_button("最近文件", |ui| {
        if app.recent.is_empty() {
            ui.label("（空）");
        }
        let recent = app.recent.clone();
        for path in recent {
            if ui.button(path.display().to_string()).clicked() {
                app.load_path(&path);
            }
        }
    });
    if clicked(ui, "退出", "") {
        if app.dirty {
            app.dialog = Some(Dialog::Quit);
        } else {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn edit_menu(app: &mut SciPlotApp, ui: &mut egui::Ui) {
    if clicked(ui, "撤销", "Ctrl+Z") {
        app.undo();
    }
    if clicked(ui, "重做", "Ctrl+Y") {
        app.redo();
    }
    if clicked(ui, "删除当前对象", "") {
        delete_focused(app);
    }
}

fn table_menu(app: &mut SciPlotApp, ui: &mut egui::Ui) {
    if clicked(ui, "新建表格", "") {
        if let Some(id) = app.edit("新建表格", |project| Ok(project.new_table("表格"))) {
            app.open_tab(Tab::Table(id));
        }
    }
    if clicked(ui, "加列", "") {
        let id = app.active_table();
        if let Some(id) = id {
            app.edit("加列", |project| {
                let table = project.table_mut(id).ok_or("找不到表格")?;
                let n = table.columns.len() + 1;
                table.add_column(format!("C{n}"), ColumnMode::Y);
                Ok(())
            });
        }
    }
    if clicked(ui, "加行", "") {
        if let Some(id) = app.active_table() {
            app.edit("加行", |project| {
                project.table_mut(id).ok_or("找不到表格")?.add_row();
                Ok(())
            });
        }
    }
    if clicked(ui, "删除列", "") {
        if let Some((id, column)) = app.selected_column.or_else(|| app.active_table().map(|id| (id, 0))) {
            app.edit("删除列", |project| {
                if !project.table_mut(id).ok_or("找不到表格")?.remove_column(column) {
                    return Err("没有这一列".into());
                }
                Ok(())
            });
        }
    }
    if clicked(ui, "删除末行", "") {
        if let Some(id) = app.active_table() {
            app.edit("删除末行", |project| {
                let table = project.table_mut(id).ok_or("找不到表格")?;
                let n = table.nrows();
                if n == 0 {
                    return Err("表是空的".into());
                }
                table.remove_row(n - 1);
                Ok(())
            });
        }
    }
    if clicked(ui, "列公式…", "") {
        if let Some((table, column)) = app.selected_column.or_else(|| app.active_table().map(|id| (id, 0))) {
            let expr = app.project.table(table).and_then(|t| t.columns.get(column)).map(|c| c.formula.clone()).unwrap_or_default();
            app.dialog = Some(Dialog::Formula { table, column, expr });
        }
    }
    if clicked(ui, "重算公式", "") {
        if let Some(id) = app.active_table() {
            let formulas: Vec<(usize, String)> = app
                .project
                .table(id)
                .map(|t| t.columns.iter().enumerate().filter(|(_, c)| !c.formula.is_empty()).map(|(i, c)| (i, c.formula.clone())).collect())
                .unwrap_or_default();
            app.edit("重算公式", |project| {
                let table = project.table_mut(id).ok_or("找不到表格")?;
                for (index, formula) in &formulas {
                    sciplot_core::apply_formula(table, *index, formula)?;
                }
                Ok(())
            });
            app.status = "已重算列公式".into();
        }
    }
    if clicked(ui, "排序…", "") {
        if let Some(table) = app.active_table() {
            let column = app.selected_column.filter(|(id, _)| *id == table).map(|(_, c)| c).unwrap_or(0);
            app.dialog = Some(Dialog::Sort { table, column, ascending: true });
        }
    }
    if clicked(ui, "统计", "") {
        if let Some((table, column)) = app.selected_column.or_else(|| app.active_table().map(|id| (id, 0))) {
            match ops::column_stats(&app.project, table, column) {
                Ok(text) => app.dialog = Some(Dialog::Message(text)),
                Err(err) => app.status = err,
            }
        }
    }
    if clicked(ui, "列类型与角色…", "") {
        if let Some((table, column)) = app.selected_column.or_else(|| app.active_table().map(|id| (id, 0))) {
            app.dialog = Some(Dialog::ColumnProps { table, column });
        }
    }
}

fn matrix_menu(app: &mut SciPlotApp, ui: &mut egui::Ui) {
    if clicked(ui, "新建矩阵…", "") {
        app.dialog = Some(Dialog::NewMatrix { name: "矩阵".into(), rows: "10".into(), cols: "10".into() });
    }
    if clicked(ui, "公式填充…", "") {
        if let Some(matrix) = app.active_matrix() {
            let expr = app.project.matrix(matrix).map(|m| m.formula.clone()).unwrap_or_default();
            app.dialog = Some(Dialog::MatrixFormula { matrix, expr });
        }
    }
    if clicked(ui, "转置", "") {
        if let Some(matrix) = app.active_matrix() {
            if let Some(id) = app.edit("转置", |project| ops::transpose_into_new(project, matrix)) {
                app.open_tab(Tab::Matrix(id));
            }
        }
    }
    if clicked(ui, "求逆", "") {
        if let Some(matrix) = app.active_matrix() {
            if let Some(id) = app.edit("求逆", |project| ops::invert_into_new(project, matrix)) {
                app.open_tab(Tab::Matrix(id));
            }
        }
    }
    if clicked(ui, "转为表格", "") {
        if let Some(matrix) = app.active_matrix() {
            if let Some(id) = app.edit("矩阵转表", |project| project.table_from_matrix(matrix)) {
                app.open_tab(Tab::Table(id));
            }
        }
    }
    if clicked(ui, "从表格生成", "") {
        if let Some(table) = app.active_table() {
            if let Some(id) = app.edit("表转矩阵", |project| project.matrix_from_table(table)) {
                app.open_tab(Tab::Matrix(id));
            }
        }
    }
}

fn plot_menu(app: &mut SciPlotApp, ui: &mut egui::Ui) {
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
        CurveKind::Heatmap,
        CurveKind::Contour,
    ] {
        if clicked(ui, kind.label(), "") {
            app.plot_kind(kind);
        }
    }
    ui.separator();
    if clicked(ui, "添加到当前图…", "") {
        if let Some(graph) = app.active_graph() {
            let table = app.active_table().unwrap_or(TableId(0));
            app.dialog = Some(Dialog::AddCurve { graph, table, kind: CurveKind::Line });
        }
    }
    if clicked(ui, "新建图层", "") {
        if let Some(graph) = app.active_graph() {
            app.edit("新建图层", |project| {
                let graph = project.graph_mut(graph).ok_or("找不到图形")?;
                graph.layers.push(Layer::default());
                graph.reflow_layers();
                Ok(())
            });
        }
    }
    if clicked(ui, "删除图层", "") {
        if let Some(graph) = app.active_graph() {
            let layer = app.layer_index(graph);
            app.edit("删除图层", |project| {
                let graph = project.graph_mut(graph).ok_or("找不到图形")?;
                if graph.layers.len() <= 1 {
                    return Err("至少保留一个图层".into());
                }
                graph.layers.remove(layer.min(graph.layers.len() - 1));
                graph.reflow_layers();
                Ok(())
            });
        }
    }
    ui.separator();
    if clicked(ui, "三维曲面", "") {
        app.plot_surface(SurfaceMode::Surface);
    }
    if clicked(ui, "三维线框", "") {
        app.plot_surface(SurfaceMode::Wireframe);
    }
    if clicked(ui, "三维散点", "") {
        app.plot_surface(SurfaceMode::Scatter);
    }
    if clicked(ui, "函数曲线…", "") {
        let graph = app.active_graph();
        app.dialog = Some(Dialog::Function {
            graph,
            layer: graph.map(|id| app.layer_index(id)).unwrap_or(0),
            expr: "sin(x)".into(),
            xmin: "0".into(),
            xmax: "6.2832".into(),
            ymin: "-3.14".into(),
            ymax: "3.14".into(),
            samples: "200".into(),
            surface: false,
            mode: SurfaceMode::Surface,
        });
    }
    if clicked(ui, "函数曲面…", "") {
        app.dialog = Some(Dialog::Function {
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
        });
    }
}

fn analysis_menu(app: &mut SciPlotApp, ui: &mut egui::Ui) {
    let Some((graph, layer)) = app.require_graph() else {
        ui.label("先打开一张图");
        return;
    };
    let curve = app.curve;
    if clicked(ui, "线性拟合…", "") {
        app.dialog = Some(fit_dialog(graph, layer, curve, FitWhich::Linear));
    }
    if clicked(ui, "多项式拟合…", "") {
        app.dialog = Some(fit_dialog(graph, layer, curve, FitWhich::Poly));
    }
    if clicked(ui, "指数拟合…", "") {
        app.dialog = Some(fit_dialog(graph, layer, curve, FitWhich::Exp));
    }
    if clicked(ui, "高斯拟合…", "") {
        app.dialog = Some(fit_dialog(graph, layer, curve, FitWhich::Gauss));
    }
    if clicked(ui, "洛伦兹拟合…", "") {
        app.dialog = Some(fit_dialog(graph, layer, curve, FitWhich::Lorentz));
    }
    if clicked(ui, "自定义拟合…", "") {
        app.dialog = Some(fit_dialog(graph, layer, curve, FitWhich::Custom));
    }
    ui.separator();
    if clicked(ui, "微分", "") {
        if let Some((table, gid)) = app.edit("微分", |project| {
            let table = ops::derivative_table(project, graph, layer, curve)?;
            let gid = project.plot_table(table, CurveKind::Line)?;
            Ok((table, gid))
        }) {
            app.open_tab(Tab::Table(table));
            app.open_tab(Tab::Graph(gid));
        }
    }
    if clicked(ui, "积分…", "") {
        app.dialog = Some(range_dialog(graph, layer, curve, RangeOp::Integral));
    }
    if clicked(ui, "插值…", "") {
        app.dialog = Some(range_dialog(graph, layer, curve, RangeOp::Interp));
    }
    if clicked(ui, "平滑…", "") {
        app.dialog = Some(range_dialog(graph, layer, curve, RangeOp::Smooth));
    }
    if clicked(ui, "FFT", "") {
        if let Some((table, gid)) = app.edit("FFT", |project| {
            let table = ops::fft_table(project, graph, layer, curve)?;
            let gid = project.plot_table(table, CurveKind::Line)?;
            Ok((table, gid))
        }) {
            app.open_tab(Tab::Table(table));
            app.open_tab(Tab::Graph(gid));
        }
    }
    if clicked(ui, "FFT 滤波…", "") {
        app.dialog = Some(range_dialog(graph, layer, curve, RangeOp::Filter));
    }
    if clicked(ui, "相关性", "") {
        if let Some(table) = app.active_table() {
            match ops::correlation_report(&app.project, table) {
                Ok(text) => {
                    app.project.log_line(&text);
                    app.dialog = Some(Dialog::Message(text));
                }
                Err(err) => app.status = err,
            }
        }
    }
    ui.separator();
    if clicked(ui, "求交点…", "") {
        let (xmin, xmax) = app
            .xsel
            .as_ref()
            .filter(|sel| sel.graph == graph)
            .map(|sel| (number_text(Some(sel.x0.min(sel.x1))), number_text(Some(sel.x0.max(sel.x1)))))
            .unwrap_or_default();
        app.dialog = Some(Dialog::Intersect {
            graph,
            layer,
            curve_a: 0,
            curve_b: 1,
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
}

fn fit_dialog(graph: GraphId, layer: usize, curve: usize, which: FitWhich) -> Dialog {
    Dialog::Fit { graph, layer, curve, which, degree: "2".into(), expr: "a*x+b".into(), initial: "a=1, b=0".into() }
}

fn range_dialog(graph: GraphId, layer: usize, curve: usize, op: RangeOp) -> Dialog {
    Dialog::Range {
        graph,
        layer,
        curve,
        op,
        a: "0".into(),
        b: "1".into(),
        n: "200".into(),
        method: sciplot_analysis::InterpMethod::CubicSpline,
        window: "7".into(),
        order: "2".into(),
        sg: false,
        highpass: false,
    }
}

fn format_menu(app: &mut SciPlotApp, ui: &mut egui::Ui) {
    let Some(graph) = app.active_graph() else {
        ui.label("先打开一张图");
        return;
    };
    let layer = app.layer_index(graph);
    if clicked(ui, "坐标轴…", "") {
        if let Some(axis) = app.project.graph(graph).and_then(|g| g.layers.get(layer)).map(|l| l.x_axis.clone()) {
            app.dialog = Some(Dialog::Axis { graph, layer, y_axis: false, min: number_text(axis.min), max: number_text(axis.max), axis });
        }
    }
    if clicked(ui, "纵轴…", "") {
        if let Some(axis) = app.project.graph(graph).and_then(|g| g.layers.get(layer)).map(|l| l.y_axis.clone()) {
            app.dialog = Some(Dialog::Axis { graph, layer, y_axis: true, min: number_text(axis.min), max: number_text(axis.max), axis });
        }
    }
    if clicked(ui, "图例…", "") {
        if let Some(legend) = app.project.graph(graph).and_then(|g| g.layers.get(layer)).map(|l| l.legend.clone()) {
            app.dialog = Some(Dialog::Legend { graph, layer, legend });
        }
    }
    if clicked(ui, "标题…", "") {
        if let Some(layer_ref) = app.project.graph(graph).and_then(|g| g.layers.get(layer)) {
            app.dialog = Some(Dialog::Title { graph, layer, title: layer_ref.title.clone(), size: format!("{}", layer_ref.title_size) });
        }
    }
    if clicked(ui, "曲线样式…", "") {
        let curve = app.curve;
        if let Some(curve_ref) = app.project.graph(graph).and_then(|g| g.layers.get(layer)).and_then(|l| l.curves.get(curve)) {
            app.dialog = Some(Dialog::Style { graph, layer, curve, style: curve_ref.style.clone(), name: curve_ref.name.clone() });
        } else {
            app.status = "这一层没有曲线".into();
        }
    }
    if clicked(ui, "图层位置…", "") {
        if let Some(frame) = app.project.graph(graph).and_then(|g| g.layers.get(layer)).map(|l| l.frame) {
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
    if clicked(ui, "文本标注…", "") {
        app.dialog = Some(Dialog::TextAnn { graph, layer, x: "0".into(), y: "0".into(), text: "标注".into() });
    }
    if clicked(ui, "箭头…", "") {
        app.dialog = Some(Dialog::ArrowAnn { graph, layer, x1: "0".into(), y1: "0".into(), x2: "1".into(), y2: "1".into(), line_only: false });
    }
    if clicked(ui, "线段…", "") {
        app.dialog = Some(Dialog::ArrowAnn { graph, layer, x1: "0".into(), y1: "0".into(), x2: "1".into(), y2: "1".into(), line_only: true });
    }
    if clicked(ui, "删除选中标注", "") {
        delete_selected_annotation(app);
    }
    if clicked(ui, "数据读取器", "") {
        app.reader = !app.reader;
        app.status = if app.reader { "数据读取器已打开".into() } else { "数据读取器已关闭".into() };
    }
    if clicked(ui, "清除缩放", "") {
        app.views.remove(&graph);
        app.status = "已恢复自动范围".into();
    }
}

fn window_menu(app: &mut SciPlotApp, ui: &mut egui::Ui) {
    ui.checkbox(&mut app.show_tree, "项目树");
    ui.checkbox(&mut app.show_log, "日志");
    if clicked(ui, "示例项目", "") {
        let mut project = demo_project();
        ops::enrich_demo(&mut project);
        app.project = project;
        app.history = History::new();
        app.views.clear();
        app.path = None;
        app.dirty = false;
        app.rebuild_dock(true);
        app.status = "已重新载入示例项目".into();
    }
}

fn tree(app: &mut SciPlotApp, ui: &mut egui::Ui) {
    ui.heading(&app.project.name);
    let mut name = app.project.name.clone();
    if ui.text_edit_singleline(&mut name).changed() {
        app.project.name = name;
        app.dirty = true;
    }
    egui::ScrollArea::vertical().show(ui, |ui| {
        let tables: Vec<(TableId, String)> = app.project.tables.iter().map(|t| (t.id, t.name.clone())).collect();
        let matrices: Vec<(MatrixId, String)> = app.project.matrices.iter().map(|m| (m.id, m.name.clone())).collect();
        let graphs: Vec<(GraphId, String)> = app.project.graphs.iter().map(|g| (g.id, g.name.clone())).collect();
        let notes: Vec<(NoteId, String)> = app.project.notes.iter().map(|n| (n.id, n.name.clone())).collect();
        egui::CollapsingHeader::new("表格").default_open(true).show(ui, |ui| {
            for (id, name) in tables {
                if ui.selectable_label(app.focus == Some(Tab::Table(id)), name).clicked() {
                    app.open_tab(Tab::Table(id));
                }
            }
        });
        egui::CollapsingHeader::new("矩阵").default_open(true).show(ui, |ui| {
            for (id, name) in matrices {
                if ui.selectable_label(app.focus == Some(Tab::Matrix(id)), name).clicked() {
                    app.open_tab(Tab::Matrix(id));
                }
            }
        });
        egui::CollapsingHeader::new("图形").default_open(true).show(ui, |ui| {
            for (id, name) in graphs {
                if ui.selectable_label(app.focus == Some(Tab::Graph(id)), name).clicked() {
                    app.open_tab(Tab::Graph(id));
                }
            }
        });
        egui::CollapsingHeader::new("备注").default_open(true).show(ui, |ui| {
            for (id, name) in notes {
                if ui.selectable_label(app.focus == Some(Tab::Note(id)), name).clicked() {
                    app.open_tab(Tab::Note(id));
                }
            }
        });
    });
}

fn log_view(app: &SciPlotApp, ui: &mut egui::Ui) {
    ui.label("日志");
    egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
        for line in &app.project.log {
            ui.label(line);
        }
        if app.project.log.is_empty() {
            ui.label("（空）");
        }
    });
}

struct Viewer<'a> {
    app: &'a mut SciPlotApp,
}

impl TabViewer for Viewer<'_> {
    type Tab = Tab;

    fn title(&mut self, tab: &mut Tab) -> egui::WidgetText {
        self.app.tab_title(tab).into()
    }

    fn id(&mut self, tab: &mut Tab) -> egui::Id {
        egui::Id::new(tab.key())
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
        self.app.focus = Some(tab.clone());
        match *tab {
            Tab::Table(id) => table_view(self.app, ui, id),
            Tab::Matrix(id) => matrix_view(self.app, ui, id),
            Tab::Graph(id) => graph_view(self.app, ui, id),
            Tab::Note(id) => note_view(self.app, ui, id),
        }
    }
}

fn column_header(column: &sciplot_core::Column) -> String {
    if column.mode == sciplot_core::ColumnMode::None || column.mode.label() == column.name {
        column.name.clone()
    } else {
        format!("{} ({})", column.name, column.mode.label())
    }
}

fn table_view(app: &mut SciPlotApp, ui: &mut egui::Ui, id: TableId) {
    let Some(table) = app.project.table(id) else {
        ui.label("表格已删除");
        return;
    };
    let mut name = table.name.clone();
    let ncols = table.columns.len();
    let nrows = table.nrows().max(1);
    let headers: Vec<String> = table.columns.iter().map(column_header).collect();
    ui.horizontal(|ui| {
        ui.label("名称");
        if ui.text_edit_singleline(&mut name).changed() {
            let renamed = name.clone();
            app.edit("重命名", |project| project.rename_table(id, &renamed));
        }
    });
    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .column(Column::exact(46.0))
        .columns(Column::initial(108.0).resizable(true).clip(true), ncols)
        .header(24.0, |mut header| {
            header.col(|ui| {
                ui.label("#");
            });
            for (index, header_text) in headers.iter().enumerate() {
                header.col(|ui| {
                    let selected = app.selected_column == Some((id, index));
                    if ui.selectable_label(selected, header_text).clicked() {
                        app.selected_column = Some((id, index));
                    }
                });
            }
        })
        .body(|body| {
            body.rows(22.0, nrows, |mut row| {
                let row_index = row.index();
                row.col(|ui| {
                    ui.label(format!("{}", row_index + 1));
                });
                for column in 0..ncols {
                    row.col(|ui| {
                        let mut text = app.project.table(id).and_then(|t| t.columns.get(column)).map(|c| c.display(row_index)).unwrap_or_default();
                        let response = ui.add(egui::TextEdit::singleline(&mut text).id(ui.id().with(("cell", column, row_index))));
                        if response.changed() {
                            if !app.pushed_edit {
                                app.history.push("编辑单元格", &app.project);
                                app.pushed_edit = true;
                                app.dirty = true;
                            }
                            if let Some(column) = app.project.table_mut(id).and_then(|t| t.columns.get_mut(column)) {
                                column.set_text(row_index, &text);
                            }
                        }
                        if response.lost_focus() {
                            app.pushed_edit = false;
                        }
                    });
                }
            });
        });
}

fn matrix_view(app: &mut SciPlotApp, ui: &mut egui::Ui, id: MatrixId) {
    let Some(matrix) = app.project.matrix(id) else {
        ui.label("矩阵已删除");
        return;
    };
    let rows = matrix.rows;
    let cols = matrix.cols;
    ui.label(format!("{}   {rows}×{cols}   x [{} , {}]   y [{} , {}]", matrix.name, matrix.x_min, matrix.x_max, matrix.y_min, matrix.y_max));
    TableBuilder::new(ui)
        .striped(true)
        .column(Column::exact(40.0))
        .columns(Column::initial(72.0).resizable(true), cols)
        .header(22.0, |mut header| {
            header.col(|ui| {
                ui.label("#");
            });
            for col in 0..cols {
                header.col(|ui| {
                    ui.label(format!("{}", col + 1));
                });
            }
        })
        .body(|body| {
            body.rows(22.0, rows, |mut row| {
                let r = row.index();
                row.col(|ui| {
                    ui.label(format!("{}", r + 1));
                });
                for c in 0..cols {
                    row.col(|ui| {
                        let mut text = app.project.matrix(id).and_then(|m| m.get(r, c)).map(sciplot_core::format_number).unwrap_or_default();
                        let response = ui.add(egui::TextEdit::singleline(&mut text));
                        if response.changed() {
                            if !app.pushed_edit {
                                app.history.push("编辑矩阵", &app.project);
                                app.pushed_edit = true;
                                app.dirty = true;
                            }
                            let value = if text.trim().is_empty() { None } else { text.trim().parse::<f64>().ok() };
                            if let Some(matrix) = app.project.matrix_mut(id) {
                                matrix.set(r, c, value);
                            }
                        }
                        if response.lost_focus() {
                            app.pushed_edit = false;
                        }
                    });
                }
            });
        });
}

fn note_view(app: &mut SciPlotApp, ui: &mut egui::Ui, id: NoteId) {
    let Some(text) = app.project.notes.iter().find(|n| n.id == id).map(|n| n.text.clone()) else {
        ui.label("备注已删除");
        return;
    };
    let mut text = text;
    let response = ui.add(egui::TextEdit::multiline(&mut text).desired_width(f32::INFINITY).desired_rows(16));
    if response.changed() {
        if !app.pushed_edit {
            app.history.push("编辑备注", &app.project);
            app.pushed_edit = true;
            app.dirty = true;
        }
        if let Some(note) = app.project.note_mut(id) {
            note.text = text;
        }
    }
    if response.lost_focus() {
        app.pushed_edit = false;
    }
}

fn graph_view(app: &mut SciPlotApp, ui: &mut egui::Ui, id: GraphId) {
    let Some(graph) = app.project.graph(id).cloned() else {
        ui.label("图形已删除");
        return;
    };
    let surface = graph.surface.is_some();
    ui.horizontal(|ui| {
        if !graph.layers.is_empty() && graph.surface.is_none() {
            let names: Vec<String> = graph.layers.get(app.layer_index(id)).map(|l| l.curves.iter().map(|c| c.name.clone()).collect()).unwrap_or_default();
            if !names.is_empty() {
                app.curve = app.curve.min(names.len() - 1);
                let selected = names[app.curve].clone();
                egui::ComboBox::from_id_salt(("curve-pick", id.0)).selected_text(selected).show_ui(ui, |ui| {
                    for (index, name) in names.iter().enumerate() {
                        ui.selectable_value(&mut app.curve, index, name);
                    }
                });
            }
        }
        ui.checkbox(&mut app.reader, "读取");
        if ui.button("清除缩放").clicked() {
            app.views.remove(&id);
        }
        ui.label("拖拽平移，滚轮缩放，双击恢复，Shift 拖拽选择 X 区间");
    });
    let available = ui.available_size();
    if available.x < 30.0 || available.y < 30.0 {
        return;
    }
    let (rect, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());
    if graph.surface.is_some() {
        if response.drag_started() {
            app.history.push("旋转三维图", &app.project);
            app.dirty = true;
        }
        if response.dragged() {
            let delta = response.drag_delta();
            if let Some(spec) = &mut app.project.graph_mut(id).and_then(|g| g.surface.as_mut()) {
                spec.azimuth += delta.x * 0.45;
                spec.elevation = (spec.elevation - delta.y * 0.35).clamp(-85.0, 85.0);
            }
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.0 {
                if let Some(spec) = &mut app.project.graph_mut(id).and_then(|g| g.surface.as_mut()) {
                    spec.zoom = (spec.zoom * (1.0 + scroll * 0.001)).clamp(0.25, 8.0);
                }
            }
        }
        let graph = app.project.graph(id).cloned().unwrap_or(graph);
        let scene = render(&app.project, &graph, &[], rect.width(), rect.height(), 1.0);
        sciplot_plot::paint(ui.painter(), rect.min, &scene);
        return;
    }

    let mut views = app.views.get(&id).cloned().unwrap_or_default();
    if views.len() < graph.layers.len() {
        views.resize(graph.layers.len(), LayerView::default());
    }
    let mut scene = render(&app.project, &graph, &views, rect.width(), rect.height(), 1.0);
    let mut changed = false;
    let pointer = response.interact_pointer_pos().or(response.hover_pos());
    if let Some(pos) = pointer {
        let local = pos - rect.min;
        if let Some(layer) = scene.layers.iter().position(|frame| frame.plot.contains(local.x, local.y)) {
            app.active_layer = layer;
            let frame = &scene.layers[layer];
            if response.double_clicked() {
                views[layer] = LayerView::default();
                changed = true;
            }
            let shift = ui.input(|i| i.modifiers.shift);
            if response.drag_started() && shift {
                if let Some((x, _)) = screen_to_data(frame, local.x, local.y) {
                    app.xsel = Some(XSel { graph: id, layer, x0: x, x1: x });
                }
            } else if response.drag_started() {
                app.ann_drag = None;
                if let Some((ann_id, origin, _, _)) = hit_annotation(&graph, layer, frame, local.x, local.y) {
                    if let Some((sx, sy)) = screen_to_data(frame, local.x, local.y) {
                        app.history.push("移动标注", &app.project);
                        app.dirty = true;
                        app.selected_ann = Some((id, layer, ann_id));
                        app.ann_drag = Some(AnnDrag { graph: id, layer, id: ann_id, origin, start_x: sx, start_y: sy });
                    }
                }
            }
            if response.dragged() && shift {
                if let Some((x, _)) = screen_to_data(frame, local.x, local.y) {
                    if let Some(sel) = &mut app.xsel {
                        sel.x1 = x;
                    }
                }
            } else if response.dragged() {
                let drag = app.ann_drag.as_ref().map(|drag| (drag.graph, drag.layer, drag.id, drag.origin.clone(), drag.start_x, drag.start_y));
                if let Some((drag_graph, drag_layer, ann_id, origin, start_x, start_y)) = drag {
                    if drag_graph == id && drag_layer == layer {
                        if let Some((x, y)) = screen_to_data(frame, local.x, local.y) {
                            let kind = shift_kind(&origin, x - start_x, y - start_y);
                            if let Some(layer_ref) = app.project.graph_mut(id).and_then(|g| g.layers.get_mut(layer)) {
                                if let Some(annotation) = layer_ref.annotations.iter_mut().find(|a| a.id == ann_id) {
                                    annotation.kind = kind;
                                }
                            }
                            changed = true;
                        }
                    }
                } else if !shift {
                    let delta = response.drag_delta();
                    if let Some(((xmin, xmax), (ymin, ymax))) = pan_limits(frame, delta.x, delta.y) {
                        views[layer].x_min = Some(xmin);
                        views[layer].x_max = Some(xmax);
                        views[layer].y_min = Some(ymin);
                        views[layer].y_max = Some(ymax);
                        changed = true;
                    }
                }
            }
            if response.hovered() {
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                let zoom = ui.input(|i| i.zoom_delta());
                if scroll.abs() > 0.2 || (zoom - 1.0).abs() > 0.01 {
                    if let Some((x, y)) = screen_to_data(frame, local.x, local.y) {
                        let factor = if scroll.abs() > 0.2 { (1.0 + scroll as f64 * 0.0015).clamp(0.75, 1.35) } else { zoom as f64 };
                        if let Some((a, b)) = zoom_limits(frame.x_scale, frame.x_min, frame.x_max, x, factor) {
                            views[layer].x_min = Some(a);
                            views[layer].x_max = Some(b);
                        }
                        if let Some((a, b)) = zoom_limits(frame.y_scale, frame.y_min, frame.y_max, y, factor) {
                            views[layer].y_min = Some(a);
                            views[layer].y_max = Some(b);
                        }
                        changed = true;
                    }
                }
            }
        }
    }
    if response.drag_stopped() {
        app.ann_drag = None;
    }
    if changed {
        let graph = app.project.graph(id).cloned().unwrap_or(graph);
        scene = render(&app.project, &graph, &views, rect.width(), rect.height(), 1.0);
    }
    app.views.insert(id, views);
    paint_scene(app, ui, rect, &scene, surface);
    paint_overlays(app, ui, id, rect, &scene);
}

fn paint_scene(app: &mut SciPlotApp, ui: &egui::Ui, rect: egui::Rect, scene: &Scene, surface: bool) {
    if surface {
        if app.glyphs.is_none() {
            app.glyphs = sciplot_plot::GlyphFont::from_bytes(&app.font);
        }
        if let Ok((width, height, rgba)) = sciplot_plot::to_rgba(scene, app.glyphs.as_ref()) {
            let image = egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &rgba);
            let texture = ui.ctx().load_texture("sciplot-surface", image, egui::TextureOptions::LINEAR);
            ui.painter().image(
                texture.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            return;
        }
    }
    sciplot_plot::paint(ui.painter(), rect.min, scene);
}

fn paint_overlays(app: &SciPlotApp, ui: &egui::Ui, id: GraphId, rect: egui::Rect, scene: &Scene) {
    let painter = ui.painter();
    if let Some(sel) = &app.xsel {
        if sel.graph == id {
            if let Some(frame) = scene.layers.get(sel.layer) {
                if let (Some((x0, _)), Some((x1, _))) = (data_to_screen(frame, sel.x0, frame.y_min), data_to_screen(frame, sel.x1, frame.y_min)) {
                    let band = egui::Rect::from_min_max(
                        rect.min + egui::vec2(x0.min(x1), frame.plot.y),
                        rect.min + egui::vec2(x0.max(x1), frame.plot.bottom()),
                    );
                    painter.rect_filled(band, 0.0, egui::Color32::from_rgba_unmultiplied(40, 110, 210, 48));
                }
            }
        }
    }
    if app.reader {
        if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
            let local = pos - rect.min;
            if let Some(graph) = app.project.graph(id) {
                for (layer, frame) in scene.layers.iter().enumerate() {
                    if !frame.plot.contains(local.x, local.y) {
                        continue;
                    }
                    if let Some(hit) = nearest_point(&app.project, graph, layer, frame, local.x, local.y) {
                        let color = egui::Color32::from_rgb(20, 20, 20);
                        let stroke = egui::Stroke::new(1.0_f32, color);
                        let p = rect.min + egui::vec2(hit.px, hit.py);
                        painter.line_segment([egui::pos2(rect.min.x + frame.plot.x, p.y), egui::pos2(rect.min.x + frame.plot.right(), p.y)], stroke);
                        painter.line_segment([egui::pos2(p.x, rect.min.y + frame.plot.y), egui::pos2(p.x, rect.min.y + frame.plot.bottom())], stroke);
                        painter.circle_stroke(p, 4.0, stroke);
                        let name = graph.layers.get(layer).and_then(|l| l.curves.get(hit.curve)).map(|c| c.name.as_str()).unwrap_or("点");
                        painter.text(
                            p + egui::vec2(8.0, -8.0),
                            egui::Align2::LEFT_BOTTOM,
                            format!("{name} ({}, {})", sciplot_core::format_number(hit.x), sciplot_core::format_number(hit.y)),
                            egui::FontId::proportional(13.0),
                            color,
                        );
                    }
                }
            }
        }
    }
}

fn hit_annotation(graph: &Graph, layer: usize, frame: &LayerFrame, x: f32, y: f32) -> Option<(u64, AnnotationKind, f64, f64)> {
    let layer = graph.layers.get(layer)?;
    let mut best = None;
    let mut best_d = 14.0_f32;
    for annotation in &layer.annotations {
        let (ax, ay) = anchor(&annotation.kind);
        let Some((px, py)) = data_to_screen(frame, ax, ay) else { continue };
        let distance = (px - x).hypot(py - y);
        if distance < best_d {
            best_d = distance;
            best = Some((annotation.id, annotation.kind.clone(), ax, ay));
        }
    }
    best
}

fn anchor(kind: &AnnotationKind) -> (f64, f64) {
    match kind {
        AnnotationKind::Text { x, y, .. } | AnnotationKind::Marker { x, y, .. } => (*x, *y),
        AnnotationKind::Arrow { x2, y2, .. } | AnnotationKind::Line { x2, y2, .. } => (*x2, *y2),
    }
}

fn shift_kind(kind: &AnnotationKind, dx: f64, dy: f64) -> AnnotationKind {
    match kind.clone() {
        AnnotationKind::Text { x, y, text, size, color } => AnnotationKind::Text { x: x + dx, y: y + dy, text, size, color },
        AnnotationKind::Marker { x, y, label, color } => AnnotationKind::Marker { x: x + dx, y: y + dy, label, color },
        AnnotationKind::Arrow { x1, y1, x2, y2, color } => AnnotationKind::Arrow { x1: x1 + dx, y1: y1 + dy, x2: x2 + dx, y2: y2 + dy, color },
        AnnotationKind::Line { x1, y1, x2, y2, color } => AnnotationKind::Line { x1: x1 + dx, y1: y1 + dy, x2: x2 + dx, y2: y2 + dy, color },
    }
}

fn delete_selected_annotation(app: &mut SciPlotApp) {
    let Some((graph, layer, id)) = app.selected_ann else {
        app.status = "先在图上点选一个标注".into();
        return;
    };
    app.edit("删除标注", |project| {
        let layer = project.graph_mut(graph).ok_or("找不到图形")?.layers.get_mut(layer).ok_or("找不到图层")?;
        let before = layer.annotations.len();
        layer.annotations.retain(|a| a.id != id);
        if layer.annotations.len() == before {
            return Err("没有这个标注".into());
        }
        Ok(())
    });
    app.selected_ann = None;
}

fn delete_focused(app: &mut SciPlotApp) {
    let Some(tab) = app.focus.clone() else { return };
    let removed = app.edit("删除", |project| {
        let ok = match &tab {
            Tab::Table(id) => project.delete_table(*id),
            Tab::Matrix(id) => project.delete_matrix(*id),
            Tab::Graph(id) => project.delete_graph(*id),
            Tab::Note(id) => project.delete_note(*id),
        };
        if ok {
            Ok(())
        } else {
            Err("没有找到要删除的对象".into())
        }
    });
    if removed.is_some() {
        if let Some(found) = app.dock.find_tab(&tab) {
            app.dock.remove_tab(found);
        }
        app.focus = None;
    }
}

fn new_project(app: &mut SciPlotApp) {
    app.project = Project::new("未命名项目");
    app.history = History::new();
    app.views.clear();
    app.path = None;
    app.dirty = false;
    app.dock = DockState::new(Vec::new());
    chinese_dock(&mut app.dock);
    if let Some(id) = Some(app.project.new_table("表格1")) {
        app.open_tab(Tab::Table(id));
    }
    app.status = "已新建项目".into();
}

fn browse_open(app: &mut SciPlotApp) {
    if std::env::var("SCIPLOT_NO_DIALOG").is_err() {
        if let Some(path) = rfd::FileDialog::new().add_filter("SciPlot", &["splot"]).pick_file() {
            app.load_path(&path);
            return;
        }
    }
    app.dialog = Some(Dialog::Path { action: PathAction::Open, path: String::new(), extra: String::new() });
}

fn browse_or_dialog(app: &mut SciPlotApp, action: PathAction, fallback: &str) {
    if std::env::var("SCIPLOT_NO_DIALOG").is_err() {
        if let Some(path) = rfd::FileDialog::new().add_filter("CSV", &["csv", "txt"]).pick_file() {
            app.perform_path(action, &path.display().to_string(), "");
            return;
        }
    }
    app.dialog = Some(Dialog::Path { action, path: fallback.into(), extra: String::new() });
}

fn chinese_dock(dock: &mut DockState<Tab>) {
    dock.translations.tab_context_menu.close_button = "关闭".into();
    dock.translations.tab_context_menu.eject_button = "弹出为窗口".into();
    let leaf = &mut dock.translations.leaf;
    leaf.close_button_disabled_tooltip = "这个标签不能关闭".into();
    leaf.close_all_button = "关闭窗口".into();
    leaf.close_all_button_menu_hint = "右键关闭窗口".into();
    leaf.close_all_button_modifier_hint = "按住 Shift 关闭窗口".into();
    leaf.close_all_button_modifier_menu_hint = "按住 Shift 或右键关闭窗口".into();
    leaf.close_all_button_disabled_tooltip = "窗口里有不能关闭的标签".into();
    leaf.minimize_button = "最小化".into();
    leaf.minimize_button_menu_hint = "右键最小化".into();
    leaf.minimize_button_modifier_hint = "按住 Shift 最小化".into();
    leaf.minimize_button_modifier_menu_hint = "按住 Shift 或右键最小化".into();
}

fn load_recent() -> Vec<PathBuf> {
    let path = recent_path();
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
    serde_json::from_str::<Vec<String>>(&text).unwrap_or_default().into_iter().map(PathBuf::from).collect()
}

fn remember(recent: &mut Vec<PathBuf>, path: &Path) {
    recent.retain(|item| item != path);
    recent.insert(0, path.to_path_buf());
    recent.truncate(8);
    let names: Vec<String> = recent.iter().map(|p| p.display().to_string()).collect();
    std::fs::write(recent_path(), serde_json::to_string_pretty(&names).unwrap_or_else(|_| "[]".into())).ok();
}

fn recent_path() -> PathBuf {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    home.join(".sciplot_recent.json")
}
