use std::collections::HashMap;
use std::path::{Path, PathBuf};

use egui_dock::{DockArea, DockState, TabViewer};
use egui_extras::{Column, TableBuilder};
use sciplot_core::{
    demo_project, export_csv_with, import_csv_with, tr, AnnotationKind, CsvEncoding, CurveKind,
    DataSource, Graph, GraphId, History, MatrixId, NoteId, Project, SurfaceMode, SurfaceSource,
    TableId,
};
use sciplot_plot::{
    data_to_screen, nearest_curve, nearest_point, pan_limits, render, screen_to_data, to_png,
    to_rgba, to_svg, zoom_limits, LayerFrame, LayerView, Scene,
};

use crate::dialogs;
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
            LineChoice::Curve => tr!("曲线", "Curve"),
            LineChoice::Horizontal => tr!("水平线 y=c", "Horizontal y=c"),
            LineChoice::Vertical => tr!("竖直线 x=c", "Vertical x=c"),
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
            FitWhich::Linear => tr!("线性", "Linear"),
            FitWhich::Poly => tr!("多项式", "Polynomial"),
            FitWhich::Exp => tr!("指数", "Exponential"),
            FitWhich::Gauss => tr!("高斯", "Gaussian"),
            FitWhich::Lorentz => tr!("洛伦兹", "Lorentzian"),
            FitWhich::Custom => tr!("自定义", "Custom"),
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
pub enum AfterSave {
    Quit,
    NewProject,
    Open(PathBuf),
    OpenPicker,
    Demo,
}

#[derive(Clone)]
pub enum Dialog {
    Message {
        title: String,
        text: String,
    },
    Unsaved {
        then: AfterSave,
    },
    Help,
    About,
    Path {
        action: PathAction,
        path: String,
        extra: String,
    },
    Formula {
        table: TableId,
        column: usize,
        expr: String,
        error: String,
        warning: String,
    },
    MatrixFormula {
        matrix: MatrixId,
        expr: String,
    },
    NewMatrix {
        name: String,
        rows: String,
        cols: String,
    },
    ColumnProps {
        table: TableId,
        column: usize,
    },
    RenameColumn {
        table: TableId,
        column: usize,
        name: String,
        error: String,
    },
    Sort {
        table: TableId,
        column: usize,
        ascending: bool,
    },
    Axis {
        graph: GraphId,
        layer: usize,
        y_axis: bool,
        axis: sciplot_core::Axis,
        min: String,
        max: String,
        step: String,
        minor: String,
        error: String,
    },
    ConfirmDelete {
        target: DeleteTarget,
        detail: String,
    },
    Legend {
        graph: GraphId,
        layer: usize,
        legend: sciplot_core::Legend,
    },
    Title {
        graph: GraphId,
        layer: usize,
        title: String,
        size: String,
    },
    Style {
        graph: GraphId,
        layer: usize,
        curve: usize,
        style: sciplot_core::CurveStyle,
        name: String,
    },
    Frame {
        graph: GraphId,
        layer: usize,
        x: String,
        y: String,
        w: String,
        h: String,
    },
    TextAnn {
        graph: GraphId,
        layer: usize,
        x: String,
        y: String,
        text: String,
    },
    ArrowAnn {
        graph: GraphId,
        layer: usize,
        x1: String,
        y1: String,
        x2: String,
        y2: String,
        line_only: bool,
    },
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
    Fit {
        graph: GraphId,
        layer: usize,
        curve: usize,
        which: FitWhich,
        degree: String,
        expr: String,
        initial: String,
        xmin: String,
        xmax: String,
    },
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
    AddCurve {
        graph: GraphId,
        table: TableId,
        kind: CurveKind,
    },
    Prefs(crate::prefs::Prefs),
    ImportInstead {
        path: String,
    },
    Update(crate::update::UpdateView),
}

#[derive(Clone)]
pub(crate) enum DeleteTarget {
    Table(TableId),
    Matrix(MatrixId),
}

pub(crate) struct XSel {
    pub(crate) graph: GraphId,
    layer: usize,
    pub(crate) x0: f64,
    pub(crate) x1: f64,
}

#[derive(Clone, Debug)]
pub(crate) enum PlotHit {
    Curve {
        graph: GraphId,
        layer: usize,
        curve: usize,
    },
    Annotation {
        graph: GraphId,
        layer: usize,
        id: u64,
    },
    Layer {
        graph: GraphId,
        layer: usize,
    },
    Surface {
        graph: GraphId,
    },
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
    status_note: Option<String>,
    path: Option<PathBuf>,
    pub dirty: bool,
    recent: Vec<PathBuf>,
    focus: Option<Tab>,
    pub(crate) views: HashMap<GraphId, Vec<LayerView>>,
    show_tree: bool,
    show_log: bool,
    pub(crate) reader: bool,
    pub(crate) xsel: Option<XSel>,
    ann_drag: Option<AnnDrag>,
    pub(crate) selected_ann: Option<(GraphId, usize, u64)>,
    pub(crate) selected_column: Option<(TableId, usize)>,
    active_layer: usize,
    pub(crate) curve: usize,
    pushed_edit: bool,
    pushed_name: bool,
    arm_cell_focus: Option<TableId>,
    visible_now: Vec<Tab>,
    visible_last: Vec<Tab>,
    fonts: crate::fonts::LoadedFonts,
    glyphs: Option<sciplot_plot::GlyphFont>,
    frames: u32,
    pub(crate) prefs: crate::prefs::Prefs,
    pub(crate) plot_hit: Option<PlotHit>,
    pending_tabs: Vec<Tab>,
    pub(crate) pending_after: Option<AfterSave>,
    pub(crate) confirm_style: bool,
    narrow_collapsed: bool,
    pub(crate) update_rx: Option<std::sync::mpsc::Receiver<crate::update::UpdateView>>,
}

pub fn run() -> eframe::Result<()> {
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 800.0])
        .with_title(i18n::APP_TITLE);
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        i18n::APP_TITLE,
        options,
        Box::new(|cc| {
            let app = SciPlotApp::demo();
            fonts::install(&cc.egui_ctx, &app.fonts);
            Ok(Box::new(app))
        }),
    )
}

impl SciPlotApp {
    fn demo() -> Self {
        sciplot_core::set_language(crate::prefs::load().language);
        let mut project = demo_project();
        ops::enrich_demo(&mut project);
        let mut app = Self::empty(project);
        app.rebuild_dock(true);
        app.status = sciplot_core::t("已载入示例。正弦与余弦的交点已标在图上。").into();
        app
    }

    fn empty(project: Project) -> Self {
        let mut dock = DockState::new(Vec::<Tab>::new());
        chinese_dock(&mut dock);
        let mut app = Self {
            project,
            history: History::new(),
            dock,
            dialog: None,
            status: sciplot_core::t("就绪").into(),
            status_note: None,
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
            pushed_name: false,
            arm_cell_focus: None,
            visible_now: Vec::new(),
            visible_last: Vec::new(),
            prefs: crate::prefs::load(),
            plot_hit: None,
            fonts: crate::fonts::load(None),
            glyphs: None,
            frames: 0,
            pending_tabs: Vec::new(),
            pending_after: None,
            confirm_style: false,
            narrow_collapsed: false,
            update_rx: None,
        };
        app.reload_font_silent();
        app
    }

    fn reload_font_silent(&mut self) {
        sciplot_core::set_language(self.prefs.language);
        self.fonts = fonts::load(self.prefs.font_request());
        self.glyphs = None;
    }

    pub(crate) fn reload_font(&mut self, ctx: &egui::Context) -> bool {
        sciplot_core::set_language(self.prefs.language);
        self.fonts = fonts::load(self.prefs.font_request());
        self.glyphs = None;
        fonts::install(ctx, &self.fonts);
        self.fonts.ok
    }

    pub fn edit<T>(
        &mut self,
        label: &str,
        f: impl FnOnce(&mut Project) -> Result<T, String>,
    ) -> Option<T> {
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
        // 停靠区绘制时 `dock` 被临时换走，这里先排队，等换回来再真正打开。
        self.focus = Some(tab.clone());
        self.pending_tabs.push(tab);
    }

    fn flush_tabs(&mut self) {
        ensure_main_surface(&mut self.dock);
        let tabs = std::mem::take(&mut self.pending_tabs);
        for tab in tabs {
            if self.dock.find_tab(&tab).is_none() {
                self.dock.push_to_focused_leaf(tab.clone());
            }
            if let Some(found) = self.dock.find_tab(&tab) {
                self.dock.set_active_tab(found);
                self.dock.set_focused_node_and_surface((found.0, found.1));
            }
        }
    }

    fn rebuild_dock(&mut self, demo: bool) {
        self.dock = DockState::new(Vec::<Tab>::new());
        chinese_dock(&mut self.dock);
        self.pending_tabs.clear();
        if demo {
            if let Some(table) = self
                .project
                .tables
                .iter()
                .find(|t| demo_name(&t.name, "三角函数", "Trigonometry"))
                .map(|t| t.id)
            {
                self.open_tab(Tab::Table(table));
            }
            if let Some(graph) = self
                .project
                .graphs
                .iter()
                .find(|g| demo_name(&g.name, "正弦与余弦", "Sine and cosine"))
                .map(|g| g.id)
            {
                self.open_tab(Tab::Graph(graph));
            }
            return;
        }
        if let Some(graph) = self.project.graphs.first() {
            let graph_id = graph.id;
            let mut tables = Vec::new();
            let mut matrix = None;
            for layer in &graph.layers {
                for curve in &layer.curves {
                    if let DataSource::Columns { table, .. } = curve.source {
                        if !tables.contains(&table) {
                            tables.push(table);
                        }
                    }
                }
            }
            if let Some(SurfaceSource::Matrix(id)) = graph.surface.as_ref().map(|spec| &spec.source)
            {
                matrix = Some(*id);
            }
            for table in tables {
                self.open_tab(Tab::Table(table));
            }
            if let Some(id) = matrix {
                self.open_tab(Tab::Matrix(id));
            }
            self.open_tab(Tab::Graph(graph_id));
            return;
        }
        if let Some(table) = self.project.tables.first() {
            self.open_tab(Tab::Table(table.id));
        }
    }

