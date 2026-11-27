//! 列公式与拟合表达式。
//!
//! 语法是 evalexpr：`+ - * / % ^`、括号、比较运算。
//! 另外注册了 SciDAVis / muParser 风格的函数名：`sin(x)`、`ln(x)`、`log(x)`（以 10 为底）。
//! 列公式里可以用 `col("列名")`、`col("A")`（列名优先，没有同名列时才按字母列标）、`col(1)`（从 1 开始的位置）、`cell("A", 行号)`、`i`（从 1 开始的行号），以及 `tablecol("表","列")` 读另一张表的同一行。
//! 某一格还可以有自己的公式，算的时候盖过这一列的公式。
//! 整数相除会截断，需要小数时写成 `1.0/2`。

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use evalexpr::{
    build_operator_tree, ContextWithMutableFunctions, ContextWithMutableVariables, EvalexprError,
    Function, HashMapContext, Node, Value,
};

use crate::table::{ColumnType, Table};

struct ColumnStore {
    names: HashMap<String, usize>,
    values: Vec<Vec<Option<f64>>>,
}

thread_local! {
    static COLS: RefCell<ColumnStore> = RefCell::new(ColumnStore {
        names: HashMap::new(),
        values: Vec::new(),
    });
    static TABLES: RefCell<HashMap<String, ColumnStore>> = RefCell::new(HashMap::new());
    static ROW: Cell<usize> = const { Cell::new(0) };
    static MATH: RefCell<Option<HashMapContext>> = const { RefCell::new(None) };
}

const FUNCTIONS: &[&str] = &[
    "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "sinh", "cosh", "tanh", "asinh", "acosh",
    "atanh", "exp", "ln", "log", "log10", "log2", "sqrt", "abs", "floor", "ceil", "round", "hypot",
    "sign", "pow", "min", "max", "cbrt", "col", "cell", "tablecol",
];

pub struct CompiledExpr {
    tree: Node,
}

impl CompiledExpr {
    pub fn compile(expr: &str) -> Result<Self, String> {
        let expr = expr.trim();
        if expr.is_empty() {
            return Err(crate::locale::t("表达式为空").to_string());
        }
        let tree = build_operator_tree(expr).map_err(|err| err.to_string())?;
        Ok(Self { tree })
    }

    pub fn eval(&self, vars: &[(&str, f64)]) -> Result<f64, String> {
        eval_tree(&self.tree, vars)
    }
}

pub struct EvalColumn {
    pub values: Vec<Option<f64>>,
    pub warnings: usize,
    pub first_error: Option<String>,
}

/// 逐行计算公式。空单元格和计算失败的行记为 `None`，不中断其余行。
pub fn eval_column(table: &Table, formula: &str) -> Result<EvalColumn, String> {
    eval_column_with(table, formula, &[])
}

/// 和 `eval_column` 一样，另外可以用 `tablecol("表","列")` 读其他表的同一行。
pub fn eval_column_with(table: &Table, formula: &str, others: &[Table]) -> Result<EvalColumn, String> {
    let compiled = CompiledExpr::compile(formula)?;
    publish(table, others);
    let n = table.nrows();
    let mut values = Vec::with_capacity(n);
    let mut warnings = 0;
    let mut first_error = None;
    for row in 0..n {
        ROW.with(|r| r.set(row));
        let row_number = row as f64 + 1.0;
        match compiled.eval(&[("i", row_number), ("row", row_number)]) {
            Ok(value) if value.is_finite() => values.push(Some(value)),
            Ok(_) => {
                warnings += 1;
                values.push(None);
            }
            Err(err) => {
                warnings += 1;
                if first_error.is_none() {
                    first_error = Some(crate::trf!("第 {} 行：{err}", "Row {}: {err}", row + 1, err = err));
                }
                values.push(None);
            }
        }
    }
    Ok(EvalColumn {
        values,
        warnings,
        first_error,
    })
}

/// 把公式结果写回列，并记下公式文本。这一列上的单元格公式会被清掉。
pub fn apply_formula(table: &mut Table, column: usize, formula: &str) -> Result<EvalColumn, String> {
    apply_formula_with(table, column, formula, &[])
}

/// 同 `apply_formula`，计算时能看见 `others` 里的表。
pub fn apply_formula_with(table: &mut Table, column: usize, formula: &str, others: &[Table]) -> Result<EvalColumn, String> {
    let evaluated = eval_column_with(table, formula, others)?;
    if column >= table.columns.len() {
        return Err(crate::locale::t("列不存在").to_string());
    }
    let n = evaluated.values.len();
    table.ensure_rows(n.max(table.nrows()));
    table.columns[column].ty = ColumnType::Numeric;
    let texts: Vec<String> = evaluated
        .values
        .iter()
        .map(|value| value.map(|number| table.columns[column].format_value(number)).unwrap_or_default())
        .collect();
    let col = &mut table.columns[column];
    col.formula = formula.to_string();
    col.numbers = evaluated.values.clone();
    col.texts = texts;
    col.cell_formulas = vec![String::new(); n];
    Ok(evaluated)
}

