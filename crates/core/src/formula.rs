//! 列公式与拟合表达式。
//!
//! 求值器是本 crate 的 `expr`：所有数值都是 `f64`，`1/2` 等于 0.5，`1==1.0` 为真。
//! 不再使用 evalexpr（11.3.1 的整数除法和严格相等，12 及以上是 AGPL）。
//! 另外注册了 SciDAVis / muParser 风格的函数名：`sin(x)`、`ln(x)`、`log(x)`（以 10 为底）。
//! 列公式里可以用 `col("列名")`、`col("A")`（列名优先，没有同名列时才按字母列标）、`col(1)`（从 1 开始的位置）、`cell("A", 行号)`、`i`（从 1 开始的行号），以及 `tablecol("表","列")` 读另一张表的同一行。
//! 某一格还可以有自己的公式，算的时候盖过这一列的公式。套用列公式不会清掉这些单元格公式。
//! 重算按依赖排序，每个公式只算一次；有环时按列号、行号各算一次，不迭代到不动点。
//! `if(条件, 真, 假)`、`条件 ? 真 : 假` 和缩进的 `if / elif / else` 只计算被选中的一边。
//! 以 `python:` 开头的列公式交给内嵌脚本引擎；脚本引擎没装上时会报错，普通公式不受影响。
//! 备注求值会先清掉线程里的列上下文，备注里的 `col()` / `cell()` / `tablecol()` 不会读到上一张表的最后一行。

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

use crate::expr::{self, Host, Value};
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
    static NOTE: Cell<bool> = const { Cell::new(false) };
}

const FUNCTIONS: &[&str] = &[
    "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "sinh", "cosh", "tanh", "asinh", "acosh",
    "atanh", "exp", "ln", "log", "log10", "log2", "sqrt", "abs", "floor", "ceil", "round", "hypot",
    "sign", "pow", "min", "max", "cbrt", "col", "cell", "tablecol", "if", "elif", "else",
];

/// 以 `python:` 或 `#python` 开头的列公式交给脚本引擎，不走表达式引擎。
pub fn is_python_formula(formula: &str) -> bool {
    let trimmed = formula.trim();
    trimmed.starts_with("python:") || trimmed.starts_with("#python")
}

pub type PythonColumnFn = fn(&Table, &str, &[Table]) -> Result<EvalColumn, String>;

thread_local! {
    static PYTHON_COLUMN: Cell<Option<PythonColumnFn>> = const { Cell::new(None) };
}

pub fn set_python_column_hook(hook: Option<PythonColumnFn>) {
    PYTHON_COLUMN.with(|slot| slot.set(hook));
}

fn python_column(table: &Table, formula: &str, others: &[Table]) -> Result<EvalColumn, String> {
    let hook = PYTHON_COLUMN.with(|slot| slot.get());
    match hook {
        Some(hook) => hook(table, formula, others),
        None => Err(crate::tr!(
            "这一列是 Python 公式，但脚本引擎还没有装上。",
            "This column is a Python formula, but the scripting engine is not installed."
        )
        .into()),
    }
}

pub struct CompiledExpr {
    expr: expr::Expr,
}

impl CompiledExpr {
    pub fn compile(expr: &str) -> Result<Self, String> {
        let expr = expr.trim();
        if expr.is_empty() {
            return Err(crate::locale::t("表达式为空").to_string());
        }
        Ok(Self {
            expr: expr::compile(expr)?,
        })
    }

