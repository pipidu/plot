use serde::{Deserialize, Serialize};

use crate::datetime::{self, TemporalKind};
use crate::tr;
use crate::util::{default_true, format_fixed, format_number, format_scientific};

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
            ColumnMode::XErr => tr!("x误差", "x error"),
            ColumnMode::YErr => tr!("y误差", "y error"),
            ColumnMode::Label => tr!("标签", "Label"),
            ColumnMode::None => tr!("无", "None"),
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
    Time,
    DateTime,
}

impl ColumnType {
    pub fn label(self) -> &'static str {
        match self {
            ColumnType::Numeric => tr!("数值", "Numeric"),
            ColumnType::Text => tr!("文本", "Text"),
            ColumnType::Date => tr!("日期", "Date"),
            ColumnType::Time => tr!("时刻", "Time"),
            ColumnType::DateTime => tr!("日期时间", "Date and time"),
        }
    }

    pub fn all() -> &'static [ColumnType] {
        &[
            ColumnType::Numeric,
            ColumnType::Text,
            ColumnType::Date,
            ColumnType::Time,
            ColumnType::DateTime,
        ]
    }

    pub fn temporal(self) -> Option<TemporalKind> {
        match self {
            ColumnType::Date => Some(TemporalKind::Date),
            ColumnType::Time => Some(TemporalKind::Time),
            ColumnType::DateTime => Some(TemporalKind::DateTime),
            ColumnType::Numeric | ColumnType::Text => None,
        }
    }
}

/// 数值列的显示方式。`General` 沿用原来的 `format_number`。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum NumericFormat {
    #[default]
    General,
    Fixed,
    Scientific,
}

impl NumericFormat {
    pub fn label(self) -> &'static str {
        match self {
            NumericFormat::General => tr!("自动", "General"),
            NumericFormat::Fixed => tr!("固定小数", "Fixed decimals"),
            NumericFormat::Scientific => tr!("科学计数", "Scientific"),
        }
    }

    pub fn all() -> &'static [NumericFormat] {
        &[NumericFormat::General, NumericFormat::Fixed, NumericFormat::Scientific]
    }
}

fn default_places() -> u8 {
    6
}

/// 排序作用的范围。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SortMode {
    /// 整表按一列重排，同一行的其他列跟着走。
    #[default]
    Whole,
    /// 所选列（以及排序键那一列）用同一个顺序一起重排，其余列不动。
    Together,
    /// 每一列按自己的值分别排序，列与列不再对齐。
    Separate,
}

impl SortMode {
    pub fn label(self) -> &'static str {
        match self {
            SortMode::Whole => tr!("整表", "Whole table"),
            SortMode::Together => tr!("所选列一起", "Selected columns together"),
            SortMode::Separate => tr!("所选列分别", "Selected columns separately"),
        }
    }

    pub fn all() -> &'static [SortMode] {
        &[SortMode::Whole, SortMode::Together, SortMode::Separate]
    }
}