#[derive(Clone, Debug, Default)]
pub struct RecalcReport {
    pub written: usize,
    pub warnings: usize,
    pub first_error: Option<String>,
}

/// 先按从左到右重算列公式（已有单元格公式的格子跳过），再按行重算单元格公式。
/// 后面的公式能看见前面刚写进去的数。`tablecol` 能读 `others`。
pub fn recalculate_table(table: &mut Table, others: &[Table]) -> RecalcReport {
    let mut report = RecalcReport::default();
    let rows = table.nrows();
    publish(table, others);
    for index in 0..table.columns.len() {
        let formula = table.columns[index].formula.trim().to_string();
        if formula.is_empty() {
            continue;
        }
        let compiled = match CompiledExpr::compile(&formula) {
            Ok(compiled) => compiled,
            Err(err) => {
                note_formula_error(&mut report, &err);
                continue;
            }
        };
        for row in 0..rows {
            if !table.columns[index].cell_formula(row).trim().is_empty() {
                continue;
            }
            write_evaluated(&mut table.columns[index], row, eval_at(&compiled, row), &mut report);
        }
        publish(table, others);
    }
    for index in 0..table.columns.len() {
        for row in 0..rows {
            let raw = table.columns[index].cell_formula(row).trim().to_string();
            if raw.is_empty() {
                continue;
            }
            let expr = raw.trim_start_matches('=').trim().to_string();
            table.columns[index].cell_formulas[row] = expr.clone();
            if expr.is_empty() {
                continue;
            }
            let compiled = match CompiledExpr::compile(&expr) {
                Ok(compiled) => compiled,
                Err(err) => {
                    note_formula_error(&mut report, &err);
                    continue;
                }
            };
            write_evaluated(&mut table.columns[index], row, eval_at(&compiled, row), &mut report);
            publish(table, others);
        }
    }
    report
}

/// 结束编辑一个格子。公式模式，或以 `=` 开头的文本，写成这一格自己的公式并立刻计算。
pub fn commit_cell(table: &mut Table, column: usize, row: usize, formula_mode: bool, others: &[Table]) -> Result<(), String> {
    if column >= table.columns.len() {
        return Err(crate::locale::t("列不存在").into());
    }
    table.ensure_rows(row + 1);
    let raw = if formula_mode {
        table.columns[column].cell_formula(row).to_string()
    } else {
        table.columns[column].texts.get(row).cloned().unwrap_or_default()
    };
    let trimmed = raw.trim();
    if formula_mode || trimmed.starts_with('=') {
        let expr = trimmed.trim_start_matches('=').trim().to_string();
        table.columns[column].cell_formulas[row] = expr.clone();
        if expr.is_empty() {
            return Ok(());
        }
        publish(table, others);
        let compiled = CompiledExpr::compile(&expr).map_err(|err| {
            if row < table.columns[column].texts.len() {
                table.columns[column].texts[row] = raw.trim().to_string();
            }
            explain_error(&err)
        })?;
        let mut report = RecalcReport::default();
        write_evaluated(&mut table.columns[column], row, eval_at(&compiled, row), &mut report);
        if let Some(err) = report.first_error {
            // 计算失败时把公式留在格子里，否则下一帧编辑框会从空白文本重新加载。
            if row < table.columns[column].texts.len() {
                table.columns[column].texts[row] = raw.trim().to_string();
            }
            return Err(err);
        }
        Ok(())
    } else {
        table.columns[column].cell_formulas[row].clear();
        let column = &mut table.columns[column];
        column.set_text(row, trimmed);
        if column.ty != ColumnType::Text {
            if let Some(value) = column.numbers[row] {
                column.texts[row] = column.format_value(value);
            }
        }
        Ok(())
    }
}

fn publish(table: &Table, others: &[Table]) {
    let mut tables = HashMap::new();
    tables.insert(table.name.clone(), snapshot(table));
    for other in others {
        tables.insert(other.name.clone(), snapshot(other));
    }
    let current = snapshot(table);
    COLS.with(|cols| *cols.borrow_mut() = current);
    TABLES.with(|slot| *slot.borrow_mut() = tables);
}

fn snapshot(table: &Table) -> ColumnStore {
    let mut names = HashMap::new();
    let mut values = Vec::with_capacity(table.columns.len());
    for (index, column) in table.columns.iter().enumerate() {
        names.insert(column.name.clone(), index);
        values.push(column.numbers.clone());
    }
    ColumnStore { names, values }
}

