//! 列公式与拟合表达式。
//!
//! 语法是 evalexpr：`+ - * / % ^`、括号、比较运算。
//! 另外注册了 SciDAVis / muParser 风格的函数名：`sin(x)`、`ln(x)`、`log(x)`（以 10 为底）。
//! 列公式里可以用 `col("列名")`、`col("A")`、`col(1)`（从 1 开始）、`cell("A", 行号)`、`i`（从 1 开始的行号）。
//! 整数相除会截断，需要小数时写成 `1.0/2`。

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use evalexpr::{
    build_operator_tree, ContextWithMutableFunctions, ContextWithMutableVariables, EvalexprError,
    Function, HashMapContext, Node, Value,
};

use crate::table::{column_letter, ColumnType, Table};
use crate::util::format_number;

thread_local! {
    static COLS: RefCell<HashMap<String, Vec<Option<f64>>>> = RefCell::new(HashMap::new());
    static ROW: Cell<usize> = const { Cell::new(0) };
    static MATH: RefCell<Option<HashMapContext>> = const { RefCell::new(None) };
}

const FUNCTIONS: &[&str] = &[
    "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "sinh", "cosh", "tanh", "asinh", "acosh",
    "atanh", "exp", "ln", "log", "log10", "log2", "sqrt", "abs", "floor", "ceil", "round", "hypot",
    "sign", "pow", "min", "max", "cbrt", "col", "cell",
];

pub struct CompiledExpr {
    tree: Node,
}

impl CompiledExpr {
    pub fn compile(expr: &str) -> Result<Self, String> {
        let expr = expr.trim();
        if expr.is_empty() {
            return Err("表达式为空".to_string());
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
    let compiled = CompiledExpr::compile(formula)?;
    let mut map = HashMap::new();
    for (index, column) in table.columns.iter().enumerate() {
        map.insert(column.name.clone(), column.numbers.clone());
        map.entry(column_letter(index))
            .or_insert_with(|| column.numbers.clone());
    }
    COLS.with(|cols| *cols.borrow_mut() = map);
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
                    first_error = Some(format!("第 {} 行：{err}", row + 1));
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

/// 把公式结果写回列，并记下公式文本。
pub fn apply_formula(table: &mut Table, column: usize, formula: &str) -> Result<EvalColumn, String> {
    let evaluated = eval_column(table, formula)?;
    if column >= table.columns.len() {
        return Err("列不存在".to_string());
    }
    let n = evaluated.values.len();
    table.ensure_rows(n.max(table.nrows()));
    let col = &mut table.columns[column];
    col.ty = ColumnType::Numeric;
    col.formula = formula.to_string();
    col.numbers = evaluated.values.clone();
    col.texts = evaluated
        .values
        .iter()
        .map(|v| v.map(format_number).unwrap_or_default())
        .collect();
    Ok(evaluated)
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
            .ok_or_else(|| format!("初值应为 a=1, b=0.5，无法解析「{part}」"))?;
        let name = name.trim();
        if name.is_empty() || !is_ident_start(name.chars().next().unwrap()) {
            return Err(format!("无效的参数名「{name}」"));
        }
        let value: f64 = value
            .trim()
            .parse()
            .map_err(|_| format!("无法解析初值「{part}」"))?;
        if !value.is_finite() {
            return Err(format!("初值不是有限数：{part}"));
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
    let key = column_key(arg)?;
    Ok(Value::Float(lookup(&key, ROW.with(|row| row.get()))))
}

fn cell_fn(arg: &Value) -> Result<Value, EvalexprError> {
    let tuple = arg.as_fixed_len_tuple(2)?;
    let key = if let Ok(name) = tuple[0].as_string() {
        name
    } else {
        column_letter(index_from_value(&tuple[0])?)
    };
    let row_number = tuple[1].as_number()?;
    if row_number < 1.0 {
        return Ok(Value::Float(f64::NAN));
    }
    Ok(Value::Float(lookup(&key, (row_number as usize).saturating_sub(1))))
}

fn column_key(arg: &Value) -> Result<String, EvalexprError> {
    if let Ok(name) = arg.as_string() {
        return Ok(name);
    }
    let index = index_from_value(arg)?;
    Ok(column_letter(index))
}

fn index_from_value(arg: &Value) -> Result<usize, EvalexprError> {
    let number = arg.as_number()?;
    if number < 1.0 {
        return Err(EvalexprError::CustomMessage("列号从 1 开始".to_string()));
    }
    Ok((number as usize) - 1)
}

fn lookup(key: &str, row: usize) -> f64 {
    COLS.with(|cols| {
        cols.borrow()
            .get(key)
            .and_then(|values| values.get(row).copied().flatten())
            .unwrap_or(f64::NAN)
    })
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
        table.add_numeric_column("时间", ColumnMode::X, &[1.0, 2.0, 3.0]);
        table.add_column("结果", ColumnMode::Y);
        apply_formula(&mut table, 1, "col(\"时间\")*2 + col(\"A\")").unwrap();
        assert_eq!(table.columns[1].numbers[2], Some(9.0));

        let names = parameter_names("a*exp(-b*x)+c + sin(x)");
        assert_eq!(names, vec!["a".to_string(), "b".to_string(), "c".to_string()]);
        let initials = parse_initials("a=1, b=0.5, c=-2").unwrap();
        assert_eq!(initials[2], ("c".to_string(), -2.0));
        let subst = substitute_params("a*x+c", &[("a".into(), 2.5), ("c".into(), -1.0)]);
        assert_eq!(subst, "2.5*x+(-1)");
    }

    #[test]
    fn log_is_base_10() {
        let value = CompiledExpr::compile("log(100)").unwrap().eval(&[]).unwrap();
        assert!((value - 2.0).abs() < 1e-12);
        let value = CompiledExpr::compile("ln(e)").unwrap().eval(&[]).unwrap();
        assert!((value - 1.0).abs() < 1e-12);
    }
}