/// 把所选数值列，或这些列放在一起，线性缩放到 \[0, 1\]。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NormalizeMode {
    /// 每一列用自己的最小和最大。
    Columns,
    /// 所有选中单元格共用一个最小和最大。
    Table,
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
    pub numeric_format: NumericFormat,
    #[serde(default = "default_places")]
    pub decimal_places: u8,
    /// 日期、时刻、日期时间的显示格式。空字符串用该类型的默认格式。
    pub time_format: String,
    /// 与行对齐。空字符串表示这一格没有自己的公式。
    pub cell_formulas: Vec<String>,
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
            numeric_format: NumericFormat::General,
            decimal_places: default_places(),
            time_format: String::new(),
            cell_formulas: Vec::new(),
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
        self.numbers.len().max(self.texts.len()).max(self.cell_formulas.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn normalize(&mut self) {
        let n = self.len();
        self.resize(n);
    }

    pub fn resize(&mut self, n: usize) {
        self.numbers.resize(n, None);
        self.texts.resize(n, String::new());
        self.cell_formulas.resize(n, String::new());
    }

    /// 这一格自己的公式。列公式不算在内。
    pub fn cell_formula(&self, row: usize) -> &str {
        self.cell_formulas.get(row).map(String::as_str).unwrap_or("")
    }

    /// 公式编辑模式里文本框应显示的内容：单元格公式优先，否则列公式，都没有则显示值。
    pub fn editor_text(&self, row: usize, formula_mode: bool) -> String {
        if formula_mode {
            let cell = self.cell_formula(row);
            if !cell.is_empty() {
                return cell.to_string();
            }
            if !self.formula.is_empty() {
                return self.formula.clone();
            }
        }
        self.display(row)
    }

    pub fn set_cell_formula_raw(&mut self, row: usize, text: &str) {
        if row >= self.len() {
            self.resize(row + 1);
        }
        self.cell_formulas[row] = text.to_string();
    }

    pub fn format_value(&self, value: f64) -> String {
        if let Some(kind) = self.ty.temporal() {
            return datetime::format_temporal(kind, value, &self.time_format);
        }
        if self.ty == ColumnType::Text {
            return format_number(value);
        }
        match self.numeric_format {
            NumericFormat::Fixed => format_fixed(value, self.decimal_places),
            NumericFormat::Scientific => format_scientific(value, self.decimal_places),
            NumericFormat::General => format_number(value),
        }
    }

    /// 用当前类型和格式，把已解析的数值重新写进显示文本。
    pub fn refresh_display(&mut self) {
        if self.ty == ColumnType::Text {
            return;
        }
        self.normalize();
        for row in 0..self.len() {
            if let Some(value) = self.numbers[row] {
                self.texts[row] = self.format_value(value);
            }
        }
    }

    /// 更换列类型。能从现有文本重新解析的就重新解析，否则按存储单位换算。
    pub fn set_type(&mut self, new_ty: ColumnType) {
        if self.ty == new_ty {
            return;
        }
        let old = self.ty;
        self.normalize();
        self.ty = new_ty;
        for row in 0..self.len() {
            let text = self.texts[row].clone();
            if let Some(parsed) = parse_user_number(new_ty, &text, &self.time_format) {
                self.numbers[row] = Some(parsed);
            } else if let Some(value) = self.numbers[row] {
                self.numbers[row] = convert_stored(old, new_ty, value);
            }
        }
        self.refresh_display();
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
        self.numbers[row] = parse_user_number(self.ty, text, &self.time_format);
    }

    pub fn set_numbers(&mut self, values: &[f64]) {
        let texts: Vec<String> = values
            .iter()
            .map(|v| if v.is_finite() { self.format_value(*v) } else { String::new() })
            .collect();
        self.numbers = values
            .iter()
            .map(|v| if v.is_finite() { Some(*v) } else { None })
            .collect();
        self.texts = texts;
        self.cell_formulas.resize(values.len(), String::new());
    }

    pub fn finite_values(&self) -> Vec<f64> {
        self.numbers.iter().copied().flatten().filter(|v| v.is_finite()).collect()
    }
}

pub fn parse_user_number(ty: ColumnType, text: &str, pattern: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let parsed = if let Some(kind) = ty.temporal() {
        datetime::parse_temporal(kind, text, pattern)
    } else {
        text.parse::<f64>().ok()
    };
    parsed.filter(|v| v.is_finite())
}

fn convert_stored(from: ColumnType, to: ColumnType, value: f64) -> Option<f64> {
    if !value.is_finite() {
        return None;
    }
    let seconds_of_day = match from {
        ColumnType::Time => value.rem_euclid(86_400.0),
        ColumnType::Date | ColumnType::DateTime => (value - value.floor()) * 86_400.0,
        ColumnType::Numeric | ColumnType::Text => value,
    };
    let converted = match to {
        ColumnType::Time => {
            if matches!(from, ColumnType::Date | ColumnType::DateTime) {
                seconds_of_day.rem_euclid(86_400.0)
            } else {
                value
            }
        }
        ColumnType::Date | ColumnType::DateTime => {
            if from == ColumnType::Time {
                seconds_of_day / 86_400.0
            } else {
                value
            }
        }
        ColumnType::Numeric | ColumnType::Text => value,
    };
    converted.is_finite().then_some(converted)
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
    /// 所在文件夹。空表示项目根。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<crate::FolderId>,
}