fn eval_at(compiled: &CompiledExpr, row: usize) -> Result<f64, String> {
    ROW.with(|current| current.set(row));
    let row_number = row as f64 + 1.0;
    compiled.eval(&[("i", row_number), ("row", row_number)])
}

fn write_evaluated(column: &mut crate::table::Column, row: usize, value: Result<f64, String>, report: &mut RecalcReport) {
    match value {
        Ok(number) if number.is_finite() => {
            write_number(column, row, Some(number));
            report.written += 1;
        }
        Ok(_) => {
            write_number(column, row, None);
            report.warnings += 1;
            if report.first_error.is_none() {
                report.first_error = Some(crate::locale::t("公式没有得到有限数值").into());
            }
        }
        Err(err) => {
            write_number(column, row, None);
            note_formula_error(report, &err);
        }
    }
}

fn write_number(column: &mut crate::table::Column, row: usize, value: Option<f64>) {
    if row >= column.len() {
        column.resize(row + 1);
    }
    column.numbers[row] = value.filter(|number| number.is_finite());
    column.texts[row] = match column.numbers[row] {
        Some(number) => column.format_value(number),
        None => String::new(),
    };
}

fn note_formula_error(report: &mut RecalcReport, err: &str) {
    report.warnings += 1;
    if report.first_error.is_none() {
        report.first_error = Some(explain_error(err));
    }
}

pub fn parameter_names(expr: &str) -> Vec<String> {
    let mut names = Vec::new();
    let chars: Vec<char> = expr.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '"' {
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            continue;
        }
        if is_ident_start(chars[i]) {
            let start = i;
            i += 1;
            while i < chars.len() && is_ident_continue(chars[i]) {
                i += 1;
            }
            let name: String = chars[start..i].iter().collect();
            let next_non_space = chars[i..].iter().find(|c| !c.is_whitespace()).copied();
            let is_call = next_non_space == Some('(') || name.contains("::");
            let reserved = matches!(name.as_str(), "x" | "y" | "z" | "t" | "i" | "row" | "pi" | "e" | "true" | "false");
            if !is_call && !reserved && !FUNCTIONS.contains(&name.as_str()) && !names.iter().any(|n| n == &name)
            {
                names.push(name);
            }
            continue;
        }
        i += 1;
    }
    names
}

pub fn parse_initials(text: &str) -> Result<Vec<(String, f64)>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for part in text.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (name, value) = part
            .split_once('=')
            .ok_or_else(|| crate::trf!("初值应为 a=1, b=0.5，无法解析「{part}」", "Initial values should look like a=1, b=0.5. Could not parse \"{part}\".", part = part))?;
        let name = name.trim();
        if name.is_empty() || !is_ident_start(name.chars().next().unwrap()) {
            return Err(crate::trf!("无效的参数名「{name}」", "Invalid parameter name \"{name}\"", name = name));
        }
        let value: f64 = value
            .trim()
            .parse()
            .map_err(|_| crate::trf!("无法解析初值「{part}」", "Could not parse the initial value \"{part}\"", part = part))?;
        if !value.is_finite() {
            return Err(crate::trf!("初值不是有限数：{part}", "Initial value is not finite: {part}", part = part));
        }
        out.push((name.to_string(), value));
    }
    Ok(out)
}

