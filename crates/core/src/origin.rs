//! Origin 项目的只读导入。不能写回 `.opj` / `.opju`。
//!
//! 二进制（`CPYA` / `CPYUA`，以及 ZIP 里的同样内容）按公开的对象布局读取：
//! 列头宽度在 `0x3D`（0 当成 8），短名在 `0x58`。宽度 8 是小端 `f64`，4 是 `f32`，
//! 大于 8 是定宽文本。`0x16` 的类型带 `0x100` 时，单元格是「标志、填充、值」：
//! 标志为 1 则值是文本，否则是双精度。短名里最后一个 `_` 前面当作表名
//! （`Histo_Counts` 读成表 Histo、列 Counts）。表头里另一段可打印字符串当作显示名，不必比短名更长。
//! 列属性对象的短名在 `0x12`，必须是在 `0x1E` 之前结束的 C 字符串（最长 11 个字符）。
//! `0x1E` 的种类是 `0x03`/`0x33` 日期、`0x02` 时刻、`0x31` 文本。
//! 日期按 Excel 序列日减去 25569，存成 Unix 纪元起的天数。
//! 已有数据集列（`0x58` 处有合法列名）不会被当成属性对象。
//!
//! 二进制里的图窗口没有稳定到可以安全还原，这里不猜测。XML 里的
//! `<graph><curve table="" x="" y=""/></graph>` 会连到已读入的表。
//! `<column longname="" type="text|date|time">` 会保留显示名和列类型。
//! 官方的 `Histo.opj`、`USstates.opj` 字节没能取到，对应行为用合成文件锁定。

use std::io::Read;

use flate2::read::DeflateDecoder;

use crate::exchange::{parse_xml, XmlNode};
use crate::graph::{Curve, CurveStyle, DataSource};
use crate::table::{ColumnMode, ColumnType, Table};
use crate::Project;

pub fn load_origin(bytes: &[u8]) -> Result<Project, String> {
    if bytes.starts_with(b"PK\x03\x04") {
        return load_zip(bytes);
    }
    let start = trim_ascii(bytes);
    if start.starts_with(b"<") || start.starts_with(b"<?xml") {
        let text = std::str::from_utf8(start).map_err(|_| {
            crate::tr!("Origin XML 不是 UTF-8", "The Origin XML is not UTF-8").to_string()
        })?;
        return load_origin_xml(text);
    }
    if start.starts_with(b"CPYA") || start.starts_with(b"CPYUA") {
        return load_binary(start);
    }
    Err(unsupported())
}

fn unsupported() -> String {
    crate::tr!(
        "这不是可读取的 Origin 项目。可以读 CPYA/CPYUA 里的数值、文本和日期列，ZIP 里的这些内容或 CSV，以及 <origin> XML 工作表和其中的曲线。二进制图窗口和分析不会导入，也不能写回 Origin。",
        "This Origin project is outside the reader. SciPlot reads numeric, text, and date columns from CPYA/CPYUA projects, those columns or CSV inside a zip, and <origin> XML sheets with curves. Binary graph windows and analysis are not imported, and Origin files are not written back."
    )
    .into()
}

fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let mut start = 0;
    while start < bytes.len() && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    &bytes[start..]
}

fn load_zip(bytes: &[u8]) -> Result<Project, String> {
    let members = unzip(bytes)?;
    let mut project = Project::new("Origin");
    let mut found = false;
    for (name, payload) in members {
        if payload.is_empty() {
            continue;
        }
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".csv") || lower.ends_with(".txt") || lower.ends_with(".tsv") {
            if let Ok(mut table) = crate::import_csv(&payload) {
                let stem = std::path::Path::new(&name)
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("Origin");
                table.name = stem.to_string();
                project.insert_table(table);
                found = true;
            }
            continue;
        }
        match load_origin(&payload) {
            Ok(part) => {
                project.append_project(&part);
                found = true;
            }
            Err(_) => continue,
        }
    }
    if !found || (project.tables.is_empty() && project.notes.is_empty() && project.matrices.is_empty()) {
        return Err(unsupported());
    }
    if project.name == "Origin" {
        if let Some(folder) = project.folders.first() {
            if folder.name != "Origin" {
                project.name = folder.name.clone();
            }
        }
    }
    Ok(project)
}

