//! 列公式里的分支。`if()` 和 `?:` 只计算被选中的一边，不交给 evalexpr 的函数调用。
//! 缩进的 `if / elif / else` 也在这里解释。叶子表达式仍是 evalexpr，整数除法不变。

use evalexpr::Value;

pub(crate) fn uses_branching(expr: &str) -> bool {
    has_if_call(expr) || has_qmark(expr) || has_block_header(expr)
}

pub(crate) fn value_as_f64(value: Value) -> Result<f64, String> {
    match value {
        Value::Boolean(true) => Ok(1.0),
        Value::Boolean(false) => Ok(0.0),
        other => other.as_number().map_err(|err| err.to_string()),
    }
}

pub(crate) fn eval_extended(expr: &str, vars: &[(&str, f64)]) -> Result<Value, String> {
    if has_block_header(expr) {
        eval_block(expr, vars)
    } else {
        eval_expr(expr, vars, 0)
    }
}

fn eval_expr(expr: &str, vars: &[(&str, f64)], depth: usize) -> Result<Value, String> {
    if depth > 64 {
        return Err(crate::tr!("公式分支嵌套太深", "The formula branches are nested too deeply").into());
    }
    let expr = expr.trim();
    if expr.is_empty() {
        return Err(crate::locale::t("表达式为空").into());
    }
    if let Some((cond, yes, no)) = split_ternary(expr) {
        let cond = eval_expr(cond, vars, depth + 1)?;
        let taken = if truthy(&cond)? { yes } else { no };
        return eval_expr(taken, vars, depth + 1);
    }
    if let Some(call) = innermost_if(expr) {
        let cond = eval_expr(&call.cond, vars, depth + 1)?;
        let taken = if truthy(&cond)? { call.yes } else { call.no };
        let value = eval_expr(&taken, vars, depth + 1)?;
        let mut next = String::new();
        next.push_str(&expr[..call.start]);
        next.push_str(&literal_token(&value)?);
        next.push_str(&expr[call.end..]);
        return eval_expr(&next, vars, depth + 1);
    }
    crate::formula::eval_with_vars(expr, vars)
}

fn truthy(value: &Value) -> Result<bool, String> {
    match value {
        Value::Boolean(flag) => Ok(*flag),
        Value::Int(number) => Ok(*number != 0),
        Value::Float(number) => {
            if !number.is_finite() {
                return Err(crate::tr!("条件不是有限数", "The condition is not a finite number").into());
            }
            Ok(*number != 0.0)
        }
        _ => Err(crate::tr!("条件必须是数值或比较", "The condition must be a number or a comparison").into()),
    }
}

fn literal_token(value: &Value) -> Result<String, String> {
    match value {
        Value::Int(number) => {
            if *number < 0 {
                Ok(format!("({number})"))
            } else {
                Ok(number.to_string())
            }
        }
        Value::Float(number) => {
            if !number.is_finite() {
                return Err(crate::tr!("分支结果不是有限数", "The branch result is not a finite number").into());
            }
            let text = format!("{number:?}");
            if *number < 0.0 {
                Ok(format!("({text})"))
            } else {
                Ok(text)
            }
        }
        Value::Boolean(true) => Ok("1".into()),
        Value::Boolean(false) => Ok("0".into()),
        _ => Err(crate::tr!("分支结果不是数", "The branch result is not a number").into()),
    }
}

struct IfCall {
    start: usize,
    end: usize,
    cond: String,
    yes: String,
    no: String,
}

fn innermost_if(expr: &str) -> Option<IfCall> {
    let chars: Vec<(usize, char)> = expr.char_indices().collect();
    let mut i = 0;
    let mut found = None;
    while i < chars.len() {
        if chars[i].1 == '"' {
            i = skip_string(&chars, i);
            continue;
        }
        if let Some(open) = if_paren_at(&chars, i) {
            if let Some((args, end)) = split_call_args(expr, chars[open].0) {
                if args.len() == 3 && !has_if_call(&args[0]) && !has_if_call(&args[1]) && !has_if_call(&args[2]) {
                    found = Some(IfCall {
                        start: chars[i].0,
                        end,
                        cond: args[0].clone(),
                        yes: args[1].clone(),
                        no: args[2].clone(),
                    });
                } else if args.len() != 3 && found.is_none() {
                    found = Some(IfCall {
                        start: chars[i].0,
                        end,
                        cond: String::new(),
                        yes: String::new(),
                        no: String::new(),
                    });
                }
            }
        }
        i += 1;
    }
    if let Some(call) = &found {
        if call.cond.is_empty() && call.yes.is_empty() && call.no.is_empty() {
            return None;
        }
    }
    found
}

