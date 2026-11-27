//! `.splot`、压缩包，以及和 SciDAVis `.sciprj` 的有限交换。
//!
//! `.splot` 仍是 `FORMAT_VERSION = 1` 的 JSON。`.splot.gz` 是同一份 JSON 的 gzip。
//! `.sciprj` / `.sciprj.gz` 是下面这个 XML 子集，不是 SciDAVis 2.9 的逐字节副本：
//! 表格列、矩阵数值、备注、日志、文件夹，以及按表名引用的曲线。
//! 单元格公式、线型、三维图和标注不在交换范围内。Origin 文件只读，见 `origin`。

use std::io::{Read, Write};
use std::path::Path;

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;

use crate::graph::{Curve, CurveKind, DataSource};
use crate::table::{Column, ColumnMode};
use crate::{FolderId, GraphId, Project};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectFormat {
    Splot,
    SplotGz,
    Sciprj,
    SciprjGz,
    Origin,
}

impl ProjectFormat {
    pub fn from_path(path: &Path) -> Option<Self> {
        let name = path.file_name()?.to_string_lossy();
        Self::from_name(&name)
    }

    pub fn from_name(name: &str) -> Option<Self> {
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".sciprj.gz") {
            Some(Self::SciprjGz)
        } else if lower.ends_with(".splot.gz") {
            Some(Self::SplotGz)
        } else if lower.ends_with(".sciprj") {
            Some(Self::Sciprj)
        } else if lower.ends_with(".splot") {
            Some(Self::Splot)
        } else if lower.ends_with(".opj")
            || lower.ends_with(".opju")
            || lower.ends_with(".ogw")
            || lower.ends_with(".ogg")
            || lower.ends_with(".ogm")
        {
            Some(Self::Origin)
        } else {
            None
        }
    }
}

pub fn load_project_bytes(name: &str, bytes: &[u8]) -> Result<Project, String> {
    match ProjectFormat::from_name(name) {
        Some(ProjectFormat::Splot) => Project::from_json(&utf8(bytes)?),
        Some(ProjectFormat::SplotGz) => Project::from_json(&utf8(&gunzip(bytes)?)?),
        Some(ProjectFormat::Sciprj) => load_sciprj_str(&utf8(bytes)?),
        Some(ProjectFormat::SciprjGz) => load_sciprj_str(&utf8(&gunzip(bytes)?)?),
        Some(ProjectFormat::Origin) => crate::origin::load_origin(bytes),
        None => sniff(name, bytes),
    }
}

pub fn save_project_bytes(project: &Project, format: Option<ProjectFormat>) -> Result<Vec<u8>, String> {
    match format.unwrap_or(ProjectFormat::Splot) {
        ProjectFormat::Splot => Ok(project.to_json()?.into_bytes()),
        ProjectFormat::SplotGz => gzip(project.to_json()?.as_bytes()),
        ProjectFormat::Sciprj => Ok(to_sciprj(project).into_bytes()),
        ProjectFormat::SciprjGz => gzip(to_sciprj(project).as_bytes()),
        ProjectFormat::Origin => Err(crate::tr!(
            "Origin 项目只能打开，不能写回去。请另存为 .splot 或 .sciprj。",
            "Origin projects can be opened but not written back. Save as .splot or .sciprj."
        )
        .into()),
    }
}

pub fn save_project_path(project: &Project, path: &Path) -> Result<(), String> {
    let bytes = save_project_bytes(project, ProjectFormat::from_path(path))?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
    }
    std::fs::write(path, bytes).map_err(|err| err.to_string())
}

fn sniff(name: &str, bytes: &[u8]) -> Result<Project, String> {
    if bytes.starts_with(&[0x1f, 0x8b]) {
        let inner = gunzip(bytes)?;
        let hint = name
            .strip_suffix(".gz")
            .or_else(|| name.strip_suffix(".GZ"))
            .unwrap_or(name);
        return load_project_bytes(hint, &inner);
    }
    let start = trim_ascii(bytes);
    if start.starts_with(b"PK\x03\x04") || start.starts_with(b"CPYA") || start.starts_with(b"CPYUA") {
        return crate::origin::load_origin(bytes);
    }
    if start.starts_with(b"{") {
        return Project::from_json(&utf8(bytes)?);
    }
    if start.starts_with(b"<") || start.starts_with(b"<?xml") {
        let text = utf8(bytes)?;
        let root = parse_xml(&text)?;
        let tag = root.name.as_str();
        if matches!(tag, "origin" | "workbook" | "sheet" | "worksheet") {
            return crate::origin::load_origin(bytes);
        }
        return load_sciprj_root(root);
    }
    Err(crate::tr!(
        "认不出这个项目文件。可以打开 .splot、.splot.gz、.sciprj、.sciprj.gz，以及一部分 Origin 文件。",
        "Unrecognized project file. SciPlot opens .splot, .splot.gz, .sciprj, .sciprj.gz, and a subset of Origin files."
    )
    .into())
}

fn utf8(bytes: &[u8]) -> Result<String, String> {
    String::from_utf8(bytes.to_vec()).map_err(|_| crate::tr!("项目文件不是 UTF-8", "The project file is not UTF-8").to_string())
}

fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let mut start = 0;
    while start < bytes.len() && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    &bytes[start..]
}

fn gzip(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).map_err(|err| err.to_string())?;
    encoder.finish().map_err(|err| err.to_string())
}

fn gunzip(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoder = GzDecoder::new(bytes);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).map_err(|err| {
        crate::trf!("压缩项目读不出来：{err}", "Could not read the compressed project: {err}", err = err)
    })?;
    Ok(out)
}

fn to_sciprj(project: &Project) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!("<sciprj version=\"1\" name=\"{}\">\n", escape(&project.name)));
    write_contents(&mut out, project, None, 1);
    if !project.log.is_empty() {
        out.push_str("  <log><![CDATA[");
        out.push_str(&project.log.join("\n").replace("]]>", "]]]]><![CDATA[>"));
        out.push_str("]]></log>\n");
    }
    out.push_str("</sciprj>\n");
    out
}

fn write_contents(out: &mut String, project: &Project, folder: Option<FolderId>, depth: usize) {
    let pad = "  ".repeat(depth);
    for table in project.tables.iter().filter(|table| table.folder == folder) {
        out.push_str(&pad);
        out.push_str(&format!("<table name=\"{}\">\n", escape(&table.name)));
        for column in &table.columns {
            out.push_str(&pad);
            out.push_str(&format!(
                "  <column name=\"{}\" designation=\"{}\">\n",
                escape(&column.name),
                designation(column.mode)
            ));
            if !column.formula.trim().is_empty() {
                out.push_str(&pad);
                out.push_str("    <formula>");
                out.push_str(&escape(&column.formula));
                out.push_str("</formula>\n");
            }
            for row in 0..column.len() {
                out.push_str(&pad);
                out.push_str("    <row>");
                out.push_str(&escape(&cell_text(column, row)));
                out.push_str("</row>\n");
            }
            out.push_str(&pad);
            out.push_str("  </column>\n");
        }
        out.push_str(&pad);
        out.push_str("</table>\n");
    }
    for matrix in project.matrices.iter().filter(|matrix| matrix.folder == folder) {
        out.push_str(&pad);
        out.push_str(&format!(
            "<matrix name=\"{}\" rows=\"{}\" cols=\"{}\">\n",
            escape(&matrix.name),
            matrix.rows,
            matrix.cols
        ));
        for row in 0..matrix.rows {
            out.push_str(&pad);
            out.push_str("  <row>");
            for col in 0..matrix.cols {
                if col > 0 {
                    out.push(' ');
                }
                match matrix.get(row, col) {
                    Some(value) => out.push_str(&crate::format_short(value)),
                    None => out.push_str("--"),
                }
            }
            out.push_str("</row>\n");
        }
        out.push_str(&pad);
        out.push_str("</matrix>\n");
    }
    for note in project.notes.iter().filter(|note| note.folder == folder) {
        out.push_str(&pad);
        out.push_str(&format!("<note name=\"{}\"><![CDATA[", escape(&note.name)));
        out.push_str(&note.text.replace("]]>", "]]]]><![CDATA[>"));
        out.push_str("]]></note>\n");
    }
    for graph in project.graphs.iter().filter(|graph| graph.folder == folder) {
        out.push_str(&pad);
        out.push_str(&format!("<graph name=\"{}\">\n", escape(&graph.name)));
        for layer in &graph.layers {
            out.push_str(&pad);
            out.push_str(&format!("  <layer title=\"{}\">\n", escape(&layer.title)));
            if !layer.x_axis.label.is_empty() {
                out.push_str(&pad);
                out.push_str(&format!("    <xlabel>{}</xlabel>\n", escape(&layer.x_axis.label)));
            }
            if !layer.y_axis.label.is_empty() {
                out.push_str(&pad);
                out.push_str(&format!("    <ylabel>{}</ylabel>\n", escape(&layer.y_axis.label)));
            }
            for curve in &layer.curves {
                write_curve(out, project, curve, depth + 2);
            }
            out.push_str(&pad);
            out.push_str("  </layer>\n");
        }
        out.push_str(&pad);
        out.push_str("</graph>\n");
    }
    for id in project.child_folders(folder) {
        let Some(child) = project.folder(id) else {
            continue;
        };
        out.push_str(&pad);
        out.push_str(&format!("<folder name=\"{}\">\n", escape(&child.name)));
        write_contents(out, project, Some(id), depth + 1);
        out.push_str(&pad);
        out.push_str("</folder>\n");
    }
}

fn write_curve(out: &mut String, project: &Project, curve: &Curve, depth: usize) {
    let pad = "  ".repeat(depth);
    match &curve.source {
        DataSource::Columns { table, x, y, .. } => {
            let Some(table) = project.table(*table) else {
                return;
            };
            out.push_str(&pad);
            out.push_str(&format!(
                "<curve name=\"{}\" table=\"{}\" x=\"{}\" y=\"{}\" kind=\"{}\"/>\n",
                escape(&curve.name),
                escape(&table.name),
                x,
                y,
                kind_name(curve.kind)
            ));
        }
        DataSource::Function { expr, x_min, x_max, samples } => {
            out.push_str(&pad);
            out.push_str(&format!(
                "<curve name=\"{}\" kind=\"function\" expr=\"{}\" xmin=\"{}\" xmax=\"{}\" samples=\"{}\"/>\n",
                escape(&curve.name),
                escape(expr),
                crate::format_short(*x_min),
                crate::format_short(*x_max),
                samples
            ));
        }
        _ => {}
    }
}

