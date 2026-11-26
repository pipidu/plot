use serde::{Deserialize, Serialize};

use crate::util::{default_true, format_number, parse_date};

/// 表格标识。用新类型避免和矩阵、图形的 id 混用。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TableId(pub u64);

impl TableId {
    pub fn raw(self) -> u64 {
        self.0
    }
}

/// 列在绘图时扮演的角色，对应 SciDAVis 的 X/Y/Z/误差/标签。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ColumnMode {
    X,
    #[default]
    Y,
    Z,
    XErr,
    YErr,
    Label,
    None,
}

impl ColumnMode {
    pub fn label(self) -> &'static str {
        match self {
            ColumnMode::X => "X",
            ColumnMode::Y => "Y",
            ColumnMode::Z => "Z",
            ColumnMode::XErr => "x误差",
            ColumnMode::YErr => "y误差",
            ColumnMode::Label => "标签",
            ColumnMode::None => "无",
        }
    }

    pub fn all() -> &'static [ColumnMode] {
        &[
            ColumnMode::X,
            ColumnMode::Y,
            ColumnMode::Z,
            ColumnMode::XErr,
            ColumnMode::YErr,
            ColumnMode::Label,
            ColumnMode::None,
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ColumnType {
    #[default]
    Numeric,
    Text,
    Date,
}

impl ColumnType {
    pub fn label(self) -> &'static str {
        match self {
            ColumnType::Numeric => "数值",
            ColumnType::Text => "文本",
            ColumnType::Date => "日期",
        }
    }

    pub fn all() -> &'static [ColumnType] {
        &[ColumnType::Numeric, ColumnType::Text, ColumnType::Date]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Column {
    pub name: String,
    pub ty: ColumnType,
    pub mode: ColumnMode,
    pub numbers: Vec<Option<f64>>,
    pub texts: Vec<String>,
    pub formula: String,
    pub comment: String,
}

impl Default for Column {
    fn default() -> Self {
        Self {
            name: "A".to_string(),
            ty: ColumnType::Numeric,
            mode: ColumnMode::Y,
            numbers: Vec::new(),
            texts: Vec::new(),
            formula: String::new(),
            comment: String::new(),
        }
    }
}

impl Column {
    pub fn new(name: impl Into<String>, mode: ColumnMode) -> Self {
        Self {
            name: name.into(),
            mode,
            ..Self::default()
        }
    }

    pub fn len(&self) -> usize {
        self.numbers.len().max(self.texts.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn normalize(&mut self) {
        let n = self.len();
        self.numbers.resize(n, None);
        self.texts.resize(n, String::new());
    }

    pub fn resize(&mut self, n: usize) {
        self.numbers.resize(n, None);
        self.texts.resize(n, String::new());
    }

    pub fn display(&self, row: usize) -> String {
        if row >= self.texts.len() && row >= self.numbers.len() {
            return String::new();
        }
        if row < self.texts.len() && !self.texts[row].is_empty() {
            return self.texts[row].clone();
        }
        self.numbers
            .get(row)
            .copied()
            .flatten()
            .map(format_number)
            .unwrap_or_default()
    }

    pub fn number_at(&self, row: usize) -> f64 {
        self.numbers
            .get(row)
            .copied()
            .flatten()
            .unwrap_or(f64::NAN)
    }

    /// 写入用户输入。文本原样保留，同时尽量解析成数值（日期列解析成距 1970-01-01 的天数）。
    pub fn set_text(&mut self, row: usize, text: &str) {
        if row >= self.len() {
            self.resize(row + 1);
        }
        self.texts[row] = text.to_string();
        self.numbers[row] = parse_user_number(self.ty, text);
    }

    pub fn set_numbers(&mut self, values: &[f64]) {
        self.numbers = values
            .iter()
            .map(|v| if v.is_finite() { Some(*v) } else { None })
            .collect();
        self.texts = values
            .iter()
            .map(|v| if v.is_finite() { format_number(*v) } else { String::new() })
            .collect();
    }

    pub fn finite_values(&self) -> Vec<f64> {
        self.numbers.iter().copied().flatten().filter(|v| v.is_finite()).collect()
    }
}

pub fn parse_user_number(ty: ColumnType, text: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    match ty {
        ColumnType::Date => parse_date(text).or_else(|| text.parse::<f64>().ok()),
        ColumnType::Numeric | ColumnType::Text => text.parse::<f64>().ok(),
    }
    .filter(|v| v.is_finite())
}

/// 0 -> A, 25 -> Z, 26 -> AA，与电子表格列标一致。
pub fn column_letter(index: usize) -> String {
    let mut n = index + 1;
    let mut out = String::new();
    while n > 0 {
        n -= 1;
        out.insert(0, char::from(b'A' + (n % 26) as u8));
        n /= 26;
    }
    out
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Table {
    pub id: TableId,
    pub name: String,
    pub columns: Vec<Column>,
    pub comment: String,
    #[serde(default = "default_true")]
    pub show_headers: bool,
}

impl Default for Table {
    fn default() -> Self {
        Self {
            id: TableId(0),
            name: "表".to_string(),
            columns: Vec::new(),
            comment: String::new(),
            show_headers: true,
        }
    }
}

impl Table {
    pub fn new(id: TableId, name: impl Into<String>) -> Self {
        let mut table = Self {
            id,
            name: name.into(),
            ..Self::default()
        };
        table.add_column("A", ColumnMode::X);
        table.add_column("B", ColumnMode::Y);
        table
    }

    pub fn nrows(&self) -> usize {
        self.columns.iter().map(Column::len).max().unwrap_or(0)
    }

    pub fn normalize(&mut self) {
        let n = self.nrows();
        for column in &mut self.columns {
            column.normalize();
            column.resize(n);
        }
    }

    pub fn ensure_rows(&mut self, n: usize) {
        for column in &mut self.columns {
            if column.len() < n {
                column.resize(n);
            }
        }
    }

    pub fn add_column(&mut self, name: impl Into<String>, mode: ColumnMode) -> usize {
        let n = self.nrows();
        let mut column = Column::new(name, mode);
        column.resize(n);
        self.columns.push(column);
        self.columns.len() - 1
    }

    pub fn add_numeric_column(&mut self, name: impl Into<String>, mode: ColumnMode, values: &[f64]) -> usize {
        let idx = self.add_column(name, mode);
        let n = self.nrows().max(values.len());
        self.ensure_rows(n);
        self.columns[idx].set_numbers(values);
        if self.columns[idx].len() < n {
            self.columns[idx].resize(n);
        }
        idx
    }

    pub fn add_row(&mut self) {
        let n = self.nrows() + 1;
        self.ensure_rows(n);
    }

    /// 在 `index` 处插入一行。超出末尾时加在最后。没有列时不做任何事。
    pub fn insert_row(&mut self, index: usize) {
        if self.columns.is_empty() {
            return;
        }
        let n = self.nrows();
        self.ensure_rows(n);
        let index = index.min(self.nrows());
        for column in &mut self.columns {
            column.normalize();
            column.numbers.insert(index, None);
            column.texts.insert(index, String::new());
        }
    }

    /// 在 `index` 处插入一列，并补齐到当前行数。
    pub fn insert_column(&mut self, index: usize, name: impl Into<String>, mode: ColumnMode) -> usize {
        let n = self.nrows();
        let mut column = Column::new(name, mode);
        column.resize(n);
        let index = index.min(self.columns.len());
        self.columns.insert(index, column);
        index
    }

    pub fn remove_row(&mut self, row: usize) {
        for column in &mut self.columns {
            column.normalize();
            if row < column.len() {
                column.numbers.remove(row);
                column.texts.remove(row);
            }
        }
    }

    pub fn remove_column(&mut self, col: usize) -> bool {
        if col < self.columns.len() {
            self.columns.remove(col);
            true
        } else {
            false
        }
    }

    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name == name)
    }

    /// 按某一列排序，其他列的同一行一起移动。空值排在最后。
    pub fn sort_by(&mut self, col: usize, ascending: bool) {
        if col >= self.columns.len() {
            return;
        }
        let n = self.nrows();
        self.ensure_rows(n);
        let mut order: Vec<usize> = (0..n).collect();
        let key_col = &self.columns[col];
        order.sort_by(|&a, &b| {
            let cmp = cmp_optional(key_col.numbers.get(a).copied().flatten(), key_col.numbers.get(b).copied().flatten())
                .then_with(|| key_col.texts[a].cmp(&key_col.texts[b]));
            if ascending {
                cmp
            } else {
                cmp.reverse()
            }
        });
        for column in &mut self.columns {
            reorder(&mut column.numbers, &order);
            reorder(&mut column.texts, &order);
        }
    }
}

fn cmp_optional(a: Option<f64>, b: Option<f64>) -> std::cmp::Ordering {
    match (a, b) {
        (Some(a), Some(b)) => a.total_cmp(&b),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

fn reorder<T: Clone>(values: &mut [T], order: &[usize]) {
    let old = values.to_vec();
    for (i, &src) in order.iter().enumerate() {
        values[i] = old[src].clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters() {
        assert_eq!(column_letter(0), "A");
        assert_eq!(column_letter(25), "Z");
        assert_eq!(column_letter(26), "AA");
    }

    #[test]
    fn sort_keeps_rows_together() {
        let mut table = Table::new(TableId(1), "t");
        table.columns.clear();
        table.add_numeric_column("X", ColumnMode::X, &[3.0, 1.0, 2.0]);
        table.add_numeric_column("Y", ColumnMode::Y, &[30.0, 10.0, 20.0]);
        table.sort_by(0, true);
        assert_eq!(table.columns[0].numbers, vec![Some(1.0), Some(2.0), Some(3.0)]);
        assert_eq!(table.columns[1].numbers, vec![Some(10.0), Some(20.0), Some(30.0)]);
    }

    #[test]
    fn insert_row_and_column_keep_alignment() {
        let mut table = Table::new(TableId(1), "t");
        table.columns.clear();
        table.add_numeric_column("X", ColumnMode::X, &[1.0, 2.0]);
        table.insert_row(1);
        assert_eq!(table.columns[0].numbers, vec![Some(1.0), None, Some(2.0)]);
        table.insert_column(0, "A", ColumnMode::Y);
        assert_eq!(table.columns[0].name, "A");
        assert_eq!(table.columns[0].len(), 3);
        assert_eq!(table.columns[1].name, "X");
        assert_eq!(table.columns[1].numbers[2], Some(2.0));
        table.insert_row(99);
        assert_eq!(table.nrows(), 4);
    }
}