    pub fn eval(&self, vars: &[(&str, f64)]) -> Result<f64, String> {
        eval_compiled(&self.expr, vars)?.as_number()
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
    if is_python_formula(formula) {
        return python_column(table, formula, others);
    }
    if crate::branch::uses_branching(formula) {
        publish(table, others);
        let n = table.nrows();
        let mut values = Vec::with_capacity(n);
        let mut warnings = 0;
        let mut first_error = None;
        for row in 0..n {
            match eval_stored(formula, row) {
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
        return Ok(EvalColumn {
            values,
            warnings,
            first_error,
        });
    }
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

/// 把公式结果写回列，并记下公式文本。这一列上已有的单元格公式会留下来，并在列公式之后重新计算。
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
    let saved = table.columns[column].cell_formulas.clone();
    table.columns[column].ty = ColumnType::Numeric;
    let texts: Vec<String> = evaluated
        .values
        .iter()
        .map(|value| value.map(|number| table.columns[column].format_value(number)).unwrap_or_default())
        .collect();
    {
        let col = &mut table.columns[column];
        col.formula = formula.to_string();
        col.numbers = evaluated.values.clone();
        col.texts = texts;
        let mut formulas = saved;
        formulas.resize(n, String::new());
        col.cell_formulas = formulas;
    }
    publish(table, others);
    for row in 0..n {
        let raw = table.columns[column].cell_formula(row).trim().to_string();
        if raw.is_empty() {
            continue;
        }
        let expr = raw.trim_start_matches('=').trim().to_string();
        table.columns[column].cell_formulas[row] = expr.clone();
        if expr.is_empty() || is_python_formula(&expr) {
            continue;
        }
        let mut report = RecalcReport::default();
        write_evaluated(&mut table.columns[column], row, eval_stored(&expr, row), &mut report);
        publish(table, others);
    }
    Ok(evaluated)
}

#[derive(Clone, Debug, Default)]
pub struct RecalcReport {
    pub written: usize,
    pub warnings: usize,
    pub first_error: Option<String>,
}

/// 按依赖各算一次。右边的列公式能读到左边刚算完的单元格公式；左边引用右边时也会先算右边。
/// 不是不动点迭代：互相引用时按列号、行号各算一次，并记一条警告。
/// 套用列公式不会清掉单元格公式。`tablecol` 读 `others` 里已经发布的值。
pub fn recalculate_table(table: &mut Table, others: &[Table]) -> RecalcReport {
    let mut report = RecalcReport::default();
    let rows = table.nrows();
    publish(table, others);
    let plan = schedule(table, rows);
    if plan.cycle {
        report.warnings += 1;
        if report.first_error.is_none() {
            report.first_error = Some(
                crate::tr!(
                    "公式互相引用，已按列号和行号各计算一次。",
                    "Formulas refer to each other. Each one was evaluated once, in column and row order."
                )
                .into(),
            );
        }
    }
    for node in plan.order {
        match node.kind {
            NodeKind::Python => {
                let formula = table.columns[node.col].formula.trim().to_string();
                match python_column(table, &formula, others) {
                    Ok(evaluated) => {
                        for row in 0..rows {
                            if !table.columns[node.col].cell_formula(row).trim().is_empty() {
                                continue;
                            }
                            let value = evaluated.values.get(row).copied().flatten().ok_or_else(|| {
                                evaluated
                                    .first_error
                                    .clone()
                                    .unwrap_or_else(|| crate::locale::t("公式没有得到有限数值").into())
                            });
                            write_evaluated(&mut table.columns[node.col], row, value, &mut report);
                        }
                        if evaluated.warnings > 0 {
                            if let Some(err) = &evaluated.first_error {
                                note_formula_error(&mut report, err);
                            }
                        }
                    }
                    Err(err) => note_formula_error(&mut report, &err),
                }
                publish(table, others);
            }
            NodeKind::Cell { formula } => {
                if is_python_formula(&formula) {
                    note_formula_error(
                        &mut report,
                        crate::tr!(
                            "单元格公式请用 if() 或三元运算符，不要写 python:。",
                            "Cell formulas use if() or ? :, not a python: block."
                        ),
                    );
                    continue;
                }
                write_evaluated(&mut table.columns[node.col], node.row, eval_stored(&formula, node.row), &mut report);
                publish(table, others);
            }
        }
    }
    report
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct NodeKey {
    col: usize,
    row: u32,
}

struct WorkNode {
    col: usize,
    row: usize,
    kind: NodeKind,
}

enum NodeKind {
    Cell { formula: String },
    Python,
}

struct Plan {
    order: Vec<WorkNode>,
    cycle: bool,
}

fn schedule(table: &Table, rows: usize) -> Plan {
    let mut nodes: Vec<(NodeKey, NodeKind)> = Vec::new();
    let mut index_of: HashMap<NodeKey, usize> = HashMap::new();
    for (col, column) in table.columns.iter().enumerate() {
        let formula = column.formula.trim();
        if is_python_formula(formula) {
            let key = NodeKey { col, row: u32::MAX };
            index_of.insert(key, nodes.len());
            nodes.push((key, NodeKind::Python));
        }
        for row in 0..rows {
            let cell = column.cell_formula(row).trim().trim_start_matches('=').trim();
            if !cell.is_empty() {
                let key = NodeKey { col, row: row as u32 };
                index_of.insert(key, nodes.len());
                nodes.push((key, NodeKind::Cell { formula: cell.to_string() }));
            } else if !formula.is_empty() && !is_python_formula(formula) {
                let key = NodeKey { col, row: row as u32 };
                index_of.insert(key, nodes.len());
                nodes.push((key, NodeKind::Cell { formula: formula.to_string() }));
            }
        }
    }
    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    let mut indegree = vec![0usize; nodes.len()];
    for (at, (key, kind)) in nodes.iter().enumerate() {
        let formula = match kind {
            NodeKind::Cell { formula } => formula.as_str(),
            NodeKind::Python => table.columns[key.col].formula.trim(),
        };
        let row = if key.row == u32::MAX { 0 } else { key.row as usize };
        for (dep_col, dep_row) in formula_deps(formula, row, table) {
            let dep_key = if is_python_formula(table.columns[dep_col].formula.trim())
                && table.columns[dep_col].cell_formula(dep_row).trim().is_empty()
            {
                NodeKey { col: dep_col, row: u32::MAX }
            } else {
                NodeKey {
                    col: dep_col,
                    row: dep_row as u32,
                }
            };
            if dep_key == *key {
                continue;
            }
            if let Some(&dep) = index_of.get(&dep_key) {
                if !edges[dep].contains(&at) {
                    edges[dep].push(at);
                    indegree[at] += 1;
                }
            }
        }
    }
    let mut ready: Vec<usize> = indegree.iter().enumerate().filter(|(_, degree)| **degree == 0).map(|(index, _)| index).collect();
    ready.sort_by_key(|index| nodes[*index].0);
    let mut order = Vec::new();
    let mut seen = HashSet::new();
    while !ready.is_empty() {
        let index = ready.remove(0);
        if !seen.insert(index) {
            continue;
        }
        order.push(index);
        let mut next = Vec::new();
        for &child in &edges[index] {
            indegree[child] = indegree[child].saturating_sub(1);
            if indegree[child] == 0 {
                next.push(child);
            }
        }
        ready.extend(next);
        ready.sort_by_key(|index| nodes[*index].0);
        ready.dedup();
    }
    let cycle = order.len() < nodes.len();
    if cycle {
        let mut rest: Vec<usize> = (0..nodes.len()).filter(|index| !seen.contains(index)).collect();
        rest.sort_by_key(|index| nodes[*index].0);
        order.extend(rest);
    }
    Plan {
        order: order
            .into_iter()
            .map(|index| {
                let (key, kind) = &nodes[index];
                WorkNode {
                    col: key.col,
                    row: if key.row == u32::MAX { 0 } else { key.row as usize },
                    kind: match kind {
                        NodeKind::Python => NodeKind::Python,
                        NodeKind::Cell { formula } => NodeKind::Cell { formula: formula.clone() },
                    },
                }
            })
            .collect(),
        cycle,
    }
}

fn formula_deps(expr: &str, row: usize, table: &Table) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
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
            let mut j = i;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && chars[j] == '(' && matches!(name.as_str(), "col" | "cell") {
                if let Some((args, next)) = split_call_args(&chars, j) {
                    push_dep(&name, &args, row, table, &mut out);
                    i = next;
                    continue;
                }
            }
            continue;
        }
        i += 1;
    }
    out
}

fn split_call_args(chars: &[char], open: usize) -> Option<(Vec<String>, usize)> {
    let mut depth = 0i32;
    let mut args = Vec::new();
    let mut current = String::new();
    let mut i = open;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '(' {
            depth += 1;
            if depth > 1 {
                current.push(ch);
            }
        } else if ch == ')' {
            depth -= 1;
            if depth == 0 {
                args.push(current.trim().to_string());
                return Some((args, i + 1));
            }
            current.push(ch);
        } else if ch == ',' && depth == 1 {
            args.push(current.trim().to_string());
            current.clear();
        } else if ch == '"' {
            current.push(ch);
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                current.push(chars[i]);
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 1;
                    current.push(chars[i]);
                }
                i += 1;
            }
            if i < chars.len() {
                current.push('"');
            }
        } else {
            current.push(ch);
        }
        i += 1;
    }
    None
}

