//! 官方 SciDAVis / QtiPlot 项目文件。
//!
//! 文件以 `SciDAVis x.y.z project file`（或 QtiPlot 的同一句式）开头，后面是
//! `<scripting-lang>`、`<windows>`，然后逐个窗口。表格和矩阵不是一整份 XML：
//! 先写 `<table>` 或 `<matrix>`，下一行是内层 XML 的字节数，再是那段 XML，
//! 最后才是 geometry 和外层结束标签。备注、文件夹、多层图和曲面图是文本块。
//!
//! 读入的内容：表格（列名、角色、数值、列公式）、矩阵、备注、文件夹、
//! 曲线和它所绑的轴、坐标范围、图层标题，以及曲面图所引用的列。
//! 官方格式没有单元格公式。曲线的笔、符号和画布百分比坐标的图例不逐项还原。
//! 自由 XYZ 点不是规则网格，曲面图按 Z（没有 Z 时用 Y）画成表格三维图，
//! 不重建三角网格。打开这类文件后保存应走 `.splot`，不要写回原文件。

use crate::exchange::{XmlElem, XmlNode};
use crate::graph::{
    AxisScale, Curve, CurveAxes, CurveKind, CurveStyle, DataSource, SurfaceMode, SurfaceSource, SurfaceSpec, TablePlot3D,
    XAxisSide, YAxisSide,
};
use crate::project::Project;
use crate::table::{Column, ColumnMode, ColumnType};
use crate::{GraphId, TableId};

pub fn is_official(text: &str) -> bool {
    let line = text.lines().next().unwrap_or("").trim_start_matches('\u{feff}').trim();
    let lower = line.to_ascii_lowercase();
    (lower.contains("scidavis") || lower.contains("qtiplot")) && lower.contains("project file")
}

pub fn load_official(text: &str) -> Result<Project, String> {
    if !is_official(text) {
        return Err(crate::tr!(
            "这不是 SciDAVis 或 QtiPlot 的项目文件。",
            "This is not a SciDAVis or QtiPlot project file."
        )
        .into());
    }
    let bytes = text.as_bytes();
    let mut project = Project::new(header_name(text));
    let mut i = 0usize;
    let mut folders: Vec<Option<crate::FolderId>> = Vec::new();
    let mut curves = Vec::new();
    let mut surfaces = Vec::new();
    while i < bytes.len() {
        if bytes[i] == b'\n' || bytes[i] == b'\r' {
            i += 1;
            continue;
        }
        let (line, next) = read_line(bytes, i);
        let trimmed = line.trim();
        if trimmed == "<table>" || trimmed == "<matrix>" {
            let (xml, after) = take_sized(bytes, next)?;
            let xml = std::str::from_utf8(xml).map_err(|_| {
                crate::tr!("官方项目里的 XML 不是 UTF-8。", "The XML in the official project is not UTF-8.").to_string()
            })?;
            let root = crate::exchange::parse_xml(xml)?;
            if trimmed == "<table>" {
                read_table(&mut project, &root)?;
            } else {
                read_matrix(&mut project, &root)?;
            }
            i = after;
        } else if trimmed == "<note>" {
            let (block, after) = take_until(bytes, next, b"</note>");
            read_note(&mut project, std::str::from_utf8(block).unwrap_or(""));
            i = after;
        } else if trimmed == "<multiLayer>" {
            let (block, after) = take_until(bytes, next, b"</multiLayer>");
            read_multilayer(&mut project, std::str::from_utf8(block).unwrap_or(""), &mut curves);
            i = after;
        } else if trimmed == "<SurfacePlot>" {
            let (block, after) = take_until(bytes, next, b"</SurfacePlot>");
            read_surface(&mut project, std::str::from_utf8(block).unwrap_or(""), &mut surfaces);
            i = after;
        } else if trimmed == "<log>" {
            let (block, after) = take_until(bytes, next, b"</log>");
            for line in std::str::from_utf8(block).unwrap_or("").lines() {
                let line = line.trim();
                if !line.is_empty() {
                    project.log_line(line);
                }
            }
            i = after;
        } else if trimmed.starts_with("<folder>") {
            let name = field(trimmed, 1);
            if !name.is_empty() {
                let parent = project.active_folder;
                if let Ok(id) = project.new_folder(name, parent) {
                    folders.push(parent);
                    project.active_folder = Some(id);
                }
            }
            i = next;
        } else if trimmed.starts_with("</folder>") {
            project.active_folder = folders.pop().flatten();
            i = next;
        } else {
            i = next;
        }
    }
    bind_curves(&mut project, curves);
    bind_surfaces(&mut project, surfaces);
    project.active_folder = None;
    project.fix_ids();
    project.sanitize_folders();
    if project.tables.is_empty() && project.matrices.is_empty() && project.graphs.is_empty() && project.notes.is_empty() {
        return Err(crate::tr!(
            "SciDAVis 项目里没有读到表格、矩阵、图形或备注。",
            "The SciDAVis project has no tables, matrices, graphs, or notes."
        )
        .into());
    }
    Ok(project)
}