/// 把参数替换成数值，得到只含 `x` 的表达式，供拟合曲线保存。
pub fn substitute_params(expr: &str, params: &[(String, f64)]) -> String {
    let chars: Vec<char> = expr.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '"' {
            let start = i;
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' {
                    i += 1;
                }
                i += 1;
            }
            if i < chars.len() {
                i += 1;
            }
            out.extend(chars[start..i].iter());
            continue;
        }
        if is_ident_start(chars[i]) {
            let start = i;
            i += 1;
            while i < chars.len() && is_ident_continue(chars[i]) {
                i += 1;
            }
            let name: String = chars[start..i].iter().collect();
            if let Some((_, value)) = params.iter().find(|(n, _)| n == &name) {
                if *value < 0.0 {
                    out.push('(');
                    out.push_str(&format_coeff(*value));
                    out.push(')');
                } else {
                    out.push_str(&format_coeff(*value));
                }
            } else {
                out.push_str(&name);
            }
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn format_coeff(v: f64) -> String {
    format!("{v:.10}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

fn eval_tree(tree: &Node, vars: &[(&str, f64)]) -> Result<f64, String> {
    MATH.with(|slot| {
        let mut borrowed = slot.borrow_mut();
        if borrowed.is_none() {
            *borrowed = Some(math_context());
        }
        let ctx = borrowed.as_mut().unwrap();
        ctx.clear_variables();
        ctx.set_value("pi".to_string(), Value::Float(std::f64::consts::PI))
            .map_err(|err| err.to_string())?;
        ctx.set_value("e".to_string(), Value::Float(std::f64::consts::E))
            .map_err(|err| err.to_string())?;
        for (name, value) in vars {
            ctx.set_value((*name).to_string(), Value::Float(*value))
                .map_err(|err| err.to_string())?;
        }
        let value = tree.eval_with_context(ctx).map_err(|err| err.to_string())?;
        value.as_number().map_err(|err| err.to_string())
    })
}

fn math_context() -> HashMapContext {
    let mut ctx = HashMapContext::new();
    type Unary = fn(f64) -> f64;
    let unary: &[(&str, Unary)] = &[
        ("sin", f64::sin),
        ("cos", f64::cos),
        ("tan", f64::tan),
        ("asin", f64::asin),
        ("acos", f64::acos),
        ("atan", f64::atan),
        ("sinh", f64::sinh),
        ("cosh", f64::cosh),
        ("tanh", f64::tanh),
        ("asinh", f64::asinh),
        ("acosh", f64::acosh),
        ("atanh", f64::atanh),
        ("exp", f64::exp),
        ("ln", f64::ln),
        ("log", f64::log10),
        ("log10", f64::log10),
        ("log2", f64::log2),
        ("sqrt", f64::sqrt),
        ("abs", f64::abs),
        ("floor", f64::floor),
        ("ceil", f64::ceil),
        ("round", f64::round),
        ("cbrt", f64::cbrt),
        ("sign", signum),
    ];
    for &(name, func) in unary {
        ctx.set_function(name.to_string(), Function::new(move |arg| unary_call(arg, func)))
            .expect("register unary");
    }
    ctx.set_function(
        "atan2".to_string(),
        Function::new(|arg| {
            let (y, x) = two_numbers(arg)?;
            Ok(Value::Float(y.atan2(x)))
        }),
    )
    .expect("atan2");
    ctx.set_function(
        "hypot".to_string(),
        Function::new(|arg| {
            let (a, b) = two_numbers(arg)?;
            Ok(Value::Float(a.hypot(b)))
        }),
    )
    .expect("hypot");
    ctx.set_function(
        "pow".to_string(),
        Function::new(|arg| {
            let (a, b) = two_numbers(arg)?;
            Ok(Value::Float(a.powf(b)))
        }),
    )
    .expect("pow");
    ctx.set_function("col".to_string(), Function::new(col_fn))
        .expect("col");
    ctx.set_function("cell".to_string(), Function::new(cell_fn))
        .expect("cell");
    ctx.set_function("tablecol".to_string(), Function::new(tablecol_fn))
        .expect("tablecol");
    ctx
}

fn signum(v: f64) -> f64 {
    if v > 0.0 {
        1.0
    } else if v < 0.0 {
        -1.0
    } else {
        0.0
    }
}

fn unary_call(arg: &Value, func: fn(f64) -> f64) -> Result<Value, EvalexprError> {
    Ok(Value::Float(func(arg.as_number()?)))
}

fn two_numbers(arg: &Value) -> Result<(f64, f64), EvalexprError> {
    let tuple = arg.as_fixed_len_tuple(2)?;
    Ok((tuple[0].as_number()?, tuple[1].as_number()?))
}

fn col_fn(arg: &Value) -> Result<Value, EvalexprError> {
    let row = ROW.with(|row| row.get());
    if let Ok(name) = arg.as_string() {
        return Ok(Value::Float(lookup_name(&name, row)));
    }
    let index = index_from_value(arg)?;
    Ok(Value::Float(lookup_index(index, row)))
}

fn tablecol_fn(arg: &Value) -> Result<Value, EvalexprError> {
    let tuple = arg.as_fixed_len_tuple(2)?;
    let table = tuple[0].as_string()?;
    let row = ROW.with(|row| row.get());
    let value = if let Ok(name) = tuple[1].as_string() {
        lookup_foreign(&table, Some(name.as_str()), None, row)
    } else {
        let index = index_from_value(&tuple[1])?;
        lookup_foreign(&table, None, Some(index), row)
    };
    Ok(Value::Float(value))
}

fn lookup_foreign(table: &str, name: Option<&str>, index: Option<usize>, row: usize) -> f64 {
    TABLES.with(|tables| {
        let tables = tables.borrow();
        let Some(store) = tables.get(table) else {
            return f64::NAN;
        };
        let index = if let Some(name) = name {
            if let Some(index) = store.names.get(name).copied() {
                index
            } else if let Some(index) = letter_index(name) {
                index
            } else {
                return f64::NAN;
            }
        } else {
            index.unwrap_or(usize::MAX)
        };
        store
            .values
            .get(index)
            .and_then(|values| values.get(row).copied().flatten())
            .unwrap_or(f64::NAN)
    })
}

fn cell_fn(arg: &Value) -> Result<Value, EvalexprError> {
    let tuple = arg.as_fixed_len_tuple(2)?;
    let row_number = tuple[1].as_number()?;
    if row_number < 1.0 {
        return Ok(Value::Float(f64::NAN));
    }
    let row = (row_number as usize).saturating_sub(1);
    if let Ok(name) = tuple[0].as_string() {
        return Ok(Value::Float(lookup_name(&name, row)));
    }
    let index = index_from_value(&tuple[0])?;
    Ok(Value::Float(lookup_index(index, row)))
}

fn index_from_value(arg: &Value) -> Result<usize, EvalexprError> {
    let number = arg.as_number()?;
    if number < 1.0 {
        return Err(EvalexprError::CustomMessage(crate::locale::t("列号从 1 开始").to_string()));
    }
    Ok((number as usize) - 1)
}

fn lookup_index(index: usize, row: usize) -> f64 {
    COLS.with(|cols| {
        cols.borrow()
            .values
            .get(index)
            .and_then(|values| values.get(row).copied().flatten())
            .unwrap_or(f64::NAN)
    })
}

/// 先按列名，没有同名列时再把 `A`、`B`、`AA` 当成字母列标。
fn lookup_name(key: &str, row: usize) -> f64 {
    COLS.with(|cols| {
        let store = cols.borrow();
        let index = if let Some(index) = store.names.get(key).copied() {
            index
        } else if let Some(index) = letter_index(key) {
            index
        } else {
            return f64::NAN;
        };
        store
            .values
            .get(index)
            .and_then(|values| values.get(row).copied().flatten())
            .unwrap_or(f64::NAN)
    })
}

fn letter_index(text: &str) -> Option<usize> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return None;
    }
    let mut n = 0usize;
    for byte in text.bytes() {
        let value = byte.to_ascii_uppercase();
        if !value.is_ascii_uppercase() {
            return None;
        }
        n = n.checked_mul(26)?.checked_add((value - b'A') as usize + 1)?;
    }
    n.checked_sub(1)
}

/// 把 evalexpr 的英文报错换成学生能看懂的中文。已经是中文的原文保留。
pub fn explain_error(err: &str) -> String {
    let err = err.trim();
    let (prefix, body) = split_row_prefix(err);
    if body.chars().any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch)) {
        return err.to_string();
    }
    let text = explain_evalexpr(body);
    if prefix.is_empty() {
        text
    } else {
        format!("{prefix}{text}")
    }
}

