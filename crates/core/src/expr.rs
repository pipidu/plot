//! SciPlot 自己的公式求值器。
//!
//! 所有数值都是 `f64`：`1/2` 是 0.5，`1 == 1.0` 为真。
//! 不再使用 evalexpr。11.3.1 会把整数相除截成 0，并且 `==` 区分整数和浮点；
//! 12 及以上是 AGPL，不能升级进这个 MIT / Apache 仓库。
//!
//! 支持 `+ - * / % ^`、括号、比较、`&&` `||` `!`、函数调用和字符串参数。
//! `if()`、`?:` 和缩进的 `if/elif/else` 仍由 `branch` 只计算被选中的一边，再把叶子交给这里。

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Number(f64),
    Bool(bool),
    Str(String),
}

impl Value {
    pub fn as_number(&self) -> Result<f64, String> {
        match self {
            Value::Number(number) => Ok(*number),
            Value::Bool(true) => Ok(1.0),
            Value::Bool(false) => Ok(0.0),
            Value::Str(_) => Err("expected value of type number".into()),
        }
    }

    pub fn truthy(&self) -> Result<bool, String> {
        match self {
            Value::Bool(flag) => Ok(*flag),
            Value::Number(number) => {
                if !number.is_finite() {
                    return Err("expected value of type number".into());
                }
                Ok(*number != 0.0)
            }
            Value::Str(_) => Err("expected value of type number".into()),
        }
    }
}

pub trait Host {
    fn var(&mut self, name: &str) -> Result<Value, String>;
    fn call(&mut self, name: &str, args: &[Value]) -> Result<Value, String>;
}

#[derive(Clone, Debug)]
pub struct Expr {
    node: Node,
}

#[derive(Clone, Debug)]
enum Node {
    Number(f64),
    Bool(bool),
    Str(String),
    Var(String),
    Unary(UnaryOp, Box<Node>),
    Binary(BinaryOp, Box<Node>, Box<Node>),
    Call(String, Vec<Node>),
}

#[derive(Clone, Copy, Debug)]
enum UnaryOp {
    Neg,
    Not,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
}

pub fn compile(source: &str) -> Result<Expr, String> {
    let mut parser = Parser { text: source, index: 0 };
    parser.skip();
    if parser.eof() {
        return Err("empty expression".into());
    }
    let node = parser.parse_or()?;
    parser.skip();
    if !parser.eof() {
        return Err("unexpected trailing input".into());
    }
    Ok(Expr { node })
}

pub fn eval(expr: &Expr, host: &mut dyn Host) -> Result<Value, String> {
    eval_node(&expr.node, host)
}

fn eval_node(node: &Node, host: &mut dyn Host) -> Result<Value, String> {
    match node {
        Node::Number(number) => Ok(Value::Number(*number)),
        Node::Bool(flag) => Ok(Value::Bool(*flag)),
        Node::Str(text) => Ok(Value::Str(text.clone())),
        Node::Var(name) => host.var(name),
        Node::Unary(UnaryOp::Neg, inner) => Ok(Value::Number(-eval_node(inner, host)?.as_number()?)),
        Node::Unary(UnaryOp::Not, inner) => Ok(Value::Bool(!eval_node(inner, host)?.truthy()?)),
        Node::Binary(op, left, right) => eval_binary(*op, left, right, host),
        Node::Call(name, args) => {
            let mut values = Vec::with_capacity(args.len());
            for arg in args {
                values.push(eval_node(arg, host)?);
            }
            host.call(name, &values)
        }
    }
}

fn eval_binary(op: BinaryOp, left: &Node, right: &Node, host: &mut dyn Host) -> Result<Value, String> {
    if op == BinaryOp::And {
        if !eval_node(left, host)?.truthy()? {
            return Ok(Value::Bool(false));
        }
        return Ok(Value::Bool(eval_node(right, host)?.truthy()?));
    }
    if op == BinaryOp::Or {
        if eval_node(left, host)?.truthy()? {
            return Ok(Value::Bool(true));
        }
        return Ok(Value::Bool(eval_node(right, host)?.truthy()?));
    }
    let left = eval_node(left, host)?;
    let right = eval_node(right, host)?;
    let number = match op {
        BinaryOp::Add => left.as_number()? + right.as_number()?,
        BinaryOp::Sub => left.as_number()? - right.as_number()?,
        BinaryOp::Mul => left.as_number()? * right.as_number()?,
        BinaryOp::Div => {
            let divisor = right.as_number()?;
            if divisor == 0.0 {
                return Err("division by zero".into());
            }
            left.as_number()? / divisor
        }
        BinaryOp::Rem => {
            let divisor = right.as_number()?;
            if divisor == 0.0 {
                return Err("division by zero".into());
            }
            left.as_number()? % divisor
        }
        BinaryOp::Pow => left.as_number()?.powf(right.as_number()?),
        BinaryOp::Eq => return Ok(Value::Bool(left.as_number()? == right.as_number()?)),
        BinaryOp::Ne => return Ok(Value::Bool(left.as_number()? != right.as_number()?)),
        BinaryOp::Lt => return Ok(Value::Bool(left.as_number()? < right.as_number()?)),
        BinaryOp::Gt => return Ok(Value::Bool(left.as_number()? > right.as_number()?)),
        BinaryOp::Le => return Ok(Value::Bool(left.as_number()? <= right.as_number()?)),
        BinaryOp::Ge => return Ok(Value::Bool(left.as_number()? >= right.as_number()?)),
        BinaryOp::And | BinaryOp::Or => unreachable!("logical operators return early"),
    };
    Ok(Value::Number(number))
}