impl Default for Table {
    fn default() -> Self {
        Self {
            id: TableId(0),
            name: crate::locale::t("表").to_string(),
            columns: Vec::new(),
            comment: String::new(),
            show_headers: true,
            folder: None,
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
            column.cell_formulas.insert(index, String::new());
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
                column.cell_formulas.remove(row);
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
        self.sort_columns(&[col], col, ascending, SortMode::Whole);
    }

    /// `Whole` 重排整表。`Together` 只重排所选列和排序键。`Separate` 让每一列按自己的值排序。
    pub fn sort_columns(&mut self, columns: &[usize], key: usize, ascending: bool, mode: SortMode) {
        if self.columns.is_empty() {
            return;
        }
        match mode {
            SortMode::Whole => {
                if key >= self.columns.len() {
                    return;
                }
                let order = self.row_order(key, ascending);
                for column in &mut self.columns {
                    apply_order(column, &order);
                }
            }
            SortMode::Together => {
                if key >= self.columns.len() {
                    return;
                }
                let mut moving: Vec<usize> = columns.iter().copied().filter(|index| *index < self.columns.len()).collect();
                if !moving.contains(&key) {
                    moving.push(key);
                }
                let order = self.row_order(key, ascending);
                for index in moving {
                    apply_order(&mut self.columns[index], &order);
                }
            }
            SortMode::Separate => {
                let targets: Vec<usize> = columns.iter().copied().filter(|index| *index < self.columns.len()).collect();
                for index in targets {
                    let order = self.row_order(index, ascending);
                    apply_order(&mut self.columns[index], &order);
                }
            }
        }
    }

    /// 用 1、2、3… 填满这一列。表是空的时先造 10 行。已有的列公式会被清掉，避免重算把填好的数盖掉。
    pub fn fill_row_numbers(&mut self, col: usize) -> Result<(), String> {
        self.fill_generated(col, true, 1)
    }

    /// 用 \[0, 1) 上的均匀随机数填满这一列。同一个 `seed` 得到同一串数。
    pub fn fill_random(&mut self, col: usize, seed: u64) -> Result<(), String> {
        self.fill_generated(col, false, seed)
    }

    fn fill_generated(&mut self, col: usize, rows: bool, seed: u64) -> Result<(), String> {
        if col >= self.columns.len() {
            return Err(crate::locale::t("找不到列").into());
        }
        let n = self.nrows().max(1);
        let n = if self.nrows() == 0 { 10 } else { n };
        self.ensure_rows(n);
        let mut state = seed | 1;
        let values: Vec<f64> = if rows {
            (1..=n).map(|i| i as f64).collect()
        } else {
            (0..n).map(|_| next_unit(&mut state)).collect()
        };
        let column = &mut self.columns[col];
        column.ty = ColumnType::Numeric;
        column.formula.clear();
        column.cell_formulas.clear();
        column.set_numbers(&values);
        if column.len() < n {
            column.resize(n);
        }
        Ok(())
    }

    /// 把数值列线性缩放到 \[0, 1\]。最大值等于最小值时写成 0。文本、日期和时刻列跳过。
    pub fn normalize_unit(&mut self, columns: &[usize], mode: NormalizeMode) -> Result<usize, String> {
        let indexes: Vec<usize> = columns
            .iter()
            .copied()
            .filter(|index| {
                self.columns
                    .get(*index)
                    .is_some_and(|column| column.ty == ColumnType::Numeric && column.numbers.iter().flatten().any(|v| v.is_finite()))
            })
            .collect();
        if indexes.is_empty() {
            return Err(crate::locale::t("没有可归一化的数值列").into());
        }
        let span_of = |values: &[f64]| {
            let min = values.iter().copied().fold(f64::INFINITY, f64::min);
            let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            (min, max)
        };
        let global = if mode == NormalizeMode::Table {
            let mut values = Vec::new();
            for index in &indexes {
                values.extend(self.columns[*index].finite_values());
            }
            Some(span_of(&values))
        } else {
            None
        };
        for index in &indexes {
            let (min, max) = global.unwrap_or_else(|| span_of(&self.columns[*index].finite_values()));
            let span = max - min;
            let column = &mut self.columns[*index];
            column.formula.clear();
            for formula in &mut column.cell_formulas {
                formula.clear();
            }
            column.normalize();
            for slot in &mut column.numbers {
                if let Some(value) = *slot {
                    let scaled = if span.abs() < 1e-15 { 0.0 } else { (value - min) / span };
                    *slot = Some(scaled);
                }
            }
            column.refresh_display();
        }
        Ok(indexes.len())
    }

    /// 把一块单元格写成制表符分隔的文本，方便贴进 Excel 或另一张表。
    pub fn copy_tsv(&self, row: usize, col: usize, nrows: usize, ncols: usize) -> String {
        let mut lines = Vec::with_capacity(nrows);
        for r in 0..nrows {
            let mut cells = Vec::with_capacity(ncols);
            for c in 0..ncols {
                let text = self
                    .columns
                    .get(col + c)
                    .map(|column| column.display(row + r))
                    .unwrap_or_default();
                cells.push(text);
            }
            lines.push(cells.join("\t"));
        }
        lines.join("\n")
    }

    /// 从 `row`,`col` 起粘贴制表符或逗号分隔的文本。不够的行列会补上。
    pub fn paste_text(&mut self, row: usize, col: usize, text: &str) -> Result<(usize, usize), String> {
        let grid = parse_clipboard(text);
        if grid.is_empty() || grid.iter().all(|line| line.iter().all(|cell| cell.is_empty())) {
            return Err(crate::locale::t("剪贴板里没有单元格").into());
        }
        let nrows = grid.len();
        let ncols = grid.iter().map(|line| line.len()).max().unwrap_or(0);
        if nrows > 20_000 || ncols > 512 || nrows.saturating_mul(ncols) > 200_000 {
            return Err(crate::locale::t("一次粘贴的单元格太多").into());
        }
        while self.columns.len() < col + ncols {
            let index = self.columns.len();
            let mode = if index == 0 { ColumnMode::X } else { ColumnMode::Y };
            self.add_column(column_letter(index), mode);
        }
        self.ensure_rows(row + nrows);
        for (r, line) in grid.iter().enumerate() {
            for (c, cell) in line.iter().enumerate() {
                let column = &mut self.columns[col + c];
                if row + r < column.cell_formulas.len() {
                    column.cell_formulas[row + r].clear();
                }
                column.set_text(row + r, cell);
            }
        }
        Ok((nrows, ncols))
    }

    fn row_order(&mut self, col: usize, ascending: bool) -> Vec<usize> {
        let n = self.nrows();
        self.ensure_rows(n);
        let mut order: Vec<usize> = (0..n).collect();
        let key_col = &self.columns[col];
        order.sort_by(|&a, &b| cmp_cells(key_col, a, b, ascending));
        order
    }
}

fn cmp_cells(column: &Column, a: usize, b: usize, ascending: bool) -> std::cmp::Ordering {
    let left_num = column.numbers.get(a).copied().flatten();
    let right_num = column.numbers.get(b).copied().flatten();
    let left_text = column.texts.get(a).map(String::as_str).unwrap_or("");
    let right_text = column.texts.get(b).map(String::as_str).unwrap_or("");
    let left_empty = left_num.is_none() && left_text.trim().is_empty();
    let right_empty = right_num.is_none() && right_text.trim().is_empty();
    // 空单元格在升序和降序里都留在最后。降序只翻转有值的比较，不再把空值翻到前面。
    match (left_empty, right_empty) {
        (true, true) => std::cmp::Ordering::Equal,
        (true, false) => std::cmp::Ordering::Greater,
        (false, true) => std::cmp::Ordering::Less,
        (false, false) => {
            let cmp = cmp_optional(left_num, right_num).then_with(|| left_text.cmp(right_text));
            if ascending {
                cmp
            } else {
                cmp.reverse()
            }
        }
    }
}

fn apply_order(column: &mut Column, order: &[usize]) {
    column.normalize();
    if column.len() < order.len() {
        column.resize(order.len());
    }
    reorder(&mut column.numbers, order);
    reorder(&mut column.texts, order);
    reorder(&mut column.cell_formulas, order);
}

fn next_unit(state: &mut u64) -> f64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    ((x >> 11) as f64) / ((1u64 << 53) as f64)
}