fn header_name(text: &str) -> String {
    let line = text.lines().next().unwrap_or("").trim();
    line.trim_end_matches("project file").trim().trim_end_matches("Project File").trim().to_string()
}

fn read_line(bytes: &[u8], start: usize) -> (String, usize) {
    let mut end = start;
    while end < bytes.len() && bytes[end] != b'\n' {
        end += 1;
    }
    let next = if end < bytes.len() { end + 1 } else { end };
    let text = String::from_utf8_lossy(&bytes[start..end]).trim_end_matches('\r').to_string();
    (text, next)
}

fn take_sized(bytes: &[u8], start: usize) -> Result<(&[u8], usize), String> {
    let (len_line, after) = read_line(bytes, start);
    let n: usize = len_line.trim().parse().map_err(|_| {
        crate::tr!(
            "官方项目的窗口长度不是整数。",
            "The official project window length is not an integer."
        )
        .to_string()
    })?;
    if after + n > bytes.len() {
        return Err(crate::tr!(
            "官方项目的窗口长度超出了文件。",
            "The official project window length runs past the end of the file."
        )
        .into());
    }
    Ok((&bytes[after..after + n], after + n))
}

fn take_until<'a>(bytes: &'a [u8], start: usize, marker: &[u8]) -> (&'a [u8], usize) {
    if let Some(pos) = bytes[start..].windows(marker.len()).position(|window| window == marker) {
        (&bytes[start..start + pos], start + pos + marker.len())
    } else {
        (&bytes[start..], bytes.len())
    }
}

fn field(line: &str, index: usize) -> &str {
    line.split('\t').nth(index).unwrap_or("").trim()
}

fn elems<'a>(parent: &'a XmlElem, name: &'a str) -> impl Iterator<Item = &'a XmlElem> {
    parent.children.iter().filter_map(move |node| match node {
        XmlNode::Elem(child) if child.name.eq_ignore_ascii_case(name) => Some(child),
        _ => None,
    })
}

fn read_table(project: &mut Project, root: &XmlElem) -> Result<(), String> {
    let name = root.attr("name").unwrap_or("Table");
    let id = project.new_table(name);
    let Some(table) = project.table_mut(id) else {
        return Ok(());
    };
    table.columns.clear();
    let mut formulas = Vec::new();
    for column in elems(root, "column") {
        let col_name = column.attr("name").unwrap_or("").trim();
        let col_name = if col_name.is_empty() {
            crate::column_letter(table.columns.len())
        } else {
            col_name.to_string()
        };
        let mut built = Column::new(col_name, designation(column.attr("plot_designation").unwrap_or("Y")));
        if column.attr("mode").is_some_and(|mode| mode.eq_ignore_ascii_case("text"))
            || column.attr("type").is_some_and(|ty| ty.eq_ignore_ascii_case("text"))
        {
            built.ty = ColumnType::Text;
        }
        built.comment = column.child_text("comment").unwrap_or_default();
        for row in elems(column, "row") {
            let index = row.attr("index").and_then(|text| text.parse().ok()).unwrap_or(built.numbers.len());
            let invalid = row.attr("invalid").is_some_and(|flag| flag.eq_ignore_ascii_case("yes"));
            let text = row.direct_text();
            put_cell(&mut built, index, invalid, text.trim());
        }
        if let Some(formula) = elems(column, "formula").next() {
            let cleaned = clean_formula(&formula.direct_text());
            if !cleaned.is_empty() {
                let start = formula.attr("start_row").and_then(|text| text.parse().ok()).unwrap_or(0usize);
                let end = formula
                    .attr("end_row")
                    .and_then(|text| text.parse::<usize>().ok())
                    .unwrap_or(built.len().saturating_sub(1));
                formulas.push((table.columns.len(), cleaned, start, end));
            }
        }
        table.columns.push(built);
    }
    for row in elems(root, "row") {
        let index = row.attr("index").and_then(|text| text.parse().ok()).unwrap_or(0usize);
        for (col_index, cell) in elems(row, "cell").enumerate() {
            if col_index >= table.columns.len() {
                break;
            }
            let invalid = cell.attr("invalid").is_some_and(|flag| flag.eq_ignore_ascii_case("yes"))
                || row.attr("invalid").is_some_and(|flag| flag.eq_ignore_ascii_case("yes"));
            put_cell(&mut table.columns[col_index], index, invalid, cell.direct_text().trim());
        }
    }
    table.normalize();
    let nrows = table.nrows();
    for (index, formula, start, end) in formulas {
        let Some(column) = table.columns.get_mut(index) else {
            continue;
        };
        let covers_all = start == 0 && end.saturating_add(1) >= nrows.max(1) && end >= nrows.saturating_sub(1);
        if covers_all || nrows == 0 {
            column.formula = formula;
        } else if start <= end {
            let end = end.min(column.len().saturating_sub(1));
            if start <= end {
                column.resize(column.len().max(end + 1));
                for slot in &mut column.cell_formulas[start..=end] {
                    *slot = formula.clone();
                }
            }
        }
    }
    Ok(())
}