fn load_origin_xml(text: &str) -> Result<Project, String> {
    let root = parse_xml(text)?;
    let mut project = Project::new(root.attr("name").unwrap_or("Origin"));
    let mut sheets = 0;
    let mut graphs = Vec::new();
    collect_xml(&root, &mut project, &mut sheets, &mut graphs);
    for (name, curves) in graphs {
        bind_xml_graph(&mut project, &name, &curves);
    }
    if sheets == 0 && project.notes.is_empty() && project.tables.is_empty() && project.graphs.is_empty() {
        return Err(unsupported());
    }
    project.fix_ids();
    Ok(project)
}

type XmlCurve = (String, String, String);
type XmlGraph = (String, Vec<XmlCurve>);

fn collect_xml(
    elem: &crate::exchange::XmlElem,
    project: &mut Project,
    sheets: &mut usize,
    graphs: &mut Vec<XmlGraph>,
) {
    let name = elem.name.to_ascii_lowercase();
    if matches!(name.as_str(), "sheet" | "worksheet" | "table" | "workbook")
        && has_column(elem)
        && name != "workbook"
    {
        push_sheet(project, elem);
        *sheets += 1;
        return;
    }
    if name == "note" {
        let title = elem.text_of(&["name"]).unwrap_or_else(|| "Note".into());
        let body = {
            let direct = elem.direct_text();
            if direct.trim().is_empty() {
                elem.child_text("content").unwrap_or(direct)
            } else {
                direct
            }
        };
        let id = project.new_note(&title);
        if let Some(note) = project.note_mut(id) {
            note.text = body;
        }
        return;
    }
    if matches!(name.as_str(), "graph" | "plot") {
        let title = elem.text_of(&["name"]).unwrap_or_else(|| "Graph".into());
        let mut curves = Vec::new();
        for child in &elem.children {
            let XmlNode::Elem(curve) = child else { continue };
            if !matches!(curve.name.to_ascii_lowercase().as_str(), "curve" | "line") {
                continue;
            }
            let table = curve.attr("table").unwrap_or("").to_string();
            let x = curve.attr("x").unwrap_or("").to_string();
            let y = curve.attr("y").unwrap_or("").to_string();
            if !table.is_empty() && !x.is_empty() && !y.is_empty() {
                curves.push((table, x, y));
            }
        }
        graphs.push((title, curves));
        return;
    }
    for child in &elem.children {
        if let XmlNode::Elem(child) = child {
            collect_xml(child, project, sheets, graphs);
        }
    }
}

fn has_column(elem: &crate::exchange::XmlElem) -> bool {
    elem.children.iter().any(|node| match node {
        XmlNode::Elem(child) => {
            let name = child.name.to_ascii_lowercase();
            name == "column" || name == "col"
        }
        XmlNode::Text(_) => false,
    })
}

fn push_sheet(project: &mut Project, elem: &crate::exchange::XmlElem) {
    let title = elem.text_of(&["name"]).unwrap_or_else(|| "Origin".into());
    let id = project.new_table(&title);
    let Some(table) = project.table_mut(id) else {
        return;
    };
    table.columns.clear();
    for child in &elem.children {
        let XmlNode::Elem(column) = child else {
            continue;
        };
        let tag = column.name.to_ascii_lowercase();
        if tag != "column" && tag != "col" {
            continue;
        }
        let short = column.text_of(&["name"]).unwrap_or_else(|| "A".into());
        let column_name = column
            .text_of(&["longname", "long_name"])
            .filter(|text| !text.trim().is_empty())
            .unwrap_or(short);
        let (ty, mode) = xml_column_kind(column);
        let index = table.add_column(column_name, mode);
        table.columns[index].ty = ty;
        let mut row = 0;
        for node in &column.children {
            match node {
                XmlNode::Elem(cell) if matches!(cell.name.to_ascii_lowercase().as_str(), "value" | "row" | "cell") => {
                    table.columns[index].set_text(row, cell.direct_text().trim());
                    row += 1;
                }
                XmlNode::Text(text) => {
                    for line in text.lines() {
                        if line.trim().is_empty() {
                            continue;
                        }
                        table.columns[index].set_text(row, line.trim());
                        row += 1;
                    }
                }
                _ => {}
            }
        }
        reinterpret_temporal(&mut table.columns[index]);
    }
    if table.columns.is_empty() {
        table.add_column("A", ColumnMode::Y);
    }
}