fn cell_text(column: &Column, row: usize) -> String {
    if let Some(text) = column.texts.get(row) {
        if !text.is_empty() {
            return text.clone();
        }
    }
    column
        .numbers
        .get(row)
        .copied()
        .flatten()
        .filter(|value| value.is_finite())
        .map(crate::format_short)
        .unwrap_or_default()
}

fn designation(mode: ColumnMode) -> &'static str {
    match mode {
        ColumnMode::X => "X",
        ColumnMode::Y => "Y",
        ColumnMode::Z => "Z",
        ColumnMode::XErr => "xErr",
        ColumnMode::YErr => "yErr",
        ColumnMode::Label => "label",
        ColumnMode::None => "none",
    }
}

fn kind_name(kind: CurveKind) -> &'static str {
    match kind {
        CurveKind::Line => "line",
        CurveKind::Scatter => "scatter",
        CurveKind::LineSymbol => "line+symbol",
        CurveKind::Steps => "steps",
        CurveKind::Bars => "bars",
        CurveKind::HorizontalBars => "horizontal-bars",
        CurveKind::Histogram => "histogram",
        CurveKind::Area => "area",
        CurveKind::ErrorBars => "error-bars",
        CurveKind::BoxPlot => "box",
        CurveKind::Pie => "pie",
        CurveKind::Vector => "vector",
        CurveKind::Contour => "contour",
        CurveKind::Heatmap => "heatmap",
        CurveKind::Function => "function",
        CurveKind::Parametric => "parametric",
        CurveKind::DropLines => "drop-lines",
        CurveKind::Spline => "spline",
        CurveKind::HorizontalSteps => "horizontal-steps",
        CurveKind::StackedHistogram => "stacked-histogram",
        CurveKind::Polar => "polar",
    }
}

fn load_sciprj_str(text: &str) -> Result<Project, String> {
    load_sciprj_root(parse_xml(text)?)
}

fn load_sciprj_root(root: XmlElem) -> Result<Project, String> {
    let name = root.attr("name").unwrap_or("SciDAVis");
    let mut import = Import {
        project: Project::new(name),
        pending: Vec::new(),
        moves: Vec::new(),
        found: false,
    };
    let tag = root.name.as_str();
    let known = matches!(tag, "sciprj" | "scidavis" | "qtiplot" | "project" | "windows" | "folder");
    if matches!(tag, "table" | "matrix" | "note" | "graph" | "multilayer" | "plot" | "folder") {
        dispatch(&root, &mut import, None);
    } else {
        walk(&root, &mut import, None);
    }
    bind_curves(&mut import);
    apply_moves(&mut import);
    if !import.found && !known {
        return Err(crate::tr!(
            "SciDAVis 项目里没有读到表格、矩阵、图形、备注或文件夹。样式和三维图不在这个交换范围内。",
            "The SciDAVis project has no table, matrix, graph, note, or folder. Styles and 3D plots are outside this interchange."
        )
        .into());
    }
    import.project.fix_ids();
    import.project.sanitize_folders();
    Ok(import.project)
}

struct PendingCurve {
    graph: GraphId,
    layer: usize,
    table: String,
    x: usize,
    y: usize,
    kind: CurveKind,
    name: String,
}

struct Import {
    project: Project,
    pending: Vec<PendingCurve>,
    moves: Vec<(String, FolderId)>,
    found: bool,
}

fn walk(elem: &XmlElem, import: &mut Import, folder: Option<FolderId>) {
    for child in &elem.children {
        if let XmlNode::Elem(child) = child {
            dispatch(child, import, folder);
        }
    }
}

fn dispatch(elem: &XmlElem, import: &mut Import, folder: Option<FolderId>) {
    match elem.name.as_str() {
        "folder" => {
            let title = label_of(elem).unwrap_or_else(|| crate::locale::t("文件夹").to_string());
            let id = import.project.new_folder(&title, folder).ok();
            import.found = true;
            walk(elem, import, id.or(folder));
        }
        "table" | "worksheet" => import_table(elem, import, folder),
        "matrix" => import_matrix(elem, import, folder),
        "note" => import_note(elem, import, folder),
        "graph" | "multilayer" | "plot" => import_graph(elem, import, folder),
        "log" | "results" => {
            let text = element_body(elem);
            for line in text.lines() {
                if !line.trim().is_empty() {
                    import.project.log_line(line.trim());
                    import.found = true;
                }
            }
        }
        "window" => {
            if !has_elements(elem) {
                let name = elem.direct_text().trim().to_string();
                if let (Some(folder), false) = (folder, name.is_empty()) {
                    import.moves.push((name, folder));
                    import.found = true;
                }
            } else {
                walk(elem, import, folder);
            }
        }
        "windows" | "subfolders" | "folderwindows" | "project" | "scidavis" | "qtiplot" | "sciprj" => {
            walk(elem, import, folder);
        }
        "name" | "caption" | "label" | "creation" | "creation_time" | "comment" | "scripting-lang" => {}
        _ => {
            if contains_known(elem) {
                walk(elem, import, folder);
            }
        }
    }
}