fn put_cell(column: &mut Column, index: usize, invalid: bool, text: &str) {
    if column.numbers.len() <= index {
        column.numbers.resize(index + 1, None);
    }
    if invalid || text.is_empty() {
        column.numbers[index] = None;
        return;
    }
    match text.parse::<f64>() {
        Ok(value) if value.is_finite() => column.numbers[index] = Some(value),
        _ => {
            column.ty = ColumnType::Text;
            if column.texts.len() <= index {
                column.texts.resize(index + 1, String::new());
            }
            column.texts[index] = text.to_string();
            column.numbers[index] = None;
        }
    }
}

fn designation(text: &str) -> ColumnMode {
    match text {
        "X" => ColumnMode::X,
        "Z" => ColumnMode::Z,
        "xErr" | "XErr" => ColumnMode::XErr,
        "yErr" | "YErr" => ColumnMode::YErr,
        "label" | "Label" => ColumnMode::Label,
        "none" | "None" => ColumnMode::None,
        _ => ColumnMode::Y,
    }
}

/// muParser 公式里 `%` 起一行注释，引号在 XML 里写成 `&quot;`。
fn clean_formula(raw: &str) -> String {
    let mut out = String::new();
    for line in raw.lines() {
        let line = line.split('%').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(line);
    }
    out
}

fn read_matrix(project: &mut Project, root: &XmlElem) -> Result<(), String> {
    let name = root.attr("name").unwrap_or("Matrix");
    let mut rows = root.attr("rows").and_then(|text| text.parse().ok()).unwrap_or(1usize);
    let mut cols = root.attr("columns").and_then(|text| text.parse().ok()).unwrap_or(1usize);
    let cells: Vec<(usize, usize, Option<f64>)> = elems(root, "cell")
        .map(|cell| {
            let row = cell.attr("row").and_then(|text| text.parse().ok()).unwrap_or(0);
            let col = cell.attr("column").and_then(|text| text.parse().ok()).unwrap_or(0);
            let value = cell.direct_text().trim().parse::<f64>().ok().filter(|value| value.is_finite());
            (row, col, value)
        })
        .collect();
    for (row, col, _) in &cells {
        rows = rows.max(row + 1);
        cols = cols.max(col + 1);
    }
    let id = project.new_matrix(name, rows.max(1), cols.max(1));
    let Some(matrix) = project.matrix_mut(id) else {
        return Ok(());
    };
    if let Some(coords) = elems(root, "coordinates").next() {
        if let Some(value) = coords.attr("x_start").and_then(|text| text.parse().ok()) {
            matrix.x_min = value;
        }
        if let Some(value) = coords.attr("x_end").and_then(|text| text.parse().ok()) {
            matrix.x_max = value;
        }
        if let Some(value) = coords.attr("y_start").and_then(|text| text.parse().ok()) {
            matrix.y_min = value;
        }
        if let Some(value) = coords.attr("y_end").and_then(|text| text.parse().ok()) {
            matrix.y_max = value;
        }
    }
    matrix.formula = root.child_text("formula").unwrap_or_default();
    matrix.comment = root.child_text("comment").unwrap_or_default();
    for (row, col, value) in cells {
        matrix.set(row, col, value);
    }
    Ok(())
}