fn push_dep(name: &str, args: &[String], row: usize, table: &Table, out: &mut Vec<(usize, usize)>) {
    if name == "col" {
        let Some(arg) = args.first() else { return };
        if let Some(col) = resolve_col_arg(arg, table) {
            if row < table.nrows() {
                out.push((col, row));
            }
        }
    } else if name == "cell" && args.len() >= 2 {
        if let (Some(col), Some(target)) = (resolve_col_arg(&args[0], table), parse_row_arg(&args[1])) {
            if target < table.nrows() {
                out.push((col, target));
            }
        }
    }
}

fn resolve_col_arg(arg: &str, table: &Table) -> Option<usize> {
    let arg = arg.trim();
    if let Some(text) = arg.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')) {
        if let Some(index) = table.columns.iter().position(|column| column.name == text) {
            return Some(index);
        }
        return letter_index(text).filter(|index| *index < table.columns.len());
    }
    let number: f64 = arg.parse().ok()?;
    if number < 1.0 {
        return None;
    }
    let index = (number as usize).saturating_sub(1);
    (index < table.columns.len()).then_some(index)
}

fn parse_row_arg(arg: &str) -> Option<usize> {
    let number: f64 = arg.trim().trim_matches('"').parse().ok()?;
    if number < 1.0 {
        return None;
    }
    Some((number as usize) - 1)
}

