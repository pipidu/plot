//! Origin 项目的一个有边界的读取器。
//!
//! 能读的范围：
//! - 文件头是 `CPYA` 或 `CPYUA`（liborigin 用来识别 OPJ / OPJU）时，扫描双精度工作表列。
//!   列头里数值宽度在 `0x3D`、列名在 `0x58`，数据和 liborigin 一样是小端 `f64`。
//! - `.opju` 这类 ZIP 包：里面的上述二进制流、Origin XML 工作表，以及 CSV 成员。
//! - 文档化的 XML 子集：`<origin>` / `<sheet>` / `<column>` / `<value>` / `<note>`。
//!
//! 不读图、文本列、日期列和分析操作。写回 Origin 文件也不在这里。

use std::io::Read;

use flate2::read::DeflateDecoder;

use crate::exchange::{parse_xml, XmlNode};
use crate::table::{ColumnMode, Table};
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
        "这不是可读取的 Origin 项目。目前只读 CPYA/CPYUA 工程里的双精度列、ZIP 包中的这些列或 CSV，以及 <origin> XML 工作表。图、文本列和分析不会导入。",
        "This Origin project is outside the reader. SciPlot reads double columns from CPYA/CPYUA projects, those columns or CSV inside a zip, and <origin> XML sheets. Graphs, text columns, and analysis are not imported."
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
    collect_xml(&root, &mut project, &mut sheets);
    if sheets == 0 && project.notes.is_empty() && project.tables.is_empty() {
        return Err(unsupported());
    }
    project.fix_ids();
    Ok(project)
}

fn collect_xml(elem: &crate::exchange::XmlElem, project: &mut Project, sheets: &mut usize) {
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
    for child in &elem.children {
        if let XmlNode::Elem(child) = child {
            collect_xml(child, project, sheets);
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
        let column_name = column.text_of(&["name"]).unwrap_or_else(|| "A".into());
        let mode = mode_of(&column.text_of(&["designation", "type", "plot_designation"]).unwrap_or_else(|| "Y".into()));
        let index = table.add_column(column_name, mode);
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
    }
    if table.columns.is_empty() {
        table.add_column("A", ColumnMode::Y);
    }
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

fn load_binary(bytes: &[u8]) -> Result<Project, String> {
    let columns = scan_columns(bytes);
    if columns.is_empty() {
        return Err(crate::tr!(
            "Origin 文件里没有找到双精度列。文本列和图不在这个读取范围内。",
            "No double-precision columns were found in the Origin file. Text columns and graphs are outside this reader."
        )
        .into());
    }
    let mut project = Project::new("Origin");
    for group in group_columns(columns) {
        let mut table = Table::new(crate::TableId(0), "Origin");
        table.columns.clear();
        for (col_index, (col_name, values)) in group.into_iter().enumerate() {
            let mode = if col_index == 0 { ColumnMode::X } else { ColumnMode::Y };
            table.add_numeric_column(col_name, mode, &values);
        }
        project.insert_table(table);
    }
    if project.tables.len() > 1 {
        for (index, table) in project.tables.iter_mut().enumerate().skip(1) {
            table.name = format!("Origin{}", index + 1);
        }
    }
    Ok(project)
}

fn group_columns(columns: Vec<(String, Vec<f64>)>) -> Vec<Vec<(String, Vec<f64>)>> {
    let mut groups = Vec::new();
    let mut current = Vec::new();
    let mut seen = Vec::new();
    for column in columns {
        if seen.iter().any(|name| name == &column.0) {
            groups.push(std::mem::take(&mut current));
            seen.clear();
        }
        seen.push(column.0.clone());
        current.push(column);
    }
    if !current.is_empty() {
        groups.push(current);
    }
    groups
}

fn scan_columns(data: &[u8]) -> Vec<(String, Vec<f64>)> {
    let mut columns = Vec::new();
    let mut index = 0;
    while index + 16 < data.len() {
        if let Some((name, values, next)) = try_column(data, index) {
            columns.push((name, values));
            index = next.max(index + 1);
        } else {
            index += 1;
        }
    }
    columns
}

fn try_column(data: &[u8], index: usize) -> Option<(String, Vec<f64>, usize)> {
    let (header_size, mut cursor) = read_size(data, index)?;
    if !(0x70..=0x300).contains(&header_size) {
        return None;
    }
    let header_end = cursor.checked_add(header_size)?;
    if header_end >= data.len() || data.get(header_end) != Some(&b'\n') {
        return None;
    }
    let header = &data[cursor..header_end];
    if header.get(0x3D).copied() != Some(8) {
        return None;
    }
    let name = column_name(header, 0x58).or_else(|| column_name(header, 0x57))?;
    cursor = header_end + 1;
    let (data_size, data_at) = read_size(data, cursor)?;
    if data_size == 0 || data_size % 8 != 0 || data_size > 8 * 1_000_000 {
        return None;
    }
    let rows = data_size / 8;
    let data_end = data_at.checked_add(data_size)?;
    if data_end >= data.len() || data.get(data_end) != Some(&b'\n') {
        return None;
    }
    let mut values = Vec::with_capacity(rows);
    let mut finite = 0usize;
    for chunk in data[data_at..data_end].chunks_exact(8) {
        let value = f64::from_le_bytes(chunk.try_into().ok()?);
        if value.is_finite() {
            finite += 1;
            values.push(value);
        } else {
            values.push(f64::NAN);
        }
    }
    if finite == 0 {
        return None;
    }
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
    Some((name, values, next))
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