fn read_note(project: &mut Project, block: &str) {
    let mut name = String::new();
    let mut text = String::new();
    let mut in_content = false;
    for line in block.lines() {
        if !in_content && name.is_empty() && !line.trim().is_empty() && !line.trim().starts_with('<') {
            name = field(line, 0).to_string();
        }
        if line.trim() == "<content>" {
            in_content = true;
            continue;
        }
        if line.trim() == "</content>" {
            in_content = false;
            continue;
        }
        if in_content {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(line);
        }
    }
    if name.is_empty() {
        name = "Note".to_string();
    }
    let id = project.new_note(&name);
    if let Some(note) = project.notes.iter_mut().find(|note| note.id == id) {
        note.text = text.trim_matches('\n').to_string();
    }
}

struct PendingCurve {
    graph: GraphId,
    layer: usize,
    x_name: String,
    y_name: String,
    kind: CurveKind,
    x_side: XAxisSide,
    y_side: YAxisSide,
    visible: bool,
}

struct PendingSurface {
    graph: GraphId,
    column_name: String,
    kind: TablePlot3D,
    mode: SurfaceMode,
    azimuth: f32,
    elevation: f32,
}

fn read_multilayer(project: &mut Project, block: &str, curves: &mut Vec<PendingCurve>) {
    let mut graph_id: Option<GraphId> = None;
    let mut layer_i = 0usize;
    let mut seen_graph = false;
    let mut in_graph = false;
    for line in block.lines() {
        let line = line.trim_end();
        let trimmed = line.trim();
        if trimmed == "<graph>" {
            if graph_id.is_none() {
                graph_id = Some(project.new_graph("Graph"));
            }
            let id = graph_id.expect("graph");
            if let Some(graph) = project.graph_mut(id) {
                if seen_graph {
                    layer_i = graph.add_layer();
                } else {
                    layer_i = 0;
                    seen_graph = true;
                }
            }
            in_graph = true;
            continue;
        }
        if trimmed == "</graph>" {
            in_graph = false;
            continue;
        }
        if graph_id.is_none() && !trimmed.is_empty() && !trimmed.starts_with('<') {
            let name = field(trimmed, 0);
            if !name.is_empty() {
                graph_id = Some(project.new_graph(name));
            }
            continue;
        }
        if !in_graph {
            continue;
        }
        let Some(id) = graph_id else { continue };
        let Some(graph) = project.graph_mut(id) else { continue };
        let Some(layer) = graph.layers.get_mut(layer_i) else { continue };
        if let Some(rest) = trimmed.strip_prefix("PlotTitle\t") {
            layer.title = strip_html(field(rest, 0));
        } else if let Some(rest) = trimmed.strip_prefix("AxesTitles\t") {
            let parts: Vec<&str> = rest.split('\t').collect();
            if let Some(text) = parts.first() {
                layer.x_axis.label = (*text).to_string();
            }
            if let Some(text) = parts.get(1) {
                layer.y_axis.label = (*text).to_string();
            }
            if let Some(text) = parts.get(2) {
                layer.x_top.label = (*text).to_string();
            }
            if let Some(text) = parts.get(3) {
                layer.y_right.label = (*text).to_string();
            }
        } else if let Some(rest) = trimmed.strip_prefix("EnabledAxes\t") {
            let flags: Vec<i32> = rest.split('\t').filter_map(|text| text.trim().parse().ok()).collect();
            if flags.len() >= 4 {
                layer.show_right = flags[1] != 0;
                layer.show_top = flags[3] != 0;
            }
        } else if let Some(rest) = trimmed.strip_prefix("scale\t") {
            apply_scale(layer, rest);
        } else if let Some(rest) = trimmed.strip_prefix("curve\t") {
            if let Some(pending) = parse_curve(id, layer_i, rest) {
                curves.push(pending);
            }
        }
    }
}