fn is_if_at(chars: &[(usize, char)], index: usize) -> bool {
    if_paren_at(chars, index).is_some()
}

fn if_paren_at(chars: &[(usize, char)], index: usize) -> Option<usize> {
    if index + 2 >= chars.len() || chars[index].1 != 'i' || chars[index + 1].1 != 'f' {
        return None;
    }
    let prev_ok = index == 0 || !is_ident_char(chars[index - 1].1);
    if !prev_ok {
        return None;
    }
    let mut j = index + 2;
    while j < chars.len() && chars[j].1.is_whitespace() {
        j += 1;
    }
    if matches!(chars.get(j).map(|item| item.1), Some('(') | Some('（')) {
        Some(j)
    } else {
        None
    }
}

fn split_call_args(expr: &str, open: usize) -> Option<(Vec<String>, usize)> {
    let chars: Vec<(usize, char)> = expr.char_indices().collect();
    let start = chars.iter().position(|(offset, _)| *offset == open)?;
    if chars[start].1 != '(' && chars[start].1 != '（' {
        return None;
    }
    let mut args = Vec::new();
    let mut buf = String::new();
    let mut depth = 0;
    let mut i = start + 1;
    while i < chars.len() {
        let ch = chars[i].1;
        if ch == '"' {
            let end = skip_string(&chars, i);
            for item in &chars[i..end.min(chars.len())] {
                buf.push(item.1);
            }
            i = end;
            continue;
        }
        match ch {
            '(' | '（' | '[' | '{' => {
                depth += 1;
                buf.push(ch);
            }
            ')' | '）' | ']' | '}' if depth > 0 => {
                depth -= 1;
                buf.push(ch);
            }
            ')' | '）' if depth == 0 => {
                args.push(buf.trim().to_string());
                return Some((args, chars[i].0 + ch.len_utf8()));
            }
            ',' | '，' if depth == 0 => {
                args.push(buf.trim().to_string());
                buf.clear();
            }
            _ => buf.push(ch),
        }
        i += 1;
    }
    None
}

fn split_ternary(expr: &str) -> Option<(&str, &str, &str)> {
    let chars: Vec<(usize, char)> = expr.char_indices().collect();
    let mut depth = 0i32;
    let mut question = None;
    let mut i = 0;
    while i < chars.len() {
        if chars[i].1 == '"' {
            i = skip_string(&chars, i);
            continue;
        }
        match chars[i].1 {
            '(' | '（' | '[' | '{' => depth += 1,
            ')' | '）' | ']' | '}' => depth -= 1,
            '?' if depth == 0 => {
                question = Some(i);
                break;
            }
            _ => {}
        }
        i += 1;
    }
    let question = question?;
    let mut nested = 0i32;
    let mut colon = None;
    let mut j = question + 1;
    while j < chars.len() {
        if chars[j].1 == '"' {
            j = skip_string(&chars, j);
            continue;
        }
        match chars[j].1 {
            '(' | '（' | '[' | '{' => nested += 1,
            ')' | '）' | ']' | '}' => nested -= 1,
            '?' if nested == 0 => nested += 1,
            ':' if nested == 0 => {
                colon = Some(j);
                break;
            }
            ':' if nested > 0 => nested -= 1,
            _ => {}
        }
        j += 1;
    }
    let colon = colon?;
    let cond = expr[..chars[question].0].trim();
    let yes = expr[chars[question].0 + 1..chars[colon].0].trim();
    let no = expr[chars[colon].0 + 1..].trim();
    if cond.is_empty() || yes.is_empty() || no.is_empty() {
        return None;
    }
    Some((cond, yes, no))
}