fn xml_column_kind(column: &crate::exchange::XmlElem) -> (ColumnType, ColumnMode) {
    let ty_attr = column.attr("type").unwrap_or("");
    let designation = column
        .attr("designation")
        .or_else(|| column.attr("plot_designation"))
        .unwrap_or("");
    let mode = if !designation.is_empty() {
        mode_of(designation)
    } else if matches!(
        ty_attr.to_ascii_lowercase().as_str(),
        "x" | "y" | "z" | "xerr" | "yerr" | "label" | "none" | "0" | "1" | "2" | "3" | "4" | "5" | "6"
    ) {
        mode_of(ty_attr)
    } else {
        ColumnMode::Y
    };
    let ty = match ty_attr.to_ascii_lowercase().as_str() {
        "text" | "string" => ColumnType::Text,
        "date" => ColumnType::Date,
        "time" => ColumnType::Time,
        "datetime" => ColumnType::DateTime,
        _ => ColumnType::Numeric,
    };
    (ty, mode)
}

/// XML 里的日期、时刻如果写成 Excel 序列或一天里的小数，换成本程序的存储单位。
fn reinterpret_temporal(column: &mut crate::table::Column) {
    match column.ty {
        ColumnType::Date | ColumnType::DateTime => {
            for row in 0..column.len() {
                let text = column.texts.get(row).map(String::as_str).unwrap_or("").trim();
                if text.contains('-') || text.contains(':') || text.contains('年') {
                    continue;
                }
                if let Ok(serial) = text.parse::<f64>() {
                    if serial.is_finite() {
                        column.numbers[row] = Some(serial - 25569.0);
                    }
                }
            }
        }
        ColumnType::Time => {
            for row in 0..column.len() {
                let text = column.texts.get(row).map(String::as_str).unwrap_or("").trim();
                if text.contains(':') {
                    continue;
                }
                if let Ok(value) = text.parse::<f64>() {
                    if value.is_finite() && (-0.01..=1.01).contains(&value) {
                        column.numbers[row] = Some(value * 86_400.0);
                    }
                }
            }
        }
        _ => {}
    }
}

fn bind_xml_graph(project: &mut Project, name: &str, curves: &[(String, String, String)]) {
    let mut built = Vec::new();
    for (table_name, x_name, y_name) in curves {
        let Some(table) = project.tables.iter().find(|table| table.name == *table_name) else {
            continue;
        };
        let Some(x) = find_xml_column(table, x_name) else { continue };
        let Some(y) = find_xml_column(table, y_name) else { continue };
        built.push((y_name.clone(), table.id, x, y));
    }
    let id = project.new_graph(name);
    let Some(graph) = project.graph_mut(id) else { return };
    if let Some(layer) = graph.layers.first_mut() {
        for (name, table, x, y) in built {
            layer.curves.push(Curve {
                name,
                kind: crate::graph::CurveKind::Line,
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
                ..Curve::default()
            });
        }
    }
}