/// 结束编辑一个格子。公式模式，或以 `=` 开头的文本，写成这一格自己的公式并立刻计算。
/// 公式栏提交。以 `=` 开头的文本写成单元格公式并立刻重算；否则写成单元格的值并清掉这一格的公式。
pub fn apply_formula_bar(table: &mut Table, column: usize, row: usize, text: &str, others: &[Table]) -> Result<(), String> {
    if column >= table.columns.len() {
        return Err(crate::locale::t("列不存在").into());
    }
    table.ensure_rows(row + 1);
    let trimmed = text.trim();
    if trimmed.starts_with('=') {
        table.columns[column].set_cell_formula_raw(row, trimmed);
        commit_cell(table, column, row, true, others)
    } else {
        if row < table.columns[column].cell_formulas.len() {
            table.columns[column].cell_formulas[row].clear();
        }
        table.columns[column].set_text(row, trimmed);
        commit_cell(table, column, row, false, others)
    }
}

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
        if is_python_formula(&expr) {
            return Err(crate::tr!(
                "单元格公式请用 if() 或三元运算符，不要写 python:。",
                "Cell formulas use if() or ? :, not a python: block."
            )
            .into());
        }
        if let Err(err) = compile_guard(&expr) {
            if row < table.columns[column].texts.len() {
                table.columns[column].texts[row] = raw.trim().to_string();
            }
            return Err(explain_error(&err));
        }
        let mut report = RecalcReport::default();
        write_evaluated(&mut table.columns[column], row, eval_stored(&expr, row), &mut report);
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

fn compile_guard(expr: &str) -> Result<(), String> {
    if crate::branch::uses_branching(expr) {
        return Ok(());
    }
    CompiledExpr::compile(expr).map(|_| ())
}

fn eval_stored(expr: &str, row: usize) -> Result<f64, String> {
    ROW.with(|current| current.set(row));
    let row_number = row as f64 + 1.0;
    let vars = [("i", row_number), ("row", row_number)];
    if crate::branch::uses_branching(expr) {
        crate::branch::eval_extended(expr, &vars).and_then(crate::branch::value_as_f64)
    } else {
        CompiledExpr::compile(expr)?.eval(&vars)
    }
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

struct FormulaHost<'a> {
    vars: &'a [(&'a str, f64)],
}

impl Host for FormulaHost<'_> {
    fn var(&mut self, name: &str) -> Result<Value, String> {
        if let Some((_, value)) = self.vars.iter().find(|(key, _)| *key == name) {
            return Ok(Value::Number(*value));
        }
        match name {
            "pi" => Ok(Value::Number(std::f64::consts::PI)),
            "e" => Ok(Value::Number(std::f64::consts::E)),
            _ => Err(expr::unknown_variable(name)),
        }
    }

    fn call(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        if NOTE.with(|note| note.get()) && matches!(name, "col" | "cell" | "tablecol") {
            return Err(crate::tr!(
                "备注里不能使用 col()、cell() 或 tablecol()。",
                "Notes cannot use col(), cell(), or tablecol()."
            )
            .into());
        }
        if let Some(value) = expr::builtin_math(name, args) {
            return value;
        }
        match name {
            "col" => Ok(Value::Number(col_value(args)?)),
            "cell" => Ok(Value::Number(cell_value(args)?)),
            "tablecol" => Ok(Value::Number(tablecol_value(args)?)),
            _ => Err(expr::unknown_function(name)),
        }
    }
}