struct Parser<'a> {
    text: &'a str,
    index: usize,
}

impl<'a> Parser<'a> {
    fn rest(&self) -> &'a str {
        &self.text[self.index..]
    }

    fn eof(&self) -> bool {
        self.index >= self.text.len()
    }

    fn skip(&mut self) {
        while let Some(ch) = self.rest().chars().next() {
            if !ch.is_whitespace() {
                break;
            }
            self.bump();
        }
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.rest().chars().next()?;
        self.index += ch.len_utf8();
        Some(ch)
    }

    fn starts(&self, pat: &str) -> bool {
        self.rest().starts_with(pat)
    }

    fn eat(&mut self, pat: &str) -> bool {
        self.skip();
        if self.starts(pat) {
            self.index += pat.len();
            true
        } else {
            false
        }
    }

    fn parse_or(&mut self) -> Result<Node, String> {
        let mut left = self.parse_and()?;
        while self.eat("||") || self.eat_word("or") {
            left = Node::Binary(BinaryOp::Or, Box::new(left), Box::new(self.parse_and()?));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Node, String> {
        let mut left = self.parse_cmp()?;
        while self.eat("&&") || self.eat_word("and") {
            left = Node::Binary(BinaryOp::And, Box::new(left), Box::new(self.parse_cmp()?));
        }
        Ok(left)
    }

    fn parse_cmp(&mut self) -> Result<Node, String> {
        let mut left = self.parse_add()?;
        loop {
            let op = if self.eat("==") {
                BinaryOp::Eq
            } else if self.eat("!=") {
                BinaryOp::Ne
            } else if self.eat("<=") {
                BinaryOp::Le
            } else if self.eat(">=") {
                BinaryOp::Ge
            } else if self.eat("<") {
                BinaryOp::Lt
            } else if self.eat(">") {
                BinaryOp::Gt
            } else {
                break;
            };
            left = Node::Binary(op, Box::new(left), Box::new(self.parse_add()?));
        }
        Ok(left)
    }

    fn parse_add(&mut self) -> Result<Node, String> {
        let mut left = self.parse_mul()?;
        loop {
            let op = if self.eat("+") {
                BinaryOp::Add
            } else if self.eat("-") {
                BinaryOp::Sub
            } else {
                break;
            };
            left = Node::Binary(op, Box::new(left), Box::new(self.parse_mul()?));
        }
        Ok(left)
    }

    fn parse_mul(&mut self) -> Result<Node, String> {
        let mut left = self.parse_unary()?;
        loop {
            let op = if self.eat("*") {
                BinaryOp::Mul
            } else if self.eat("/") {
                BinaryOp::Div
            } else if self.eat("%") {
                BinaryOp::Rem
            } else {
                break;
            };
            left = Node::Binary(op, Box::new(left), Box::new(self.parse_unary()?));
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Node, String> {
        self.skip();
        if self.eat("-") {
            return Ok(Node::Unary(UnaryOp::Neg, Box::new(self.parse_unary()?)));
        }
        if self.eat("+") {
            return self.parse_unary();
        }
        if self.eat("!") || self.eat_word("not") {
            return Ok(Node::Unary(UnaryOp::Not, Box::new(self.parse_unary()?)));
        }
        self.parse_pow()
    }

    fn parse_pow(&mut self) -> Result<Node, String> {
        let left = self.parse_primary()?;
        if self.eat("^") {
            Ok(Node::Binary(BinaryOp::Pow, Box::new(left), Box::new(self.parse_unary()?)))
        } else {
            Ok(left)
        }
    }

    fn parse_primary(&mut self) -> Result<Node, String> {
        self.skip();
        if self.eof() {
            return Err("unmatched opening parenthesis".into());
        }
        if self.eat("(") {
            let inner = self.parse_or()?;
            if !self.eat(")") {
                return Err("unmatched opening parenthesis".into());
            }
            return Ok(inner);
        }
        if self.starts("\"") {
            return Ok(Node::Str(self.parse_string()?));
        }
        if self.starts_number() {
            return Ok(Node::Number(self.parse_number()?));
        }
        if self.starts_ident() {
            let name = self.parse_ident();
            self.skip();
            if self.eat("(") {
                let mut args = Vec::new();
                self.skip();
                if !self.eat(")") {
                    loop {
                        args.push(self.parse_or()?);
                        self.skip();
                        if self.eat(")") {
                            break;
                        }
                        if !self.eat(",") {
                            return Err("unmatched opening parenthesis".into());
                        }
                    }
                }
                return Ok(Node::Call(name, args));
            }
            return Ok(match name.as_str() {
                "true" => Node::Bool(true),
                "false" => Node::Bool(false),
                _ => Node::Var(name),
            });
        }
        if self.starts(")") {
            return Err("unmatched closing parenthesis".into());
        }
        Err("unexpected trailing input".into())
    }

    fn starts_number(&self) -> bool {
        self.rest().chars().next().is_some_and(|ch| ch.is_ascii_digit())
    }

    fn starts_ident(&self) -> bool {
        self.rest().chars().next().is_some_and(is_ident_start)
    }

    fn parse_number(&mut self) -> Result<f64, String> {
        let start = self.index;
        while self.rest().chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
            self.bump();
        }
        if self.rest().starts_with('.') {
            let after = self.rest()[1..].chars().next();
            if after.is_some_and(|ch| ch.is_ascii_digit()) {
                self.bump();
                while self.rest().chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
                    self.bump();
                }
            }
        }
        if self.rest().starts_with('e') || self.rest().starts_with('E') {
            let saved = self.index;
            self.bump();
            if self.rest().starts_with('+') || self.rest().starts_with('-') {
                self.bump();
            }
            if self.rest().chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
                while self.rest().chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
                    self.bump();
                }
            } else {
                self.index = saved;
            }
        }
        self.text[start..self.index].parse::<f64>().map_err(|_| "partial token".into())
    }

    fn parse_ident(&mut self) -> String {
        let start = self.index;
        self.bump();
        while self.rest().chars().next().is_some_and(is_ident_continue) {
            self.bump();
        }
        self.text[start..self.index].to_string()
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.bump();
        let mut out = String::new();
        loop {
            match self.bump() {
                Some('"') => return Ok(out),
                Some('\\') => match self.bump() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some(other) => out.push(other),
                    None => return Err("unmatched opening parenthesis".into()),
                },
                Some(ch) => out.push(ch),
                None => return Err("unmatched opening parenthesis".into()),
            }
        }
    }

    fn eat_word(&mut self, word: &str) -> bool {
        self.skip();
        if !self.rest().starts_with(word) {
            return false;
        }
        let after = self.rest()[word.len()..].chars().next();
        if after.is_some_and(is_ident_continue) {
            return false;
        }
        self.index += word.len();
        true
    }
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

