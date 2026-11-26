use csv::{ReaderBuilder, WriterBuilder};
use serde::{Deserialize, Serialize};

use crate::tr;

use crate::table::{ColumnMode, ColumnType, Table, TableId};

/// 导入导出用的文本编码。`Auto` 导入时识别 UTF-8、UTF-16（BOM 或大量空字节）和 GBK；导出时按 UTF-8 带 BOM。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CsvEncoding {
    #[serde(rename = "auto")]
    #[default]
    Auto,
    #[serde(rename = "utf8")]
    Utf8,
    #[serde(rename = "gbk")]
    Gbk,
}

impl CsvEncoding {
    pub fn label(self) -> &'static str {
        match self {
            CsvEncoding::Auto => tr!("自动", "Auto"),
            CsvEncoding::Utf8 => "UTF-8",
            CsvEncoding::Gbk => "GBK",
        }
    }

    pub fn all() -> &'static [CsvEncoding] {
        &[CsvEncoding::Auto, CsvEncoding::Utf8, CsvEncoding::Gbk]
    }
}

/// 读入 CSV / TSV。自动识别 UTF-8（可带 BOM）、UTF-16 和 GBK，方便中文 Excel。
pub fn import_csv(bytes: &[u8]) -> Result<Table, String> {
    import_csv_with(bytes, CsvEncoding::Auto)
}

/// 按指定编码读入 CSV / TSV。表名仍是「导入」，调用方可以再改成文件名。
pub fn import_csv_with(bytes: &[u8], encoding: CsvEncoding) -> Result<Table, String> {
    if bytes.iter().all(|b| b.is_ascii_whitespace()) {
        return Err(crate::locale::t("文件是空的").into());
    }
    let text = decode_bytes(bytes, encoding)?;
    let delimiter = sniff_delimiter(&text);
    let rows = if delimiter == b' ' {
        parse_whitespace(&text)
    } else {
        parse_delimited(&text, delimiter)?
    };
    let rows: Vec<Vec<String>> = rows
        .into_iter()
        .filter(|row| row.iter().any(|cell| !cell.trim().is_empty()))
        .collect();
    if rows.is_empty() {
        return Err(crate::locale::t("没有读到数据行").into());
    }
    let header = is_header(&rows[0], rows.get(1));
    let (names, data) = if header {
        let width = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        let mut names = Vec::with_capacity(width);
        for index in 0..width {
            let name = rows[0]
                .get(index)
                .map(|s| s.trim())
                .filter(|s| !s.is_empty());
            names.push(name.unwrap_or("").to_string());
        }
        (names, &rows[1..])
    } else {
        let width = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        ((0..width).map(|_| String::new()).collect(), rows.as_slice())
    };
    let width = names
        .len()
        .max(data.iter().map(|r| r.len()).max().unwrap_or(0));
    if width == 0 {
        return Err(crate::locale::t("没有读到列").into());
    }
    let mut table = Table::new(TableId(0), crate::locale::t("导入"));
    table.columns.clear();
    for index in 0..width {
        let mut name = names.get(index).cloned().unwrap_or_default();
        if name.is_empty() {
            name = crate::table::column_letter(index);
        }
        let mode = if index == 0 {
            ColumnMode::X
        } else {
            ColumnMode::Y
        };
        table.add_column(name, mode);
        let cells: Vec<String> = data
            .iter()
            .map(|row| row.get(index).cloned().unwrap_or_default())
            .collect();
        let ty = infer_type(&cells);
        table.columns[index].ty = ty;
        table.columns[index].resize(cells.len());
        for (row, cell) in cells.iter().enumerate() {
            table.columns[index].set_text(row, cell);
        }
    }
    Ok(table)
}

pub fn export_csv(table: &Table) -> Vec<u8> {
    export_csv_with(table, CsvEncoding::Utf8)
}

/// 导出 CSV。UTF-8 和自动都带 BOM；GBK 不带 BOM，给只认 ANSI 的旧版 Excel。
pub fn export_csv_with(table: &Table, encoding: CsvEncoding) -> Vec<u8> {
    let mut writer = WriterBuilder::new().from_writer(Vec::new());
    let names: Vec<&str> = table.columns.iter().map(|c| c.name.as_str()).collect();
    writer.write_record(&names).ok();
    for row in 0..table.nrows() {
        let record: Vec<String> = table
            .columns
            .iter()
            .map(|column| column.display(row))
            .collect();
        writer.write_record(&record).ok();
    }
    let body = writer.into_inner().unwrap_or_default();
    let text = String::from_utf8_lossy(&body);
    match encoding {
        CsvEncoding::Gbk => encoding_rs::GBK.encode(&text).0.into_owned(),
        CsvEncoding::Auto | CsvEncoding::Utf8 => {
            let mut buffer = vec![0xEF, 0xBB, 0xBF];
            buffer.extend_from_slice(text.as_bytes());
            buffer
        }
    }
}