fn apply_scale(layer: &mut crate::graph::Layer, rest: &str) {
    let parts: Vec<&str> = rest.split('\t').map(str::trim).filter(|text| !text.is_empty()).collect();
    if parts.len() < 3 {
        return;
    }
    let Ok(axis_id) = parts[0].parse::<i32>() else { return };
    let Ok(min) = parts[1].parse::<f64>() else { return };
    let Ok(max) = parts[2].parse::<f64>() else { return };
    let axis = match axis_id {
        0 => &mut layer.y_axis,
        1 => &mut layer.y_right,
        2 => &mut layer.x_axis,
        3 => &mut layer.x_top,
        _ => return,
    };
    axis.min = Some(min);
    axis.max = Some(max);
    if let Some(step) = parts.get(3).and_then(|text| text.parse::<f64>().ok()) {
        axis.step = (step > 0.0).then_some(step);
    }
    if let Some(minor) = parts.get(5).and_then(|text| text.parse::<u8>().ok()) {
        if minor > 0 {
            axis.minor_count = Some(minor);
        }
    }
    if parts.get(6).is_some_and(|text| *text == "1") {
        axis.scale = AxisScale::Log10;
    }
    if parts.get(7).is_some_and(|text| *text == "1") {
        axis.inverted = true;
    }
}

fn parse_curve(graph: GraphId, layer: usize, rest: &str) -> Option<PendingCurve> {
    let parts: Vec<&str> = rest.split('\t').collect();
    if parts.len() < 7 {
        return None;
    }
    let style: i32 = parts[2].trim().parse().unwrap_or(0);
    let tail = &parts[parts.len() - 5..];
    let x_axis: i32 = tail[0].trim().parse().unwrap_or(2);
    let y_axis: i32 = tail[1].trim().parse().unwrap_or(0);
    let visible: i32 = tail[4].trim().parse().unwrap_or(1);
    Some(PendingCurve {
        graph,
        layer,
        x_name: parts[0].trim().to_string(),
        y_name: parts[1].trim().to_string(),
        kind: curve_kind(style),
        x_side: if x_axis == 3 { XAxisSide::Top } else { XAxisSide::Bottom },
        y_side: if y_axis == 1 { YAxisSide::Right } else { YAxisSide::Left },
        visible: visible != 0,
    })
}

fn curve_kind(style: i32) -> CurveKind {
    match style {
        1 => CurveKind::Scatter,
        2 => CurveKind::LineSymbol,
        3 => CurveKind::Bars,
        4 => CurveKind::Area,
        5 => CurveKind::Pie,
        6 => CurveKind::DropLines,
        7 => CurveKind::Spline,
        8 => CurveKind::HorizontalSteps,
        9 => CurveKind::Histogram,
        10 => CurveKind::HorizontalBars,
        11 | 14 => CurveKind::Vector,
        12 => CurveKind::ErrorBars,
        13 => CurveKind::BoxPlot,
        15 => CurveKind::Steps,
        _ => CurveKind::Line,
    }
}

fn strip_html(text: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.trim().to_string()
}

fn read_surface(project: &mut Project, block: &str, surfaces: &mut Vec<PendingSurface>) {
    let mut name = String::new();
    let mut column_name = String::new();
    let mut kind = TablePlot3D::Ribbon;
    let mut mode = SurfaceMode::Surface;
    let mut azimuth = -55.0_f32;
    let mut elevation = 32.0_f32;
    for line in block.lines() {
        let line = line.trim_end();
        if name.is_empty() && !line.trim().is_empty() && !line.trim().starts_with('<') {
            name = field(line, 0).to_string();
        } else if let Some(rest) = line.trim().strip_prefix("SurfaceFunction\t") {
            column_name = surface_column(field(rest, 0));
        } else if let Some(rest) = line.trim().strip_prefix("Style\t") {
            let words: Vec<&str> = rest.split('\t').map(str::trim).collect();
            if words.iter().any(|word| word.eq_ignore_ascii_case("bars")) {
                kind = TablePlot3D::Bars;
            } else if words.iter().any(|word| word.eq_ignore_ascii_case("points")) {
                kind = TablePlot3D::Trajectory;
                mode = SurfaceMode::Scatter;
            }
            if words.iter().any(|word| word.eq_ignore_ascii_case("hiddenline") || word.contains("hidden")) {
                mode = SurfaceMode::HiddenLines;
            } else if words.iter().any(|word| word.eq_ignore_ascii_case("mesh") || word.eq_ignore_ascii_case("wireframe")) {
                mode = SurfaceMode::Wireframe;
            }
        } else if let Some(rest) = line.trim().strip_prefix("rotation\t") {
            let nums: Vec<f32> = rest.split('\t').filter_map(|text| text.trim().parse().ok()).collect();
            if let Some(value) = nums.first() {
                azimuth = *value;
            }
            if nums.len() >= 3 {
                elevation = nums[2];
            }
        }
    }
    if name.is_empty() {
        name = "Surface".to_string();
    }
    let graph = project.new_graph(&name);
    if !column_name.is_empty() {
        surfaces.push(PendingSurface {
            graph,
            column_name,
            kind,
            mode,
            azimuth,
            elevation,
        });
    }
}