fn find_xml_column(table: &Table, name: &str) -> Option<usize> {
    if let Some(index) = table.columns.iter().position(|column| column.name == name) {
        return Some(index);
    }
    let index = name.parse::<usize>().ok()?;
    let index = index.checked_sub(1)?;
    (index < table.columns.len()).then_some(index)
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

enum OriginValues {
    Numbers(Vec<f64>),
    Text(Vec<String>),
}

struct OriginColumn {
    short: String,
    display: String,
    sheet: String,
    values: OriginValues,
}

fn load_binary(bytes: &[u8]) -> Result<Project, String> {
    let (columns, properties) = scan_columns(bytes);
    if columns.is_empty() {
        return Err(crate::tr!(
            "Origin 文件里没有找到数值列或文本列。二进制图窗口不在这个读取范围内。",
            "No numeric or text columns were found in the Origin file. Binary graph windows are outside this reader."
        )
        .into());
    }
    let mut project = Project::new("Origin");
    let mut sheets: Vec<(String, Vec<OriginColumn>)> = Vec::new();
    for column in columns {
        if let Some((_, group)) = sheets.iter_mut().find(|(name, _)| name == &column.sheet) {
            group.push(column);
        } else {
            sheets.push((column.sheet.clone(), vec![column]));
        }
    }
    for (sheet, group) in sheets {
        let mut table = Table::new(crate::TableId(0), sheet);
        table.columns.clear();
        for (col_index, column) in group.into_iter().enumerate() {
            let mode = if col_index == 0 { ColumnMode::X } else { ColumnMode::Y };
            let index = match column.values {
                OriginValues::Numbers(values) => table.add_numeric_column(column.display, mode, &values),
                OriginValues::Text(values) => {
                    let index = table.add_column(column.display, mode);
                    table.columns[index].ty = ColumnType::Text;
                    for (row, text) in values.iter().enumerate() {
                        table.columns[index].set_text(row, text);
                    }
                    index
                }
            };
            if let Some(kind) = properties.iter().find(|(name, _)| name == &column.short).map(|(_, kind)| *kind) {
                apply_property(&mut table.columns[index], kind);
            }
        }
        project.insert_table(table);
    }
    Ok(project)
}

fn apply_property(column: &mut crate::table::Column, kind: u8) {
    match kind {
        0x03 | 0x33 => {
            column.ty = ColumnType::Date;
            for value in &mut column.numbers {
                if let Some(serial) = *value {
                    *value = Some(serial - 25569.0);
                }
            }
        }
        0x02 => {
            column.ty = ColumnType::Time;
            let fraction = column.numbers.iter().flatten().all(|value| (-0.01..=1.01).contains(value));
            if fraction {
                for value in &mut column.numbers {
                    if let Some(number) = *value {
                        *value = Some(number * 86_400.0);
                    }
                }
            }
        }
        0x31 => column.ty = ColumnType::Text,
        _ => {}
    }
}

fn scan_columns(data: &[u8]) -> (Vec<OriginColumn>, Vec<(String, u8)>) {
    let mut columns = Vec::new();
    let mut properties = Vec::new();
    let mut index = 0;
    while index + 16 < data.len() {
        if let Some((column, next)) = try_column(data, index) {
            columns.push(column);
            index = next.max(index + 1);
        } else if let Some((name, kind, next)) = try_property(data, index) {
            properties.push((name, kind));
            index = next.max(index + 1);
        } else {
            index += 1;
        }
    }
    (columns, properties)
}

fn try_column(data: &[u8], index: usize) -> Option<(OriginColumn, usize)> {
    let (header_size, mut cursor) = read_size(data, index)?;
    if !(0x70..=0x300).contains(&header_size) {
        return None;
    }
    let header_end = cursor.checked_add(header_size)?;
    if header_end >= data.len() || data.get(header_end) != Some(&b'\n') {
        return None;
    }
    let header = &data[cursor..header_end];
    let short = column_name(header, 0x58).or_else(|| column_name(header, 0x57))?;
    let width = match header.get(0x3D).copied().unwrap_or(0) {
        0 => 8usize,
        width => width as usize,
    };
    if !(width == 4 || width == 8 || (9..=256).contains(&width)) {
        return None;
    }
    let mixed = header.len() > 0x17 && (u16::from_le_bytes([header[0x16], header[0x17]]) & 0x100) != 0;
    cursor = header_end + 1;
    let (data_size, data_at) = read_size(data, cursor)?;
    if data_size == 0 || data_size > 8 * 1_000_000 {
        return None;
    }
    let data_end = data_at.checked_add(data_size)?;
    if data_end >= data.len() || data.get(data_end) != Some(&b'\n') {
        return None;
    }
    let payload = &data[data_at..data_end];
    let values = if mixed && width == 8 {
        read_mixed(payload)?
    } else if width == 8 {
        read_f64s(payload)?
    } else if width == 4 {
        read_f32s(payload)?
    } else {
        read_fixed_text(payload, width)?
    };
    cursor = data_end + 1;
    let (mask_size, mask_at) = read_size(data, cursor)?;
    if mask_size > 1_000_000 {
        return None;
    }
    let mut next = mask_at;
    if mask_size > 0 {
        let mask_end = mask_at.checked_add(mask_size)?;
        if data.get(mask_end) != Some(&b'\n') {
            return None;
        }
        next = mask_end + 1;
    }
    let (sheet, column) = sheet_and_column(&short);
    let display = long_name(header, &short).unwrap_or(column);
    Some((
        OriginColumn {
            short,
            display,
            sheet,
            values,
        },
        next,
    ))
}

fn try_property(data: &[u8], index: usize) -> Option<(String, u8, usize)> {
    let (header_size, cursor) = read_size(data, index)?;
    if !(0x20..=0x300).contains(&header_size) {
        return None;
    }
    let header_end = cursor.checked_add(header_size)?;
    if header_end >= data.len() || data.get(header_end) != Some(&b'\n') {
        return None;
    }
    let header = &data[cursor..header_end];
    if column_name(header, 0x58).is_some() {
        return None;
    }
    let kind = *header.get(0x1E)?;
    if !matches!(kind, 0x02 | 0x03 | 0x31 | 0x33) {
        return None;
    }
    let name = column_name(header, 0x12)?;
    Some((name, kind, header_end + 1))
}

fn read_f64s(payload: &[u8]) -> Option<OriginValues> {
    if payload.len() % 8 != 0 {
        return None;
    }
    let mut values = Vec::new();
    let mut finite = 0usize;
    for chunk in payload.chunks_exact(8) {
        let value = f64::from_le_bytes(chunk.try_into().ok()?);
        if value.is_finite() {
            finite += 1;
            values.push(value);
        } else {
            values.push(f64::NAN);
        }
    }
    (finite > 0).then_some(OriginValues::Numbers(values))
}

fn read_f32s(payload: &[u8]) -> Option<OriginValues> {
    if payload.len() % 4 != 0 {
        return None;
    }
    let mut values = Vec::new();
    let mut finite = 0usize;
    for chunk in payload.chunks_exact(4) {
        let value = f32::from_le_bytes(chunk.try_into().ok()?) as f64;
        if value.is_finite() {
            finite += 1;
            values.push(value);
        } else {
            values.push(f64::NAN);
        }
    }
    (finite > 0).then_some(OriginValues::Numbers(values))
}

fn read_fixed_text(payload: &[u8], width: usize) -> Option<OriginValues> {
    if width == 0 || payload.len() % width != 0 {
        return None;
    }
    let mut values = Vec::new();
    let mut any = false;
    for chunk in payload.chunks_exact(width) {
        let end = chunk.iter().position(|byte| *byte == 0).unwrap_or(chunk.len());
        let text = String::from_utf8_lossy(&chunk[..end]).trim().to_string();
        if !text.is_empty() {
            any = true;
        }
        values.push(text);
    }
    any.then_some(OriginValues::Text(values))
}

fn read_mixed(payload: &[u8]) -> Option<OriginValues> {
    const STRIDE: usize = 10;
    if payload.len() % STRIDE != 0 {
        return None;
    }
    let mut texts = Vec::new();
    let mut any = false;
    for chunk in payload.chunks_exact(STRIDE) {
        if chunk[0] == 1 {
            let end = chunk[2..].iter().position(|byte| *byte == 0).unwrap_or(8);
            let text = String::from_utf8_lossy(&chunk[2..2 + end]).trim().to_string();
            if !text.is_empty() {
                any = true;
            }
            texts.push(text);
        } else {
            let value = f64::from_le_bytes(chunk[2..10].try_into().ok()?);
            if value.is_finite() {
                any = true;
                texts.push(crate::format_short(value));
            } else {
                texts.push(String::new());
            }
        }
    }
    any.then_some(OriginValues::Text(texts))
}

fn sheet_and_column(short: &str) -> (String, String) {
    if let Some((sheet, column)) = short.rsplit_once('_') {
        if !sheet.is_empty()
            && !column.is_empty()
            && sheet.chars().next().is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
            && sheet.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            && column.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            return (sheet.to_string(), column.to_string());
        }
    }
    ("Origin".to_string(), short.to_string())
}