/// 公式里注册的数学函数。未知名字交给宿主，宿主再决定是变量还是报错。
pub fn builtin_math(name: &str, args: &[Value]) -> Option<Result<Value, String>> {
    let unary = |func: fn(f64) -> f64| -> Result<Value, String> {
        one(args).map(func).map(Value::Number)
    };
    let value = match name {
        "sin" => unary(f64::sin),
        "cos" => unary(f64::cos),
        "tan" => unary(f64::tan),
        "asin" => unary(f64::asin),
        "acos" => unary(f64::acos),
        "atan" => unary(f64::atan),
        "sinh" => unary(f64::sinh),
        "cosh" => unary(f64::cosh),
        "tanh" => unary(f64::tanh),
        "asinh" => unary(f64::asinh),
        "acosh" => unary(f64::acosh),
        "atanh" => unary(f64::atanh),
        "exp" => unary(f64::exp),
        "ln" => unary(f64::ln),
        "log" | "log10" => unary(f64::log10),
        "log2" => unary(f64::log2),
        "sqrt" => unary(f64::sqrt),
        "abs" => unary(f64::abs),
        "floor" => unary(f64::floor),
        "ceil" => unary(f64::ceil),
        "round" => unary(f64::round),
        "cbrt" => unary(f64::cbrt),
        "sign" => unary(|value| if value > 0.0 { 1.0 } else if value < 0.0 { -1.0 } else { 0.0 }),
        "atan2" => two(args).map(|(y, x)| Value::Number(y.atan2(x))),
        "hypot" => two(args).map(|(a, b)| Value::Number(a.hypot(b))),
        "pow" => two(args).map(|(a, b)| Value::Number(a.powf(b))),
        "min" => nary(args, f64::min),
        "max" => nary(args, f64::max),
        _ => return None,
    };
    Some(value)
}

fn one(args: &[Value]) -> Result<f64, String> {
    if args.len() != 1 {
        return Err(format!("expected 1 argument, got {}", args.len()));
    }
    args[0].as_number()
}

fn two(args: &[Value]) -> Result<(f64, f64), String> {
    if args.len() != 2 {
        return Err(format!("expected 2 arguments, got {}", args.len()));
    }
    Ok((args[0].as_number()?, args[1].as_number()?))
}

fn nary(args: &[Value], fold: fn(f64, f64) -> f64) -> Result<Value, String> {
    if args.len() < 2 {
        return Err(format!("expected 2 arguments, got {}", args.len()));
    }
    let mut acc = args[0].as_number()?;
    for arg in &args[1..] {
        acc = fold(acc, arg.as_number()?);
    }
    Ok(Value::Number(acc))
}

pub fn unknown_function(name: &str) -> String {
    format!("Function identifier is not bound: '{name}'.")
}

pub fn unknown_variable(name: &str) -> String {
    format!("Variable identifier is not bound to anything: {name}.")
}