fn surface_column(spec: &str) -> String {
    let refs: Vec<(String, String)> = spec
        .split(',')
        .filter_map(|part| {
            let (name, role) = part.trim().split_once('(')?;
            Some((name.trim().to_string(), role.trim_end_matches(')').trim().to_string()))
        })
        .collect();
    refs.iter()
        .find(|(_, role)| role.eq_ignore_ascii_case("Z"))
        .or_else(|| refs.iter().find(|(_, role)| role.eq_ignore_ascii_case("Y")))
        .map(|(name, _)| name.clone())
        .unwrap_or_default()
}

fn resolve(project: &Project, spec: &str) -> Option<(TableId, usize)> {
    let mut best: Option<(usize, TableId, usize)> = None;
    for table in &project.tables {
        let prefix = format!("{}_", table.name);
        let Some(rest) = spec.strip_prefix(&prefix) else { continue };
        let Some(index) = table.columns.iter().position(|column| column.name == rest) else {
            continue;
        };
        if best.as_ref().map(|(len, _, _)| table.name.len() > *len).unwrap_or(true) {
            best = Some((table.name.len(), table.id, index));
        }
    }
    best.map(|(_, id, column)| (id, column))
}

fn bind_curves(project: &mut Project, curves: Vec<PendingCurve>) {
    for curve in curves {
        let Some((table, x)) = resolve(project, &curve.x_name) else {
            project.log_line(crate::trf!(
                "没有找到曲线的 X 列 {name}。",
                "Curve X column {name} was not found.",
                name = curve.x_name
            ));
            continue;
        };
        let Some((y_table, y)) = resolve(project, &curve.y_name) else {
            project.log_line(crate::trf!(
                "没有找到曲线的 Y 列 {name}。",
                "Curve Y column {name} was not found.",
                name = curve.y_name
            ));
            continue;
        };
        if y_table != table {
            continue;
        }
        let Some(graph) = project.graph_mut(curve.graph) else { continue };
        let Some(layer) = graph.layers.get_mut(curve.layer) else { continue };
        if curve.x_side == XAxisSide::Top {
            layer.show_top = true;
        }
        if curve.y_side == YAxisSide::Right {
            layer.show_right = true;
        }
        let name = curve.y_name.clone();
        layer.curves.push(Curve {
            name,
            kind: curve.kind,
            style: CurveStyle::from_index(layer.curves.len()),
            source: DataSource::Columns {
                table,
                x,
                y,
                x_err: None,
                y_err: None,
                dx: None,
                dy: None,
            },
            visible: curve.visible,
            axes: CurveAxes {
                x: curve.x_side,
                y: curve.y_side,
            },
            ..Curve::default()
        });
    }
}