fn import_table(elem: &XmlElem, import: &mut Import, folder: Option<FolderId>) {
    let title = label_of(elem).unwrap_or_else(|| "Table".to_string());
    let id = import.project.new_table(&title);
    let Some(table) = import.project.table_mut(id) else {
        return;
    };
    table.folder = folder;
    table.columns.clear();
    for child in named(elem, &["column", "col"]) {
        let column_name = label_of(child).unwrap_or_else(|| format!("{}", table.columns.len() + 1));
        let mode = mode_of(&child.text_of(&["designation", "type", "plot_designation"]).unwrap_or_else(|| "Y".into()));
        let index = table.add_column(column_name, mode);
        if let Some(formula) = child.child_text("formula").or_else(|| child.attr("formula").map(str::to_string)) {
            table.columns[index].formula = formula.trim().to_string();
        }
        for (row, text) in column_cells(child).into_iter().enumerate() {
            table.columns[index].set_text(row, &text);
        }
    }
    if table.columns.is_empty() {
        table.add_column("A", ColumnMode::Y);
    }
    import.found = true;
}

fn import_matrix(elem: &XmlElem, import: &mut Import, folder: Option<FolderId>) {
    let title = label_of(elem).unwrap_or_else(|| "Matrix".to_string());
    let mut rows = Vec::new();
    for child in named(elem, &["row"]) {
        let mut cells = Vec::new();
        for part in child.direct_text().split_whitespace() {
            if part == "--" || part == "-" {
                cells.push(None);
            } else {
                cells.push(part.parse::<f64>().ok().filter(|value| value.is_finite()));
            }
        }
        if !cells.is_empty() {
            rows.push(cells);
        }
    }
    let cols = rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
    let height = rows.len().max(1);
    let id = import.project.new_matrix(&title, height, cols);
    if let Some(matrix) = import.project.matrix_mut(id) {
        matrix.folder = folder;
        for (row, line) in rows.iter().enumerate() {
            for (col, value) in line.iter().enumerate() {
                matrix.set(row, col, *value);
            }
        }
    }
    import.found = true;
}

fn import_note(elem: &XmlElem, import: &mut Import, folder: Option<FolderId>) {
    let title = label_of(elem).unwrap_or_else(|| "Note".to_string());
    let id = import.project.new_note(&title);
    if let Some(note) = import.project.note_mut(id) {
        note.folder = folder;
        note.text = element_body(elem);
    }
    import.found = true;
}

fn import_graph(elem: &XmlElem, import: &mut Import, folder: Option<FolderId>) {
    let title = label_of(elem).unwrap_or_else(|| "Graph".to_string());
    let id = import.project.new_graph(&title);
    if let Some(graph) = import.project.graph_mut(id) {
        graph.folder = folder;
    }
    let layers = named(elem, &["layer"]);
    let layers: Vec<&XmlElem> = if layers.is_empty() { vec![elem] } else { layers };
    for (index, layer) in layers.iter().enumerate() {
        if index > 0 {
            if let Some(graph) = import.project.graph_mut(id) {
                graph.add_layer();
            }
        }
        if let Some(graph) = import.project.graph_mut(id) {
            if let Some(slot) = graph.layers.get_mut(index) {
                if let Some(title) = layer.attr("title").map(str::to_string).or_else(|| layer.child_text("title")) {
                    let title = title.trim();
                    if !title.is_empty() {
                        slot.title = title.to_string();
                    }
                }
                if let Some(label) = first_text(layer, &["xlabel", "x_axis"]) {
                    slot.x_axis.label = label;
                }
                if let Some(label) = first_text(layer, &["ylabel", "y_axis"]) {
                    slot.y_axis.label = label;
                }
            }
        }
        for curve in named(layer, &["curve"]) {
            push_curve(import, id, index, curve);
        }
    }
    import.found = true;
}

fn push_curve(import: &mut Import, graph: GraphId, layer: usize, curve: &XmlElem) {
    let name = label_of(curve).unwrap_or_else(|| crate::locale::t("曲线").to_string());
    let kind = kind_of(&curve.text_of(&["kind"]).unwrap_or_else(|| "line".into()));
    let expr = curve
        .attr("expr")
        .map(str::to_string)
        .or_else(|| curve.child_text("expr"))
        .unwrap_or_default();
    if kind == CurveKind::Function || (!expr.trim().is_empty() && curve.attr("table").is_none() && curve.child_text("table").is_none()) {
        let x_min = parse_f64(curve.attr("xmin").unwrap_or("0"), 0.0);
        let x_max = parse_f64(curve.attr("xmax").unwrap_or("1"), 1.0);
        let samples = parse_usize(curve.attr("samples").unwrap_or("200"), 200).max(2);
        if let Some(graph) = import.project.graph_mut(graph) {
            if let Some(slot) = graph.layers.get_mut(layer) {
                slot.curves.push(Curve {
                    name,
                    kind: CurveKind::Function,
                    source: DataSource::Function {
                        expr,
                        x_min,
                        x_max,
                        samples,
                    },
                    ..Curve::default()
                });
            }
        }
        return;
    }
    let table = curve
        .attr("table")
        .map(str::to_string)
        .or_else(|| curve.child_text("table"))
        .unwrap_or_default();
    if table.trim().is_empty() {
        return;
    }
    import.pending.push(PendingCurve {
        graph,
        layer,
        table: table.trim().to_string(),
        x: parse_usize(&curve.text_of(&["x"]).unwrap_or_else(|| "0".into()), 0),
        y: parse_usize(&curve.text_of(&["y"]).unwrap_or_else(|| "1".into()), 1),
        kind,
        name,
    });
}