    fn active_table(&self) -> Option<TableId> {
        match self.focus {
            Some(Tab::Table(id)) => Some(id),
            _ => self
                .selected_column
                .map(|(id, _)| id)
                .or_else(|| self.project.tables.first().map(|t| t.id)),
        }
    }

    pub(crate) fn active_graph(&self) -> Option<GraphId> {
        match self.focus {
            Some(Tab::Graph(id)) => Some(id),
            _ => self
                .project
                .graphs
                .iter()
                .find(|g| g.surface.is_none())
                .map(|g| g.id),
        }
    }

    fn layer_index(&self, graph: GraphId) -> usize {
        let n = self
            .project
            .graph(graph)
            .map(|g| g.layers.len())
            .unwrap_or(1);
        self.active_layer.min(n.saturating_sub(1))
    }

    pub fn save_current(&mut self) -> bool {
        if let Some(path) = self.path.clone() {
            self.write_path(&path)
        } else {
            self.status = sciplot_core::t("还没有文件名，请用「另存为」").into();
            start_file(self, PathAction::SaveAs, "");
            false
        }
    }

    fn write_path(&mut self, path: &Path) -> bool {
        let path = with_splot_extension(path);
        match self
            .project
            .to_json()
            .and_then(|text| std::fs::write(&path, text).map_err(|err| err.to_string()))
        {
            Ok(()) => {
                self.path = Some(path.to_path_buf());
                self.dirty = false;
                remember(&mut self.recent, &path);
                self.note_dir(&path);
                self.status = sciplot_core::trf!("已保存 {}", "Saved {}", path.display());
                true
            }
            Err(err) => {
                self.status = err;
                false
            }
        }
    }

    pub fn perform_path(&mut self, action: PathAction, path: &str, extra: &str) {
        let path = resolve_user_path(self, path);
        if path.as_os_str().is_empty() || path.is_dir() {
            self.status = sciplot_core::t("请填写文件路径，不能只写文件夹").into();
            return;
        }
        match action {
            PathAction::Open => self.load_path(&path),
            PathAction::SaveAs => {
                self.write_path(&path);
            }
            PathAction::ImportCsv => {
                let encoding = self.prefs.csv_encoding;
                match std::fs::read(&path)
                    .map_err(|err| err.to_string())
                    .and_then(|bytes| import_csv_with(&bytes, encoding))
                {
                    Ok(mut table) => {
                        if let Some(stem) = path
                            .file_stem()
                            .and_then(|stem| stem.to_str())
                            .filter(|stem| !stem.is_empty())
                        {
                            table.name = stem.to_string();
                        }
                        let shown = path.display().to_string();
                        if let Some(id) = self.edit(sciplot_core::t("导入 CSV"), move |project| {
                            Ok(project.insert_table(table))
                        }) {
                            self.open_tab(Tab::Table(id));
                            self.note_dir(&path);
                            self.status = sciplot_core::trf!(
                                "已导入 {shown}（{}）",
                                "Imported {shown} ({})",
                                encoding.label(),
                                shown = shown
                            );
                        }
                    }
                    Err(err) => self.status = err,
                }
            }
            PathAction::ExportCsv => {
                let Some(id) = self.active_table() else {
                    self.status = sciplot_core::t("没有可导出的表格").into();
                    return;
                };
                let Some(table) = self.project.table(id).cloned() else {
                    return;
                };
                let encoding = match self.prefs.csv_encoding {
                    CsvEncoding::Gbk => CsvEncoding::Gbk,
                    CsvEncoding::Auto | CsvEncoding::Utf8 => CsvEncoding::Utf8,
                };
                let bytes = export_csv_with(&table, encoding);
                match std::fs::write(&path, bytes) {
                    Ok(()) => {
                        self.note_dir(&path);
                        self.status = sciplot_core::trf!(
                            "已导出 {}（{}）",
                            "Exported {} ({})",
                            path.display(),
                            encoding.label()
                        );
                    }
                    Err(err) => self.status = err.to_string(),
                }
            }
            PathAction::ExportPng | PathAction::ExportSvg => {
                self.export_graph(action, &path, extra)
            }
        }
    }

    fn note_dir(&mut self, path: &Path) {
        let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) else {
            return;
        };
        self.prefs.last_dir = dir.display().to_string();
        crate::prefs::save(&self.prefs);
    }

    fn load_path(&mut self, path: &Path) {
        let ext = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if matches!(ext.as_str(), "csv" | "txt" | "tsv") {
            self.status =
                sciplot_core::t("这不是 SciPlot 项目。项目文件的扩展名是 .splot，表格请用导入。")
                    .into();
            self.dialog = Some(Dialog::ImportInstead {
                path: path.display().to_string(),
            });
            return;
        }
        match std::fs::read_to_string(path)
            .map_err(|err| err.to_string())
            .and_then(|text| Project::from_json(&text))
        {
            Ok(project) => {
                self.project = project;
                self.history = History::new();
                self.views.clear();
                self.path = Some(path.to_path_buf());
                self.dirty = false;
                remember(&mut self.recent, path);
                self.note_dir(path);
                self.rebuild_dock(false);
                self.status = sciplot_core::trf!("已打开 {}", "Opened {}", path.display());
            }
            Err(err) => {
                self.status = sciplot_core::trf!(
                    "打不开这个项目。请选择 .splot 文件。{err}",
                    "Could not open this project. Choose a .splot file. {err}",
                    err = err
                )
            }
        }
    }

    fn export_graph(&mut self, action: PathAction, path: &Path, extra: &str) {
        let Some(scene) = self.scene_of_active(extra) else {
            self.status = sciplot_core::t("没有可导出的图形").into();
            return;
        };
        let result = match action {
            PathAction::ExportSvg => {
                std::fs::write(path, to_svg(&scene)).map_err(|err| err.to_string())
            }
            _ => {
                let font = crate::fonts::plot_glyphs(&self.fonts);
                to_png(&scene, font.as_ref())
                    .and_then(|bytes| std::fs::write(path, bytes).map_err(|err| err.to_string()))
            }
        };
        self.status = match result {
            Ok(()) => {
                self.note_dir(path);
                sciplot_core::trf!("已导出 {}", "Exported {}", path.display())
            }
            Err(err) => err,
        };
    }

    fn scene_of_active(&self, dpi_text: &str) -> Option<Scene> {
        let id = self.active_graph()?;
        let graph = self.project.graph(id)?.clone();
        let views = self.views.get(&id).cloned().unwrap_or_default();
        let dpi = dpi_text
            .trim()
            .parse::<f32>()
            .unwrap_or(144.0)
            .clamp(72.0, 600.0);
        let scale = dpi / 96.0;
        Some(render(
            &self.project,
            &graph,
            &views,
            960.0 * scale,
            640.0 * scale,
            scale,
        ))
    }

    pub(crate) fn copy_png(&mut self) {
        let Some(scene) = self.scene_of_active("144") else {
            self.status = sciplot_core::t("没有可复制的图形").into();
            return;
        };
        let font = crate::fonts::plot_glyphs(&self.fonts);
        match to_rgba(&scene, font.as_ref()) {
            Ok((w, h, bytes)) => match arboard::Clipboard::new().and_then(|mut clip| {
                clip.set_image(arboard::ImageData {
                    width: w as usize,
                    height: h as usize,
                    bytes: std::borrow::Cow::Owned(bytes),
                })
            }) {
                Ok(()) => self.status = sciplot_core::t("已复制 PNG 到剪贴板").into(),
                Err(err) => {
                    self.status = sciplot_core::trf!(
                        "剪贴板不可用：{err}",
                        "Clipboard is unavailable: {err}",
                        err = err
                    )
                }
            },
            Err(err) => self.status = err,
        }
    }

    pub(crate) fn copy_svg(&mut self) {
        let Some(scene) = self.scene_of_active("96") else {
            self.status = sciplot_core::t("没有可复制的图形").into();
            return;
        };
        match arboard::Clipboard::new().and_then(|mut clip| clip.set_text(to_svg(&scene))) {
            Ok(()) => self.status = sciplot_core::t("已复制 SVG 到剪贴板").into(),
            Err(err) => {
                self.status = sciplot_core::trf!(
                    "剪贴板不可用：{err}",
                    "Clipboard is unavailable: {err}",
                    err = err
                )
            }
        }
    }

    fn undo(&mut self) {
        if let Some((label, project)) = self.history.undo(&self.project) {
            self.project = project;
            self.dirty = true;
            self.status = sciplot_core::trf!("已撤销：{label}", "Undone: {label}", label = label);
        }
    }

    fn redo(&mut self) {
        if let Some((label, project)) = self.history.redo(&self.project) {
            self.project = project;
            self.dirty = true;
            self.status = sciplot_core::trf!("已重做：{label}", "Redone: {label}", label = label);
        }
    }

    pub(crate) fn plot_table_as(&mut self, table: TableId, kind: CurveKind) {
        let prefs = self.prefs.clone();
        if let Some(id) = self.edit(sciplot_core::t("绘图"), |project| {
            let id = project.plot_table(table, kind)?;
            prefs.apply_new_graph(project, id);
            Ok(id)
        }) {
            self.open_tab(Tab::Graph(id));
            self.status = sciplot_core::trf!("已新建{}", "Created {}", kind.label());
        }
    }

    pub(crate) fn plot_matrix_as(&mut self, matrix: MatrixId, kind: CurveKind) {
        let prefs = self.prefs.clone();
        if let Some(id) = self.edit(sciplot_core::t("绘图"), |project| {
            let id = project.plot_matrix(matrix, kind)?;
            prefs.apply_new_graph(project, id);
            Ok(id)
        }) {
            self.open_tab(Tab::Graph(id));
            self.status = sciplot_core::trf!("已新建{}", "Created {}", kind.label());
        }
    }

    pub(crate) fn plot_surface_as(&mut self, matrix: MatrixId, mode: SurfaceMode) {
        let prefs = self.prefs.clone();
        let base = self
            .project
            .matrix(matrix)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| sciplot_core::t("三维图").into());
        let title = format!("{base}-{}", mode.label());
        if let Some(id) = self.edit(sciplot_core::t("三维图"), |project| {
            let id = project.plot_surface(SurfaceSource::Matrix(matrix), mode, &title);
            prefs.apply_new_graph(project, id);
            Ok(id)
        }) {
            self.open_tab(Tab::Graph(id));
            self.status = sciplot_core::trf!("已新建三维{}", "Created 3D {}", mode.label());
        }
    }

    fn tab_title(&self, tab: &Tab) -> String {
        match tab {
            Tab::Table(id) => sciplot_core::trf!(
                "表·{}",
                "Table · {}",
                self.project
                    .table(*id)
                    .map(|t| t.name.clone())
                    .unwrap_or_else(|| sciplot_core::t("表格").into())
            ),
            Tab::Matrix(id) => sciplot_core::trf!(
                "矩阵·{}",
                "Matrix · {}",
                self.project
                    .matrix(*id)
                    .map(|m| m.name.clone())
                    .unwrap_or_else(|| sciplot_core::t("矩阵").into())
            ),
            Tab::Graph(id) => sciplot_core::trf!(
                "图·{}",
                "Graph · {}",
                self.project
                    .graph(*id)
                    .map(|g| g.name.clone())
                    .unwrap_or_else(|| sciplot_core::t("图").into())
            ),
            Tab::Note(id) => sciplot_core::trf!(
                "备注·{}",
                "Note · {}",
                self.project
                    .notes
                    .iter()
                    .find(|n| n.id == *id)
                    .map(|n| n.name.clone())
                    .unwrap_or_else(|| sciplot_core::t("备注").into())
            ),
        }
    }
}