fn has_if_call(expr: &str) -> bool {
    let chars: Vec<(usize, char)> = expr.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].1 == '"' {
            i = skip_string(&chars, i);
            continue;
        }
        if is_if_at(&chars, i) {
            return true;
        }
        i += 1;
    }
    false
}

fn has_qmark(expr: &str) -> bool {
    let chars: Vec<(usize, char)> = expr.char_indices().collect();
    let mut i = 0;
    let mut depth = 0i32;
    while i < chars.len() {
        if chars[i].1 == '"' {
            i = skip_string(&chars, i);
            continue;
        }
        match chars[i].1 {
            '(' | '（' | '[' | '{' => depth += 1,
            ')' | '）' | ']' | '}' => depth -= 1,
            '?' if depth == 0 => return true,
            _ => {}
        }
        i += 1;
    }
    false
}

fn has_block_header(expr: &str) -> bool {
    expr.lines().any(|line| {
        let text = line.trim();
        let word = text.split(|ch: char| ch.is_whitespace() || ch == ':').next().unwrap_or("");
        matches!(word, "if" | "elif" | "else") && text.contains(':')
    })
}

fn skip_string(chars: &[(usize, char)], start: usize) -> usize {
    let mut i = start + 1;
    while i < chars.len() {
        if chars[i].1 == '\\' {
            i += 2;
            continue;
        }
        if chars[i].1 == '"' {
            return i + 1;
        }
        i += 1;
    }
    chars.len()
}

fn is_ident_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn eval_block(expr: &str, vars: &[(&str, f64)]) -> Result<Value, String> {
    let lines = block_lines(expr);
    if lines.is_empty() {
        return Err(crate::locale::t("表达式为空").into());
    }
    let mut index = 0;
    let value = eval_suite(&lines, &mut index, lines[0].1, vars)?;
    if index < lines.len() {
        return Err(crate::trf!(
            "第 {} 行：if 没有配好",
            "Line {}: the if statement is not closed",
            lines[index].0
        ));
    }
    Ok(value)
}

fn block_lines(expr: &str) -> Vec<(usize, usize, String)> {
    let mut lines = Vec::new();
    for (offset, raw) in expr.lines().enumerate() {
        let mut indent = 0usize;
        let mut start = 0usize;
        for ch in raw.chars() {
            if ch == ' ' {
                indent += 1;
                start += 1;
            } else if ch == '\t' {
                indent += 4;
                start += 1;
            } else {
                break;
            }
        }
        let text = raw[start.min(raw.len())..].trim().to_string();
        if text.is_empty() {
            continue;
        }
        lines.push((offset + 1, indent, text));
    }
    lines
}

fn eval_suite(lines: &[(usize, usize, String)], index: &mut usize, indent: usize, vars: &[(&str, f64)]) -> Result<Value, String> {
    let mut last = Value::Float(f64::NAN);
    let mut any = false;
    while *index < lines.len() {
        let (no, ind, text) = &lines[*index];
        if *ind < indent {
            break;
        }
        if *ind > indent {
            return Err(crate::trf!("第 {} 行：缩进对不上", "Line {}: the indentation does not match", no));
        }
        any = true;
        if header_word(text) == "if" {
            last = eval_if_chain(lines, index, indent, vars)?;
        } else if matches!(header_word(text), "elif" | "else") {
            return Err(crate::trf!("第 {} 行：else 没有对应的 if", "Line {}: else does not belong to an if", no));
        } else {
            *index += 1;
            last = eval_expr(text, vars, 0)?;
        }
    }
    if !any {
        return Err(crate::tr!("分支是空的", "The branch is empty").into());
    }
    Ok(last)
}