fn decode_bytes(bytes: &[u8], encoding: CsvEncoding) -> Result<String, String> {
    if encoding != CsvEncoding::Auto && (null_heavy(bytes) || forced_text_corrupt(bytes, encoding))
    {
        return Err(crate::t("指定的编码和文件对不上（文件里有很多空字节或替换字符）。请改回「自动」，或换成正确的编码。").into());
    }
    let text = match encoding {
        CsvEncoding::Utf8 => {
            let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
            String::from_utf8_lossy(bytes).into_owned()
        }
        CsvEncoding::Gbk => encoding_rs::GBK.decode(bytes).0.into_owned(),
        CsvEncoding::Auto => decode_auto(bytes),
    };
    if encoding == CsvEncoding::Auto && text_looks_corrupt(&text) {
        return Err(crate::t("无法识别文件编码。自动模式支持 UTF-8、GBK，以及带 BOM 或含空字节的 UTF-16。请在设置里指定编码，或从 Excel 另存为 CSV UTF-8。").into());
    }
    Ok(text)
}

fn forced_text_corrupt(bytes: &[u8], encoding: CsvEncoding) -> bool {
    let text = match encoding {
        CsvEncoding::Utf8 => {
            let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
            String::from_utf8_lossy(bytes).into_owned()
        }
        CsvEncoding::Gbk => encoding_rs::GBK.decode(bytes).0.into_owned(),
        CsvEncoding::Auto => return false,
    };
    text_looks_corrupt(&text)
}

fn decode_auto(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(&bytes[3..]).into_owned();
    }
    if let Some(text) = utf16_from_bom(bytes) {
        return text;
    }
    if null_heavy(bytes) {
        return decode_utf16(bytes, utf16_is_le(bytes));
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_string();
    }
    encoding_rs::GBK.decode(bytes).0.into_owned()
}

fn utf16_from_bom(bytes: &[u8]) -> Option<String> {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return Some(decode_utf16(&bytes[2..], true));
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return Some(decode_utf16(&bytes[2..], false));
    }
    None
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> String {
    let encoding = if little_endian {
        encoding_rs::UTF_16LE
    } else {
        encoding_rs::UTF_16BE
    };
    let (text, _, _) = encoding.decode(bytes);
    text.trim_start_matches('\u{FEFF}').to_string()
}

fn null_heavy(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes.iter().filter(|byte| **byte == 0).count() * 5 >= bytes.len()
}

fn utf16_is_le(bytes: &[u8]) -> bool {
    let odd = bytes
        .iter()
        .skip(1)
        .step_by(2)
        .filter(|byte| **byte == 0)
        .count();
    let even = bytes.iter().step_by(2).filter(|byte| **byte == 0).count();
    odd >= even
}

fn text_looks_corrupt(text: &str) -> bool {
    let mut count = 0usize;
    let mut bad = 0usize;
    for ch in text.chars() {
        count += 1;
        if ch == '\0' || ch == '\u{FFFD}' {
            bad += 1;
        }
    }
    count > 0 && bad >= 4 && bad * 5 >= count
}

fn sniff_delimiter(text: &str) -> u8 {
    let line = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    let candidates = [
        (b',', line.matches(',').count()),
        (b'\t', line.matches('\t').count()),
        (b';', line.matches(';').count()),
    ];
    let (delimiter, count) = candidates
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .unwrap();
    if count == 0 {
        b' '
    } else {
        delimiter
    }
}

fn parse_delimited(text: &str, delimiter: u8) -> Result<Vec<Vec<String>>, String> {
    let mut reader = ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|err| err.to_string())?;
        rows.push(record.iter().map(|cell| cell.to_string()).collect());
    }
    Ok(rows)
}

fn parse_whitespace(text: &str) -> Vec<Vec<String>> {
    text.lines()
        .map(|line| line.split_whitespace().map(|s| s.to_string()).collect())
        .filter(|row: &Vec<String>| !row.is_empty())
        .collect()
}

fn is_header(row: &[String], next: Option<&Vec<String>>) -> bool {
    let nonempty: Vec<&str> = row
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if nonempty.is_empty() {
        return false;
    }
    let numeric = nonempty
        .iter()
        .filter(|cell| cell.parse::<f64>().is_ok())
        .count();
    if numeric == nonempty.len() {
        return false;
    }
    if numeric == 0 {
        return true;
    }
    if let Some(next) = next {
        let next_nonempty = next.iter().filter(|s| !s.trim().is_empty()).count();
        let next_numeric = next
            .iter()
            .filter(|s| s.trim().parse::<f64>().is_ok())
            .count();
        return next_nonempty == 0 || next_numeric * 2 >= next_nonempty;
    }
    true
}