fn split_row_prefix(err: &str) -> (&str, &str) {
    if let Some(rest) = err.strip_prefix("第 ") {
        if let Some((head, body)) = rest.split_once('：') {
            let rowish = head.chars().all(|ch| ch.is_ascii_digit() || matches!(ch, '行' | '第' | '列' | ' '));
            if rowish && (head.ends_with('行') || head.ends_with('列')) {
                let split = err.len() - body.len();
                return (&err[..split], body.trim());
            }
        }
    }
    if let Some(rest) = err.strip_prefix("Row ") {
        if let Some((head, body)) = rest.split_once(':') {
            if english_row_head(head) {
                let mut split = err.len() - body.len();
                if err.as_bytes().get(split) == Some(&b' ') {
                    split += 1;
                }
                return (&err[..split], body.trim());
            }
        }
    }
    ("", err)
}

fn english_row_head(head: &str) -> bool {
    let mut parts = head.split(',');
    let Some(row) = parts.next() else { return false };
    if row.trim().is_empty() || !row.trim().chars().all(|ch| ch.is_ascii_digit()) {
        return false;
    }
    match parts.next() {
        None => true,
        Some(column) => {
            column.trim().strip_prefix("column ").is_some_and(|n| !n.is_empty() && n.chars().all(|ch| ch.is_ascii_digit()))
                && parts.next().is_none()
        }
    }
}