fn bind_curves(import: &mut Import) {
    let pending = std::mem::take(&mut import.pending);
    for curve in pending {
        let Some(table) = import.project.tables.iter().find(|table| table.name == curve.table).map(|table| table.id) else {
            continue;
        };
        let Some(graph) = import.project.graph_mut(curve.graph) else {
            continue;
        };
        let Some(layer) = graph.layers.get_mut(curve.layer) else {
            continue;
        };
        layer.curves.push(Curve {
            name: curve.name,
            kind: curve.kind,
            source: DataSource::Columns {
                table,
                x: curve.x,
                y: curve.y,
                x_err: None,
                y_err: None,
                dx: None,
                dy: None,
            },
            ..Curve::default()
        });
    }
}

fn apply_moves(import: &mut Import) {
    for (name, folder) in import.moves.clone() {
        for table in &mut import.project.tables {
            if table.name == name {
                table.folder = Some(folder);
            }
        }
        for matrix in &mut import.project.matrices {
            if matrix.name == name {
                matrix.folder = Some(folder);
            }
        }
        for graph in &mut import.project.graphs {
            if graph.name == name {
                graph.folder = Some(folder);
            }
        }
        for note in &mut import.project.notes {
            if note.name == name {
                note.folder = Some(folder);
            }
        }
    }
}