fn long_name(header: &[u8], short: &str) -> Option<String> {
    let mut best: Option<String> = None;
    let mut index = 0;
    while index < header.len() {
        if index > 0 && header[index - 1] != 0 {
            index += 1;
            continue;
        }
        if let Some(text) = printable_string(header, index) {
            if text != short && best.as_ref().map(|current| text.len() > current.len()).unwrap_or(true) {
                best = Some(text.clone());
            }
            index += text.len() + 1;
        } else {
            index += 1;
        }
    }
    best
}

fn printable_string(header: &[u8], at: usize) -> Option<String> {
    if at >= header.len() {
        return None;
    }
    let end = header[at..].iter().position(|byte| *byte == 0).unwrap_or(header.len() - at);
    if !(2..=64).contains(&end) {
        return None;
    }
    let text = std::str::from_utf8(&header[at..at + end]).ok()?.trim();
    if text.chars().all(|ch| ch.is_ascii_graphic() || ch == ' ') && text.chars().any(|ch| ch.is_ascii_alphabetic()) {
        Some(text.to_string())
    } else {
        None
    }
}

fn column_name(header: &[u8], at: usize) -> Option<String> {
    if at >= header.len() {
        return None;
    }
    let bytes = &header[at..header.len().min(at + 25)];
    let end = bytes.iter().position(|byte| *byte == 0).unwrap_or(bytes.len());
    let text = std::str::from_utf8(&bytes[..end]).ok()?.trim();
    if text.is_empty() || text.len() > 24 {
        return None;
    }
    let mut chars = text.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    if !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
        return None;
    }
    Some(text.to_string())
}