impl eframe::App for SciPlotApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.visible_now.clear();
        chinese_dock(&mut self.dock);
        self.frames += 1;
        if let Ok(limit) = std::env::var("SCIPLOT_EXIT_FRAMES") {
            if self.frames > limit.parse::<u32>().unwrap_or(40) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if ctx.input(|i| i.viewport().close_requested())
            && self.dirty
            && !matches!(self.dialog, Some(Dialog::Unsaved { .. }))
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.dialog = Some(Dialog::Unsaved {
                then: AfterSave::Quit,
            });
        }
        shortcuts(self, ctx);
        egui::TopBottomPanel::top("menu").show(ctx, |ui| menus(self, ui));
        let screen_w = ctx.screen_rect().width();
        if screen_w < 520.0 {
            if self.show_tree && !self.narrow_collapsed {
                self.show_tree = false;
                self.narrow_collapsed = true;
                self.status =
                    sciplot_core::t("窗口较窄，已收起项目树。可在「窗口」菜单里重新打开。").into();
            }
        } else if screen_w > 900.0 && self.narrow_collapsed {
            self.narrow_collapsed = false;
            self.show_tree = true;
            self.status = sciplot_core::t("窗口已变宽，已重新打开项目树。").into();
        }
        if self.show_tree {
            let max_w = if screen_w < 800.0 {
                (screen_w * 0.30).clamp(140.0, 220.0)
            } else {
                360.0
            };
            egui::SidePanel::left("tree")
                .resizable(true)
                .default_width(200.0)
                .max_width(max_w)
                .show(ctx, |ui| tree(self, ui));
        }
        egui::TopBottomPanel::bottom("status")
            .exact_height(24.0)
            .show(ctx, |ui| {
                ui.label(&self.status);
            });
        if self.show_log {
            egui::TopBottomPanel::bottom("log")
                .resizable(true)
                .default_height(120.0)
                .show(ctx, |ui| log_view(self, ui));
        }
        self.flush_tabs();
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut dock = std::mem::replace(&mut self.dock, DockState::new(Vec::new()));
            DockArea::new(&mut dock).show_inside(ui, &mut Viewer { app: self });
            self.dock = dock;
        });
        self.flush_tabs();
        drop_stale_tabs(self);
        crate::update::poll(self);
        if self.update_rx.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
        dialogs::show(self, ctx);
        self.visible_last.clone_from(&self.visible_now);
        if self.dialog.is_none() {
            if let Some(then) = self.pending_after.take() {
                if !self.dirty {
                    run_after(self, then, ctx);
                }
            }
        }
    }
}

fn shortcuts(app: &mut SciPlotApp, ctx: &egui::Context) {
    let cmd = egui::Modifiers::COMMAND;
    if ctx.input_mut(|i| i.consume_key(cmd, egui::Key::N)) {
        request_after(app, AfterSave::NewProject);
    }
    if ctx.input_mut(|i| i.consume_key(cmd, egui::Key::O)) {
        request_after(app, AfterSave::OpenPicker);
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
        ui.menu_button(i18n::file(), |ui| file_menu(app, ui));
        ui.menu_button(i18n::edit(), |ui| edit_menu(app, ui));
        ui.menu_button(i18n::settings(), |ui| {
            if clicked(ui, sciplot_core::t("默认样式与字体…"), "") {
                app.dialog = Some(Dialog::Prefs(app.prefs.clone()));
            }
        });
        ui.menu_button(i18n::window(), |ui| window_menu(app, ui));
        ui.menu_button(i18n::help(), |ui| {
            if clicked(ui, sciplot_core::t("公式说明"), "") {
                app.dialog = Some(Dialog::Help);
            }
            if clicked(ui, sciplot_core::t("检查更新"), "") {
                crate::update::start(app);
            }
            if clicked(ui, sciplot_core::t("关于"), "") {
                app.dialog = Some(Dialog::About);
            }
        });
    });
}

pub(crate) fn clicked_menu(ui: &mut egui::Ui, label: &str) -> bool {
    clicked(ui, label, "")
}

fn relax_menu(ui: &mut egui::Ui) {
    ui.spacing_mut().button_padding = egui::vec2(8.0, 5.0);
    ui.spacing_mut().item_spacing.y = 3.0;
}