fn explain_evalexpr(err: &str) -> String {
    let lower = err.to_ascii_lowercase();
    if lower.contains("unmatched opening parenthesis") || (lower.contains("unmatched brace") && lower.contains("opening")) {
        return crate::locale::t("括号没有配平，多了一个左括号。").into();
    }
    if lower.contains("unmatched closing parenthesis") || lower.contains("unexpected closing parenthesis") {
        return crate::locale::t("括号没有配平，多了一个右括号。").into();
    }
    if lower.contains("partial token") {
        return crate::locale::t("表达式没写完，请检查运算符和括号。").into();
    }
    if let Some(name) = quoted_after(err, "variable identifier is not bound") {
        return crate::trf!("不认识变量「{name}」。列要写成 col(\"列名\")，当前行号用 i 或 row。", "Unknown variable \"{name}\". Write a column as col(\"name\"). The current row is i or row.", name = name);
    }
    if let Some(name) = quoted_after(err, "function identifier is not bound") {
        return crate::trf!("没有函数「{name}」。可用 sin、cos、exp、ln、log、sqrt 等，列引用用 col(\"列名\") 或 tablecol(\"表\",\"列\")。", "Unknown function \"{name}\". You can use sin, cos, exp, ln, log, sqrt, col(\"name\"), or tablecol(\"table\", \"column\").", name = name);
    }
    if lower.contains("expected") && lower.contains("argument") {
        return crate::trf!("函数的参数个数不对。{err}", "Wrong number of function arguments. {err}", err = err);
    }
    if lower.contains("expected") && (lower.contains("type") || lower.contains("value")) {
        return crate::locale::t("类型不对：这里需要数值。文本列不能直接做加减乘除。").into();
    }
    if lower.contains("division by zero") {
        return crate::locale::t("除数是 0。").into();
    }
    if lower.contains("empty") {
        return crate::locale::t("表达式是空的。").into();
    }
    crate::trf!("公式有误：{err}", "Invalid formula: {err}", err = err)
}

fn quoted_after(err: &str, marker: &str) -> Option<String> {
    if !err.to_ascii_lowercase().contains(&marker.to_ascii_lowercase()) {
        return None;
    }
    err.rsplit([':', ' ', '\'', '"', '.']).find(|part| !part.is_empty()).map(|part| part.to_string())
}

/// 把 `tablecol("表","旧列")` 里的列名换成新列名。表名不符的不动。
pub(crate) fn rewrite_tablecol_column(formula: &str, table: &str, old: &str, new: &str) -> String {
    rewrite_tablecol(formula, table, old, new, true)
}

/// 把 `tablecol("旧表", …)` 的表名换成新表名。
pub(crate) fn rewrite_tablecol_table(formula: &str, old: &str, new: &str) -> String {
    rewrite_tablecol(formula, old, new, "", false)
}