fn eval_compiled(expr: &expr::Expr, vars: &[(&str, f64)]) -> Result<Value, String> {
    let mut host = FormulaHost { vars };
    expr::eval(expr, &mut host)
}

pub(crate) fn eval_with_vars(expr: &str, vars: &[(&str, f64)]) -> Result<Value, String> {
    let compiled = expr::compile(expr.trim())?;
    eval_compiled(&compiled, vars)
}

fn col_value(args: &[Value]) -> Result<f64, String> {
    let row = ROW.with(|row| row.get());
    match args {
        [Value::Str(name)] => Ok(lookup_name(name, row)),
        [other] => Ok(lookup_index(index_from_value(other)?, row)),
        _ => Err("expected 1 argument".into()),
    }
}

fn tablecol_value(args: &[Value]) -> Result<f64, String> {
    if args.len() != 2 {
        return Err("expected 2 arguments".into());
    }
    let Value::Str(table) = &args[0] else {
        return Err("expected value of type number".into());
    };
    let row = ROW.with(|row| row.get());
    let value = if let Value::Str(name) = &args[1] {
        lookup_foreign(table, Some(name.as_str()), None, row)
    } else {
        lookup_foreign(table, None, Some(index_from_value(&args[1])?), row)
    };
    Ok(value)
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

fn cell_value(args: &[Value]) -> Result<f64, String> {
    if args.len() != 2 {
        return Err("expected 2 arguments".into());
    }
    let row_number = args[1].as_number()?;
    if row_number < 1.0 {
        return Ok(f64::NAN);
    }
    let row = (row_number as usize).saturating_sub(1);
    if let Value::Str(name) = &args[0] {
        return Ok(lookup_name(name, row));
    }
    Ok(lookup_index(index_from_value(&args[0])?, row))
}

fn index_from_value(arg: &Value) -> Result<usize, String> {
    let number = arg.as_number()?;
    if number < 1.0 {
        return Err(crate::locale::t("列号从 1 开始").to_string());
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

/// 备注里的一行求值结果。`line` 从 1 开始。
#[derive(Clone, Debug)]
pub struct NoteLine {
    pub line: usize,
    pub source: String,
    pub output: String,
    pub ok: bool,
}

#[derive(Clone, Debug)]
pub struct NoteRun {
    pub lines: Vec<NoteLine>,
}

/// 用现有公式引擎逐行求值。`名字 = 表达式` 会记住这个数，后面的行可以用它。
/// 空行、`#` 和 `//` 跳过。这里没有表格，也不能跑 Python。
/// 进入之前会清掉线程里的列上下文，所以刚算完一张表再跑备注，`col()` 不会读到那张表的最后一行。
pub fn eval_note(text: &str) -> NoteRun {
    NOTE.with(|note| note.set(true));
    COLS.with(|cols| {
        *cols.borrow_mut() = ColumnStore {
            names: HashMap::new(),
            values: Vec::new(),
        };
    });
    TABLES.with(|tables| tables.borrow_mut().clear());
    ROW.with(|row| row.set(0));
    let run = eval_note_body(text);
    NOTE.with(|note| note.set(false));
    run
}

fn eval_note_body(text: &str) -> NoteRun {
    let mut lines = Vec::new();
    let mut vars: Vec<(String, f64)> = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let source = raw.trim();
        if source.is_empty() || source.starts_with('#') || source.starts_with("//") {
            continue;
        }
        let (name, expr) = split_assignment(source);
        let borrowed: Vec<(&str, f64)> = vars.iter().map(|(key, value)| (key.as_str(), *value)).collect();
        let value = if crate::branch::uses_branching(expr) {
            crate::branch::eval_extended(expr, &borrowed).and_then(crate::branch::value_as_f64)
        } else {
            CompiledExpr::compile(expr).and_then(|compiled| compiled.eval(&borrowed))
        };
        match value {
            Ok(value) => {
                if let Some(name) = name {
                    if let Some(slot) = vars.iter_mut().find(|(key, _)| key == name) {
                        slot.1 = value;
                    } else {
                        vars.push((name.to_string(), value));
                    }
                }
                lines.push(NoteLine {
                    line: index + 1,
                    source: source.to_string(),
                    output: format!("{value}"),
                    ok: true,
                });
            }
            Err(err) => lines.push(NoteLine {
                line: index + 1,
                source: source.to_string(),
                output: err,
                ok: false,
            }),
        }
    }
    NoteRun { lines }
}

fn split_assignment(source: &str) -> (Option<&str>, &str) {
    let bytes = source.as_bytes();
    let mut index = 0;
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    let start = index;
    if index >= bytes.len() || !is_ident_start(bytes[index] as char) {
        return (None, source);
    }
    index += 1;
    while index < bytes.len() && is_note_ident(bytes[index] as char) {
        index += 1;
    }
    let name_end = index;
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    if index >= bytes.len() || bytes[index] != b'=' {
        return (None, source);
    }
    let next = bytes.get(index + 1).copied();
    if matches!(next, Some(b'=') | Some(b'!') | Some(b'<') | Some(b'>')) {
        return (None, source);
    }
    let expr = source[index + 1..].trim();
    if expr.is_empty() {
        return (None, source);
    }
    (Some(&source[start..name_end]), expr)
}

fn is_note_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
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
    fn note_evaluates_assignments_and_skips_comments() {
        let run = eval_note("1+2\n# 说明\na = 3\na*4\n// 再来一行\n");
        assert_eq!(run.lines.len(), 3);
        assert_eq!(run.lines[0].output, "3");
        assert!(run.lines[1].ok);
        assert_eq!(run.lines[2].output, "12");
        let bad = eval_note("1 = 2");
        assert!(!bad.lines[0].ok);
    }

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

    #[test]
    fn right_column_formula_sees_left_cell_formula_in_one_pass() {
        let mut table = Table::new(TableId(1), "t");
        table.columns.clear();
        table.add_column("A", ColumnMode::X);
        table.add_column("B", ColumnMode::Y);
        table.ensure_rows(1);
        table.columns[0].numbers[0] = None;
        table.columns[0].texts[0].clear();
        table.columns[0].set_cell_formula_raw(0, "5");
        table.columns[1].formula = "col(\"A\")+1".into();
        table.columns[1].numbers[0] = None;
        let report = recalculate_table(&mut table, &[]);
        assert!(report.first_error.is_none(), "{:?}", report.first_error);
        assert_eq!(table.columns[0].numbers[0], Some(5.0));
        assert_eq!(table.columns[1].numbers[0], Some(6.0));

        // 仍只走一轮：左边的列公式看不到右边这一轮才写上的值。
        table.columns[0].cell_formulas[0].clear();
        table.columns[0].formula = "col(\"B\")+1".into();
        table.columns[1].formula = "10".into();
        table.columns[1].numbers[0] = Some(5.0);
        table.columns[0].numbers[0] = Some(0.0);
        let report = recalculate_table(&mut table, &[]);
        assert!(report.first_error.is_none(), "{:?}", report.first_error);
        assert_eq!(table.columns[1].numbers[0], Some(10.0));
        assert_eq!(table.columns[0].numbers[0], Some(11.0));

        table.columns[0].formula.clear();
        table.columns[0].set_cell_formula_raw(0, "col(\"B\")+1");
        table.columns[1].formula.clear();
        table.columns[1].set_cell_formula_raw(0, "10");
        table.columns[0].numbers[0] = Some(0.0);
        table.columns[1].numbers[0] = Some(0.0);
        let report = recalculate_table(&mut table, &[]);
        assert!(report.first_error.is_none(), "{:?}", report.first_error);
        assert_eq!(table.columns[1].numbers[0], Some(10.0));
        assert_eq!(table.columns[0].numbers[0], Some(11.0));

        let saved = table.columns[1].cell_formula(0).to_string();
        apply_formula(&mut table, 1, "col(\"A\")").unwrap();
        assert_eq!(table.columns[1].cell_formula(0), saved);
        assert_eq!(table.columns[1].numbers[0], Some(10.0));
    }

    #[test]
    fn formula_bar_keeps_other_cell_formulas() {
        let mut table = Table::new(TableId(1), "t");
        table.columns.clear();
        table.add_numeric_column("A", ColumnMode::X, &[1.0, 2.0]);
        table.add_column("B", ColumnMode::Y);
        table.columns[1].set_cell_formula_raw(0, "col(\"A\")+3");
        table.columns[1].set_cell_formula_raw(1, "9");
        apply_formula_bar(&mut table, 1, 0, "=col(\"A\")*10", &[]).unwrap();
        assert_eq!(table.columns[1].cell_formula(0), "col(\"A\")*10");
        assert_eq!(table.columns[1].numbers[0], Some(10.0));
        assert_eq!(table.columns[1].cell_formula(1), "9");
        apply_formula(&mut table, 1, "col(\"A\")").unwrap();
        assert_eq!(table.columns[1].cell_formula(0), "col(\"A\")*10");
        assert_eq!(table.columns[1].numbers[0], Some(10.0));
        assert_eq!(table.columns[1].cell_formula(1), "9");
        assert_eq!(table.columns[1].numbers[1], Some(9.0));
    }

    #[test]
    fn note_does_not_read_the_previous_table_row() {
        let mut table = Table::new(TableId(1), "t");
        table.columns.clear();
        table.add_numeric_column("A", ColumnMode::X, &[1.0, 2.0, 7.0]);
        table.add_column("B", ColumnMode::Y);
        apply_formula(&mut table, 1, "col(\"A\")+1").unwrap();
        assert_eq!(table.columns[1].numbers[2], Some(8.0));
        let run = eval_note("col(\"A\")\n1+1\n");
        assert_eq!(run.lines.len(), 2);
        assert!(!run.lines[0].ok, "{}", run.lines[0].output);
        assert_ne!(run.lines[0].output.trim(), "7");
        assert!(run.lines[0].output.contains("备注") || run.lines[0].output.contains("col"), "{}", run.lines[0].output);
        assert!(run.lines[1].ok, "{}", run.lines[1].output);
        assert_eq!(run.lines[1].output, "2");
    }

    #[test]
    fn branching_is_lazy_and_division_is_real() {
        assert!((CompiledExpr::compile("1/2").unwrap().eval(&[]).unwrap() - 0.5).abs() < 1e-12);
        assert!((CompiledExpr::compile("1.0/2").unwrap().eval(&[]).unwrap() - 0.5).abs() < 1e-12);
        assert_eq!(CompiledExpr::compile("1==1.0").unwrap().eval(&[]).unwrap(), 1.0);
        assert_eq!(CompiledExpr::compile("1!=1.0").unwrap().eval(&[]).unwrap(), 0.0);

        let mut table = Table::new(TableId(1), "t");
        table.columns.clear();
        table.add_numeric_column("A", ColumnMode::X, &[0.0, 2.0, 5.0]);
        table.add_column("B", ColumnMode::Y);
        apply_formula(&mut table, 1, "if(col(\"A\")>1, col(\"A\"), 1/0)").unwrap();
        assert!(table.columns[1].numbers[0].is_none());
        assert_eq!(table.columns[1].numbers[1], Some(2.0));
        assert_eq!(table.columns[1].numbers[2], Some(5.0));

        apply_formula(&mut table, 1, "col(\"A\")>1 ? col(\"A\")*2 : 0").unwrap();
        assert_eq!(table.columns[1].numbers[0], Some(0.0));
        assert_eq!(table.columns[1].numbers[1], Some(4.0));

        apply_formula(&mut table, 1, "if(1, 1, 2)/2").unwrap();
        assert!((table.columns[1].numbers[0].unwrap() - 0.5).abs() < 1e-12);

        let block = "if col(\"A\") > 1:\n    col(\"A\")\nelif col(\"A\") < 1:\n    7\nelse:\n    1/0\n";
        apply_formula(&mut table, 1, block).unwrap();
        assert_eq!(table.columns[1].numbers[0], Some(7.0));
        assert_eq!(table.columns[1].numbers[1], Some(2.0));
        assert_eq!(table.columns[1].numbers[2], Some(5.0));

        let run = eval_note("a = if(1, 2, 1/0)\na+1\n");
        assert_eq!(run.lines[0].output, "2");
        assert_eq!(run.lines[1].output, "3");

        let err = match eval_column(&table, "python:\n1\n") {
            Err(err) => err,
            Ok(_) => panic!("python formula should fail without a hook"),
        };
        assert!(err.contains("Python") || err.contains("脚本"), "{err}");
    }
}