fn infer_type(cells: &[String]) -> ColumnType {
    let nonempty: Vec<&str> = cells
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if nonempty.is_empty() {
        return ColumnType::Numeric;
    }
    if nonempty
        .iter()
        .all(|cell| crate::util::parse_date(cell).is_some())
    {
        return ColumnType::Date;
    }
    if nonempty.iter().all(|cell| cell.parse::<f64>().is_ok()) {
        ColumnType::Numeric
    } else {
        ColumnType::Text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_and_gbk_roundtrip() {
        let bytes = crate::locale::t("名称,数值\n苹果,1.5\n香蕉,2\n").as_bytes();
        let table = import_csv(bytes).unwrap();
        assert_eq!(table.columns[0].name, "名称");
        assert_eq!(table.columns[0].texts[0], "苹果");
        assert_eq!(table.columns[1].numbers[1], Some(2.0));
        assert_eq!(table.columns[0].ty, ColumnType::Text);

        let (encoded, _, _) = encoding_rs::GBK.encode(crate::locale::t("名称,数值\n苹果,1.5\n"));
        let table = import_csv(encoded.as_ref()).unwrap();
        assert_eq!(table.columns[0].texts[0], "苹果");

        let exported = export_csv(&table);
        assert!(exported.starts_with(&[0xEF, 0xBB, 0xBF]));
        let again = import_csv(&exported).unwrap();
        assert_eq!(again.columns[0].texts[0], "苹果");
        assert_eq!(crate::format_number(1.5), again.columns[1].display(0));
    }

    #[test]
    fn gbk_can_be_forced_either_way() {
        let (encoded, _, _) = encoding_rs::GBK.encode(crate::locale::t("温度,电阻\n25,100\n"));
        let table = import_csv_with(encoded.as_ref(), CsvEncoding::Gbk).unwrap();
        assert_eq!(table.columns[0].name, "温度");
        assert_eq!(table.columns[1].numbers[0], Some(100.0));
        let out = export_csv_with(&table, CsvEncoding::Gbk);
        assert!(!out.starts_with(&[0xEF, 0xBB, 0xBF]));
        let (text, _, _) = encoding_rs::GBK.decode(&out);
        assert!(text.contains("温度"));
        assert!(text.contains("100"));
    }

    #[test]
    fn utf16_excel_with_and_without_bom() {
        let text = "温度,电阻\n25,100\n";
        let mut raw = Vec::new();
        for unit in text.encode_utf16() {
            raw.extend_from_slice(&unit.to_le_bytes());
        }
        let bare = import_csv(&raw).unwrap();
        assert_eq!(bare.columns[0].name, "温度");
        assert_eq!(bare.columns[1].numbers[0], Some(100.0));
        let mut with_bom = vec![0xFF, 0xFE];
        with_bom.extend_from_slice(&raw);
        let table = import_csv(&with_bom).unwrap();
        assert_eq!(table.columns[0].name, "温度");
        assert_eq!(table.columns[1].name, "电阻");
        assert_eq!(table.columns[1].numbers[0], Some(100.0));
    }

    #[test]
    fn forced_encoding_rejects_utf16_without_losing_real_gbk() {
        let text = "温度,电阻\n25,100\n";
        let mut raw = Vec::new();
        for unit in text.encode_utf16() {
            raw.extend_from_slice(&unit.to_le_bytes());
        }
        let gbk = import_csv_with(&raw, CsvEncoding::Gbk).unwrap_err();
        assert!(gbk.contains("自动") || gbk.contains("Auto"), "{gbk}");
        let utf8 = import_csv_with(&raw, CsvEncoding::Utf8).unwrap_err();
        assert!(utf8.contains("自动") || utf8.contains("Auto"), "{utf8}");
        let (encoded, _, _) = encoding_rs::GBK.encode(text);
        let table = import_csv_with(encoded.as_ref(), CsvEncoding::Gbk).unwrap();
        assert_eq!(table.columns[0].name, "温度");
    }

    #[test]
    fn numeric_file_has_no_header() {
        let table = import_csv(b"1,2\n3,4\n").unwrap();
        assert_eq!(table.nrows(), 2);
        assert_eq!(table.columns[0].numbers[0], Some(1.0));
        assert_eq!(table.columns[1].numbers[1], Some(4.0));
    }
}