fn column_cells(column: &XmlElem) -> Vec<String> {
    let mut cells = Vec::new();
    let mut any = false;
    for node in &column.children {
        if let XmlNode::Elem(cell) = node {
            if matches!(cell.name.as_str(), "row" | "cell" | "value") {
                any = true;
                cells.push(cell.direct_text().trim().to_string());
            }
        }
    }
    if any {
        return cells;
    }
    column
        .direct_text()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn label_of(elem: &XmlElem) -> Option<String> {
    for key in ["name", "caption", "label"] {
        if let Some(value) = elem.attr(key) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    for key in ["name", "caption", "label"] {
        if let Some(value) = elem.child_text(key) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn element_body(elem: &XmlElem) -> String {
    for key in ["content", "text", "body"] {
        if let Some(text) = elem.child_text(key) {
            if !text.trim().is_empty() {
                return text.trim().to_string();
            }
        }
    }
    elem.direct_text().trim().to_string()
}

fn first_text(elem: &XmlElem, names: &[&str]) -> Option<String> {
    for name in names {
        if let Some(text) = elem.child_text(name) {
            let text = text.trim();
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }
    None
}

fn named<'a>(elem: &'a XmlElem, names: &[&str]) -> Vec<&'a XmlElem> {
    elem.children
        .iter()
        .filter_map(|node| match node {
            XmlNode::Elem(child) if names.iter().any(|name| child.name == *name) => Some(child),
            _ => None,
        })
        .collect()
}

fn has_elements(elem: &XmlElem) -> bool {
    elem.children.iter().any(|node| matches!(node, XmlNode::Elem(_)))
}

fn contains_known(elem: &XmlElem) -> bool {
    const KNOWN: &[&str] = &[
        "table", "worksheet", "matrix", "note", "graph", "multilayer", "plot", "folder", "log", "results", "window", "windows",
    ];
    if KNOWN.contains(&elem.name.as_str()) {
        return true;
    }
    elem.children.iter().any(|node| match node {
        XmlNode::Elem(child) => contains_known(child),
        XmlNode::Text(_) => false,
    })
}

fn mode_of(text: &str) -> ColumnMode {
    match text.trim().to_ascii_lowercase().as_str() {
        "0" | "none" | "无" => ColumnMode::None,
        "1" | "x" => ColumnMode::X,
        "2" | "y" => ColumnMode::Y,
        "3" | "z" => ColumnMode::Z,
        "4" | "xerr" | "xerror" => ColumnMode::XErr,
        "5" | "yerr" | "yerror" => ColumnMode::YErr,
        "6" | "label" | "标签" => ColumnMode::Label,
        _ => ColumnMode::Y,
    }
}

fn kind_of(text: &str) -> CurveKind {
    match text.trim().to_ascii_lowercase().as_str() {
        "scatter" | "散点" => CurveKind::Scatter,
        "line+symbol" | "linesymbol" | "line + symbol" | "点线" => CurveKind::LineSymbol,
        "steps" | "阶梯" => CurveKind::Steps,
        "bars" | "bar" | "柱状" => CurveKind::Bars,
        "horizontal-bars" | "horizontalbars" => CurveKind::HorizontalBars,
        "histogram" | "直方图" => CurveKind::Histogram,
        "area" | "面积" => CurveKind::Area,
        "error-bars" | "errorbars" => CurveKind::ErrorBars,
        "box" | "boxplot" | "箱线" => CurveKind::BoxPlot,
        "pie" | "饼图" => CurveKind::Pie,
        "vector" => CurveKind::Vector,
        "contour" | "等高线" => CurveKind::Contour,
        "heatmap" | "热力图" => CurveKind::Heatmap,
        "function" | "函数" => CurveKind::Function,
        "parametric" => CurveKind::Parametric,
        "drop-lines" => CurveKind::DropLines,
        "spline" | "样条" => CurveKind::Spline,
        "horizontal-steps" => CurveKind::HorizontalSteps,
        "stacked-histogram" => CurveKind::StackedHistogram,
        "polar" => CurveKind::Polar,
        _ => CurveKind::Line,
    }
}

fn parse_usize(text: &str, default: usize) -> usize {
    text.trim().parse().unwrap_or(default)
}

fn parse_f64(text: &str, default: f64) -> f64 {
    text.trim().parse().unwrap_or(default)
}

fn escape(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

#[derive(Clone, Debug)]
pub(crate) struct XmlElem {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<XmlNode>,
}

#[derive(Clone, Debug)]
pub(crate) enum XmlNode {
    Elem(XmlElem),
    Text(String),
}

impl XmlElem {
    pub(crate) fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub(crate) fn child_text(&self, name: &str) -> Option<String> {
        self.children.iter().find_map(|node| match node {
            XmlNode::Elem(child) if child.name.eq_ignore_ascii_case(name) => Some(child.direct_text()),
            _ => None,
        })
    }

    pub(crate) fn direct_text(&self) -> String {
        let mut out = String::new();
        for node in &self.children {
            if let XmlNode::Text(text) = node {
                out.push_str(text);
            }
        }
        out
    }

    pub(crate) fn text_of(&self, names: &[&str]) -> Option<String> {
        for name in names {
            if let Some(value) = self.attr(name) {
                if !value.trim().is_empty() {
                    return Some(value.to_string());
                }
            }
        }
        for name in names {
            if let Some(value) = self.child_text(name) {
                if !value.trim().is_empty() {
                    return Some(value);
                }
            }
        }
        None
    }
}

pub(crate) fn parse_xml(text: &str) -> Result<XmlElem, String> {
    let text = text.trim_start_matches('\u{feff}');
    let mut parser = Parser { text, index: 0 };
    parser.skip_misc()?;
    if parser.eof() {
        return Err(crate::tr!("XML 是空的", "The XML is empty").into());
    }
    parser.parse_element()
}

struct Parser<'a> {
    text: &'a str,
    index: usize,
}

impl<'a> Parser<'a> {
    fn eof(&self) -> bool {
        self.index >= self.text.len()
    }

    fn rest(&self) -> &'a str {
        &self.text[self.index..]
    }

    fn starts_with(&self, pat: &str) -> bool {
        self.rest().starts_with(pat)
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.rest().chars().next()?;
        self.index += ch.len_utf8();
        Some(ch)
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.rest().chars().next() {
            if !ch.is_whitespace() {
                break;
            }
            self.bump();
        }
    }

    fn skip_misc(&mut self) -> Result<(), String> {
        loop {
            self.skip_ws();
            if self.starts_with("<?") {
                self.skip_until("?>")?;
            } else if self.starts_with("<!--") {
                self.skip_until("-->")?;
            } else if self.starts_with("<!") {
                self.skip_until(">")?;
            } else {
                break;
            }
        }
        Ok(())
    }

    fn skip_until(&mut self, pat: &str) -> Result<(), String> {
        if let Some(pos) = self.rest().find(pat) {
            self.index += pos + pat.len();
            Ok(())
        } else {
            Err(crate::trf!("XML 没有结束 {pat}", "XML is missing the closing {pat}", pat = pat))
        }
    }

    fn parse_element(&mut self) -> Result<XmlElem, String> {
        if !self.starts_with("<") {
            return Err(crate::tr!("XML 标签不完整", "The XML tag is incomplete").into());
        }
        self.bump();
        let name = local_name(&self.read_name());
        if name.is_empty() {
            return Err(crate::tr!("XML 标签没有名字", "The XML tag has no name").into());
        }
        let attrs = self.parse_attrs()?;
        self.skip_ws();
        if self.starts_with("/>") {
            self.index += 2;
            return Ok(XmlElem { name, attrs, children: Vec::new() });
        }
        if self.bump() != Some('>') {
            return Err(crate::trf!("标签 {name} 没有结束", "Tag {name} is not closed", name = name));
        }
        let children = self.parse_children(&name)?;
        Ok(XmlElem { name, attrs, children })
    }

    fn parse_children(&mut self, end_tag: &str) -> Result<Vec<XmlNode>, String> {
        let mut nodes = Vec::new();
        loop {
            if self.eof() {
                return Err(crate::trf!("缺少 </{end_tag}>", "Missing </{end_tag}>", end_tag = end_tag));
            }
            if self.starts_with("</") {
                self.index += 2;
                self.skip_ws();
                let close = local_name(&self.read_name());
                self.skip_ws();
                if self.bump() != Some('>') {
                    return Err(crate::tr!("结束标签不完整", "The closing tag is incomplete").into());
                }
                if close != end_tag {
                    return Err(crate::trf!(
                        "结束标签是 {close}，期望 {end_tag}",
                        "Closing tag is {close}, expected {end_tag}",
                        close = close,
                        end_tag = end_tag
                    ));
                }
                return Ok(nodes);
            }
            if self.starts_with("<!--") {
                self.skip_until("-->")?;
                continue;
            }
            if self.starts_with("<![CDATA[") {
                self.index += 9;
                let end = self
                    .rest()
                    .find("]]>")
                    .ok_or_else(|| crate::tr!("CDATA 没有结束", "CDATA is not closed").to_string())?;
                nodes.push(XmlNode::Text(self.rest()[..end].to_string()));
                self.index += end + 3;
                continue;
            }
            if self.starts_with("<?") {
                self.skip_until("?>")?;
                continue;
            }
            if self.starts_with("<!") {
                self.skip_until(">")?;
                continue;
            }
            if self.starts_with("<") {
                nodes.push(XmlNode::Elem(self.parse_element()?));
                continue;
            }
            let start = self.index;
            while !self.eof() && !self.starts_with("<") {
                self.bump();
            }
            let decoded = decode_entities(&self.text[start..self.index]);
            if !decoded.is_empty() {
                nodes.push(XmlNode::Text(decoded));
            }
        }
    }

    fn parse_attrs(&mut self) -> Result<Vec<(String, String)>, String> {
        let mut attrs = Vec::new();
        loop {
            self.skip_ws();
            if self.starts_with("/>") || self.starts_with(">") || self.eof() {
                break;
            }
            let name = self.read_name();
            if name.is_empty() {
                return Err(crate::tr!("XML 属性没有名字", "The XML attribute has no name").into());
            }
            self.skip_ws();
            if self.bump() != Some('=') {
                return Err(crate::trf!("属性 {name} 缺少等号", "Attribute {name} has no equals sign", name = name));
            }
            self.skip_ws();
            let value = self.read_attr_value()?;
            attrs.push((local_name(&name), decode_entities(&value)));
        }
        Ok(attrs)
    }

    fn read_name(&mut self) -> String {
        let mut name = String::new();
        while let Some(ch) = self.rest().chars().next() {
            if ch.is_whitespace() || matches!(ch, '=' | '/' | '>' | '<') {
                break;
            }
            name.push(ch);
            self.bump();
        }
        name
    }

    fn read_attr_value(&mut self) -> Result<String, String> {
        let quote = self.rest().chars().next();
        if matches!(quote, Some('"') | Some('\'')) {
            let quote = self.bump().unwrap();
            let start = self.index;
            while let Some(ch) = self.rest().chars().next() {
                if ch == quote {
                    let value = self.text[start..self.index].to_string();
                    self.bump();
                    return Ok(value);
                }
                self.bump();
            }
            return Err(crate::tr!("属性引号没有结束", "The attribute quote is not closed").into());
        }
        let start = self.index;
        while let Some(ch) = self.rest().chars().next() {
            if ch.is_whitespace() || matches!(ch, '/' | '>') {
                break;
            }
            self.bump();
        }
        Ok(self.text[start..self.index].to_string())
    }
}

fn local_name(name: &str) -> String {
    name.rsplit(':').next().unwrap_or(name).to_ascii_lowercase()
}

fn decode_entities(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at + 1..];
        let Some(end) = rest.find(';') else {
            out.push('&');
            break;
        };
        let token = &rest[..end];
        rest = &rest[end + 1..];
        match token {
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "amp" => out.push('&'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            _ if token.len() > 2 && (token.starts_with("#x") || token.starts_with("#X")) => {
                if let Some(ch) = u32::from_str_radix(&token[2..], 16).ok().and_then(char::from_u32) {
                    out.push(ch);
                }
            }
            _ if token.len() > 1 && token.starts_with('#') => {
                if let Some(ch) = token[1..].parse::<u32>().ok().and_then(char::from_u32) {
                    out.push(ch);
                }
            }
            _ => {
                out.push('&');
                out.push_str(token);
                out.push(';');
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Curve, CurveKind, DataSource};
    use crate::table::ColumnMode;

    #[test]
    fn sciprj_roundtrip_and_compressed_save_and_foreign_windows() {
        let mut project = Project::new("实验");
        let table_id = project.new_table("数据");
        {
            let table = project.table_mut(table_id).unwrap();
            table.columns.clear();
            table.add_numeric_column("X", ColumnMode::X, &[1.0, 2.0]);
            let y = table.add_column("Y", ColumnMode::Y);
            table.columns[y].formula = "col(\"X\")+1".into();
            table.columns[y].set_text(0, "2");
            table.columns[y].set_text(1, "3");
        }
        let matrix_id = project.new_matrix("波", 2, 2);
        project.matrix_mut(matrix_id).unwrap().set(1, 1, Some(4.0));
        let folder = project.new_folder("记录", None).unwrap();
        project.active_folder = Some(folder);
        let note_id = project.new_note("说明");
        project.note_mut(note_id).unwrap().text = "你好 <科学>".into();
        project.active_folder = None;
        let graph_id = project.new_graph("图一");
        {
            let graph = project.graph_mut(graph_id).unwrap();
            graph.layers[0].title = "层".into();
            graph.layers[0].curves.push(Curve {
                name: "线".into(),
                kind: CurveKind::Line,
                source: DataSource::Columns {
                    table: table_id,
                    x: 0,
                    y: 1,
                    x_err: None,
                    y_err: None,
                    dx: None,
                    dy: None,
                },
                ..Curve::default()
            });
        }
        project.log_line("已保存");

        let xml = to_sciprj(&project);
        let back = load_project_bytes("实验.sciprj", xml.as_bytes()).unwrap();
        assert_eq!(back.name, "实验");
        assert_eq!(back.tables[0].columns[1].formula, "col(\"X\")+1");
        assert_eq!(back.tables[0].columns[1].numbers[1], Some(3.0));
        assert_eq!(back.matrices[0].get(1, 1), Some(4.0));
        assert_eq!(back.notes[0].text, "你好 <科学>");
        let note_folder = back.notes[0].folder.unwrap();
        assert_eq!(back.folder(note_folder).unwrap().name, "记录");
        let DataSource::Columns { table, x, y, .. } = back.graphs[0].layers[0].curves[0].source else {
            panic!("curve");
        };
        assert_eq!(table, back.tables[0].id);
        assert_eq!((x, y), (0, 1));
        assert_eq!(back.graphs[0].layers[0].title, "层");
        assert_eq!(back.log, vec!["已保存".to_string()]);

        let gz = save_project_bytes(&project, Some(ProjectFormat::SciprjGz)).unwrap();
        assert_eq!(&gz[..2], &[0x1f, 0x8b]);
        let from_gz = load_project_bytes("实验.sciprj.gz", &gz).unwrap();
        assert_eq!(from_gz.notes[0].text, "你好 <科学>");
        let splot_gz = save_project_bytes(&project, Some(ProjectFormat::SplotGz)).unwrap();
        let from_splot = load_project_bytes("实验.splot.gz", &splot_gz).unwrap();
        assert_eq!(from_splot.tables[0].columns[0].numbers[0], Some(1.0));
        assert!(save_project_bytes(&project, Some(ProjectFormat::Origin)).is_err());

        let dir = std::env::temp_dir().join(format!("sciplot-exchange-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("实验.splot.gz");
        save_project_path(&project, &path).unwrap();
        let loaded = load_project_bytes("实验.splot.gz", &std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(loaded.name, "实验");
        std::fs::remove_dir_all(&dir).ok();

        let foreign = r#"<?xml version="1.0"?>
<sciDAVis version="2.9">
  <windows>
    <table name="T">
      <column>
        <name>A</name>
        <plot_designation>1</plot_designation>
        <row>1</row>
        <row>2</row>
      </column>
      <column>
        <name>B</name>
        <plot_designation>2</plot_designation>
        <row>10</row>
        <row>20</row>
      </column>
    </table>
    <note caption="N"><content>hello</content></note>
    <matrix caption="M"><row>1 2</row><row>3 --</row></matrix>
    <graph name="G">
      <layer>
        <title>Layer</title>
        <curve>
          <table>T</table>
          <x>0</x>
          <y>1</y>
          <kind>scatter</kind>
          <name>pts</name>
        </curve>
      </layer>
    </graph>
  </windows>
  <folder name="Root">
    <window>T</window>
    <folder name="Sub">
      <window>N</window>
    </folder>
  </folder>
  <log>opened</log>
</sciDAVis>"#;
        let foreign = load_project_bytes("sample.sciprj", foreign.as_bytes()).unwrap();
        assert_eq!(foreign.tables[0].columns[0].mode, ColumnMode::X);
        assert_eq!(foreign.tables[0].columns[1].numbers[1], Some(20.0));
        assert_eq!(foreign.notes[0].text, "hello");
        assert_eq!(foreign.matrices[0].get(1, 1), None);
        assert_eq!(foreign.matrices[0].get(1, 0), Some(3.0));
        let DataSource::Columns { table, .. } = foreign.graphs[0].layers[0].curves[0].source else {
            panic!("foreign curve");
        };
        assert_eq!(table, foreign.tables[0].id);
        assert_eq!(foreign.graphs[0].layers[0].curves[0].kind, CurveKind::Scatter);
        let root = foreign.tables[0].folder.unwrap();
        let sub = foreign.notes[0].folder.unwrap();
        assert_eq!(foreign.folder(root).unwrap().name, "Root");
        assert_eq!(foreign.folder(sub).unwrap().parent, Some(root));
        assert_eq!(foreign.log, vec!["opened".to_string()]);
        let plain = load_project_bytes(
            "old.splot",
            br#"{"format":1,"project":{"name":"old","tables":[],"matrices":[],"graphs":[],"notes":[],"log":[],"next_id":1}}"#,
        )
        .unwrap();
        assert_eq!(plain.name, "old");
        assert!(plain.folders.is_empty());
    }
}