fn rewrite_tablecol(formula: &str, first_old: &str, first_new: &str, second_new: &str, column: bool) -> String {
    if formula.is_empty() || (column && first_new == second_new) || (!column && first_old == first_new) {
        return formula.to_string();
    }
    let mut out = String::with_capacity(formula.len());
    let mut i = 0;
    while i < formula.len() {
        let prev_ok = i == 0 || !is_ident_continue(formula[..i].chars().next_back().unwrap_or(' '));
        if prev_ok {
            if let Some((text, consumed)) = replace_tablecol(&formula[i..], first_old, first_new, second_new, column) {
                out.push_str(&text);
                i += consumed;
                continue;
            }
        }
        let ch = formula[i..].chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn replace_tablecol(rest: &str, first_old: &str, first_new: &str, second_new: &str, column: bool) -> Option<(String, usize)> {
    let after = rest.strip_prefix("tablecol")?;
    if after.chars().next().is_some_and(is_ident_continue) {
        return None;
    }
    let mut offset = "tablecol".len();
    offset += skip_ws(&rest[offset..]);
    if !rest[offset..].starts_with('(') {
        return None;
    }
    offset += 1;
    offset += skip_ws(&rest[offset..]);
    let (table_name, table_len) = parse_formula_string(&rest[offset..])?;
    offset += table_len;
    offset += skip_ws(&rest[offset..]);
    if !rest[offset..].starts_with(',') {
        return None;
    }
    offset += 1;
    offset += skip_ws(&rest[offset..]);
    let (column_name, column_len) = parse_formula_string(&rest[offset..])?;
    let matched = if column {
        table_name == first_old && column_name == first_new
    } else {
        table_name == first_old
    };
    if !matched {
        return None;
    }
    offset += column_len;
    let new_table = if column { table_name } else { first_new.to_string() };
    let new_column = if column { second_new.to_string() } else { column_name };
    let text = format!(
        "tablecol(\"{}\", \"{}\"",
        escape_formula_string(&new_table),
        escape_formula_string(&new_column)
    );
    Some((text, offset))
}

/// 把公式里的 `col("旧名")` 和 `cell("旧名", …)` 换成新列名。其它写法保持不动。
pub(crate) fn rewrite_column_refs(formula: &str, old: &str, new: &str) -> String {
    if old == new || formula.is_empty() {
        return formula.to_string();
    }
    let mut out = String::with_capacity(formula.len());
    let mut i = 0;
    while i < formula.len() {
        let prev_ok = i == 0 || !is_ident_continue(formula[..i].chars().next_back().unwrap_or(' '));
        if prev_ok {
            if let Some((text, consumed)) = replace_named_call(&formula[i..], "col", old, new, true) {
                out.push_str(&text);
                i += consumed;
                continue;
            }
            if let Some((text, consumed)) = replace_named_call(&formula[i..], "cell", old, new, false) {
                out.push_str(&text);
                i += consumed;
                continue;
            }
        }
        let ch = formula[i..].chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn replace_named_call(rest: &str, func: &str, old: &str, new: &str, expect_paren: bool) -> Option<(String, usize)> {
    let after = rest.strip_prefix(func)?;
    if after.chars().next().is_some_and(is_ident_continue) {
        return None;
    }
    let mut offset = func.len();
    offset += skip_ws(&rest[offset..]);
    if !rest[offset..].starts_with('(') {
        return None;
    }
    offset += 1;
    offset += skip_ws(&rest[offset..]);
    let (value, literal_len) = parse_formula_string(&rest[offset..])?;
    if value != old {
        return None;
    }
    offset += literal_len;
    if expect_paren {
        offset += skip_ws(&rest[offset..]);
        if !rest[offset..].starts_with(')') {
            return None;
        }
        offset += 1;
    }
    let escaped = escape_formula_string(new);
    let text = if expect_paren {
        format!("{func}(\"{escaped}\")")
    } else {
        format!("{func}(\"{escaped}\"")
    };
    Some((text, offset))
}

fn skip_ws(text: &str) -> usize {
    text.chars().take_while(|c| c.is_whitespace()).map(char::len_utf8).sum()
}

fn parse_formula_string(text: &str) -> Option<(String, usize)> {
    let mut chars = text.chars();
    if chars.next() != Some('"') {
        return None;
    }
    let mut value = String::new();
    let mut len = 1usize;
    loop {
        let ch = chars.next()?;
        len += ch.len_utf8();
        match ch {
            '"' => return Some((value, len)),
            '\\' => match chars.next()? {
                '"' => {
                    len += 1;
                    value.push('"');
                }
                '\\' => {
                    len += 1;
                    value.push('\\');
                }
                _ => return None,
            },
            other => value.push(other),
        }
    }
}

fn escape_formula_string(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            other => out.push(other),
        }
    }
    out
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == ':'
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::{ColumnMode, Table, TableId};

    #[test]
    fn sin_pi_and_column_formula() {
        let expr = CompiledExpr::compile("sin(pi/2)").unwrap();
        let value = expr.eval(&[]).unwrap();
        assert!((value - 1.0).abs() < 1e-12);

        let mut table = Table::new(TableId(1), "t");
        table.columns.clear();
        table.add_numeric_column("X", ColumnMode::X, &[0.0, std::f64::consts::FRAC_PI_2, std::f64::consts::PI]);
        table.add_column("Y", ColumnMode::Y);
        let evaluated = apply_formula(&mut table, 1, "sin(col(\"X\"))+i").unwrap();
        assert!((evaluated.values[0].unwrap() - 1.0).abs() < 1e-12);
        assert!((evaluated.values[1].unwrap() - (1.0 + 2.0)).abs() < 1e-9);
        assert_eq!(table.columns[1].formula, "sin(col(\"X\"))+i");
    }

    #[test]
    fn chinese_column_and_params() {
        let mut table = Table::new(TableId(1), "t");
        table.columns.clear();
        table.add_numeric_column(crate::locale::t("时间"), ColumnMode::X, &[1.0, 2.0, 3.0]);
        table.add_column(crate::locale::t("结果"), ColumnMode::Y);
        apply_formula(&mut table, 1, crate::locale::t("col(\"时间\")*2 + col(\"A\")")).unwrap();
        assert_eq!(table.columns[1].numbers[2], Some(9.0));

        let names = parameter_names("a*exp(-b*x)+c + sin(x)");
        assert_eq!(names, vec!["a".to_string(), "b".to_string(), "c".to_string()]);
        let initials = parse_initials("a=1, b=0.5, c=-2").unwrap();
        assert_eq!(initials[2], ("c".to_string(), -2.0));
        let subst = substitute_params("a*x+c", &[("a".into(), 2.5), ("c".into(), -1.0)]);
        assert_eq!(subst, "2.5*x+(-1)");
    }

    #[test]
    fn formula_errors_are_chinese() {
        let err = match CompiledExpr::compile("zzz(") {
            Err(err) => err,
            Ok(_) => panic!("zzz( 应该无法编译"),
        };
        let text = explain_error(&err);
        assert!(text.contains("括号"), "{text} / {err}");
        assert!(!text.contains("unmatched"), "{text}");

        let expr = match CompiledExpr::compile("zzz(1)") {
            Ok(expr) => expr,
            Err(err) => panic!("zzz(1) 语法完整，应该能编译：{err}"),
        };
        let err = match expr.eval(&[]) {
            Err(err) => err,
            Ok(value) => panic!("zzz(1) 求值应该失败，得到 {value}"),
        };
        let text = explain_error(&err);
        assert!(text.contains("没有函数") || text.contains(crate::locale::t("公式有误")), "{text} / {err}");

        let row = explain_error(crate::locale::t("第 3 行：Variable identifier is not bound to anything: foo."));
        assert!(row.starts_with("第 3 行："), "{row}");
        assert!(row.contains("不认识变量"), "{row}");

        crate::set_language(crate::Language::En);
        let english = explain_error("Row 3: Variable identifier is not bound to anything: foo.");
        crate::set_language(crate::Language::Zh);
        assert!(english.starts_with("Row 3:"), "{english}");
        assert!(english.contains("Unknown variable"), "{english}");
    }

    #[test]
    fn column_number_follows_position_when_names_collide_with_letters() {
        let mut table = Table::new(TableId(1), "t");
        table.columns.clear();
        table.add_numeric_column("C", ColumnMode::None, &[100.0, 200.0, 300.0]);
        table.add_numeric_column("A", ColumnMode::X, &[10.0, 20.0, 30.0]);
        table.add_numeric_column("B", ColumnMode::Y, &[1.0, 2.0, 3.0]);
        table.add_column("结果", ColumnMode::None);
        apply_formula(&mut table, 3, "col(1)").unwrap();
        assert_eq!(table.columns[3].numbers, vec![Some(100.0), Some(200.0), Some(300.0)]);
        apply_formula(&mut table, 3, "col(3)").unwrap();
        assert_eq!(table.columns[3].numbers, vec![Some(1.0), Some(2.0), Some(3.0)]);
        apply_formula(&mut table, 3, "col(\"C\")").unwrap();
        assert_eq!(table.columns[3].numbers[0], Some(100.0));
        apply_formula(&mut table, 3, "col(\"A\")").unwrap();
        assert_eq!(table.columns[3].numbers[0], Some(10.0));
    }

    #[test]
    fn log_is_base_10() {
        let value = CompiledExpr::compile("log(100)").unwrap().eval(&[]).unwrap();
        assert!((value - 2.0).abs() < 1e-12);
        let value = CompiledExpr::compile("ln(e)").unwrap().eval(&[]).unwrap();
        assert!((value - 1.0).abs() < 1e-12);
    }

    #[test]
    fn cell_formulas_and_tablecol() {
        let mut source = Table::new(TableId(1), "Table1");
        source.columns.clear();
        source.add_numeric_column("time", ColumnMode::X, &[10.0, 20.0, 30.0]);

        let mut table = Table::new(TableId(2), "Table2");
        table.columns.clear();
        table.add_numeric_column("A", ColumnMode::X, &[1.0, 2.0, 3.0]);
        table.add_column("B", ColumnMode::Y);
        table.add_column("C", ColumnMode::Y);
        table.columns[1].set_cell_formula_raw(0, "col(\"A\")+10");
        table.columns[1].set_cell_formula_raw(1, "=col(\"A\")*col(\"A\")");
        table.columns[2].formula = "tablecol(\"Table1\", \"time\")+1".into();
        let report = recalculate_table(&mut table, &[source]);
        assert!(report.first_error.is_none(), "{:?}", report.first_error);
        assert_eq!(table.columns[1].numbers[0], Some(11.0));
        assert_eq!(table.columns[1].numbers[1], Some(4.0));
        assert!(table.columns[1].numbers[2].is_none());
        assert_eq!(table.columns[2].numbers, vec![Some(11.0), Some(21.0), Some(31.0)]);

        table.columns[1].set_text(2, "=tablecol(\"Table1\", 1)+5");
        commit_cell(&mut table, 1, 2, false, &[]).unwrap_err();
        let mut source = Table::new(TableId(1), "Table1");
        source.columns.clear();
        source.add_numeric_column("time", ColumnMode::X, &[10.0, 20.0, 30.0]);
        commit_cell(&mut table, 1, 2, false, &[source]).unwrap();
        assert_eq!(table.columns[1].numbers[2], Some(35.0));
        assert_eq!(
            rewrite_tablecol_column("tablecol(\"Table1\", \"time\")+col(\"A\")", "Table1", "time", "t"),
            "tablecol(\"Table1\", \"t\")+col(\"A\")"
        );
        assert_eq!(
            rewrite_tablecol_table("tablecol(\"Table1\", \"t\")", "Table1", "数据"),
            "tablecol(\"数据\", \"t\")"
        );
    }
}