fn read_size(data: &[u8], index: usize) -> Option<(usize, usize)> {
    if index + 5 > data.len() || data[index + 4] != b'\n' {
        return None;
    }
    let size = u32::from_le_bytes(data[index..index + 4].try_into().ok()?) as usize;
    Some((size, index + 5))
}

fn unzip(data: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut out = Vec::new();
    let mut index = 0;
    while index + 30 <= data.len() && data[index..index + 4] == *b"PK\x03\x04" {
        let flags = u16::from_le_bytes([data[index + 6], data[index + 7]]);
        let method = u16::from_le_bytes([data[index + 8], data[index + 9]]);
        let comp = u32::from_le_bytes(data[index + 18..index + 22].try_into().unwrap()) as usize;
        let name_len = u16::from_le_bytes([data[index + 26], data[index + 27]]) as usize;
        let extra_len = u16::from_le_bytes([data[index + 28], data[index + 29]]) as usize;
        let name_at = index + 30;
        let data_at = name_at + name_len + extra_len;
        if data_at + comp > data.len() {
            break;
        }
        if (flags & 0x1) != 0 {
            return Err(crate::tr!("Origin 包被加密了", "The Origin package is encrypted").into());
        }
        let name = String::from_utf8_lossy(&data[name_at..name_at + name_len]).to_string();
        let payload = &data[data_at..data_at + comp];
        let bytes = match method {
            0 => payload.to_vec(),
            8 => {
                let mut decoder = DeflateDecoder::new(payload);
                let mut buf = Vec::new();
                decoder.read_to_end(&mut buf).map_err(|err| err.to_string())?;
                buf
            }
            other => {
                return Err(crate::trf!(
                    "Origin 包用了不支持的压缩方法 {other}",
                    "The Origin package uses unsupported compression method {other}",
                    other = other
                ))
            }
        };
        if !name.ends_with('/') {
            out.push((name, bytes));
        }
        index = data_at + comp;
    }
    if out.is_empty() {
        return Err(crate::tr!("ZIP 里没有可读的文件", "The zip archive has no readable members").into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_obj(buf: &mut Vec<u8>, payload: &[u8]) {
        buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        buf.push(b'\n');
        if !payload.is_empty() {
            buf.extend_from_slice(payload);
            buf.push(b'\n');
        }
    }

    fn header(name: &str) -> Vec<u8> {
        let mut bytes = vec![0u8; 0x80];
        bytes[0x3D] = 8;
        let raw = name.as_bytes();
        bytes[0x58..0x58 + raw.len()].copy_from_slice(raw);
        bytes
    }

    fn doubles(values: &[f64]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn binary_opj_reads_double_columns_and_xml_subset_and_zip() {
        let mut file = b"CPYA 8.50#\n".to_vec();
        let mut global = vec![0u8; 0x30];
        global[0x1B..0x23].copy_from_slice(&8.5f64.to_le_bytes());
        push_obj(&mut file, &global);
        push_obj(&mut file, &[]);
        push_obj(&mut file, &header("Time"));
        push_obj(&mut file, &doubles(&[1.0, 2.0, 3.0]));
        push_obj(&mut file, &[]);
        push_obj(&mut file, &header("Value"));
        push_obj(&mut file, &doubles(&[10.0, 20.0, 30.0]));
        push_obj(&mut file, &[]);
        let project = load_origin(&file).unwrap();
        assert_eq!(project.tables.len(), 1);
        assert_eq!(project.tables[0].columns[0].name, "Time");
        assert_eq!(project.tables[0].columns[0].numbers, vec![Some(1.0), Some(2.0), Some(3.0)]);
        assert_eq!(project.tables[0].columns[1].numbers[2], Some(30.0));
        assert!(matches!(project.tables[0].columns[0].mode, ColumnMode::X));

        let xml = r#"<?xml version="1.0"?>
<origin name="实验">
  <sheet name="Book1">
    <column name="A" designation="X"><value>1</value><value>2</value></column>
    <column name="B" designation="Y"><value>4</value><value>8</value></column>
  </sheet>
  <note name="记录">导入说明</note>
</origin>"#;
        let project = load_origin(xml.as_bytes()).unwrap();
        assert_eq!(project.name, "实验");
        assert_eq!(project.tables[0].name, "Book1");
        assert_eq!(project.tables[0].columns[1].numbers[1], Some(8.0));
        assert_eq!(project.notes[0].text, "导入说明");

        let zipped = zip_store("book.xml", xml.as_bytes());
        let project = load_origin(&zipped).unwrap();
        assert!(project.tables.iter().any(|table| table.columns.len() == 2));
        assert!(load_origin(b"not an origin file").is_err());
    }

    #[test]
    fn origin_reads_text_dates_friendly_names_and_xml_graphs() {
        let mut file = b"CPYA 8.50#\n".to_vec();
        push_obj(&mut file, &[0u8; 0x30]);
        let mut counts = header("Histo_Day");
        counts[0x72..0x72 + b"Bin center".len()].copy_from_slice(b"Bin center");
        push_obj(&mut file, &counts);
        push_obj(&mut file, &doubles(&[25569.0, 25570.0]));
        push_obj(&mut file, &[]);
        let mut day = vec![0u8; 0x40];
        day[0x12..0x12 + b"Histo_Day".len()].copy_from_slice(b"Histo_Day");
        day[0x1E] = 0x03;
        push_obj(&mut file, &day);
        let mut names = header("States_Name");
        names[0x3D] = 16;
        push_obj(&mut file, &names);
        let mut text = Vec::new();
        for label in ["California", "New York"] {
            let mut cell = vec![0u8; 16];
            cell[..label.len()].copy_from_slice(label.as_bytes());
            text.extend_from_slice(&cell);
        }
        push_obj(&mut file, &text);
        push_obj(&mut file, &[]);
        let mut narrow = header("States_Score");
        narrow[0x3D] = 4;
        push_obj(&mut file, &narrow);
        let mut floats = Vec::new();
        floats.extend_from_slice(&1.5f32.to_le_bytes());
        floats.extend_from_slice(&2.5f32.to_le_bytes());
        push_obj(&mut file, &floats);
        push_obj(&mut file, &[]);

        let project = load_origin(&file).unwrap();
        let histo = project.tables.iter().find(|table| table.name == "Histo").unwrap();
        assert_eq!(histo.columns[0].name, "Bin center");
        assert!(matches!(histo.columns[0].ty, ColumnType::Date));
        assert_eq!(histo.columns[0].numbers[0], Some(0.0));
        assert_eq!(histo.columns[0].numbers[1], Some(1.0));
        let states = project.tables.iter().find(|table| table.name == "States").unwrap();
        assert_eq!(states.columns[0].name, "Name");
        assert_eq!(states.columns[0].texts[0], "California");
        assert_eq!(states.columns[1].name, "Score");
        assert_eq!(states.columns[1].numbers[0], Some(1.5));

        let mut text_only = b"CPYA 8.50#\n".to_vec();
        let mut only = header("US_State");
        only[0x3D] = 12;
        push_obj(&mut text_only, &only);
        let mut cells = Vec::new();
        for label in ["CA", "NY"] {
            let mut cell = vec![0u8; 12];
            cell[..label.len()].copy_from_slice(label.as_bytes());
            cells.extend_from_slice(&cell);
        }
        push_obj(&mut text_only, &cells);
        push_obj(&mut text_only, &[]);
        let project = load_origin(&text_only).unwrap();
        assert_eq!(project.tables[0].name, "US");
        assert_eq!(project.tables[0].columns[0].texts, vec!["CA".to_string(), "NY".to_string()]);

        let xml = r#"<?xml version="1.0"?>
<origin name="States">
  <sheet name="Book1">
    <column name="A" longname="State" type="text" designation="X"><value>CA</value><value>NY</value></column>
    <column name="B" longname="Admitted" type="date" designation="Y"><value>25569</value></column>
    <column name="C" type="time"><value>0.5</value></column>
  </sheet>
  <graph name="Map">
    <curve table="Book1" x="State" y="Admitted"/>
  </graph>
</origin>"#;
        let project = load_origin(xml.as_bytes()).unwrap();
        assert_eq!(project.tables[0].columns[0].name, "State");
        assert_eq!(project.tables[0].columns[0].texts[1], "NY");
        assert!(matches!(project.tables[0].columns[1].ty, ColumnType::Date));
        assert_eq!(project.tables[0].columns[1].numbers[0], Some(0.0));
        assert!(matches!(project.tables[0].columns[2].ty, ColumnType::Time));
        assert_eq!(project.tables[0].columns[2].numbers[0], Some(43_200.0));
        let graph = project.graphs.iter().find(|graph| graph.name == "Map").unwrap();
        assert!(matches!(
            graph.layers[0].curves[0].source,
            crate::graph::DataSource::Columns { .. }
        ));
    }

    fn zip_store(name: &str, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"PK\x03\x04");
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(payload);
        out
    }
}