fn bind_surfaces(project: &mut Project, surfaces: Vec<PendingSurface>) {
    for surface in surfaces {
        let Some((table, column)) = resolve(project, &surface.column_name) else {
            project.log_line(crate::trf!(
                "没有找到曲面图的列 {name}。",
                "Surface column {name} was not found.",
                name = surface.column_name
            ));
            continue;
        };
        let Some(graph) = project.graph_mut(surface.graph) else { continue };
        graph.surface = Some(SurfaceSpec {
            source: SurfaceSource::Table {
                table,
                column,
                kind: surface.kind,
            },
            mode: surface.mode,
            azimuth: surface.azimuth,
            elevation: surface.elevation,
            ..SurfaceSpec::default()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exchange::load_project_bytes;
    use crate::graph::{SurfaceSource, XAxisSide, YAxisSide};
    use crate::table::ColumnMode;

    fn sized(tag: &str, xml: &str) -> String {
        format!("<{tag}>\n{}\n{xml}\n</{tag}>\n", xml.len())
    }

    #[test]
    fn official_header_reads_tables_formulas_folders_notes_and_curves() {
        let table = r#"<table name="Table1" columns="2"><column name="angle" plot_designation="X"><row index="0">0</row><row index="1">90</row></column><column name="y" plot_designation="Y"><formula start_row="0" end_row="1">cos(col(&quot;angle&quot;)*2*pi/360) %
+ sin(col(&quot;angle&quot;)*2*pi/360)</formula><row index="0">1</row><row index="1">1</row></column></table>"#;
        let nested = r#"<table name="Table2" columns="1"><column name="1" plot_designation="X"><row index="0">4</row></column></table>"#;
        let matrix = r#"<matrix name="Matrix1" columns="2" rows="2"><coordinates x_start="1" x_end="10" y_start="1" y_end="10"/><cell row="0" column="1">3.5</cell></matrix>"#;
        let graph = "\
<multiLayer>
Graph1\t1\t1\tnow
<graph>
PlotTitle\tSimple <b>2D</b> plot\t#000000\t4
EnabledAxes\t1\t1\t1\t1\t
AxesTitles\tX1\tY1\tX2\tY2\t
curve\tTable1_angle\tTable1_y\t0\t0\t0\t0\t1\t2\t0\t0\t6\t1
curve\tTable1_angle\tTable1_y\t1\t0\t0\t0\t1\t3\t1\t0\t6\t1
scale\t0\t-4\t6\t0\t8\t5\t0\t0
scale\t2\t0\t10\t0\t10\t5\t1\t0
scale\t3\t1\t9\t0\t8\t5\t0\t1
</graph>
</multiLayer>
";
        let surface = "\
<SurfacePlot>
Graph2\tnow
SurfaceFunction\tTable1_angle(X),Table1_y(Z)\t1\t2
Style\tbox\tnofloor\tpoints\t5\t0
rotation\t30\t0\t15
</SurfacePlot>
";
        let doc = format!(
            "SciDAVis 2.9.0 project file\n<scripting-lang>\tmuParser\n<windows>\t5\n{}{}<note>\nNotes1\tnow\n<content>\nalpha\nbeta\n</content>\n</note>\n<folder>\tData\t1/1/20\t\n{}</folder>\n{graph}{surface}",
            sized("table", table),
            sized("matrix", matrix),
            sized("table", nested),
        );
        let project = load_project_bytes("sample.sciprj", doc.as_bytes()).unwrap();
        let table = project.tables.iter().find(|table| table.name == "Table1").unwrap();
        assert_eq!(table.columns[0].mode, ColumnMode::X);
        assert_eq!(table.columns[0].numbers[1], Some(90.0));
        assert_eq!(
            table.columns[1].formula,
            "cos(col(\"angle\")*2*pi/360) + sin(col(\"angle\")*2*pi/360)"
        );
        let nested = project.tables.iter().find(|table| table.name == "Table2").unwrap();
        assert!(nested.folder.is_some());
        assert_eq!(nested.columns[0].numbers[0], Some(4.0));
        let matrix = project.matrices.iter().find(|matrix| matrix.name == "Matrix1").unwrap();
        assert_eq!(matrix.get(0, 1), Some(3.5));
        assert_eq!(matrix.x_max, 10.0);
        let note = project.notes.iter().find(|note| note.name == "Notes1").unwrap();
        assert!(note.text.contains("alpha"));
        assert!(note.text.contains("beta"));
        let folder = project.folders.iter().find(|folder| folder.name == "Data").unwrap();
        assert_eq!(nested.folder, Some(folder.id));
        let graph = project.graphs.iter().find(|graph| graph.name == "Graph1").unwrap();
        let layer = &graph.layers[0];
        assert_eq!(layer.title, "Simple 2D plot");
        assert!(layer.show_top && layer.show_right);
        assert_eq!(layer.curves.len(), 2);
        assert!(matches!(layer.curves[0].kind, CurveKind::Line));
        assert_eq!(layer.curves[1].axes.x, XAxisSide::Top);
        assert_eq!(layer.curves[1].axes.y, YAxisSide::Right);
        assert_eq!(layer.x_axis.scale, AxisScale::Log10);
        assert_eq!(layer.x_top.min, Some(1.0));
        assert!(layer.x_top.inverted);
        let surface = project.graphs.iter().find(|graph| graph.name == "Graph2").unwrap();
        let spec = surface.surface.as_ref().unwrap();
        assert!(matches!(spec.mode, SurfaceMode::Scatter));
        assert!(matches!(spec.source, SurfaceSource::Table { .. }));
        assert_eq!(spec.azimuth, 30.0);
    }

    #[test]
    fn sciplot_dialect_still_loads_and_official_magic_is_detected() {
        assert!(is_official("SciDAVis 1.22 project file\n"));
        assert!(is_official("QtiPlot 0.9.8 project file\n"));
        assert!(!is_official("<?xml version=\"1.0\"?>\n<sciprj></sciprj>\n"));
        let own = "<sciprj version=\"1\" name=\"保持\"></sciprj>\n";
        let project = load_project_bytes("保持.sciprj", own.as_bytes()).unwrap();
        assert_eq!(project.name, "保持");
    }

    #[test]
    fn bundled_scidavis_samples_open_when_present() {
        let dir = std::path::Path::new("/tmp/scidavis");
        if !dir.is_dir() {
            return;
        }
        for name in [
            "simple-2dplot.sciprj",
            "exemple-session.sciprj",
            "2axes-2dplot.sciprj",
            "testProject.sciprj",
            "3dplot.sciprj",
            "3dplotTable.sciprj",
            "Establishing_contact.sciprj",
            "integration.sciprj",
        ] {
            let path = dir.join(name);
            if !path.exists() {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            let project = load_project_bytes(name, &bytes).unwrap_or_else(|err| panic!("{name}: {err}"));
            assert!(
                !project.tables.is_empty() || !project.matrices.is_empty() || !project.notes.is_empty(),
                "{name} opened empty"
            );
        }
        let simple = std::fs::read(dir.join("simple-2dplot.sciprj")).unwrap();
        let project = load_project_bytes("simple-2dplot.sciprj", &simple).unwrap();
        let table = project.tables.iter().find(|table| table.name == "Table1").unwrap();
        assert_eq!(table.columns[0].name, "1");
        assert_eq!(table.columns[0].mode, ColumnMode::X);
        assert!((table.columns[0].numbers[0].unwrap() - 1.0).abs() < 1e-9);
        let graph = project.graphs.iter().find(|graph| graph.name == "Graph1").unwrap();
        assert!(graph.layers[0].title.contains("Simple"));
        assert!(!graph.layers[0].curves.is_empty());
        let exemple = std::fs::read(dir.join("exemple-session.sciprj")).unwrap();
        let project = load_project_bytes("exemple-session.sciprj", &exemple).unwrap();
        let formula = &project.tables[0].columns.iter().find(|column| column.name == "y").unwrap().formula;
        assert!(formula.contains("col(\"angle\")"), "{formula}");
        assert!(formula.contains("sin("), "{formula}");
        assert!(!formula.contains('%'));
        let axes = std::fs::read(dir.join("2axes-2dplot.sciprj")).unwrap();
        let project = load_project_bytes("2axes-2dplot.sciprj", &axes).unwrap();
        let layer = &project.graphs[0].layers[0];
        assert!(layer.show_top && layer.show_right);
        assert_eq!(layer.curves[1].axes.x, XAxisSide::Top);
        assert_eq!(layer.curves[1].axes.y, YAxisSide::Right);
        assert_eq!(layer.x_axis.label, "X1");
        assert_eq!(layer.y_right.label, "Y2");
        let contact = std::fs::read(dir.join("Establishing_contact.sciprj")).unwrap();
        let project = load_project_bytes("Establishing_contact.sciprj", &contact).unwrap();
        assert!(project.folders.iter().any(|folder| folder.name == "New Folder"));
        let note = project.notes.iter().find(|note| note.name == "TestNote").unwrap();
        assert!(note.text.contains("newTable"));
        let matrix = project.matrices.iter().find(|matrix| matrix.name == "Matrix1").unwrap();
        assert_eq!((matrix.rows, matrix.cols), (32, 32));
        let nested = project.tables.iter().find(|table| table.name == "Table2").unwrap();
        assert!(nested.folder.is_some());
        let plot3d = std::fs::read(dir.join("3dplot.sciprj")).unwrap();
        let project = load_project_bytes("3dplot.sciprj", &plot3d).unwrap();
        assert!(project.graphs.iter().any(|graph| graph.surface.is_some()));
    }
}