fn clicked(ui: &mut egui::Ui, label: &str, shortcut: &str) -> bool {
    relax_menu(ui);
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
    relax_menu(ui);
    if clicked(ui, sciplot_core::t("新建项目"), "Ctrl+N") {
        request_after(app, AfterSave::NewProject);
    }
    if clicked(ui, sciplot_core::t("打开…"), "Ctrl+O") {
        request_after(app, AfterSave::OpenPicker);
    }
    if clicked(ui, sciplot_core::t("保存"), "Ctrl+S") {
        app.save_current();
    }
    if clicked(ui, sciplot_core::t("另存为…"), "") {
        start_file(app, PathAction::SaveAs, "");
    }
    if clicked(ui, sciplot_core::t("导入 CSV…"), "") {
        start_file(app, PathAction::ImportCsv, "");
    }
    if clicked(ui, sciplot_core::t("导出当前图形 PNG…"), "") {
        start_file(app, PathAction::ExportPng, "144");
    }
    if clicked(ui, sciplot_core::t("导出当前图形 SVG…"), "") {
        start_file(app, PathAction::ExportSvg, "");
    }
    ui.separator();
    ui.menu_button(sciplot_core::t("最近文件"), |ui| {
        relax_menu(ui);
        if app.recent.is_empty() {
            ui.label(sciplot_core::t("（空）"));
        }
        ui.set_min_width(320.0);
        let recent = app.recent.clone();
        for path in recent {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| path.display().to_string());
            let exists = path.exists();
            let response = ui
                .add_enabled(exists, egui::Button::new(name))
                .on_hover_text(path.display().to_string());
            if response.clicked() {
                ui.close_menu();
                request_after(app, AfterSave::Open(path));
            }
        }
    });
    ui.separator();
    ui.add_space(8.0);
    if clicked(ui, sciplot_core::t("退出"), "") {
        if app.dirty {
            app.dialog = Some(Dialog::Unsaved {
                then: AfterSave::Quit,
            });
        } else {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn edit_menu(app: &mut SciPlotApp, ui: &mut egui::Ui) {
    if clicked(ui, sciplot_core::t("撤销"), "Ctrl+Z") {
        app.undo();
    }
    if clicked(ui, sciplot_core::t("重做"), "Ctrl+Y") {
        app.redo();
    }
    let delete_label = match app.focus {
        Some(Tab::Table(_)) => sciplot_core::t("删除当前表格"),
        Some(Tab::Matrix(_)) => sciplot_core::t("删除当前矩阵"),
        Some(Tab::Graph(_)) => sciplot_core::t("删除当前图形"),
        Some(Tab::Note(_)) => sciplot_core::t("删除当前备注"),
        None => sciplot_core::t("删除当前对象"),
    };
    if clicked(ui, delete_label, "") {
        delete_focused(app);
    }
}

pub(crate) fn fit_dialog(
    graph: GraphId,
    layer: usize,
    curve: usize,
    which: FitWhich,
    xmin: String,
    xmax: String,
) -> Dialog {
    Dialog::Fit {
        graph,
        layer,
        curve,
        which,
        degree: "2".into(),
        expr: "a*x+b".into(),
        initial: "a=1, b=0".into(),
        xmin,
        xmax,
    }
}

pub(crate) fn range_dialog(graph: GraphId, layer: usize, curve: usize, op: RangeOp) -> Dialog {
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

fn window_menu(app: &mut SciPlotApp, ui: &mut egui::Ui) {
    ui.checkbox(&mut app.show_tree, sciplot_core::t("项目树"));
    ui.checkbox(&mut app.show_log, sciplot_core::t("日志"));
    if clicked(ui, sciplot_core::t("示例项目"), "") {
        request_after(app, AfterSave::Demo);
    }
}

fn tree(app: &mut SciPlotApp, ui: &mut egui::Ui) {
    ui.heading(&app.project.name);
    let mut name = app.project.name.clone();
    let response = ui.text_edit_singleline(&mut name);
    if response.gained_focus() && !app.pushed_name {
        app.history.push(sciplot_core::t("重命名"), &app.project);
        app.pushed_name = true;
    }
    if response.changed() {
        if !app.pushed_name {
            app.history.push(sciplot_core::t("重命名"), &app.project);
            app.pushed_name = true;
        }
        app.project.name = name;
        app.dirty = true;
    }
    if response.lost_focus() {
        app.pushed_name = false;
    }
    egui::ScrollArea::vertical().show(ui, |ui| {
        let tables: Vec<(TableId, String)> = app
            .project
            .tables
            .iter()
            .map(|t| (t.id, t.name.clone()))
            .collect();
        let matrices: Vec<(MatrixId, String)> = app
            .project
            .matrices
            .iter()
            .map(|m| (m.id, m.name.clone()))
            .collect();
        let graphs: Vec<(GraphId, String)> = app
            .project
            .graphs
            .iter()
            .map(|g| (g.id, g.name.clone()))
            .collect();
        let notes: Vec<(NoteId, String)> = app
            .project
            .notes
            .iter()
            .map(|n| (n.id, n.name.clone()))
            .collect();
        let tables_header = egui::CollapsingHeader::new(sciplot_core::t("表格"))
            .default_open(true)
            .show(ui, |ui| {
                for (id, name) in tables {
                    let response = tree_row(ui, app.focus == Some(Tab::Table(id)), &name);
                    if response.clicked() {
                        app.open_tab(Tab::Table(id));
                    }
                    response.context_menu(|ui| crate::context::tree_table(app, ui, id));
                }
            });
        tables_header
            .header_response
            .context_menu(|ui| crate::context::section_tables(app, ui));
        let matrices_header = egui::CollapsingHeader::new(sciplot_core::t("矩阵"))
            .default_open(true)
            .show(ui, |ui| {
                for (id, name) in matrices {
                    let response = tree_row(ui, app.focus == Some(Tab::Matrix(id)), &name);
                    if response.clicked() {
                        app.open_tab(Tab::Matrix(id));
                    }
                    response.context_menu(|ui| crate::context::tree_matrix(app, ui, id));
                }
            });
        matrices_header
            .header_response
            .context_menu(|ui| crate::context::section_matrices(app, ui));
        let graphs_header = egui::CollapsingHeader::new(sciplot_core::t("图形"))
            .default_open(true)
            .show(ui, |ui| {
                for (id, name) in graphs {
                    let response = tree_row(ui, app.focus == Some(Tab::Graph(id)), &name);
                    if response.clicked() {
                        app.open_tab(Tab::Graph(id));
                    }
                    response.context_menu(|ui| crate::context::tree_graph(app, ui, id));
                }
            });
        graphs_header
            .header_response
            .context_menu(|ui| crate::context::section_graphs(app, ui));
        let notes_header = egui::CollapsingHeader::new(sciplot_core::t("备注"))
            .default_open(true)
            .show(ui, |ui| {
                for (id, name) in notes {
                    let response = tree_row(ui, app.focus == Some(Tab::Note(id)), &name);
                    if response.clicked() {
                        app.open_tab(Tab::Note(id));
                    }
                    response.context_menu(|ui| crate::context::tree_note(app, ui, id));
                }
            });
        notes_header
            .header_response
            .context_menu(|ui| crate::context::section_notes(app, ui));
    });
}

fn tab_still_exists(app: &SciPlotApp, tab: &Tab) -> bool {
    match tab {
        Tab::Table(id) => app.project.table(*id).is_some(),
        Tab::Matrix(id) => app.project.matrix(*id).is_some(),
        Tab::Graph(id) => app.project.graph(*id).is_some(),
        Tab::Note(id) => app.project.notes.iter().any(|note| note.id == *id),
    }
}

fn drop_stale_tabs(app: &mut SciPlotApp) {
    // `DockState::retain_tabs` 会把已经空掉的主表面删掉。标签关光后再从项目树打开，
    // `push_to_focused_leaf` 会因为没有主表面直接崩溃。
    let stale: Vec<Tab> = app
        .dock
        .iter_all_tabs()
        .filter(|(_, tab)| !tab_still_exists(app, tab))
        .map(|(_, tab)| tab.clone())
        .collect();
    for tab in stale {
        if let Some(path) = app.dock.find_tab(&tab) {
            app.dock.remove_tab(path);
        }
    }
}

fn ensure_main_surface(dock: &mut DockState<Tab>) {
    let missing = match dock.get_surface(egui_dock::SurfaceIndex::main()) {
        Some(surface) => surface.is_empty(),
        None => true,
    };
    if !missing {
        return;
    }
    let translations = dock.translations.clone();
    let tabs: Vec<Tab> = dock.iter_all_tabs().map(|(_, tab)| tab.clone()).collect();
    *dock = DockState::new(tabs);
    dock.translations = translations;
}

fn tree_row(ui: &mut egui::Ui, selected: bool, name: &str) -> egui::Response {
    ui.add_sized(
        [ui.available_width(), 22.0],
        egui::SelectableLabel::new(selected, name),
    )
}

fn log_view(app: &SciPlotApp, ui: &mut egui::Ui) {
    ui.label(sciplot_core::t("日志"));
    egui::ScrollArea::vertical()
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for line in &app.project.log {
                ui.add(egui::Label::new(line).wrap_mode(egui::TextWrapMode::Wrap));
            }
            if app.project.log.is_empty() {
                ui.label(sciplot_core::t("（空）"));
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
        let fresh = !self.app.visible_last.iter().any(|item| item == tab);
        if fresh {
            if let Tab::Table(id) = *tab {
                self.app.arm_cell_focus = Some(id);
            }
        }
        if !self.app.visible_now.iter().any(|item| item == tab) {
            self.app.visible_now.push(tab.clone());
        }
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
    let role = column.mode.label();
    if column.mode == sciplot_core::ColumnMode::None || role.is_empty() || role == column.name {
        column.name.clone()
    } else {
        format!("{} ({role})", column.name)
    }
}

fn table_view(app: &mut SciPlotApp, ui: &mut egui::Ui, id: TableId) {
    let Some(table) = app.project.table(id) else {
        ui.label(sciplot_core::t("表格已删除"));
        return;
    };
    let mut name = table.name.clone();
    let ncols = table.columns.len();
    let nrows = table.nrows().max(1);
    let headers: Vec<String> = table.columns.iter().map(column_header).collect();
    ui.horizontal(|ui| {
        ui.label(sciplot_core::t("名称"));
        let response = ui.text_edit_singleline(&mut name);
        if response.gained_focus() && !app.pushed_name {
            app.history.push(sciplot_core::t("重命名"), &app.project);
            app.pushed_name = true;
        }
        if response.changed() {
            if !app.pushed_name {
                app.history.push(sciplot_core::t("重命名"), &app.project);
                app.pushed_name = true;
            }
            if let Err(err) = app.project.rename_table(id, &name) {
                app.status = err;
            } else {
                app.dirty = true;
            }
        }
        if response.lost_focus() {
            app.pushed_name = false;
        }
    });
    ui.separator();
    let min_width = 46.0 + 120.0 * ncols as f32;
    egui::ScrollArea::horizontal().show(ui, |ui| {
        ui.set_min_width(min_width);
        TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .column(Column::exact(46.0).clip(true))
            .columns(
                Column::initial(120.0)
                    .range(96.0..=280.0)
                    .resizable(true)
                    .clip(true),
                ncols,
            )
            .header(32.0, |mut header| {
                header.col(|ui| {
                    ui.label("#");
                });
                for (index, header_text) in headers.iter().enumerate() {
                    header.col(|ui| {
                        let selected = app.selected_column == Some((id, index));
                        let response = ui.add_sized(
                            ui.available_size(),
                            egui::SelectableLabel::new(selected, header_text),
                        );
                        if response.clicked() || response.secondary_clicked() {
                            app.selected_column = Some((id, index));
                        }
                        if response.double_clicked() {
                            crate::context::open_rename_column(app, id, index);
                        }
                        response.context_menu(|ui| {
                            crate::context::table_column(app, ui, id, index, None)
                        });
                    });
                }
            })
            .body(|body| {
                body.rows(22.0, nrows, |mut row| {
                    let row_index = row.index();
                    row.col(|ui| {
                        let response = ui.add(
                            egui::Label::new(format!("{}", row_index + 1))
                                .sense(egui::Sense::click()),
                        );
                        response
                            .context_menu(|ui| crate::context::table_row(app, ui, id, row_index));
                    });
                    for column in 0..ncols {
                        row.col(|ui| {
                            let mut text = app
                                .project
                                .table(id)
                                .and_then(|t| t.columns.get(column))
                                .map(|c| c.display(row_index))
                                .unwrap_or_default();
                            let mut output = egui::TextEdit::singleline(&mut text)
                                .id(ui.id().with(("cell", column, row_index)))
                                .show(ui);
                            let response = output.response.clone();
                            let arm_cell = app.arm_cell_focus == Some(id)
                                && app.dialog.is_none()
                                && row_index == 0
                                && column == 0;
                            if arm_cell {
                                response.request_focus();
                                ui.ctx().request_repaint();
                                app.arm_cell_focus = None;
                            }
                            if response.gained_focus() || arm_cell {
                                let end = egui::text::CCursor::new(text.chars().count());
                                output.state.cursor.set_char_range(Some(
                                    egui::text::CCursorRange::two(
                                        egui::text::CCursor::default(),
                                        end,
                                    ),
                                ));
                                output.state.store(ui.ctx(), response.id);
                            }
                            if response.lost_focus()
                                && ui.input(|input| input.key_pressed(egui::Key::Enter))
                            {
                                app.status = sciplot_core::t("已写入单元格").into();
                            }
                            if response.secondary_clicked() {
                                app.selected_column = Some((id, column));
                            }
                            response.context_menu(|ui| {
                                crate::context::table_column(app, ui, id, column, Some(row_index))
                            });
                            if response.changed() {
                                if !app.pushed_edit {
                                    app.history
                                        .push(sciplot_core::t("编辑单元格"), &app.project);
                                    app.pushed_edit = true;
                                    app.dirty = true;
                                }
                                if let Some(column) = app
                                    .project
                                    .table_mut(id)
                                    .and_then(|t| t.columns.get_mut(column))
                                {
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
    });
    if app.arm_cell_focus == Some(id) && app.dialog.is_none() {
        app.arm_cell_focus = None;
    }
}

fn matrix_view(app: &mut SciPlotApp, ui: &mut egui::Ui, id: MatrixId) {
    let Some(matrix) = app.project.matrix(id) else {
        ui.label(sciplot_core::t("矩阵已删除"));
        return;
    };
    let rows = matrix.rows;
    let cols = matrix.cols;
    let peak = matrix
        .data
        .iter()
        .flatten()
        .fold(0.0_f64, |acc, value| acc.max(value.abs()));
    let noise = (peak * 1e-12).max(0.0);
    let banner = ui.add(
        egui::Label::new(format!(
            "{}   {rows}×{cols}   x [{} , {}]   y [{} , {}]",
            matrix.name,
            sciplot_core::format_number(matrix.x_min),
            sciplot_core::format_number(matrix.x_max),
            sciplot_core::format_number(matrix.y_min),
            sciplot_core::format_number(matrix.y_max)
        ))
        .sense(egui::Sense::click()),
    );
    banner.context_menu(|ui| crate::context::matrix_menu(app, ui, id));
    TableBuilder::new(ui)
        .striped(true)
        .column(Column::exact(40.0))
        .columns(
            Column::initial(96.0).range(84.0..=180.0).resizable(true),
            cols,
        )
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
                        let mut text = app
                            .project
                            .matrix(id)
                            .and_then(|m| m.get(r, c))
                            .map(|value| {
                                if value.abs() <= noise {
                                    "0".into()
                                } else {
                                    sciplot_core::format_number(value)
                                }
                            })
                            .unwrap_or_default();
                        let response = ui.add(egui::TextEdit::singleline(&mut text));
                        response.context_menu(|ui| crate::context::matrix_menu(app, ui, id));
                        if response.changed() {
                            if !app.pushed_edit {
                                app.history.push(sciplot_core::t("编辑矩阵"), &app.project);
                                app.pushed_edit = true;
                                app.dirty = true;
                            }
                            let value = if text.trim().is_empty() {
                                None
                            } else {
                                text.trim().parse::<f64>().ok()
                            };
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
    let Some(text) = app
        .project
        .notes
        .iter()
        .find(|n| n.id == id)
        .map(|n| n.text.clone())
    else {
        ui.label(sciplot_core::t("备注已删除"));
        return;
    };
    let mut text = text;
    let rows = ((ui.available_height() / 22.0).floor() as usize).clamp(12, 80);
    let response = ui.add(
        egui::TextEdit::multiline(&mut text)
            .desired_width(f32::INFINITY)
            .desired_rows(rows),
    );
    if response.changed() {
        if !app.pushed_edit {
            app.history.push(sciplot_core::t("编辑备注"), &app.project);
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
    let Some(mut graph) = app.project.graph(id).cloned() else {
        ui.label(sciplot_core::t("图形已删除"));
        return;
    };
    let surface = graph.surface.is_some();
    let mut visibility_changed = false;
    ui.horizontal_wrapped(|ui| {
        if !graph.layers.is_empty() && graph.surface.is_none() {
            let layer_index = app.layer_index(id);
            let curves: Vec<(String, bool)> = graph
                .layers
                .get(layer_index)
                .map(|layer| {
                    layer
                        .curves
                        .iter()
                        .map(|curve| (curve.name.clone(), curve.visible))
                        .collect()
                })
                .unwrap_or_default();
            if !curves.is_empty() {
                app.curve = app.curve.min(curves.len() - 1);
                let (name, visible) = &curves[app.curve];
                let selected = if *visible {
                    name.clone()
                } else {
                    sciplot_core::trf!("{name}（隐藏）", "{name} (hidden)", name = name)
                };
                let mut toggle = None;
                egui::ComboBox::from_id_salt(("curve-pick", id.0))
                    .selected_text(selected)
                    .show_ui(ui, |ui| {
                        for (index, (name, visible)) in curves.iter().enumerate() {
                            ui.horizontal(|ui| {
                                let mut on = *visible;
                                if ui.checkbox(&mut on, "").changed() {
                                    toggle = Some(index);
                                }
                                let label = if *visible {
                                    name.clone()
                                } else {
                                    sciplot_core::trf!(
                                        "{name}（隐藏）",
                                        "{name} (hidden)",
                                        name = name
                                    )
                                };
                                ui.selectable_value(&mut app.curve, index, label);
                            });
                        }
                    });
                if let Some(index) = toggle {
                    let visible = !curves[index].1;
                    set_curve_visible(app, id, layer_index, index, visible);
                    visibility_changed = true;
                }
                if curves.get(app.curve).is_some_and(|(_, visible)| !visible)
                    && ui.button(sciplot_core::t("显示")).clicked()
                {
                    set_curve_visible(app, id, layer_index, app.curve, true);
                    visibility_changed = true;
                }
            }
        }
        ui.checkbox(&mut app.reader, sciplot_core::t("读取"));
        if ui.button(sciplot_core::t("清除缩放")).clicked() {
            reset_graph_view(app, id);
        }
    });
    let hint = if surface {
        sciplot_core::t("左键拖拽旋转，滚轮缩放，双击或「清除缩放」恢复视角。右键选择显示方式")
    } else {
        sciplot_core::t("左键平移，滚轮缩放，双击恢复，Shift 选择 X 区间。右键改曲线")
    };
    ui.label(hint).on_hover_text(if surface {
        sciplot_core::t("在图上按住左键拖拽可以旋转，滚轮改变远近。双击图面，或点「清除缩放」，恢复默认视角。右键打开菜单，选择曲面、线框或散点。")
    } else {
        sciplot_core::t("左键拖拽平移，滚轮缩放，双击恢复视图。按住 Shift 拖拽只选择 X 区间，拟合和求交点会用到它。右键曲线可以改属性、隐藏或分析。")
    });
    if visibility_changed {
        if let Some(updated) = app.project.graph(id).cloned() {
            graph = updated;
        }
    }
    let available = ui.available_size();
    if available.x < 30.0 || available.y < 30.0 {
        return;
    }
    let (rect, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());
    if graph.surface.is_some() {
        rotate_surface(app, ui, id, rect, &response);
        if response.double_clicked() {
            reset_graph_view(app, id);
        } else if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.0 {
                let zoomed = app
                    .project
                    .graph_mut(id)
                    .and_then(|g| g.surface.as_mut())
                    .is_some_and(|spec| {
                        spec.zoom = (spec.zoom * (1.0 + scroll * 0.001)).clamp(0.25, 8.0);
                        true
                    });
                if zoomed {
                    app.dirty = true;
                }
            }
        }
        let graph = app.project.graph(id).cloned().unwrap_or(graph);
        let scene = render(&app.project, &graph, &[], rect.width(), rect.height(), 1.0);
        remember_plot_notes(app, &scene);
        paint_scene(app, ui, rect, &scene, true);
        attach_plot_menu(app, &response, id, rect, &scene);
        return;
    }

    let mut views = app.views.get(&id).cloned().unwrap_or_default();
    if views.len() < graph.layers.len() {
        views.resize(graph.layers.len(), LayerView::default());
    }
    let mut scene = render(
        &app.project,
        &graph,
        &views,
        rect.width(),
        rect.height(),
        1.0,
    );
    remember_plot_notes(app, &scene);
    let mut changed = false;
    let pointer = response.interact_pointer_pos().or(response.hover_pos());
    if let Some(pos) = pointer {
        let local = pos - rect.min;
        if let Some(layer) = scene
            .layers
            .iter()
            .position(|frame| frame.plot.contains(local.x, local.y))
        {
            app.active_layer = layer;
            let frame = &scene.layers[layer];
            if response.double_clicked() {
                views[layer] = LayerView::default();
                changed = true;
            }
            let shift = ui.input(|i| i.modifiers.shift);
            let primary_started = response.drag_started_by(egui::PointerButton::Primary);
            if primary_started && shift {
                if let Some((x, _)) = screen_to_data(frame, local.x, local.y) {
                    app.xsel = Some(XSel {
                        graph: id,
                        layer,
                        x0: x,
                        x1: x,
                    });
                }
            } else if primary_started {
                app.ann_drag = None;
                if let Some((ann_id, origin, _, _)) =
                    hit_annotation(&graph, layer, frame, local.x, local.y)
                {
                    if let Some((sx, sy)) = screen_to_data(frame, local.x, local.y) {
                        app.history.push(sciplot_core::t("移动标注"), &app.project);
                        app.dirty = true;
                        app.selected_ann = Some((id, layer, ann_id));
                        app.ann_drag = Some(AnnDrag {
                            graph: id,
                            layer,
                            id: ann_id,
                            origin,
                            start_x: sx,
                            start_y: sy,
                        });
                    }
                }
            }
            let primary_drag = response.dragged_by(egui::PointerButton::Primary);
            if primary_drag && shift {
                if let Some((x, _)) = screen_to_data(frame, local.x, local.y) {
                    if let Some(sel) = &mut app.xsel {
                        sel.x1 = x;
                    }
                }
            } else if primary_drag {
                let drag = app.ann_drag.as_ref().map(|drag| {
                    (
                        drag.graph,
                        drag.layer,
                        drag.id,
                        drag.origin.clone(),
                        drag.start_x,
                        drag.start_y,
                    )
                });
                if let Some((drag_graph, drag_layer, ann_id, origin, start_x, start_y)) = drag {
                    if drag_graph == id && drag_layer == layer {
                        if let Some((x, y)) = screen_to_data(frame, local.x, local.y) {
                            let kind = shift_kind(&origin, x - start_x, y - start_y);
                            if let Some(layer_ref) = app
                                .project
                                .graph_mut(id)
                                .and_then(|g| g.layers.get_mut(layer))
                            {
                                if let Some(annotation) =
                                    layer_ref.annotations.iter_mut().find(|a| a.id == ann_id)
                                {
                                    annotation.kind = kind;
                                }
                            }
                            changed = true;
                        }
                    }
                } else if !shift {
                    let delta = response.drag_delta();
                    if let Some(((xmin, xmax), (ymin, ymax))) = pan_limits(frame, delta.x, delta.y)
                    {
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
                        let factor = if scroll.abs() > 0.2 {
                            (1.0 + scroll as f64 * 0.0015).clamp(0.75, 1.35)
                        } else {
                            zoom as f64
                        };
                        if let Some((a, b)) =
                            zoom_limits(frame.x_scale, frame.x_min, frame.x_max, x, factor)
                        {
                            views[layer].x_min = Some(a);
                            views[layer].x_max = Some(b);
                        }
                        if let Some((a, b)) =
                            zoom_limits(frame.y_scale, frame.y_min, frame.y_max, y, factor)
                        {
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
        if let Some(sel) = &app.xsel {
            if sel.graph == id && (sel.x0 - sel.x1).abs() > 1e-9 {
                let lo = sel.x0.min(sel.x1);
                let hi = sel.x0.max(sel.x1);
                app.status = sciplot_core::trf!(
                    "已选择 X 区间 [{}, {}]，拟合和求交点会用这个区间",
                    "X range [{}, {}] selected. Fitting and intersections will use it.",
                    sciplot_core::format_number(lo),
                    sciplot_core::format_number(hi)
                );
            }
        }
        app.ann_drag = None;
    }
    if response.clicked() && !response.double_clicked() {
        if let Some(pos) = response.interact_pointer_pos().or(response.hover_pos()) {
            let local = pos - rect.min;
            if let Some((layer, curve)) = scene
                .legend_hits
                .iter()
                .find(|hit| hit.rect.contains(local.x, local.y))
                .map(|hit| (hit.layer, hit.curve))
            {
                let visible = app
                    .project
                    .graph(id)
                    .and_then(|item| item.layers.get(layer))
                    .and_then(|item| item.curves.get(curve))
                    .map(|curve| curve.visible)
                    .unwrap_or(true);
                set_curve_visible(app, id, layer, curve, !visible);
                changed = true;
            }
        }
    }
    if changed {
        let graph = app.project.graph(id).cloned().unwrap_or(graph);
        scene = render(
            &app.project,
            &graph,
            &views,
            rect.width(),
            rect.height(),
            1.0,
        );
    }
    app.views.insert(id, views);
    paint_scene(app, ui, rect, &scene, surface);
    paint_overlays(app, ui, id, rect, &scene);
    attach_plot_menu(app, &response, id, rect, &scene);
}

fn rotate_surface(
    app: &mut SciPlotApp,
    ui: &mut egui::Ui,
    id: GraphId,
    rect: egui::Rect,
    response: &egui::Response,
) {
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    let drag_id = ui.id().with(("surface-drag", id.0));
    let primary = egui::PointerButton::Primary;
    let (press_in, delta, down) = ui.input(|input| {
        let press = input
            .pointer
            .press_origin()
            .is_some_and(|pos| rect.contains(pos));
        (
            press,
            input.pointer.delta(),
            input.pointer.button_down(primary),
        )
    });
    let dragging = down && press_in && !response.double_clicked();
    let mut pushed = ui.data(|data| data.get_temp::<bool>(drag_id).unwrap_or(false));
    if dragging && delta.length_sq() > 0.0 {
        if !pushed {
            app.history
                .push(sciplot_core::t("旋转三维图"), &app.project);
            app.dirty = true;
            pushed = true;
        }
        if let Some(spec) = app
            .project
            .graph_mut(id)
            .and_then(|graph| graph.surface.as_mut())
        {
            spec.azimuth += delta.x * 0.9;
            spec.elevation = (spec.elevation - delta.y * 0.7).clamp(-85.0, 85.0);
        }
    }
    if !down {
        pushed = false;
    }
    ui.data_mut(|data| data.insert_temp(drag_id, pushed));
}

pub(crate) fn reset_graph_view(app: &mut SciPlotApp, id: GraphId) {
    app.views.remove(&id);
    let needs_camera = app
        .project
        .graph(id)
        .and_then(|graph| graph.surface.as_ref())
        .is_some_and(|spec| !spec.camera_is_default());
    if needs_camera {
        app.edit(sciplot_core::t("清除缩放"), |project| {
            if let Some(spec) = project
                .graph_mut(id)
                .and_then(|graph| graph.surface.as_mut())
            {
                spec.restore_camera();
            }
            Ok(())
        });
    }
    app.status = sciplot_core::t("已恢复自动范围").into();
}

fn attach_plot_menu(
    app: &mut SciPlotApp,
    response: &egui::Response,
    id: GraphId,
    rect: egui::Rect,
    scene: &Scene,
) {
    if response.secondary_clicked() {
        let local = response
            .interact_pointer_pos()
            .or(response.hover_pos())
            .map(|pos| pos - rect.min);
        app.plot_hit = Some(match local {
            Some(local) => classify_plot(app, id, local, scene),
            None => PlotHit::Layer {
                graph: id,
                layer: app.layer_index(id),
            },
        });
    }
    response.context_menu(|ui| crate::context::plot_menu(app, ui));
}

fn classify_plot(app: &mut SciPlotApp, id: GraphId, local: egui::Vec2, scene: &Scene) -> PlotHit {
    let Some(graph) = app.project.graph(id).cloned() else {
        return PlotHit::Layer {
            graph: id,
            layer: 0,
        };
    };
    if graph.surface.is_some() {
        return PlotHit::Surface { graph: id };
    }
    for (layer_index, frame) in scene.layers.iter().enumerate() {
        if let Some((ann_id, _, _, _)) =
            hit_annotation(&graph, layer_index, frame, local.x, local.y)
        {
            app.selected_ann = Some((id, layer_index, ann_id));
            return PlotHit::Annotation {
                graph: id,
                layer: layer_index,
                id: ann_id,
            };
        }
    }
    if let Some(hit) = scene
        .legend_hits
        .iter()
        .find(|hit| hit.rect.contains(local.x, local.y))
    {
        app.curve = hit.curve;
        app.active_layer = hit.layer;
        return PlotHit::Curve {
            graph: id,
            layer: hit.layer,
            curve: hit.curve,
        };
    }
    if let Some(hit) = nearest_curve(&app.project, &graph, &scene.layers, local.x, local.y) {
        app.curve = hit.curve;
        app.active_layer = hit.layer;
        return PlotHit::Curve {
            graph: id,
            layer: hit.layer,
            curve: hit.curve,
        };
    }
    let layer = scene
        .layers
        .iter()
        .position(|frame| frame.plot.contains(local.x, local.y))
        .unwrap_or_else(|| app.layer_index(id));
    app.active_layer = layer;
    PlotHit::Layer { graph: id, layer }
}

fn paint_scene(
    app: &mut SciPlotApp,
    ui: &egui::Ui,
    rect: egui::Rect,
    scene: &Scene,
    surface: bool,
) {
    if surface {
        if app.glyphs.is_none() {
            app.glyphs = crate::fonts::plot_glyphs(&app.fonts);
        }
        if let Ok((width, height, rgba)) = sciplot_plot::to_rgba(scene, app.glyphs.as_ref()) {
            let image =
                egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &rgba);
            let texture =
                ui.ctx()
                    .load_texture("sciplot-surface", image, egui::TextureOptions::LINEAR);
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
                if let (Some((x0, _)), Some((x1, _))) = (
                    data_to_screen(frame, sel.x0, frame.y_min),
                    data_to_screen(frame, sel.x1, frame.y_min),
                ) {
                    let band = egui::Rect::from_min_max(
                        rect.min + egui::vec2(x0.min(x1), frame.plot.y),
                        rect.min + egui::vec2(x0.max(x1), frame.plot.bottom()),
                    );
                    painter.rect_filled(
                        band,
                        0.0,
                        egui::Color32::from_rgba_unmultiplied(40, 110, 210, 96),
                    );
                    let lo = sel.x0.min(sel.x1);
                    let hi = sel.x0.max(sel.x1);
                    painter.text(
                        egui::pos2(band.center().x, band.top()),
                        egui::Align2::CENTER_BOTTOM,
                        format!(
                            "[{}, {}]",
                            sciplot_core::format_number(lo),
                            sciplot_core::format_number(hi)
                        ),
                        egui::FontId::new(
                            13.0,
                            egui::FontFamily::Name(sciplot_plot::PLOT_FONT_FAMILY.into()),
                        ),
                        egui::Color32::from_rgb(20, 60, 140),
                    );
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
                    if let Some(hit) =
                        nearest_point(&app.project, graph, layer, frame, local.x, local.y)
                    {
                        let color = egui::Color32::from_rgb(20, 20, 20);
                        let stroke = egui::Stroke::new(1.0_f32, color);
                        let p = rect.min + egui::vec2(hit.px, hit.py);
                        painter.line_segment(
                            [
                                egui::pos2(rect.min.x + frame.plot.x, p.y),
                                egui::pos2(rect.min.x + frame.plot.right(), p.y),
                            ],
                            stroke,
                        );
                        painter.line_segment(
                            [
                                egui::pos2(p.x, rect.min.y + frame.plot.y),
                                egui::pos2(p.x, rect.min.y + frame.plot.bottom()),
                            ],
                            stroke,
                        );
                        painter.circle_stroke(p, 4.0, stroke);
                        let name = graph
                            .layers
                            .get(layer)
                            .and_then(|l| l.curves.get(hit.curve))
                            .map(|c| c.name.as_str())
                            .unwrap_or(sciplot_core::t("点"));
                        painter.text(
                            p + egui::vec2(8.0, -8.0),
                            egui::Align2::LEFT_BOTTOM,
                            format!(
                                "{name} ({}, {})",
                                sciplot_core::format_number(hit.x),
                                sciplot_core::format_number(hit.y)
                            ),
                            egui::FontId::new(
                                13.0,
                                egui::FontFamily::Name(sciplot_plot::PLOT_FONT_FAMILY.into()),
                            ),
                            color,
                        );
                    }
                }
            }
        }
    }
}

fn hit_annotation(
    graph: &Graph,
    layer: usize,
    frame: &LayerFrame,
    x: f32,
    y: f32,
) -> Option<(u64, AnnotationKind, f64, f64)> {
    let layer = graph.layers.get(layer)?;
    let mut best = None;
    let mut best_d = 14.0_f32;
    for annotation in &layer.annotations {
        let (ax, ay) = anchor(&annotation.kind);
        let Some((px, py)) = data_to_screen(frame, ax, ay) else {
            continue;
        };
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
        AnnotationKind::Text {
            x,
            y,
            text,
            size,
            color,
        } => AnnotationKind::Text {
            x: x + dx,
            y: y + dy,
            text,
            size,
            color,
        },
        AnnotationKind::Marker { x, y, label, color } => AnnotationKind::Marker {
            x: x + dx,
            y: y + dy,
            label,
            color,
        },
        AnnotationKind::Arrow {
            x1,
            y1,
            x2,
            y2,
            color,
        } => AnnotationKind::Arrow {
            x1: x1 + dx,
            y1: y1 + dy,
            x2: x2 + dx,
            y2: y2 + dy,
            color,
        },
        AnnotationKind::Line {
            x1,
            y1,
            x2,
            y2,
            color,
        } => AnnotationKind::Line {
            x1: x1 + dx,
            y1: y1 + dy,
            x2: x2 + dx,
            y2: y2 + dy,
            color,
        },
    }
}

fn set_curve_visible(
    app: &mut SciPlotApp,
    graph: GraphId,
    layer: usize,
    curve: usize,
    visible: bool,
) {
    app.edit(sciplot_core::t("曲线可见性"), |project| {
        let curve = project
            .graph_mut(graph)
            .and_then(|item| item.layers.get_mut(layer))
            .and_then(|item| item.curves.get_mut(curve))
            .ok_or(sciplot_core::t("找不到曲线"))?;
        curve.visible = visible;
        Ok(())
    });
    app.status = if visible {
        sciplot_core::t("已显示曲线").into()
    } else {
        sciplot_core::t("已隐藏曲线。点图例里变灰的名字，或点工具栏「显示」，可以再显示。").into()
    };
}

fn delete_selected_annotation(app: &mut SciPlotApp) {
    let Some((graph, layer, id)) = app.selected_ann else {
        app.status = sciplot_core::t("先在图上点选一个标注").into();
        return;
    };
    app.edit(sciplot_core::t("删除标注"), |project| {
        let layer = project
            .graph_mut(graph)
            .ok_or(sciplot_core::t("找不到图形"))?
            .layers
            .get_mut(layer)
            .ok_or(sciplot_core::t("找不到图层"))?;
        let before = layer.annotations.len();
        layer.annotations.retain(|a| a.id != id);
        if layer.annotations.len() == before {
            return Err(sciplot_core::t("没有这个标注").into());
        }
        Ok(())
    });
    app.selected_ann = None;
}

fn delete_focused(app: &mut SciPlotApp) {
    let Some(tab) = app.focus.clone() else { return };
    match tab {
        Tab::Table(id) => ask_delete_table(app, id),
        Tab::Matrix(id) => ask_delete_matrix(app, id),
        Tab::Graph(id) => commit_delete(app, sciplot_core::t("删除图形"), |project| {
            project.delete_graph(id)
        }),
        Tab::Note(id) => commit_delete(app, sciplot_core::t("删除备注"), |project| {
            project.delete_note(id)
        }),
    }
}

pub(crate) fn ask_delete_table(app: &mut SciPlotApp, id: TableId) {
    let names = app.project.graphs_using_table(id);
    if names.is_empty() {
        commit_delete(app, sciplot_core::t("删除表格"), |project| {
            project.delete_table(id)
        });
        return;
    }
    app.dialog = Some(Dialog::ConfirmDelete {
        target: DeleteTarget::Table(id),
        detail: sciplot_core::trf!(
            "这些图还引用这张表，删除后图上会说明找不到数据：{}。",
            "These graphs still use this table. After deletion they will say the data is missing: {}.",
            names.join("、")
        ),
    });
}

pub(crate) fn ask_delete_matrix(app: &mut SciPlotApp, id: MatrixId) {
    let names = app.project.graphs_using_matrix(id);
    if names.is_empty() {
        commit_delete(app, sciplot_core::t("删除矩阵"), |project| {
            project.delete_matrix(id)
        });
        return;
    }
    app.dialog = Some(Dialog::ConfirmDelete {
        target: DeleteTarget::Matrix(id),
        detail: sciplot_core::trf!(
            "这些图还引用这个矩阵，删除后图上会说明找不到数据：{}。",
            "These graphs still use this matrix. After deletion they will say the data is missing: {}.",
            names.join("、")
        ),
    });
}

pub(crate) fn commit_delete_target(app: &mut SciPlotApp, target: &DeleteTarget) {
    match *target {
        DeleteTarget::Table(id) => commit_delete(app, sciplot_core::t("删除表格"), |project| {
            project.delete_table(id)
        }),
        DeleteTarget::Matrix(id) => commit_delete(app, sciplot_core::t("删除矩阵"), |project| {
            project.delete_matrix(id)
        }),
    }
}

fn commit_delete(app: &mut SciPlotApp, label: &str, delete: impl FnOnce(&mut Project) -> bool) {
    let removed = app.edit(label, |project| {
        if delete(project) {
            Ok(())
        } else {
            Err(sciplot_core::t("没有找到要删除的对象").into())
        }
    });
    if removed.is_some() {
        app.focus = None;
    }
}

fn remember_plot_notes(app: &mut SciPlotApp, scene: &Scene) {
    if scene.notes.is_empty() {
        app.status_note = None;
        return;
    }
    let joined = scene.notes.join("；");
    if app.status_note.as_deref() != Some(joined.as_str()) {
        app.status = joined.clone();
        app.status_note = Some(joined);
    }
}

fn with_splot_extension(path: &Path) -> PathBuf {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("splot") => path.to_path_buf(),
        _ => {
            let mut name = path.as_os_str().to_os_string();
            name.push(".splot");
            PathBuf::from(name)
        }
    }
}

fn new_project(app: &mut SciPlotApp) {
    app.project = Project::new(sciplot_core::t("未命名项目"));
    app.history = History::new();
    app.views.clear();
    app.path = None;
    app.dirty = false;
    app.dock = DockState::new(Vec::new());
    chinese_dock(&mut app.dock);
    if let Some(id) = Some(app.project.new_table(sciplot_core::t("表格1"))) {
        app.open_tab(Tab::Table(id));
    }
    app.status = sciplot_core::t("已新建项目").into();
}

fn request_after(app: &mut SciPlotApp, then: AfterSave) {
    if app.dirty {
        app.dialog = Some(Dialog::Unsaved { then });
    } else {
        // 没有未保存修改时，退出需要这一帧的上下文，其余动作不需要。
        match then {
            AfterSave::Quit => {}
            other => run_after_now(app, other),
        }
    }
}

fn run_after(app: &mut SciPlotApp, then: AfterSave, ctx: &egui::Context) {
    match then {
        AfterSave::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        other => run_after_now(app, other),
    }
}

fn run_after_now(app: &mut SciPlotApp, then: AfterSave) {
    match then {
        AfterSave::Quit => {}
        AfterSave::NewProject => new_project(app),
        AfterSave::Open(path) => app.load_path(&path),
        AfterSave::OpenPicker => start_file(app, PathAction::Open, ""),
        AfterSave::Demo => load_demo(app),
    }
}

fn load_demo(app: &mut SciPlotApp) {
    let mut project = demo_project();
    ops::enrich_demo(&mut project);
    app.project = project;
    app.history = History::new();
    app.views.clear();
    app.path = None;
    app.dirty = false;
    app.rebuild_dock(true);
    app.status = sciplot_core::t("已重新载入示例项目").into();
}

pub(crate) fn start_file(app: &mut SciPlotApp, action: PathAction, extra: &str) {
    let file_name = default_file_name(app, action);
    let full = if file_name.is_empty() {
        default_dir(app)
    } else {
        default_dir(app).join(&file_name)
    };
    if let Some(path) = native_dialog(app, action, &file_name) {
        app.perform_path(action, &path.display().to_string(), extra);
        return;
    }
    app.dialog = Some(Dialog::Path {
        action,
        path: full.display().to_string(),
        extra: extra.to_string(),
    });
}

pub(crate) fn native_dialog(
    app: &SciPlotApp,
    action: PathAction,
    file_name: &str,
) -> Option<PathBuf> {
    if std::env::var("SCIPLOT_NO_DIALOG").is_ok() {
        return None;
    }
    let mut dialog = rfd::FileDialog::new().set_directory(default_dir(app));
    if !file_name.is_empty() && !matches!(action, PathAction::Open | PathAction::ImportCsv) {
        dialog = dialog.set_file_name(file_name);
    }
    dialog = match action {
        PathAction::Open | PathAction::SaveAs => {
            dialog.add_filter(sciplot_core::t("SciPlot 项目"), &["splot"])
        }
        PathAction::ImportCsv | PathAction::ExportCsv => {
            dialog.add_filter("CSV", &["csv", "txt", "tsv"])
        }
        PathAction::ExportPng => dialog.add_filter("PNG", &["png"]),
        PathAction::ExportSvg => dialog.add_filter("SVG", &["svg"]),
    };
    match action {
        PathAction::Open | PathAction::ImportCsv => dialog.pick_file(),
        PathAction::SaveAs
        | PathAction::ExportCsv
        | PathAction::ExportPng
        | PathAction::ExportSvg => dialog.save_file(),
    }
}

fn default_file_name(app: &SciPlotApp, action: PathAction) -> String {
    match action {
        PathAction::Open => String::new(),
        PathAction::SaveAs => safe_file_name(&app.project.name, "splot"),
        PathAction::ImportCsv => sciplot_core::t("数据.csv").into(),
        PathAction::ExportCsv => {
            let name = app
                .active_table()
                .and_then(|id| app.project.table(id))
                .map(|table| table.name.clone())
                .unwrap_or_else(|| sciplot_core::t("数据").into());
            safe_file_name(&name, "csv")
        }
        PathAction::ExportPng => graph_file_name(app, "png"),
        PathAction::ExportSvg => graph_file_name(app, "svg"),
    }
}

fn graph_file_name(app: &SciPlotApp, ext: &str) -> String {
    let name = app
        .active_graph()
        .and_then(|id| app.project.graph(id))
        .map(|graph| graph.name.clone())
        .unwrap_or_else(|| sciplot_core::t("图形").into());
    safe_file_name(&name, ext)
}

fn safe_file_name(name: &str, ext: &str) -> String {
    let stem: String = name
        .chars()
        .map(|ch| if "\\/:*?\"<>|".contains(ch) { '_' } else { ch })
        .collect();
    let stem = stem.trim().trim_matches('.');
    let stem = if stem.is_empty() {
        sciplot_core::t("未命名")
    } else {
        stem
    };
    format!("{stem}.{ext}")
}

pub(crate) fn default_dir(app: &SciPlotApp) -> PathBuf {
    if !app.prefs.last_dir.is_empty() {
        let dir = PathBuf::from(&app.prefs.last_dir);
        if dir.is_dir() {
            return dir;
        }
    }
    if let Some(dir) = app
        .path
        .as_ref()
        .and_then(|path| path.parent())
        .filter(|dir| dir.is_dir())
    {
        return dir.to_path_buf();
    }
    if let Ok(cwd) = std::env::current_dir() {
        if cwd.is_dir() {
            return cwd;
        }
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let documents = home.join("Documents");
    if documents.is_dir() {
        return documents;
    }
    let documents = home.join(sciplot_core::t("文档"));
    if documents.is_dir() {
        documents
    } else {
        home
    }
}

pub(crate) fn resolve_user_path(app: &SciPlotApp, text: &str) -> PathBuf {
    let path = PathBuf::from(text.trim());
    if path.as_os_str().is_empty() || path.is_absolute() {
        path
    } else {
        default_dir(app).join(path)
    }
}

fn demo_name(actual: &str, zh: &str, en: &str) -> bool {
    actual == zh || actual == en
}

fn chinese_dock(dock: &mut DockState<Tab>) {
    dock.translations.tab_context_menu.close_button = sciplot_core::t("关闭").into();
    dock.translations.tab_context_menu.eject_button = sciplot_core::t("弹出为窗口").into();
    let leaf = &mut dock.translations.leaf;
    leaf.close_button_disabled_tooltip = sciplot_core::t("这个标签不能关闭").into();
    leaf.close_all_button = sciplot_core::t("关闭窗口").into();
    leaf.close_all_button_menu_hint = sciplot_core::t("右键关闭窗口").into();
    leaf.close_all_button_modifier_hint = sciplot_core::t("按住 Shift 关闭窗口").into();
    leaf.close_all_button_modifier_menu_hint = sciplot_core::t("按住 Shift 或右键关闭窗口").into();
    leaf.close_all_button_disabled_tooltip = sciplot_core::t("窗口里有不能关闭的标签").into();
    leaf.minimize_button = sciplot_core::t("最小化").into();
    leaf.minimize_button_menu_hint = sciplot_core::t("右键最小化").into();
    leaf.minimize_button_modifier_hint = sciplot_core::t("按住 Shift 最小化").into();
    leaf.minimize_button_modifier_menu_hint = sciplot_core::t("按住 Shift 或右键最小化").into();
}

fn load_recent() -> Vec<PathBuf> {
    let path = recent_path();
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<String>>(&text)
        .unwrap_or_default()
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

fn remember(recent: &mut Vec<PathBuf>, path: &Path) {
    recent.retain(|item| item != path);
    recent.insert(0, path.to_path_buf());
    recent.truncate(8);
    let names: Vec<String> = recent.iter().map(|p| p.display().to_string()).collect();
    std::fs::write(
        recent_path(),
        serde_json::to_string_pretty(&names).unwrap_or_else(|_| "[]".into()),
    )
    .ok();
}

fn recent_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".sciplot_recent.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close_every_tab(app: &mut SciPlotApp) {
        let open: Vec<Tab> = app
            .dock
            .iter_all_tabs()
            .map(|(_, tab)| tab.clone())
            .collect();
        for tab in open {
            if let Some(path) = app.dock.find_tab(&tab) {
                app.dock.remove_tab(path);
            }
        }
    }

    #[test]
    fn reopening_a_table_after_closing_every_tab_keeps_the_dock() {
        let mut app = SciPlotApp::empty(demo_project());
        app.rebuild_dock(true);
        app.flush_tabs();
        assert!(app.dock.iter_all_tabs().next().is_some());
        close_every_tab(&mut app);
        drop_stale_tabs(&mut app);
        let id = app.project.tables[0].id;
        app.open_tab(Tab::Table(id));
        app.flush_tabs();
        assert!(app.dock.find_tab(&Tab::Table(id)).is_some());
    }

    #[test]
    fn flush_rebuilds_a_dock_whose_main_surface_was_removed() {
        let mut app = SciPlotApp::empty(demo_project());
        app.rebuild_dock(true);
        app.flush_tabs();
        app.dock.retain_tabs(|_| false);
        let id = app.project.tables[0].id;
        app.open_tab(Tab::Table(id));
        app.flush_tabs();
        assert!(app.dock.find_tab(&Tab::Table(id)).is_some());
    }

    #[test]
    fn opening_a_project_shows_the_first_graph_and_its_table() {
        let mut app = SciPlotApp::empty(demo_project());
        app.rebuild_dock(false);
        app.flush_tabs();
        let tabs: Vec<Tab> = app
            .dock
            .iter_all_tabs()
            .map(|(_, tab)| tab.clone())
            .collect();
        let graphs = tabs
            .iter()
            .filter(|tab| matches!(tab, Tab::Graph(_)))
            .count();
        let tables = tabs
            .iter()
            .filter(|tab| matches!(tab, Tab::Table(_)))
            .count();
        assert_eq!(graphs, 1);
        assert!(tables >= 1);
        assert!(
            tabs.len()
                < app.project.tables.len() + app.project.graphs.len() + app.project.matrices.len()
        );
    }
}