fn eval_if_chain(lines: &[(usize, usize, String)], index: &mut usize, indent: usize, vars: &[(&str, f64)]) -> Result<Value, String> {
    let (no, _, text) = &lines[*index];
    let (cond_src, inline) = split_header(text).ok_or_else(|| {
        crate::trf!("第 {} 行：if 后面要有冒号", "Line {}: if needs a colon", no)
    })?;
    if header_word(text) != "if" || cond_src.is_empty() {
        return Err(crate::trf!("第 {} 行：if 后面要有条件", "Line {}: if needs a condition", no));
    }
    let cond_true = truthy(&eval_expr(cond_src, vars, 0)?)?;
    *index += 1;
    let mut matched = false;
    let mut result = Value::Float(f64::NAN);
    if cond_true {
        result = eval_or_skip_body(lines, index, indent, inline, vars, true)?;
        matched = true;
    } else {
        eval_or_skip_body(lines, index, indent, inline, vars, false)?;
    }
    while *index < lines.len() && lines[*index].1 == indent && header_word(&lines[*index].2) == "elif" {
        let (eno, _, etext) = &lines[*index];
        let (econd, einline) = split_header(etext).ok_or_else(|| {
            crate::trf!("第 {} 行：elif 后面要有冒号", "Line {}: elif needs a colon", eno)
        })?;
        *index += 1;
        if matched {
            eval_or_skip_body(lines, index, indent, einline, vars, false)?;
        } else if truthy(&eval_expr(econd, vars, 0)?)? {
            result = eval_or_skip_body(lines, index, indent, einline, vars, true)?;
            matched = true;
        } else {
            eval_or_skip_body(lines, index, indent, einline, vars, false)?;
        }
    }
    if *index < lines.len() && lines[*index].1 == indent && header_word(&lines[*index].2) == "else" {
        let (eno, _, etext) = &lines[*index];
        let (_, einline) = split_header(etext).ok_or_else(|| {
            crate::trf!("第 {} 行：else 后面要有冒号", "Line {}: else needs a colon", eno)
        })?;
        *index += 1;
        if matched {
            eval_or_skip_body(lines, index, indent, einline, vars, false)?;
        } else {
            result = eval_or_skip_body(lines, index, indent, einline, vars, true)?;
            matched = true;
        }
    }
    if !matched {
        return Ok(Value::Float(f64::NAN));
    }
    Ok(result)
}

fn eval_or_skip_body(
    lines: &[(usize, usize, String)],
    index: &mut usize,
    parent: usize,
    inline: Option<&str>,
    vars: &[(&str, f64)],
    run: bool,
) -> Result<Value, String> {
    if let Some(inline) = inline {
        if inline.trim().is_empty() {
            return body_from_indent(lines, index, parent, vars, run);
        }
        if run {
            eval_expr(inline, vars, 0)
        } else {
            Ok(Value::Float(f64::NAN))
        }
    } else {
        body_from_indent(lines, index, parent, vars, run)
    }
}

fn body_from_indent(
    lines: &[(usize, usize, String)],
    index: &mut usize,
    parent: usize,
    vars: &[(&str, f64)],
    run: bool,
) -> Result<Value, String> {
    if *index >= lines.len() || lines[*index].1 <= parent {
        return Err(crate::tr!("if 后面没有内容", "if has no body").into());
    }
    let body_indent = lines[*index].1;
    if run {
        eval_suite(lines, index, body_indent, vars)
    } else {
        while *index < lines.len() && lines[*index].1 >= body_indent {
            *index += 1;
        }
        Ok(Value::Float(f64::NAN))
    }
}

fn header_word(text: &str) -> &str {
    text.split(|ch: char| ch.is_whitespace() || ch == ':').next().unwrap_or("")
}

fn split_header(text: &str) -> Option<(&str, Option<&str>)> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut depth = 0i32;
    let mut i = 0;
    while i < chars.len() {
        if chars[i].1 == '"' {
            i = skip_string(&chars, i);
            continue;
        }
        match chars[i].1 {
            '(' | '（' | '[' | '{' => depth += 1,
            ')' | '）' | ']' | '}' => depth -= 1,
            ':' if depth == 0 => {
                let head = text[..chars[i].0].trim();
                let word_end = head.find(char::is_whitespace).unwrap_or(head.len());
                let cond = head[word_end..].trim();
                let rest = text[chars[i].0 + 1..].trim();
                let inline = if rest.is_empty() { None } else { Some(rest) };
                return Some((cond, inline));
            }
            _ => {}
        }
        i += 1;
    }
    None
}