fn parse_clipboard(text: &str) -> Vec<Vec<String>> {
    let text = text.trim_matches(['\u{feff}', '\r', '\n']);
    if text.is_empty() {
        return Vec::new();
    }
    let lines: Vec<&str> = text.split('\n').map(|line| line.trim_end_matches('\r')).collect();
    let tabs = lines.iter().any(|line| line.contains('\t'));
    let commas = !tabs && lines.iter().any(|line| line.contains(','));
    lines
        .into_iter()
        .map(|line| {
            if tabs {
                line.split('\t').map(|cell| cell.to_string()).collect()
            } else if commas {
                split_csv_line(line)
            } else {
                vec![line.to_string()]
            }
        })
        .collect()
}

fn split_csv_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' if !quoted => quoted = true,
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                current.push('"');
            }
            '"' if quoted => quoted = false,
            ',' if !quoted => out.push(std::mem::take(&mut current)),
            _ => current.push(ch),
        }
    }
    out.push(current);
    out
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

    #[test]
    fn time_datetime_formats_and_numeric_places() {
        let mut column = Column::new("t", ColumnMode::X);
        column.set_type(ColumnType::DateTime);
        column.time_format = "YYYY/MM/DD HH:mm".into();
        column.set_text(0, "2020-01-02 03:04:05");
        column.refresh_display();
        assert!(column.numbers[0].unwrap() > 0.0);
        assert_eq!(column.display(0), "2020/01/02 03:04");

        column.time_format.clear();
        column.set_type(ColumnType::Time);
        assert_eq!(column.display(0), "03:04:05");

        let mut numbers = Column::new("n", ColumnMode::Y);
        numbers.numeric_format = NumericFormat::Fixed;
        numbers.decimal_places = 2;
        numbers.set_numbers(&[1.5, 2.0]);
        assert_eq!(numbers.display(0), "1.50");
        assert_eq!(numbers.display(1), "2.00");
        numbers.numeric_format = NumericFormat::Scientific;
        numbers.decimal_places = 1;
        numbers.refresh_display();
        assert!(numbers.display(0).contains('e'), "{}", numbers.display(0));

        let json = serde_json::to_string(&Column::new("A", ColumnMode::X)).unwrap();
        let loaded: Column = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.decimal_places, 6);
        let legacy = r#"{"name":"A","ty":"Date","mode":"X","numbers":[0.0],"texts":["1970-01-01"],"formula":"","comment":""}"#;
        let old: Column = serde_json::from_str(legacy).unwrap();
        assert_eq!(old.ty, ColumnType::Date);
        assert!(old.cell_formulas.is_empty());
        assert_eq!(old.display(0), "1970-01-01");
    }

    #[test]
    fn fill_sort_normalize_and_paste() {
        let mut table = Table::new(TableId(1), "t");
        table.columns.clear();
        table.add_column("A", ColumnMode::X);
        table.add_column("B", ColumnMode::Y);
        table.add_column("C", ColumnMode::None);
        table.ensure_rows(3);
        table.columns[0].set_numbers(&[3.0, 1.0, 2.0]);
        table.columns[1].set_numbers(&[30.0, 10.0, 20.0]);
        table.columns[2].set_text(0, "a");
        table.columns[2].set_text(1, "b");
        table.columns[2].set_text(2, "c");

        table.sort_columns(&[0, 1], 0, true, SortMode::Together);
        assert_eq!(table.columns[0].finite_values(), vec![1.0, 2.0, 3.0]);
        assert_eq!(table.columns[1].finite_values(), vec![10.0, 20.0, 30.0]);
        assert_eq!(table.columns[2].texts, vec!["a".to_string(), "b".to_string(), "c".to_string()]);

        table.columns[0].set_numbers(&[3.0, 1.0, 2.0]);
        table.columns[1].set_numbers(&[30.0, 10.0, 20.0]);
        table.sort_columns(&[0, 1], 0, true, SortMode::Separate);
        assert_eq!(table.columns[0].finite_values(), vec![1.0, 2.0, 3.0]);
        assert_eq!(table.columns[1].finite_values(), vec![10.0, 20.0, 30.0]);
        assert_eq!(table.columns[2].texts[0], "a");

        table.fill_row_numbers(0).unwrap();
        assert_eq!(table.columns[0].finite_values(), vec![1.0, 2.0, 3.0]);
        table.fill_random(1, 7).unwrap();
        let again = {
            let mut other = table.clone();
            other.fill_random(1, 7).unwrap();
            other.columns[1].finite_values()
        };
        assert_eq!(table.columns[1].finite_values(), again);
        assert!(table.columns[1].finite_values().iter().all(|v| (0.0..1.0).contains(v) || *v == 0.0));

        table.columns[0].set_numbers(&[2.0, 4.0, 6.0]);
        table.columns[1].set_numbers(&[0.0, 10.0, 5.0]);
        table.normalize_unit(&[0], NormalizeMode::Columns).unwrap();
        let scaled = table.columns[0].finite_values();
        assert!((scaled[0]).abs() < 1e-12 && (scaled[1] - 0.5).abs() < 1e-12 && (scaled[2] - 1.0).abs() < 1e-12);

        table.columns[0].set_numbers(&[0.0, 10.0]);
        table.columns[1].set_numbers(&[5.0, 5.0]);
        table.normalize_unit(&[0, 1], NormalizeMode::Table).unwrap();
        assert!((table.columns[1].number_at(0) - 0.5).abs() < 1e-12);

        let pasted = table.paste_text(0, 0, "9\t8\n7\t6\r\n").unwrap();
        assert_eq!(pasted, (2, 2));
        assert_eq!(table.columns[0].number_at(0), 9.0);
        assert_eq!(table.columns[1].number_at(1), 6.0);
        let copied = table.copy_tsv(0, 0, 2, 2);
        assert_eq!(copied, "9\t8\n7\t6");
    }

    #[test]
    fn empty_cells_stay_last_when_sorting_descending() {
        for mode in [SortMode::Whole, SortMode::Together, SortMode::Separate] {
            let mut table = Table::new(TableId(1), "t");
            table.columns.clear();
            table.add_column("A", ColumnMode::X);
            table.add_column("B", ColumnMode::Y);
            table.ensure_rows(3);
            table.columns[0].numbers = vec![Some(1.0), Some(2.0), None];
            table.columns[0].texts = vec!["1".into(), "2".into(), String::new()];
            table.columns[0].cell_formulas = vec![String::new(), "keep".into(), String::new()];
            table.columns[1].set_numbers(&[10.0, 20.0, 30.0]);
            let columns: &[usize] = if mode == SortMode::Whole { &[] } else { &[0, 1] };
            table.sort_columns(columns, 0, false, mode);
            assert_eq!(
                table.columns[0].numbers,
                vec![Some(2.0), Some(1.0), None],
                "{mode:?}"
            );
            if mode != SortMode::Separate {
                assert_eq!(table.columns[1].numbers, vec![Some(20.0), Some(10.0), Some(30.0)], "{mode:?}");
                assert_eq!(table.columns[0].cell_formulas[0], "keep");
            }
        }
    }
}
