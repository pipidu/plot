//! 一个嵌在程序里的 Python 解释器，不是 CPython。
//!
//! 没有把 CPython 嵌进来：Windows 发行包是在 Linux 上用 mingw 交叉编译的，PyO3 需要 libpython，
//! 三个平台再带一份标准库既放不进这个仓库的体积，也会逼用户自己装 Python。
//! RustPython 的工具链要求高于本仓库声明的 1.81。
//! 因此这里继续用这个解释器，并补上字典、类、异常，以及 math、statistics、random、json、sciplot 这几个模块。
//! 不认识的 import 会报错。脚本里的 `/` 是真除法，`//` 才向下取整。
//! 列公式已经改用 `sciplot_core::expr`，`1/2` 也是 0.5。

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use sciplot_analysis::{convolve, correlate, deconvolve, fit_series, FitModel};
use sciplot_core::{
    column_letter, recalculate_table, t, trf, ColumnMode, Curve, CurveKind, DataSource, EvalColumn, Project, Table,
};
use sciplot_plot::{export_scene, print_pdf, render, ExportFormat, ExportOptions, LayerView};

const STEP_LIMIT: usize = 100_000;
const DEPTH_LIMIT: usize = 64;

#[derive(Clone, Debug)]
pub struct ScriptOutput {
    pub output: String,
}

pub fn execute(project: &mut Project, source: &str) -> Result<ScriptOutput, String> {
    let stmts = parse(source)?;
    let mut machine = Machine {
        project: Some(project),
        column: None,
        output: String::new(),
        env: Env::with_builtins(),
        fits: Vec::new(),
        depth: 0,
        steps: 0,
        loops: 0,
        in_func: false,
    };
    match machine.exec_block(&stmts)? {
        Signal::Return(_) => {
            return Err(trf!("return 不能写在函数外面", "return cannot be used outside a function"));
        }
        Signal::Break | Signal::Continue => {
            return Err(trf!("break 或 continue 不在循环里", "break or continue is outside a loop"));
        }
        Signal::Raise(message) => return Err(message),
        Signal::None(_) => {}
    }
    Ok(ScriptOutput { output: machine.output })
}

pub fn python_column(table: &Table, formula: &str, others: &[Table]) -> Result<EvalColumn, String> {
    let body = strip_python(formula);
    let stmts = parse(body)?;
    let rows = table.nrows();
    let mut values = Vec::with_capacity(rows);
    let mut warnings = 0;
    let mut first_error = None;
    for row in 0..rows {
        let mut machine = Machine {
            project: None,
            column: Some(ColumnCtx { table, others, row }),
            output: String::new(),
            env: Env::with_builtins(),
            fits: Vec::new(),
            depth: 0,
            steps: 0,
            loops: 0,
            in_func: false,
        };
        machine.env.set(
            "i",
            Val::Int(row as i64 + 1),
        );
        machine.env.set("row", Val::Int(row as i64 + 1));
        match machine.exec_block(&stmts) {
            Ok(Signal::None(value) | Signal::Return(value)) => match as_optional_number(&value) {
                Ok(number) => values.push(number),
                Err(err) => {
                    warnings += 1;
                    if first_error.is_none() {
                        first_error = Some(err);
                    }
                    values.push(None);
                }
            },
            Ok(Signal::Raise(message)) => {
                warnings += 1;
                if first_error.is_none() {
                    first_error = Some(message);
                }
                values.push(None);
            }
            Ok(_) => {
                warnings += 1;
                if first_error.is_none() {
                    first_error = Some(trf!("列公式不能 break", "A column formula cannot break"));
                }
                values.push(None);
            }
            Err(err) => {
                warnings += 1;
                if first_error.is_none() {
                    first_error = Some(err);
                }
                values.push(None);
            }
        }
    }
    Ok(EvalColumn { values, warnings, first_error })
}

fn strip_python(formula: &str) -> &str {
    let trimmed = formula.trim_start();
    if let Some(rest) = trimmed.strip_prefix("python:") {
        rest.trim_start_matches([' ', '\t'])
    } else if let Some(rest) = trimmed.strip_prefix("#python") {
        rest.trim_start_matches([' ', '\t'])
    } else {
        trimmed
    }
}

struct ColumnCtx<'a> {
    table: &'a Table,
    others: &'a [Table],
    row: usize,
}

struct Machine<'a> {
    project: Option<&'a mut Project>,
    column: Option<ColumnCtx<'a>>,
    output: String,
    env: Env,
    fits: Vec<FitObj>,
    depth: usize,
    steps: usize,
    loops: usize,
    in_func: bool,
}

impl<'a> Machine<'a> {
    fn bump(&mut self) -> Result<(), String> {
        self.steps += 1;
        if self.steps > STEP_LIMIT {
            return Err(trf!("脚本步数太多，已停下来", "The script ran too many steps and was stopped"));
        }
        Ok(())
    }

    fn exec_block(&mut self, stmts: &[Stmt]) -> Result<Signal, String> {
        let mut last = Val::None;
        for stmt in stmts {
            self.bump()?;
            match self.exec_stmt(stmt)? {
                Signal::None(value) => last = value,
                other => return Ok(other),
            }
        }
        Ok(Signal::None(last))
    }

    fn exec_stmt(&mut self, stmt: &Stmt) -> Result<Signal, String> {
        match stmt {
            Stmt::Pass => Ok(Signal::None(Val::None)),
            Stmt::ImportModules(names) => {
                for name in names {
                    self.bind_module(name)?;
                }
                Ok(Signal::None(Val::None))
            }
            Stmt::FromImport { module, names } => {
                self.bind_from(module, names)?;
                Ok(Signal::None(Val::None))
            }
            Stmt::Expr(expr) => Ok(Signal::None(self.eval(expr)?)),
            Stmt::Assign { name, expr } => {
                let value = self.eval(expr)?;
                self.env.set(name, value);
                Ok(Signal::None(Val::None))
            }
            Stmt::SetIndex { base, index, expr } => {
                let mut target = self.eval(base)?;
                let index = self.eval(index)?;
                let value = self.eval(expr)?;
                set_index(&mut target, &index, value)?;
                self.write_place(base, target)?;
                Ok(Signal::None(Val::None))
            }
            Stmt::SetAttr { base, name, expr } => {
                let target = self.eval(base)?;
                let value = self.eval(expr)?;
                set_field(&target, name, value)?;
                Ok(Signal::None(Val::None))
            }
            Stmt::Class { name, methods } => {
                self.env.set(
                    name,
                    Val::Class(ClassDef {
                        name: name.clone(),
                        methods: methods.clone(),
                    }),
                );
                Ok(Signal::None(Val::None))
            }
            Stmt::Try { body, name, handler } => match self.exec_block(body) {
                Ok(Signal::Raise(message)) => self.run_handler(name, handler, message),
                Err(message) => self.run_handler(name, handler, message),
                Ok(other) => Ok(other),
            },
            Stmt::Raise(expr) => {
                let value = self.eval(expr)?;
                Ok(Signal::Raise(display(&value)))
            }
            Stmt::If { arms, else_body } => {
                for (cond, body) in arms {
                    if truthy(&self.eval(cond)?) {
                        return self.exec_block(body);
                    }
                }
                self.exec_block(else_body)
            }
            Stmt::While { cond, body } => {
                self.loops += 1;
                let signal = self.exec_while(cond, body);
                self.loops -= 1;
                signal
            }
            Stmt::For { name, iter, body } => {
                let values = self.eval(iter)?;
                let items = iter_values(&values)?;
                self.loops += 1;
                let signal = self.exec_for(name, &items, body);
                self.loops -= 1;
                signal
            }
            Stmt::Def { name, params, body } => {
                self.env.set(
                    name,
                    Val::Func(FuncDef {
                        params: params.clone(),
                        body: body.clone(),
                    }),
                );
                Ok(Signal::None(Val::None))
            }
            Stmt::Return(expr) => {
                if !self.in_func {
                    return Err(trf!("return 不能写在函数外面", "return cannot be used outside a function"));
                }
                let value = match expr {
                    Some(expr) => self.eval(expr)?,
                    None => Val::None,
                };
                Ok(Signal::Return(value))
            }
            Stmt::Break => {
                if self.loops == 0 {
                    return Err(trf!("break 不在循环里", "break is outside a loop"));
                }
                Ok(Signal::Break)
            }
            Stmt::Continue => {
                if self.loops == 0 {
                    return Err(trf!("continue 不在循环里", "continue is outside a loop"));
                }
                Ok(Signal::Continue)
            }
        }
    }

    fn exec_while(&mut self, cond: &Expr, body: &[Stmt]) -> Result<Signal, String> {
        loop {
            self.bump()?;
            if !truthy(&self.eval(cond)?) {
                return Ok(Signal::None(Val::None));
            }
            match self.exec_block(body)? {
                Signal::Break => return Ok(Signal::None(Val::None)),
                Signal::Continue => continue,
                Signal::Return(value) => return Ok(Signal::Return(value)),
                Signal::Raise(message) => return Ok(Signal::Raise(message)),
                Signal::None(_) => {}
            }
        }
    }

    fn exec_for(&mut self, name: &str, items: &[Val], body: &[Stmt]) -> Result<Signal, String> {
        for item in items {
            self.bump()?;
            self.env.set(name, item.clone());
            match self.exec_block(body)? {
                Signal::Break => return Ok(Signal::None(Val::None)),
                Signal::Continue => continue,
                Signal::Return(value) => return Ok(Signal::Return(value)),
                Signal::Raise(message) => return Ok(Signal::Raise(message)),
                Signal::None(_) => {}
            }
        }
        Ok(Signal::None(Val::None))
    }

    fn eval(&mut self, expr: &Expr) -> Result<Val, String> {
        self.bump()?;
        if self.depth > DEPTH_LIMIT {
            return Err(trf!("脚本嵌套太深", "The script is nested too deeply"));
        }
        self.depth += 1;
        let value = self.eval_inner(expr);
        self.depth -= 1;
        value
    }

    fn eval_inner(&mut self, expr: &Expr) -> Result<Val, String> {
        match expr {
            Expr::None => Ok(Val::None),
            Expr::Bool(value) => Ok(Val::Bool(*value)),
            Expr::Int(value) => Ok(Val::Int(*value)),
            Expr::Float(value) => Ok(Val::Float(*value)),
            Expr::Str(value) => Ok(Val::Str(value.clone())),
            Expr::Name(name) => self.env.get(name).ok_or_else(|| {
                trf!("没有这个名字：{name}", "There is no such name: {name}", name = name)
            }),
            Expr::List(items) => {
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    values.push(self.eval(item)?);
                }
                Ok(Val::List(values))
            }
            Expr::Dict(pairs) => {
                let mut values = Vec::with_capacity(pairs.len());
                for (key, value) in pairs {
                    values.push((self.eval(key)?, self.eval(value)?));
                }
                Ok(Val::Dict(values))
            }
            Expr::Unary { op, expr } => self.eval_unary(op, expr),
            Expr::Binary { op, left, right } => self.eval_binary(op, left, right),
            Expr::Index { base, index } => {
                let base = self.eval(base)?;
                let index = self.eval(index)?;
                index_value(&base, &index)
            }
            Expr::Call { callee, args } => self.eval_call(callee, args),
            Expr::Attr { base, name } => {
                let object = self.eval(base)?;
                self.lookup_attr(object, name)
            }
        }
    }

    fn eval_unary(&mut self, op: &str, expr: &Expr) -> Result<Val, String> {
        let value = self.eval(expr)?;
        match op {
            "not" => Ok(Val::Bool(!truthy(&value))),
            "+" => match value {
                Val::Int(number) => Ok(Val::Int(number)),
                Val::Float(number) => Ok(Val::Float(number)),
                _ => Err(trf!("正号后面要是数", "Unary plus needs a number")),
            },
            "-" => match value {
                Val::Int(number) => Ok(Val::Int(-number)),
                Val::Float(number) => Ok(Val::Float(-number)),
                _ => Err(trf!("负号后面要是数", "Unary minus needs a number")),
            },
            _ => Err(trf!("不认识这个运算符", "Unknown operator")),
        }
    }

    fn eval_binary(&mut self, op: &str, left: &Expr, right: &Expr) -> Result<Val, String> {
        if op == "and" {
            let left = self.eval(left)?;
            if !truthy(&left) {
                return Ok(left);
            }
            return self.eval(right);
        }
        if op == "or" {
            let left = self.eval(left)?;
            if truthy(&left) {
                return Ok(left);
            }
            return self.eval(right);
        }
        let left = self.eval(left)?;
        let right = self.eval(right)?;
        match op {
            "+" => add_values(&left, &right),
            "-" => num_bin(&left, &right, |a, b| a - b, true),
            "*" => mul_values(&left, &right),
            "/" => {
                let (a, b) = (as_f64(&left)?, as_f64(&right)?);
                if b == 0.0 {
                    return Err(trf!("除数不能是 0", "Division by zero"));
                }
                Ok(Val::Float(a / b))
            }
            "//" => {
                let (a, b) = (as_f64(&left)?, as_f64(&right)?);
                if b == 0.0 {
                    return Err(trf!("除数不能是 0", "Division by zero"));
                }
                Ok(Val::Int((a / b).floor() as i64))
            }
            "%" => {
                let (a, b) = (as_f64(&left)?, as_f64(&right)?);
                if b == 0.0 {
                    return Err(trf!("除数不能是 0", "Division by zero"));
                }
                Ok(Val::Float(a % b))
            }
            "**" => Ok(Val::Float(as_f64(&left)?.powf(as_f64(&right)?))),
            "==" => Ok(Val::Bool(values_equal(&left, &right))),
            "!=" => Ok(Val::Bool(!values_equal(&left, &right))),
            "<" | ">" | "<=" | ">=" => compare_values(&left, &right, op),
            _ => Err(trf!("不认识这个运算符", "Unknown operator")),
        }
    }

    fn eval_call(&mut self, callee: &Expr, args: &[Expr]) -> Result<Val, String> {
        if let Expr::Attr { base, name } = callee {
            let object = self.eval(base)?;
            let mut values = Vec::with_capacity(args.len());
            for arg in args {
                values.push(self.eval(arg)?);
            }
            return self.call_method(object, name, values);
        }
        let func = self.eval(callee)?;
        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            values.push(self.eval(arg)?);
        }
        self.call_value(func, values)
    }

    fn call_value(&mut self, func: Val, args: Vec<Val>) -> Result<Val, String> {
        match func {
            Val::Builtin(builtin) => self.call_builtin(builtin, args),
            Val::Func(func) => self.call_func(func, args),
            Val::Class(class) => self.construct(class, args),
            Val::Bound { receiver, method } => self.call_bound(*receiver, &method, args),
            _ => Err(trf!("这不是函数", "That is not a function")),
        }
    }

    fn call_func(&mut self, func: FuncDef, args: Vec<Val>) -> Result<Val, String> {
        if args.len() != func.params.len() {
            return Err(trf!(
                "函数参数个数不对",
                "The function was called with the wrong number of arguments"
            ));
        }
        if self.depth > DEPTH_LIMIT {
            return Err(trf!("脚本嵌套太深", "The script is nested too deeply"));
        }
        self.env.push();
        for (name, value) in func.params.iter().zip(args) {
            self.env.set(name, value);
        }
        let was = self.in_func;
        self.in_func = true;
        let signal = self.exec_block(&func.body);
        self.in_func = was;
        self.env.pop();
        match signal? {
            Signal::Return(value) | Signal::None(value) => Ok(value),
            Signal::Break | Signal::Continue => Err(trf!("break 不能跳出函数", "break cannot leave a function")),
            Signal::Raise(message) => Err(message),
        }
    }

    fn call_builtin(&mut self, builtin: Builtin, args: Vec<Val>) -> Result<Val, String> {
        match builtin {
            Builtin::Print => {
                let text = args.iter().map(display).collect::<Vec<_>>().join(" ");
                self.output.push_str(&text);
                self.output.push('\n');
                Ok(Val::None)
            }
            Builtin::Range => builtin_range(&args),
            Builtin::Len => Ok(Val::Int(len_of(args.first())? as i64)),
            Builtin::StrFn => Ok(Val::Str(args.first().map(display).unwrap_or_default())),
            Builtin::IntFn => Ok(Val::Int(as_f64(args.first().unwrap_or(&Val::None))?.trunc() as i64)),
            Builtin::FloatFn => Ok(Val::Float(as_f64(args.first().unwrap_or(&Val::None))?)),
            Builtin::Abs => match args.first() {
                Some(Val::Int(number)) => Ok(Val::Int(number.abs())),
                Some(value) => Ok(Val::Float(as_f64(value)?.abs())),
                None => Err(trf!("abs 需要一个数", "abs needs a number")),
            },
            Builtin::MathFn(name) => call_named(name, &args),
            Builtin::Table => self.lookup_object(Kind::Table, &args),
            Builtin::Graph => self.lookup_object(Kind::Graph, &args),
            Builtin::Matrix => self.lookup_object(Kind::Matrix, &args),
            Builtin::Note => self.lookup_object(Kind::Note, &args),
            Builtin::Folder => self.lookup_object(Kind::Folder, &args),
            Builtin::NewTable => self.new_table(&args),
            Builtin::NewGraph => self.new_graph(&args),
            Builtin::NewMatrix => self.new_matrix(&args),
            Builtin::NewNote => self.new_note(&args),
            Builtin::NewFolder => self.new_folder(&args),
            Builtin::Plot => self.plot(&args),
            Builtin::Fit => self.make_fit(&args),
            Builtin::Convolve => Ok(Val::List(number_list(&convolve(&as_f64_list(arg(&args, 0)?)?, &as_f64_list(arg(&args, 1)?)?)?))),
            Builtin::Deconvolve => Ok(Val::List(number_list(&deconvolve(
                &as_f64_list(arg(&args, 0)?)?,
                &as_f64_list(arg(&args, 1)?)?,
            )?))),
            Builtin::Correlate => {
                let result = correlate(&as_f64_list(arg(&args, 0)?)?, &as_f64_list(arg(&args, 1)?)?)?;
                Ok(Val::List(number_list(&result.value)))
            }
            Builtin::PrintPdf => self.print_pdf_fn(&args),
            Builtin::Arrange => self.arrange_fn(&args),
            Builtin::Col => self.col_lookup(&args),
            Builtin::Cell => self.cell_lookup(&args),
            Builtin::TableCol => self.tablecol_lookup(&args),
            Builtin::TableList => self.object_list(Kind::Table),
            Builtin::GraphList => self.object_list(Kind::Graph),
            Builtin::MatrixList => self.object_list(Kind::Matrix),
            Builtin::NoteList => self.object_list(Kind::Note),
            Builtin::SaveProject => self.save_project(&args),
        }
    }

    fn lookup_attr(&mut self, object: Val, name: &str) -> Result<Val, String> {
        if let Val::Object(instance) = &object {
            if let Some(value) = instance.borrow().fields.iter().find(|(key, _)| key == name).map(|(_, value)| value.clone()) {
                return Ok(value);
            }
            if instance.borrow().methods.iter().any(|(key, _)| key == name) {
                return Ok(Val::Bound {
                    receiver: Box::new(object),
                    method: name.to_string(),
                });
            }
        }
        self.call_method(object, name, Vec::new())
    }

    fn call_method(&mut self, object: Val, name: &str, args: Vec<Val>) -> Result<Val, String> {
        match object {
            Val::Table(id) => self.table_method(id, name, args),
            Val::Graph(id) => self.graph_method(id, name, args),
            Val::Matrix(id) => self.matrix_method(id, name, args),
            Val::Note(id) => self.note_method(id, name, args),
            Val::Folder(id) => self.folder_method(id, name, args),
            Val::Fit(index) => self.fit_method(index, name, args),
            Val::Object(instance) => self.object_method(instance, name, args),
            Val::Module(module) => module_attr(&module, name, &args),
            Val::Dict(items) => dict_method(&items, name, &args),
            _ => Err(trf!("这个对象没有方法", "That object has no methods")),
        }
    }

    fn run_handler(&mut self, name: &Option<String>, handler: &[Stmt], message: String) -> Result<Signal, String> {
        if let Some(name) = name {
            self.env.set(name, Val::Str(message));
        }
        self.exec_block(handler)
    }

    fn write_place(&mut self, place: &Expr, value: Val) -> Result<(), String> {
        match place {
            Expr::Name(name) => {
                self.env.set(name, value);
                Ok(())
            }
            Expr::Index { base, index } => {
                let key = self.eval(index)?;
                let mut target = self.eval(base)?;
                set_index(&mut target, &key, value)?;
                self.write_place(base, target)
            }
            Expr::Attr { base, name } => {
                set_field(&self.eval(base)?, name, value)?;
                Ok(())
            }
            _ => Err(trf!("这里不能赋值", "That cannot be assigned")),
        }
    }

    fn construct(&mut self, class: ClassDef, args: Vec<Val>) -> Result<Val, String> {
        let object = Val::Object(Rc::new(RefCell::new(Instance {
            class: class.name,
            methods: class.methods.clone(),
            fields: Vec::new(),
        })));
        if let Some((_, init)) = class.methods.into_iter().find(|(name, _)| name == "__init__") {
            let mut full = vec![object.clone()];
            full.extend(args);
            self.call_func(init, full)?;
        } else if !args.is_empty() {
            return Err(trf!("这个类不能带参数创建", "This class does not take arguments"));
        }
        Ok(object)
    }

    fn call_bound(&mut self, receiver: Val, method: &str, args: Vec<Val>) -> Result<Val, String> {
        let Val::Object(instance) = &receiver else {
            return Err(trf!("这不是方法", "That is not a method"));
        };
        let func = instance
            .borrow()
            .methods
            .iter()
            .find(|(name, _)| name == method)
            .map(|(_, func)| func.clone())
            .ok_or_else(|| trf!("没有这个方法：{method}", "No such method: {method}", method = method))?;
        let mut full = vec![receiver];
        full.extend(args);
        self.call_func(func, full)
    }

    fn object_method(&mut self, instance: Rc<RefCell<Instance>>, name: &str, args: Vec<Val>) -> Result<Val, String> {
        let func = instance.borrow().methods.iter().find(|(key, _)| key == name).map(|(_, func)| func.clone());
        if let Some(func) = func {
            let mut full = vec![Val::Object(instance)];
            full.extend(args);
            return self.call_func(func, full);
        }
        if args.is_empty() {
            if let Some(value) = instance.borrow().fields.iter().find(|(key, _)| key == name).map(|(_, value)| value.clone()) {
                return Ok(value);
            }
        }
        Err(trf!("没有这个属性：{name}", "No such attribute: {name}", name = name))
    }

    fn bind_module(&mut self, name: &str) -> Result<(), String> {
        known_module(name)?;
        self.env.set(name, Val::Module(name.to_string()));
        Ok(())
    }

    fn bind_from(&mut self, module: &str, names: &[String]) -> Result<(), String> {
        known_module(module)?;
        if names.is_empty() {
            for (name, value) in module_exports(module) {
                self.env.set(&name, value);
            }
            return Ok(());
        }
        for name in names {
            self.env.set(name, module_attr(module, name, &[])?);
        }
        Ok(())
    }

    fn object_list(&mut self, kind: Kind) -> Result<Val, String> {
        let project = self.project()?;
        let values = match kind {
            Kind::Table => project.tables.iter().map(|item| Val::Table(item.id)).collect(),
            Kind::Graph => project.graphs.iter().map(|item| Val::Graph(item.id)).collect(),
            Kind::Matrix => project.matrices.iter().map(|item| Val::Matrix(item.id)).collect(),
            Kind::Note => project.notes.iter().map(|item| Val::Note(item.id)).collect(),
            Kind::Folder => Vec::new(),
        };
        Ok(Val::List(values))
    }

    fn save_project(&mut self, args: &[Val]) -> Result<Val, String> {
        let path = expect_str(arg(args, 0)?)?;
        let bytes = sciplot_core::save_project_bytes(self.project()?, Some(sciplot_core::ProjectFormat::Splot))?;
        std::fs::write(path, bytes).map_err(|err| err.to_string())?;
        Ok(Val::None)
    }

    fn project(&mut self) -> Result<&mut Project, String> {
        self.project.as_deref_mut().ok_or_else(|| {
            trf!("列公式里不能改项目", "A column formula cannot change the project")
        })
    }

    fn lookup_object(&mut self, kind: Kind, args: &[Val]) -> Result<Val, String> {
        let name = expect_str(arg(args, 0)?)?;
        let project = self.project()?;
        match kind {
            Kind::Table => project
                .tables
                .iter()
                .find(|table| table.name == name)
                .map(|table| Val::Table(table.id))
                .ok_or_else(|| trf!("找不到表格 {name}", "No table named {name}", name = name)),
            Kind::Graph => project
                .graphs
                .iter()
                .find(|graph| graph.name == name)
                .map(|graph| Val::Graph(graph.id))
                .ok_or_else(|| trf!("找不到图形 {name}", "No graph named {name}", name = name)),
            Kind::Matrix => project
                .matrices
                .iter()
                .find(|matrix| matrix.name == name)
                .map(|matrix| Val::Matrix(matrix.id))
                .ok_or_else(|| trf!("找不到矩阵 {name}", "No matrix named {name}", name = name)),
            Kind::Note => project
                .notes
                .iter()
                .find(|note| note.name == name)
                .map(|note| Val::Note(note.id))
                .ok_or_else(|| trf!("找不到备注 {name}", "No note named {name}", name = name)),
            Kind::Folder => project
                .folders
                .iter()
                .find(|folder| folder.name == name)
                .map(|folder| Val::Folder(folder.id))
                .ok_or_else(|| trf!("找不到文件夹 {name}", "No folder named {name}", name = name)),
        }
    }

    fn new_table(&mut self, args: &[Val]) -> Result<Val, String> {
        let name = expect_str(arg(args, 0)?)?;
        let cols = if args.len() >= 2 { as_count(arg(args, 1)?)? } else { 2 };
        let rows = if args.len() >= 3 { as_count(arg(args, 2)?)? } else { 32 };
        let project = self.project()?;
        let id = project.new_table(&name);
        let table = project.table_mut(id).expect("new table");
        table.columns.clear();
        for index in 0..cols.max(1) {
            let mode = if index == 0 { ColumnMode::X } else { ColumnMode::Y };
            table.add_column(column_letter(index), mode);
        }
        table.ensure_rows(rows.max(1));
        Ok(Val::Table(id))
    }

    fn new_graph(&mut self, args: &[Val]) -> Result<Val, String> {
        let name = expect_str(arg(args, 0)?)?;
        let id = self.project()?.new_graph(&name);
        Ok(Val::Graph(id))
    }

    fn new_matrix(&mut self, args: &[Val]) -> Result<Val, String> {
        let name = expect_str(arg(args, 0)?)?;
        let rows = if args.len() >= 2 { as_count(arg(args, 1)?)? } else { 2 };
        let cols = if args.len() >= 3 { as_count(arg(args, 2)?)? } else { 2 };
        let id = self.project()?.new_matrix(&name, rows, cols);
        Ok(Val::Matrix(id))
    }

    fn new_note(&mut self, args: &[Val]) -> Result<Val, String> {
        let name = expect_str(arg(args, 0)?)?;
        let id = self.project()?.new_note(&name);
        Ok(Val::Note(id))
    }

    fn new_folder(&mut self, args: &[Val]) -> Result<Val, String> {
        let name = expect_str(arg(args, 0)?)?;
        let id = self.project()?.new_folder(&name, None)?;
        Ok(Val::Folder(id))
    }

    fn plot(&mut self, args: &[Val]) -> Result<Val, String> {
        let table_id = match arg(args, 0)? {
            Val::Table(id) => *id,
            Val::Str(name) => {
                let project = self.project()?;
                project.tables.iter().find(|table| table.name == *name).map(|table| table.id).ok_or_else(|| {
                    trf!("找不到表格 {name}", "No table named {name}", name = name)
                })?
            }
            _ => return Err(trf!("plot 的第一个参数要是表格", "plot expects a table")),
        };
        let kind = if args.len() >= 2 {
            kind_of(&expect_str(arg(args, 1)?)?)
        } else {
            CurveKind::Line
        };
        let project = self.project()?;
        let (table_name, x, y) = {
            let table = project.table(table_id).ok_or_else(|| t("找不到表格").to_string())?;
            if table.columns.len() < 2 {
                return Err(trf!("绘图至少要两列", "Plotting needs at least two columns"));
            }
            let x = table.columns.iter().position(|column| column.mode == ColumnMode::X).unwrap_or(0);
            let y = table
                .columns
                .iter()
                .enumerate()
                .find(|(index, column)| column.mode == ColumnMode::Y && *index != x)
                .map(|(index, _)| index)
                .unwrap_or(1);
            (table.name.clone(), x, y)
        };
        let id = project.new_graph(&table_name);
        let graph = project.graph_mut(id).expect("graph");
        graph.layers[0].curves.push(Curve {
            name: table_name,
            kind,
            source: DataSource::Columns {
                table: table_id,
                x,
                y,
                x_err: None,
                y_err: None,
                dx: None,
                dy: None,
            },
            ..Curve::default()
        });
        Ok(Val::Graph(id))
    }

    fn make_fit(&mut self, args: &[Val]) -> Result<Val, String> {
        let target = Box::new(arg(args, 0)?.clone());
        let model = expect_str(arg(args, 1)?)?;
        self.fits.push(FitObj { target, model, result: None });
        Ok(Val::Fit(self.fits.len() - 1))
    }

    fn table_method(&mut self, id: sciplot_core::TableId, name: &str, args: Vec<Val>) -> Result<Val, String> {
        let project = self.project()?;
        let table = project.table_mut(id).ok_or_else(|| t("找不到表格").to_string())?;
        match name {
            "name" => Ok(Val::Str(table.name.clone())),
            "numCols" | "nc" => Ok(Val::Int(table.columns.len() as i64)),
            "numRows" | "nr" => Ok(Val::Int(table.nrows() as i64)),
            "comment" => Ok(Val::Str(table.comment.clone())),
            "setComment" => {
                table.comment = expect_str(arg(&args, 0)?)?;
                Ok(Val::None)
            }
            "colName" => {
                let index = one_based(arg(&args, 0)?, table.columns.len())?;
                Ok(Val::Str(table.columns[index].name.clone()))
            }
            "column" => {
                let index = column_ref(table, arg(&args, 0)?)?;
                Ok(column_values(&table.columns[index]))
            }
            "formula" => {
                let index = one_based(arg(&args, 0)?, table.columns.len())?;
                Ok(Val::Str(table.columns[index].formula.clone()))
            }
            "setNumRows" => {
                let rows = as_count(arg(&args, 0)?)?;
                if rows < table.nrows() {
                    for column in &mut table.columns {
                        column.resize(rows);
                    }
                } else {
                    table.ensure_rows(rows);
                }
                Ok(Val::None)
            }
            "setNumCols" => {
                let cols = as_count(arg(&args, 0)?)?.max(1);
                while table.columns.len() < cols {
                    let index = table.columns.len();
                    table.add_column(column_letter(index), ColumnMode::Y);
                }
                while table.columns.len() > cols {
                    table.columns.pop();
                }
                Ok(Val::None)
            }
            "recalculate" => {
                let _ = recalculate_table(table, &[]);
                Ok(Val::None)
            }
            "exportASCII" => {
                let path = expect_str(arg(&args, 0)?)?;
                std::fs::write(path, sciplot_core::export_csv(table)).map_err(|err| err.to_string())?;
                Ok(Val::None)
            }
            "importASCII" => {
                let path = expect_str(arg(&args, 0)?)?;
                let bytes = std::fs::read(path).map_err(|err| err.to_string())?;
                let imported = sciplot_core::import_csv(&bytes)?;
                let id = table.id;
                let name = table.name.clone();
                let folder = table.folder;
                let comment = table.comment.clone();
                *table = imported;
                table.id = id;
                table.name = name;
                table.folder = folder;
                if table.comment.is_empty() {
                    table.comment = comment;
                }
                Ok(Val::None)
            }
            "setColName" => {
                let index = one_based(arg(&args, 0)?, table.columns.len())?;
                let column_name = expect_str(arg(&args, 1)?)?;
                if table.columns.iter().enumerate().any(|(at, column)| at != index && column.name == column_name) {
                    return Err(trf!("已有同名列", "A column with that name already exists"));
                }
                table.columns[index].name = column_name;
                Ok(Val::None)
            }
            "setCell" => {
                let col = one_based(arg(&args, 0)?, table.columns.len())?;
                let row_number = as_count(arg(&args, 1)?)?;
                if row_number == 0 {
                    return Err(trf!("行号从 1 开始", "Row numbers start at 1"));
                }
                let row = row_number - 1;
                table.ensure_rows(row + 1);
                write_cell(&mut table.columns[col], row, arg(&args, 2)?)?;
                Ok(Val::None)
            }
            "cell" => {
                let col = one_based(arg(&args, 0)?, table.columns.len())?;
                let row = one_based(arg(&args, 1)?, table.nrows())?;
                Ok(number_at(&table.columns[col], row))
            }
            "columnValues" => {
                let col = one_based(arg(&args, 0)?, table.columns.len())?;
                let values = table.columns[col]
                    .numbers
                    .iter()
                    .copied()
                    .map(|value| value.map(Val::Float).unwrap_or(Val::None))
                    .collect();
                Ok(Val::List(values))
            }
            "setFormula" => {
                let col = one_based(arg(&args, 0)?, table.columns.len())?;
                let formula = expect_str(arg(&args, 1)?)?;
                table.columns[col].formula = formula;
                let _ = recalculate_table(table, &[]);
                Ok(Val::None)
            }
            _ => Err(trf!("表格没有这个方法：{name}", "Table has no method {name}", name = name)),
        }
    }

    fn graph_method(&mut self, id: sciplot_core::GraphId, name: &str, args: Vec<Val>) -> Result<Val, String> {
        match name {
            "name" => {
                let graph = self.project()?.graph(id).ok_or_else(|| t("找不到图形").to_string())?;
                Ok(Val::Str(graph.name.clone()))
            }
            "setTitle" => {
                let title = expect_str(arg(&args, 0)?)?;
                let graph = self.project()?.graph_mut(id).ok_or_else(|| t("找不到图形").to_string())?;
                graph.name = title.clone();
                if let Some(layer) = graph.layers.first_mut() {
                    layer.title = title;
                }
                Ok(Val::None)
            }
            "addLayer" => {
                let graph = self.project()?.graph_mut(id).ok_or_else(|| t("找不到图形").to_string())?;
                let index = graph.add_layer();
                Ok(Val::Int(index as i64 + 1))
            }
            "arrangeLayers" => {
                let rows = as_count(arg(&args, 0)?)?;
                let cols = as_count(arg(&args, 1)?)?;
                arrange_graph(self.project()?.graph_mut(id).ok_or_else(|| t("找不到图形").to_string())?, rows, cols);
                Ok(Val::None)
            }
            "exportImage" | "save" => {
                let path = expect_str(arg(&args, 0)?)?;
                self.export_graph(id, &path)?;
                Ok(Val::None)
            }
            "layerCount" => {
                let graph = self.project()?.graph(id).ok_or_else(|| t("找不到图形").to_string())?;
                Ok(Val::Int(graph.layers.len() as i64))
            }
            "activeLayer" => Ok(Val::Int(1)),
            "setXTitle" => self.set_axis_title(id, 0, "x", &expect_str(arg(&args, 0)?)?),
            "setYTitle" => self.set_axis_title(id, 0, "y", &expect_str(arg(&args, 0)?)?),
            "setAxisTitle" => {
                let side = expect_str(arg(&args, 0)?)?;
                let title = expect_str(arg(&args, 1)?)?;
                self.set_axis_title(id, 0, &side, &title)
            }
            "printPdf" => {
                let path = expect_str(arg(&args, 0)?)?;
                self.write_pdf(&[id], &path)?;
                Ok(Val::None)
            }
            _ => Err(trf!("图形没有这个方法：{name}", "Graph has no method {name}", name = name)),
        }
    }

    fn matrix_method(&mut self, id: sciplot_core::MatrixId, name: &str, args: Vec<Val>) -> Result<Val, String> {
        let matrix = self.project()?.matrix_mut(id).ok_or_else(|| t("找不到矩阵").to_string())?;
        match name {
            "name" => Ok(Val::Str(matrix.name.clone())),
            "numRows" => Ok(Val::Int(matrix.rows as i64)),
            "numCols" => Ok(Val::Int(matrix.cols as i64)),
            "setDimensions" => {
                let rows = as_count(arg(&args, 0)?)?.max(1);
                let cols = as_count(arg(&args, 1)?)?.max(1);
                matrix.resize(rows, cols);
                Ok(Val::None)
            }
            "setCell" => {
                let row = one_based(arg(&args, 0)?, matrix.rows)?;
                let col = one_based(arg(&args, 1)?, matrix.cols)?;
                let value = match arg(&args, 2)? {
                    Val::None => None,
                    other => Some(as_f64(other)?),
                };
                matrix.set(row, col, value);
                Ok(Val::None)
            }
            "cell" => {
                let row = one_based(arg(&args, 0)?, matrix.rows)?;
                let col = one_based(arg(&args, 1)?, matrix.cols)?;
                Ok(matrix.get(row, col).map(Val::Float).unwrap_or(Val::None))
            }
            _ => Err(trf!("矩阵没有这个方法：{name}", "Matrix has no method {name}", name = name)),
        }
    }

    fn note_method(&mut self, id: sciplot_core::NoteId, name: &str, args: Vec<Val>) -> Result<Val, String> {
        let note = self.project()?.note_mut(id).ok_or_else(|| t("备注已删除").to_string())?;
        match name {
            "name" => Ok(Val::Str(note.name.clone())),
            "text" => Ok(Val::Str(note.text.clone())),
            "setText" => {
                note.text = expect_str(arg(&args, 0)?)?;
                Ok(Val::None)
            }
            _ => Err(trf!("备注没有这个方法：{name}", "Note has no method {name}", name = name)),
        }
    }

    fn folder_method(&mut self, id: sciplot_core::FolderId, name: &str, _args: Vec<Val>) -> Result<Val, String> {
        let project = self.project()?;
        let folder = project.folder(id).ok_or_else(|| t("没有这个文件夹").to_string())?;
        match name {
            "name" => Ok(Val::Str(folder.name.clone())),
            "tables" => Ok(Val::List(project.tables.iter().filter(|table| table.folder == Some(id)).map(|table| Val::Table(table.id)).collect())),
            "windows" => {
                let mut values = Vec::new();
                values.extend(project.tables.iter().filter(|item| item.folder == Some(id)).map(|item| Val::Table(item.id)));
                values.extend(project.matrices.iter().filter(|item| item.folder == Some(id)).map(|item| Val::Matrix(item.id)));
                values.extend(project.graphs.iter().filter(|item| item.folder == Some(id)).map(|item| Val::Graph(item.id)));
                values.extend(project.notes.iter().filter(|item| item.folder == Some(id)).map(|item| Val::Note(item.id)));
                Ok(Val::List(values))
            }
            _ => Err(trf!("文件夹没有这个方法：{name}", "Folder has no method {name}", name = name)),
        }
    }

    fn set_axis_title(&mut self, id: sciplot_core::GraphId, layer: usize, side: &str, title: &str) -> Result<Val, String> {
        let graph = self.project()?.graph_mut(id).ok_or_else(|| t("找不到图形").to_string())?;
        let layer = graph.layers.get_mut(layer).ok_or_else(|| t("找不到图层").to_string())?;
        match side {
            "x" | "bottom" => layer.x_axis.label = title.to_string(),
            "y" | "left" => layer.y_axis.label = title.to_string(),
            "top" => {
                layer.show_top = true;
                layer.x_top.label = title.to_string();
            }
            "right" => {
                layer.show_right = true;
                layer.y_right.label = title.to_string();
            }
            _ => return Err(trf!("没有这根坐标轴", "No such axis")),
        }
        Ok(Val::None)
    }

    fn fit_method(&mut self, index: usize, name: &str, _args: Vec<Val>) -> Result<Val, String> {
        match name {
            "fit" => {
                let target = (*self.fits[index].target).clone();
                let model = self.fits[index].model.clone();
                let snap = {
                    let project = self.project()?;
                    let (x, y, graph) = series_for_fit(project, &target)?;
                    let result = fit_series(&x, &y, &model_of(&model)?)?;
                    if let Some(graph_id) = graph {
                        if let Some(graph) = project.graph_mut(graph_id) {
                            let x_min = x.iter().copied().fold(f64::INFINITY, f64::min);
                            let x_max = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                            if let Some(layer) = graph.layers.first_mut() {
                                layer.curves.push(Curve {
                                    name: t("拟合").to_string(),
                                    kind: CurveKind::Function,
                                    source: DataSource::Function {
                                        expr: result.expr.clone(),
                                        x_min,
                                        x_max,
                                        samples: 200,
                                    },
                                    ..Curve::default()
                                });
                            }
                        }
                    }
                    FitSnap {
                        r2: result.r2,
                        chi2: result.chi2,
                        expr: result.expr,
                        values: result.values,
                    }
                };
                self.fits[index].result = Some(snap);
                Ok(Val::Fit(index))
            }
            "r2" => Ok(Val::Float(self.fits[index].result.as_ref().ok_or_else(need_fit)?.r2)),
            "chi2" => Ok(Val::Float(self.fits[index].result.as_ref().ok_or_else(need_fit)?.chi2)),
            "expr" => Ok(Val::Str(self.fits[index].result.as_ref().ok_or_else(need_fit)?.expr.clone())),
            "parameters" => Ok(Val::List(number_list(
                &self.fits[index].result.as_ref().ok_or_else(need_fit)?.values,
            ))),
            _ => Err(trf!("拟合没有这个方法：{name}", "Fit has no method {name}", name = name)),
        }
    }

    fn arrange_fn(&mut self, args: &[Val]) -> Result<Val, String> {
        let Val::Graph(id) = arg(args, 0)? else {
            return Err(trf!("arrangeLayers 的第一个参数要是图", "arrangeLayers expects a graph"));
        };
        let id = *id;
        let rows = as_count(arg(args, 1)?)?;
        let cols = as_count(arg(args, 2)?)?;
        arrange_graph(self.project()?.graph_mut(id).ok_or_else(|| t("找不到图形").to_string())?, rows, cols);
        Ok(Val::None)
    }

    fn print_pdf_fn(&mut self, args: &[Val]) -> Result<Val, String> {
        let Val::Graph(id) = arg(args, 0)? else {
            return Err(trf!("printPdf 的第一个参数要是图", "printPdf expects a graph"));
        };
        let id = *id;
        let path = expect_str(arg(args, 1)?)?;
        self.write_pdf(&[id], &path)?;
        Ok(Val::None)
    }

    fn export_graph(&mut self, id: sciplot_core::GraphId, path: &str) -> Result<(), String> {
        let format = format_of(path);
        let project = self.project()?;
        let graph = project.graph(id).ok_or_else(|| t("找不到图形").to_string())?.clone();
        let scene = render(project, &graph, &[] as &[LayerView], 800.0, 600.0, 1.0);
        let options = ExportOptions { format, ..ExportOptions::default() };
        let bytes = export_scene(&scene, None, &[], &options)?;
        std::fs::write(path, bytes).map_err(|err| err.to_string())
    }

    fn write_pdf(&mut self, ids: &[sciplot_core::GraphId], path: &str) -> Result<(), String> {
        let project = self.project()?;
        let mut scenes = Vec::new();
        for id in ids {
            let graph = project.graph(*id).ok_or_else(|| t("找不到图形").to_string())?.clone();
            scenes.push(render(project, &graph, &[] as &[LayerView], 800.0, 600.0, 1.0));
        }
        let bytes = print_pdf(&scenes, &[], &ExportOptions::default())?;
        std::fs::write(path, bytes).map_err(|err| err.to_string())
    }

    fn col_lookup(&self, args: &[Val]) -> Result<Val, String> {
        let ctx = self.column.as_ref().ok_or_else(|| {
            trf!("col 只在列公式里可用", "col is only available in a column formula")
        })?;
        let key = arg(args, 0)?;
        let index = column_index(ctx.table, key)?;
        Ok(number_at(&ctx.table.columns[index], ctx.row))
    }

    fn cell_lookup(&self, args: &[Val]) -> Result<Val, String> {
        let ctx = self.column.as_ref().ok_or_else(|| {
            trf!("cell 只在列公式里可用", "cell is only available in a column formula")
        })?;
        let index = column_index(ctx.table, arg(args, 0)?)?;
        let row = one_based(arg(args, 1)?, ctx.table.columns[index].len())?;
        Ok(number_at(&ctx.table.columns[index], row))
    }

    fn tablecol_lookup(&self, args: &[Val]) -> Result<Val, String> {
        let ctx = self.column.as_ref().ok_or_else(|| {
            trf!("tablecol 只在列公式里可用", "tablecol is only available in a column formula")
        })?;
        let table_name = expect_str(arg(args, 0)?)?;
        let table = ctx.others.iter().find(|table| table.name == table_name).ok_or_else(|| {
            trf!("找不到表格 {table_name}", "No table named {table_name}", table_name = table_name)
        })?;
        let index = column_index(table, arg(args, 1)?)?;
        Ok(number_at(&table.columns[index], ctx.row))
    }
}

fn need_fit() -> String {
    trf!("先调用 fit()", "Call fit() first")
}

fn arrange_graph(graph: &mut sciplot_core::Graph, rows: usize, cols: usize) {
    let need = rows.saturating_mul(cols).max(1);
    while graph.layers.len() < need {
        graph.add_layer();
    }
    graph.arrangement.rows = rows as u32;
    graph.arrangement.cols = cols as u32;
    graph.reflow_layers();
}

type FitColumns = (Vec<f64>, Vec<f64>, Option<sciplot_core::GraphId>);

fn series_for_fit(project: &Project, target: &Val) -> Result<FitColumns, String> {
    let (table_id, x, y, graph) = match target {
        Val::Graph(id) => {
            let graph = project.graph(*id).ok_or_else(|| t("找不到图形").to_string())?;
            let curve = graph.layers.iter().flat_map(|layer| layer.curves.iter()).find_map(|curve| match &curve.source {
                DataSource::Columns { table, x, y, .. } => Some((*table, *x, *y)),
                _ => None,
            });
            let Some((table, x, y)) = curve else {
                return Err(trf!("这张图上没有表格曲线可以拟合", "This graph has no table curve to fit"));
            };
            (table, x, y, Some(*id))
        }
        Val::Table(id) => {
            let table = project.table(*id).ok_or_else(|| t("找不到表格").to_string())?;
            if table.columns.len() < 2 {
                return Err(trf!("拟合至少要两列", "Fitting needs at least two columns"));
            }
            let x = table.columns.iter().position(|column| column.mode == ColumnMode::X).unwrap_or(0);
            let y = table.columns.iter().enumerate().find(|(index, column)| column.mode == ColumnMode::Y && *index != x).map(|(index, _)| index).unwrap_or(1);
            (*id, x, y, None)
        }
        _ => return Err(trf!("Fit 的第一个参数要是图或表格", "Fit expects a graph or a table")),
    };
    let table = project.table(table_id).ok_or_else(|| t("找不到表格").to_string())?;
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let n = table.columns[x].len().max(table.columns[y].len());
    for row in 0..n {
        let Some(x) = table.columns[x].numbers.get(row).copied().flatten() else { continue };
        let Some(y) = table.columns[y].numbers.get(row).copied().flatten() else { continue };
        if x.is_finite() && y.is_finite() {
            xs.push(x);
            ys.push(y);
        }
    }
    if xs.len() < 2 {
        return Err(trf!("拟合至少要两个点", "Fitting needs at least two points"));
    }
    Ok((xs, ys, graph))
}

fn model_of(name: &str) -> Result<FitModel, String> {
    match name.trim().to_ascii_lowercase().as_str() {
        "linear" | "line" | "线性" => Ok(FitModel::Linear),
        "gaussian" | "gauss" | "高斯" => Ok(FitModel::Gaussian),
        "lorentzian" | "lorentz" | "洛伦兹" => Ok(FitModel::Lorentzian),
        "boltzmann" => Ok(FitModel::Boltzmann),
        "exp" | "exponential" | "指数" => Ok(FitModel::Exponential),
        "expdecay1" => Ok(FitModel::ExpDecay1),
        "expgrowth" => Ok(FitModel::ExpGrowth),
        "poly2" | "polynomial2" => Ok(FitModel::Polynomial(2)),
        "poly3" | "polynomial3" => Ok(FitModel::Polynomial(3)),
        _ => Err(trf!("不认识这个拟合模型：{name}", "Unknown fit model: {name}", name = name)),
    }
}

fn kind_of(name: &str) -> CurveKind {
    match name.trim().to_ascii_lowercase().as_str() {
        "scatter" | "散点" => CurveKind::Scatter,
        "line+symbol" | "点线" => CurveKind::LineSymbol,
        "bars" | "bar" | "柱状" => CurveKind::Bars,
        "area" | "面积" => CurveKind::Area,
        _ => CurveKind::Line,
    }
}

fn format_of(path: &str) -> ExportFormat {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".svg") {
        ExportFormat::Svg
    } else if lower.ends_with(".pdf") {
        ExportFormat::Pdf
    } else if lower.ends_with(".eps") {
        ExportFormat::Eps
    } else {
        ExportFormat::Png
    }
}

fn write_cell(column: &mut sciplot_core::Column, row: usize, value: &Val) -> Result<(), String> {
    match value {
        Val::None => column.set_text(row, ""),
        Val::Bool(true) => column.set_text(row, "1"),
        Val::Bool(false) => column.set_text(row, "0"),
        Val::Int(number) => column.set_text(row, &number.to_string()),
        Val::Float(number) => column.set_text(row, &sciplot_core::format_short(*number)),
        Val::Str(text) => column.set_text(row, text),
        _ => return Err(trf!("单元格只能写入数或文本", "A cell can only store a number or text")),
    }
    Ok(())
}

fn number_at(column: &sciplot_core::Column, row: usize) -> Val {
    column.numbers.get(row).copied().flatten().filter(|value| value.is_finite()).map(Val::Float).unwrap_or(Val::None)
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

fn column_index(table: &Table, key: &Val) -> Result<usize, String> {
    match key {
        Val::Str(name) => table
            .columns
            .iter()
            .position(|column| column.name == *name)
            .or_else(|| letter_index(name))
            .filter(|index| *index < table.columns.len())
            .ok_or_else(|| trf!("找不到列 {name}", "No column named {name}", name = name)),
        other => one_based(other, table.columns.len()),
    }
}

fn one_based(value: &Val, len: usize) -> Result<usize, String> {
    let number = as_count(value)?;
    if number == 0 || number > len {
        return Err(trf!("序号从 1 开始，并且要落在范围内", "Indexes start at 1 and must be in range"));
    }
    Ok(number - 1)
}

fn as_count(value: &Val) -> Result<usize, String> {
    let number = as_f64(value)?;
    if number < 0.0 || number.fract().abs() > 1e-9 {
        return Err(trf!("这里要一个非负整数", "A non-negative integer is required"));
    }
    Ok(number as usize)
}

fn arg(args: &[Val], index: usize) -> Result<&Val, String> {
    args.get(index).ok_or_else(|| trf!("参数不够", "Not enough arguments"))
}

fn expect_str(value: &Val) -> Result<String, String> {
    match value {
        Val::Str(text) => Ok(text.clone()),
        _ => Err(trf!("这里要一段文字", "A string is required")),
    }
}

fn column_ref(table: &sciplot_core::Table, value: &Val) -> Result<usize, String> {
    if let Val::Str(name) = value {
        return table
            .columns
            .iter()
            .position(|column| column.name == *name)
            .ok_or_else(|| trf!("找不到这一列", "That column does not exist"));
    }
    one_based(value, table.columns.len())
}

fn column_values(column: &sciplot_core::Column) -> Val {
    if column.numbers.iter().any(|value| value.is_some()) {
        Val::List(column.numbers.iter().copied().map(|value| value.map(Val::Float).unwrap_or(Val::None)).collect())
    } else {
        Val::List(column.texts.iter().cloned().map(Val::Str).collect())
    }
}

fn expect_list(value: &Val) -> Result<Vec<Val>, String> {
    match value {
        Val::List(items) => Ok(items.clone()),
        _ => Err(trf!("这里要一个列表", "A list is required")),
    }
}

fn iter_values(value: &Val) -> Result<Vec<Val>, String> {
    match value {
        Val::List(items) => Ok(items.clone()),
        Val::Dict(items) => Ok(items.iter().map(|(key, _)| key.clone()).collect()),
        Val::Str(text) => Ok(text.chars().map(|ch| Val::Str(ch.to_string())).collect()),
        _ => Err(trf!("这里要一个列表", "A list is required")),
    }
}

fn set_index(base: &mut Val, index: &Val, value: Val) -> Result<(), String> {
    match base {
        Val::List(items) => {
            let mut at = as_f64(index)?.trunc() as isize;
            if at < 0 {
                at += items.len() as isize;
            }
            if at < 0 || at as usize >= items.len() {
                return Err(trf!("下标超出范围", "Index out of range"));
            }
            items[at as usize] = value;
            Ok(())
        }
        Val::Dict(items) => {
            if let Some(slot) = items.iter_mut().find(|(key, _)| values_equal(key, index)) {
                slot.1 = value;
            } else {
                items.push((index.clone(), value));
            }
            Ok(())
        }
        _ => Err(trf!("只能给列表或字典赋值", "Only lists and dicts can be assigned by index")),
    }
}

fn set_field(object: &Val, name: &str, value: Val) -> Result<(), String> {
    let Val::Object(instance) = object else {
        return Err(trf!("只能给对象的属性赋值", "Only object attributes can be assigned"));
    };
    let mut instance = instance.borrow_mut();
    if let Some(slot) = instance.fields.iter_mut().find(|(key, _)| key == name) {
        slot.1 = value;
    } else {
        instance.fields.push((name.to_string(), value));
    }
    Ok(())
}

fn dict_method(items: &[(Val, Val)], name: &str, args: &[Val]) -> Result<Val, String> {
    match name {
        "keys" => Ok(Val::List(items.iter().map(|(key, _)| key.clone()).collect())),
        "get" => {
            let key = arg(args, 0)?;
            Ok(items.iter().find(|(item, _)| values_equal(item, key)).map(|(_, value)| value.clone()).unwrap_or(Val::None))
        }
        _ => Err(trf!("字典没有这个方法：{name}", "Dict has no method {name}", name = name)),
    }
}

fn known_module(name: &str) -> Result<(), String> {
    match name {
        "math" | "sciplot" | "statistics" | "random" | "json" => Ok(()),
        _ => Err(trf!("没有这个模块：{name}", "No such module: {name}", name = name)),
    }
}

fn module_exports(module: &str) -> Vec<(String, Val)> {
    match module {
        "math" => math_names().iter().map(|name| (name.to_string(), module_attr("math", name, &[]).unwrap_or(Val::None))).collect(),
        "sciplot" => sciplot_names().iter().map(|name| (name.to_string(), Val::Builtin(sciplot_builtin(name)))).collect(),
        "statistics" => ["mean", "median", "stdev"].into_iter().map(|name| (name.to_string(), Val::Builtin(Builtin::MathFn(name)))).collect(),
        "random" => vec![("random".into(), Val::Builtin(Builtin::MathFn("random"))), ("randint".into(), Val::Builtin(Builtin::MathFn("randint")))],
        "json" => vec![("dumps".into(), Val::Builtin(Builtin::MathFn("dumps"))), ("loads".into(), Val::Builtin(Builtin::MathFn("loads")))],
        _ => Vec::new(),
    }
}

fn intern_name(name: &str) -> Option<&'static str> {
    [
        "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "sinh", "cosh", "tanh", "exp", "log", "log10", "sqrt", "fabs", "floor", "ceil", "pow",
        "hypot", "degrees", "radians", "mean", "median", "stdev", "random", "randint", "dumps", "loads",
    ]
    .into_iter()
    .find(|item| *item == name)
}

fn math_names() -> &'static [&'static str] {
    &[
        "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "sinh", "cosh", "tanh", "exp", "log", "log10", "sqrt", "fabs", "floor", "ceil",
        "pow", "hypot", "degrees", "radians", "pi", "e",
    ]
}

fn sciplot_names() -> &'static [&'static str] {
    &[
        "table", "graph", "matrix", "note", "folder", "newTable", "newGraph", "newMatrix", "newNote", "newFolder", "plot", "Fit", "convolve",
        "deconvolve", "correlate", "printPdf", "arrangeLayers", "col", "cell", "tablecol", "tableList", "graphList", "matrixList", "noteList",
        "saveProject",
    ]
}

fn sciplot_builtin(name: &str) -> Builtin {
    match name {
        "table" => Builtin::Table,
        "graph" => Builtin::Graph,
        "matrix" => Builtin::Matrix,
        "note" => Builtin::Note,
        "folder" => Builtin::Folder,
        "newTable" => Builtin::NewTable,
        "newGraph" => Builtin::NewGraph,
        "newMatrix" => Builtin::NewMatrix,
        "newNote" => Builtin::NewNote,
        "newFolder" => Builtin::NewFolder,
        "plot" => Builtin::Plot,
        "Fit" => Builtin::Fit,
        "convolve" => Builtin::Convolve,
        "deconvolve" => Builtin::Deconvolve,
        "correlate" => Builtin::Correlate,
        "printPdf" => Builtin::PrintPdf,
        "arrangeLayers" => Builtin::Arrange,
        "col" => Builtin::Col,
        "cell" => Builtin::Cell,
        "tablecol" => Builtin::TableCol,
        "tableList" => Builtin::TableList,
        "graphList" => Builtin::GraphList,
        "matrixList" => Builtin::MatrixList,
        "noteList" => Builtin::NoteList,
        "saveProject" => Builtin::SaveProject,
        _ => Builtin::Print,
    }
}

fn module_attr(module: &str, name: &str, args: &[Val]) -> Result<Val, String> {
    if !args.is_empty() {
        return call_module(module, name, args);
    }
    match (module, name) {
        ("math", "pi") => Ok(Val::Float(std::f64::consts::PI)),
        ("math", "e") => Ok(Val::Float(std::f64::consts::E)),
        ("math" | "statistics" | "random" | "json", _) => {
            let interned = intern_name(name).ok_or_else(|| trf!("模块里没有这个名字：{name}", "The module has no name {name}", name = name))?;
            Ok(Val::Builtin(Builtin::MathFn(interned)))
        }
        ("sciplot", _) if sciplot_names().contains(&name) => Ok(Val::Builtin(sciplot_builtin(name))),
        _ => Err(trf!("模块里没有这个名字：{name}", "The module has no name {name}", name = name)),
    }
}

fn call_module(module: &str, name: &str, args: &[Val]) -> Result<Val, String> {
    match module {
        "math" | "statistics" | "random" | "json" => call_named(name, args),
        "sciplot" => Err(trf!("请直接调用这个函数", "Call this function directly")),
        _ => Err(trf!("模块里没有这个名字：{name}", "The module has no name {name}", name = name)),
    }
}

fn call_named(name: &str, args: &[Val]) -> Result<Val, String> {
    match name {
        "dumps" => Ok(Val::Str(json_dumps(arg(args, 0)?))),
        "loads" => json_loads(&expect_str(arg(args, 0)?)?),
        "randint" => {
            let low = as_f64(arg(args, 0)?)?.trunc() as i64;
            let high = as_f64(arg(args, 1)?)?.trunc() as i64;
            if high < low {
                return Err(trf!("randint 的范围不对", "randint range is empty"));
            }
            let span = (high - low + 1).max(1) as u64;
            Ok(Val::Int(low + (random_unit() * span as f64) as i64))
        }
        "random" => Ok(Val::Float(random_unit())),
        "mean" | "median" | "stdev" => Ok(Val::Float(stats_call(name, args)?)),
        _ => Ok(Val::Float(math_call(name, args)?)),
    }
}

fn random_unit() -> f64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static STATE: AtomicU64 = AtomicU64::new(0x1234_5678);
    let mut x = STATE.load(Ordering::Relaxed);
    x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
    STATE.store(x, Ordering::Relaxed);
    ((x >> 11) as f64) / ((1u64 << 53) as f64)
}

fn stats_call(name: &str, args: &[Val]) -> Result<f64, String> {
    let mut values = as_f64_list(arg(args, 0)?)?;
    if values.is_empty() {
        return Err(trf!("统计需要至少一个数", "Statistics need at least one number"));
    }
    match name {
        "mean" => Ok(values.iter().sum::<f64>() / values.len() as f64),
        "median" => {
            values.sort_by(|a, b| a.total_cmp(b));
            let mid = values.len() / 2;
            if values.len() % 2 == 0 {
                Ok((values[mid - 1] + values[mid]) * 0.5)
            } else {
                Ok(values[mid])
            }
        }
        "stdev" => {
            if values.len() < 2 {
                return Err(trf!("标准差至少要两个数", "Standard deviation needs two numbers"));
            }
            let mean = values.iter().sum::<f64>() / values.len() as f64;
            let var = values.iter().map(|value| (value - mean).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
            Ok(var.sqrt())
        }
        _ => Err(trf!("没有这个统计函数", "No such statistics function")),
    }
}

fn json_dumps(value: &Val) -> String {
    match value {
        Val::None => "null".into(),
        Val::Bool(true) => "true".into(),
        Val::Bool(false) => "false".into(),
        Val::Int(number) => number.to_string(),
        Val::Float(number) => {
            if number.is_finite() {
                format!("{number}")
            } else {
                "null".into()
            }
        }
        Val::Str(text) => format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\"")),
        Val::List(items) => {
            let parts = items.iter().map(json_dumps).collect::<Vec<_>>().join(",");
            format!("[{parts}]")
        }
        Val::Dict(items) => {
            let parts = items
                .iter()
                .map(|(key, value)| format!("{}:{}", json_dumps(key), json_dumps(value)))
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{parts}}}")
        }
        other => format!("\"{}\"", display(other).replace('"', "\\\"")),
    }
}

fn json_loads(text: &str) -> Result<Val, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut index = 0;
    let value = parse_json(&chars, &mut index)?;
    skip_json(&chars, &mut index);
    if index != chars.len() {
        return Err(trf!("JSON 没有解析完", "JSON was not fully parsed"));
    }
    Ok(value)
}

fn skip_json(chars: &[char], index: &mut usize) {
    while *index < chars.len() && chars[*index].is_whitespace() {
        *index += 1;
    }
}

fn parse_json(chars: &[char], index: &mut usize) -> Result<Val, String> {
    skip_json(chars, index);
    if *index >= chars.len() {
        return Err(trf!("JSON 是空的", "JSON is empty"));
    }
    match chars[*index] {
        'n' => {
            *index += 4;
            Ok(Val::None)
        }
        't' => {
            *index += 4;
            Ok(Val::Bool(true))
        }
        'f' => {
            *index += 5;
            Ok(Val::Bool(false))
        }
        '"' => Ok(Val::Str(parse_json_string(chars, index)?)),
        '[' => {
            *index += 1;
            let mut items = Vec::new();
            skip_json(chars, index);
            if *index < chars.len() && chars[*index] == ']' {
                *index += 1;
                return Ok(Val::List(items));
            }
            loop {
                items.push(parse_json(chars, index)?);
                skip_json(chars, index);
                if *index < chars.len() && chars[*index] == ',' {
                    *index += 1;
                    continue;
                }
                if *index < chars.len() && chars[*index] == ']' {
                    *index += 1;
                    break;
                }
                return Err(trf!("JSON 数组不对", "Bad JSON array"));
            }
            Ok(Val::List(items))
        }
        '{' => {
            *index += 1;
            let mut items = Vec::new();
            skip_json(chars, index);
            if *index < chars.len() && chars[*index] == '}' {
                *index += 1;
                return Ok(Val::Dict(items));
            }
            loop {
                let key = parse_json(chars, index)?;
                skip_json(chars, index);
                if *index >= chars.len() || chars[*index] != ':' {
                    return Err(trf!("JSON 对象不对", "Bad JSON object"));
                }
                *index += 1;
                let value = parse_json(chars, index)?;
                items.push((key, value));
                skip_json(chars, index);
                if *index < chars.len() && chars[*index] == ',' {
                    *index += 1;
                    continue;
                }
                if *index < chars.len() && chars[*index] == '}' {
                    *index += 1;
                    break;
                }
                return Err(trf!("JSON 对象不对", "Bad JSON object"));
            }
            Ok(Val::Dict(items))
        }
        _ => {
            let start = *index;
            if chars[*index] == '-' {
                *index += 1;
            }
            while *index < chars.len() && (chars[*index].is_ascii_digit() || chars[*index] == '.' || chars[*index] == 'e' || chars[*index] == 'E' || chars[*index] == '+') {
                *index += 1;
            }
            let text: String = chars[start..*index].iter().collect();
            if text.contains('.') || text.contains('e') || text.contains('E') {
                Ok(Val::Float(text.parse().map_err(|_| trf!("JSON 数字不对", "Bad JSON number"))?))
            } else {
                Ok(Val::Int(text.parse().map_err(|_| trf!("JSON 数字不对", "Bad JSON number"))?))
            }
        }
    }
}

fn parse_json_string(chars: &[char], index: &mut usize) -> Result<String, String> {
    *index += 1;
    let mut out = String::new();
    while *index < chars.len() {
        let ch = chars[*index];
        *index += 1;
        if ch == '"' {
            return Ok(out);
        }
        if ch == '\\' && *index < chars.len() {
            let next = chars[*index];
            *index += 1;
            out.push(match next {
                'n' => '\n',
                't' => '\t',
                other => other,
            });
        } else {
            out.push(ch);
        }
    }
    Err(trf!("JSON 字符串没有结束", "JSON string is not closed"))
}

fn as_f64_list(value: &Val) -> Result<Vec<f64>, String> {
    let items = expect_list(value)?;
    items.iter().map(as_f64).collect()
}

fn number_list(values: &[f64]) -> Vec<Val> {
    values.iter().copied().map(Val::Float).collect()
}

fn as_optional_number(value: &Val) -> Result<Option<f64>, String> {
    match value {
        Val::None => Ok(None),
        other => {
            let number = as_f64(other)?;
            if number.is_finite() {
                Ok(Some(number))
            } else {
                Ok(None)
            }
        }
    }
}

fn as_f64(value: &Val) -> Result<f64, String> {
    match value {
        Val::Int(number) => Ok(*number as f64),
        Val::Float(number) => Ok(*number),
        Val::Bool(true) => Ok(1.0),
        Val::Bool(false) => Ok(0.0),
        Val::Str(text) => text.trim().parse::<f64>().map_err(|_| trf!("这不是数", "That is not a number")),
        _ => Err(trf!("这不是数", "That is not a number")),
    }
}

fn truthy(value: &Val) -> bool {
    match value {
        Val::None => false,
        Val::Bool(flag) => *flag,
        Val::Int(number) => *number != 0,
        Val::Float(number) => *number != 0.0 && number.is_finite(),
        Val::Str(text) => !text.is_empty(),
        Val::List(items) => !items.is_empty(),
        Val::Dict(items) => !items.is_empty(),
        _ => true,
    }
}

fn values_equal(left: &Val, right: &Val) -> bool {
    match (left, right) {
        (Val::None, Val::None) => true,
        (Val::Bool(a), Val::Bool(b)) => a == b,
        (Val::Str(a), Val::Str(b)) => a == b,
        (Val::Int(_) | Val::Float(_) | Val::Bool(_), Val::Int(_) | Val::Float(_) | Val::Bool(_)) => {
            as_f64(left).ok() == as_f64(right).ok()
        }
        (Val::List(a), Val::List(b)) => a.len() == b.len() && a.iter().zip(b).all(|(a, b)| values_equal(a, b)),
        (Val::Dict(a), Val::Dict(b)) => {
            a.len() == b.len() && a.iter().all(|(key, value)| b.iter().any(|(other, other_value)| values_equal(key, other) && values_equal(value, other_value)))
        }
        _ => false,
    }
}

fn compare_values(left: &Val, right: &Val, op: &str) -> Result<Val, String> {
    let result = if let (Val::Str(a), Val::Str(b)) = (left, right) {
        match op {
            "<" => a < b,
            ">" => a > b,
            "<=" => a <= b,
            ">=" => a >= b,
            _ => false,
        }
    } else {
        let (a, b) = (as_f64(left)?, as_f64(right)?);
        match op {
            "<" => a < b,
            ">" => a > b,
            "<=" => a <= b,
            ">=" => a >= b,
            _ => false,
        }
    };
    Ok(Val::Bool(result))
}

fn add_values(left: &Val, right: &Val) -> Result<Val, String> {
    match (left, right) {
        (Val::Int(a), Val::Int(b)) => Ok(Val::Int(a + b)),
        (Val::Str(a), Val::Str(b)) => Ok(Val::Str(format!("{a}{b}"))),
        (Val::List(a), Val::List(b)) => {
            let mut out = a.clone();
            out.extend(b.iter().cloned());
            Ok(Val::List(out))
        }
        _ => num_bin(left, right, |a, b| a + b, false),
    }
}

fn mul_values(left: &Val, right: &Val) -> Result<Val, String> {
    match (left, right) {
        (Val::Int(a), Val::Int(b)) => Ok(Val::Int(a * b)),
        (Val::Str(text), Val::Int(count)) | (Val::Int(count), Val::Str(text)) if *count >= 0 => {
            Ok(Val::Str(text.repeat(*count as usize)))
        }
        _ => num_bin(left, right, |a, b| a * b, false),
    }
}

fn num_bin(left: &Val, right: &Val, op: impl Fn(f64, f64) -> f64, keep_int: bool) -> Result<Val, String> {
    if keep_int {
        if let (Val::Int(a), Val::Int(b)) = (left, right) {
            return Ok(Val::Int(a - b));
        }
    }
    Ok(Val::Float(op(as_f64(left)?, as_f64(right)?)))
}

fn len_of(value: Option<&Val>) -> Result<usize, String> {
    match value {
        Some(Val::List(items)) => Ok(items.len()),
        Some(Val::Dict(items)) => Ok(items.len()),
        Some(Val::Str(text)) => Ok(text.chars().count()),
        _ => Err(trf!("len 需要列表或文字", "len needs a list or a string")),
    }
}

fn index_value(base: &Val, index: &Val) -> Result<Val, String> {
    if let Val::Dict(items) = base {
        return items
            .iter()
            .find(|(key, _)| values_equal(key, index))
            .map(|(_, value)| value.clone())
            .ok_or_else(|| trf!("字典里没有这个键", "Dict has no such key"));
    }
    if let Val::Str(text) = base {
        let chars: Vec<char> = text.chars().collect();
        let mut at = as_f64(index)?.trunc() as isize;
        if at < 0 {
            at += chars.len() as isize;
        }
        if at < 0 || at as usize >= chars.len() {
            return Err(trf!("下标超出范围", "Index out of range"));
        }
        return Ok(Val::Str(chars[at as usize].to_string()));
    }
    let Val::List(items) = base else {
        return Err(trf!("只能给列表取下标", "Only lists can be indexed"));
    };
    let mut at = as_f64(index)?.trunc() as isize;
    if at < 0 {
        at += items.len() as isize;
    }
    if at < 0 || at as usize >= items.len() {
        return Err(trf!("下标超出范围", "Index out of range"));
    }
    Ok(items[at as usize].clone())
}

fn builtin_range(args: &[Val]) -> Result<Val, String> {
    let (start, stop, step) = match args.len() {
        1 => (0.0, as_f64(arg(args, 0)?)?, 1.0),
        2 => (as_f64(arg(args, 0)?)?, as_f64(arg(args, 1)?)?, 1.0),
        _ => (as_f64(arg(args, 0)?)?, as_f64(arg(args, 1)?)?, as_f64(arg(args, 2)?)?),
    };
    if step == 0.0 {
        return Err(trf!("range 的步长不能是 0", "range step cannot be 0"));
    }
    let mut out = Vec::new();
    let mut value = start;
    while out.len() < STEP_LIMIT && ((step > 0.0 && value < stop) || (step < 0.0 && value > stop)) {
        out.push(Val::Int(value.trunc() as i64));
        value += step;
    }
    Ok(Val::List(out))
}

fn math_call(name: &str, args: &[Val]) -> Result<f64, String> {
    let x = as_f64(arg(args, 0)?)?;
    let y = if args.len() >= 2 { as_f64(arg(args, 1)?)? } else { 0.0 };
    let value = match name {
        "sin" => x.sin(),
        "cos" => x.cos(),
        "tan" => x.tan(),
        "sqrt" => x.sqrt(),
        "log" | "ln" => x.ln(),
        "log10" => x.log10(),
        "exp" => x.exp(),
        "pow" => x.powf(y),
        "asin" => x.asin(),
        "acos" => x.acos(),
        "atan" => x.atan(),
        "atan2" => x.atan2(y),
        "sinh" => x.sinh(),
        "cosh" => x.cosh(),
        "tanh" => x.tanh(),
        "fabs" => x.abs(),
        "floor" => x.floor(),
        "ceil" => x.ceil(),
        "hypot" => x.hypot(y),
        "degrees" => x.to_degrees(),
        "radians" => x.to_radians(),
        _ => return Err(trf!("没有这个数学函数", "No such math function")),
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err(trf!("数学函数的结果不是有限数", "The math function did not return a finite number"))
    }
}

fn display(value: &Val) -> String {
    match value {
        Val::None => "None".into(),
        Val::Bool(true) => "True".into(),
        Val::Bool(false) => "False".into(),
        Val::Int(number) => number.to_string(),
        Val::Float(number) => {
            if number.is_finite() && number.fract() == 0.0 && number.abs() < 1e15 {
                format!("{}", *number as i64)
            } else {
                sciplot_core::format_short(*number)
            }
        }
        Val::Str(text) => text.clone(),
        Val::List(items) => {
            let parts = items.iter().map(display).collect::<Vec<_>>().join(", ");
            format!("[{parts}]")
        }
        Val::Dict(items) => {
            let parts = items.iter().map(|(key, value)| format!("{}: {}", display(key), display(value))).collect::<Vec<_>>().join(", ");
            format!("{{{parts}}}")
        }
        Val::Class(class) => format!("<class {}>", class.name),
        Val::Object(instance) => format!("<{}>", instance.borrow().class),
        Val::Bound { method, .. } => format!("<method {method}>"),
        Val::Module(name) => format!("<module {name}>"),
        Val::Table(_) => "<table>".into(),
        Val::Matrix(_) => "<matrix>".into(),
        Val::Graph(_) => "<graph>".into(),
        Val::Note(_) => "<note>".into(),
        Val::Folder(_) => "<folder>".into(),
        Val::Fit(_) => "<fit>".into(),
        Val::Func(_) | Val::Builtin(_) => "<function>".into(),
    }
}

#[derive(Clone, Debug)]
enum Val {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<Val>),
    Dict(Vec<(Val, Val)>),
    Class(ClassDef),
    Object(Rc<RefCell<Instance>>),
    Bound { receiver: Box<Val>, method: String },
    Module(String),
    Table(sciplot_core::TableId),
    Matrix(sciplot_core::MatrixId),
    Graph(sciplot_core::GraphId),
    Note(sciplot_core::NoteId),
    Folder(sciplot_core::FolderId),
    Fit(usize),
    Func(FuncDef),
    Builtin(Builtin),
}

#[derive(Clone, Copy, Debug)]
enum Builtin {
    Print,
    Range,
    Len,
    StrFn,
    IntFn,
    FloatFn,
    Abs,
    MathFn(&'static str),
    Table,
    Graph,
    Matrix,
    Note,
    Folder,
    NewTable,
    NewGraph,
    NewMatrix,
    NewNote,
    NewFolder,
    Plot,
    Fit,
    Convolve,
    Deconvolve,
    Correlate,
    PrintPdf,
    Arrange,
    Col,
    Cell,
    TableCol,
    TableList,
    GraphList,
    MatrixList,
    NoteList,
    SaveProject,
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    Table,
    Graph,
    Matrix,
    Note,
    Folder,
}

#[derive(Clone, Debug)]
struct FitObj {
    target: Box<Val>,
    model: String,
    result: Option<FitSnap>,
}

#[derive(Clone, Debug)]
struct FitSnap {
    r2: f64,
    chi2: f64,
    expr: String,
    values: Vec<f64>,
}

#[derive(Clone, Debug)]
struct FuncDef {
    params: Vec<String>,
    body: Vec<Stmt>,
}

#[derive(Clone, Debug)]
struct ClassDef {
    name: String,
    methods: Vec<(String, FuncDef)>,
}

#[derive(Clone, Debug)]
struct Instance {
    class: String,
    methods: Vec<(String, FuncDef)>,
    fields: Vec<(String, Val)>,
}

struct Env {
    frames: Vec<HashMap<String, Val>>,
}

impl Env {
    fn with_builtins() -> Self {
        let mut map = HashMap::new();
        let bind = |map: &mut HashMap<String, Val>, name: &str, builtin: Builtin| {
            map.insert(name.to_string(), Val::Builtin(builtin));
        };
        bind(&mut map, "print", Builtin::Print);
        bind(&mut map, "range", Builtin::Range);
        bind(&mut map, "len", Builtin::Len);
        bind(&mut map, "str", Builtin::StrFn);
        bind(&mut map, "int", Builtin::IntFn);
        bind(&mut map, "float", Builtin::FloatFn);
        bind(&mut map, "abs", Builtin::Abs);
        bind(&mut map, "sin", Builtin::MathFn("sin"));
        bind(&mut map, "cos", Builtin::MathFn("cos"));
        bind(&mut map, "tan", Builtin::MathFn("tan"));
        bind(&mut map, "sqrt", Builtin::MathFn("sqrt"));
        bind(&mut map, "log", Builtin::MathFn("log"));
        bind(&mut map, "log10", Builtin::MathFn("log10"));
        bind(&mut map, "exp", Builtin::MathFn("exp"));
        bind(&mut map, "pow", Builtin::MathFn("pow"));
        bind(&mut map, "table", Builtin::Table);
        bind(&mut map, "graph", Builtin::Graph);
        bind(&mut map, "matrix", Builtin::Matrix);
        bind(&mut map, "note", Builtin::Note);
        bind(&mut map, "folder", Builtin::Folder);
        bind(&mut map, "newTable", Builtin::NewTable);
        bind(&mut map, "newGraph", Builtin::NewGraph);
        bind(&mut map, "newMatrix", Builtin::NewMatrix);
        bind(&mut map, "newNote", Builtin::NewNote);
        bind(&mut map, "newFolder", Builtin::NewFolder);
        bind(&mut map, "plot", Builtin::Plot);
        bind(&mut map, "Fit", Builtin::Fit);
        bind(&mut map, "convolve", Builtin::Convolve);
        bind(&mut map, "deconvolve", Builtin::Deconvolve);
        bind(&mut map, "correlate", Builtin::Correlate);
        bind(&mut map, "printPdf", Builtin::PrintPdf);
        bind(&mut map, "arrangeLayers", Builtin::Arrange);
        bind(&mut map, "col", Builtin::Col);
        bind(&mut map, "cell", Builtin::Cell);
        bind(&mut map, "tablecol", Builtin::TableCol);
        bind(&mut map, "tableList", Builtin::TableList);
        bind(&mut map, "graphList", Builtin::GraphList);
        bind(&mut map, "matrixList", Builtin::MatrixList);
        bind(&mut map, "noteList", Builtin::NoteList);
        bind(&mut map, "saveProject", Builtin::SaveProject);
        map.insert("pi".into(), Val::Float(std::f64::consts::PI));
        map.insert("True".into(), Val::Bool(true));
        map.insert("False".into(), Val::Bool(false));
        map.insert("None".into(), Val::None);
        Self { frames: vec![map] }
    }

    fn push(&mut self) {
        self.frames.push(HashMap::new());
    }

    fn pop(&mut self) {
        if self.frames.len() > 1 {
            self.frames.pop();
        }
    }

    fn get(&self, name: &str) -> Option<Val> {
        for frame in self.frames.iter().rev() {
            if let Some(value) = frame.get(name) {
                return Some(value.clone());
            }
        }
        None
    }

    fn set(&mut self, name: &str, value: Val) {
        if let Some(frame) = self.frames.last_mut() {
            frame.insert(name.to_string(), value);
        }
    }
}

#[derive(Clone, Debug)]
enum Signal {
    None(Val),
    Return(Val),
    Break,
    Continue,
    Raise(String),
}

#[derive(Clone, Debug)]
enum Stmt {
    Pass,
    ImportModules(Vec<String>),
    FromImport { module: String, names: Vec<String> },
    Expr(Expr),
    Assign { name: String, expr: Expr },
    SetIndex { base: Expr, index: Expr, expr: Expr },
    SetAttr { base: Expr, name: String, expr: Expr },
    If { arms: Vec<(Expr, Vec<Stmt>)>, else_body: Vec<Stmt> },
    While { cond: Expr, body: Vec<Stmt> },
    For { name: String, iter: Expr, body: Vec<Stmt> },
    Def { name: String, params: Vec<String>, body: Vec<Stmt> },
    Class { name: String, methods: Vec<(String, FuncDef)> },
    Try { body: Vec<Stmt>, name: Option<String>, handler: Vec<Stmt> },
    Raise(Expr),
    Return(Option<Expr>),
    Break,
    Continue,
}

#[derive(Clone, Debug)]
enum Expr {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Name(String),
    List(Vec<Expr>),
    Dict(Vec<(Expr, Expr)>),
    Unary { op: &'static str, expr: Box<Expr> },
    Binary { op: &'static str, left: Box<Expr>, right: Box<Expr> },
    Attr { base: Box<Expr>, name: String },
    Index { base: Box<Expr>, index: Box<Expr> },
    Call { callee: Box<Expr>, args: Vec<Expr> },
}

fn parse(source: &str) -> Result<Vec<Stmt>, String> {
    let tokens = tokenize(source)?;
    let mut parser = Parser { tokens, index: 0 };
    let stmts = parser.parse_block()?;
    parser.skip_newlines();
    if !parser.eof() {
        return Err(trf!("脚本没有解析完", "The script was not fully parsed"));
    }
    Ok(stmts)
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
}

impl Parser {
    fn eof(&self) -> bool {
        self.index >= self.tokens.len()
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn bump(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.index).cloned();
        if token.is_some() {
            self.index += 1;
        }
        token
    }

    fn eat(&mut self, token: &Token) -> bool {
        if self.peek() == Some(token) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn skip_newlines(&mut self) {
        while self.eat(&Token::Newline) {}
    }

    fn expect(&mut self, token: &Token) -> Result<(), String> {
        if self.eat(token) {
            Ok(())
        } else {
            Err(trf!("脚本语法不对", "The script syntax is not valid"))
        }
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, String> {
        let mut stmts = Vec::new();
        self.skip_newlines();
        while !self.eof() && self.peek() != Some(&Token::Dedent) {
            stmts.push(self.parse_stmt()?);
            self.skip_newlines();
        }
        Ok(stmts)
    }

    fn parse_suite(&mut self) -> Result<Vec<Stmt>, String> {
        if self.peek() == Some(&Token::Newline) || self.peek() == Some(&Token::Indent) {
            self.skip_newlines();
            self.expect(&Token::Indent)?;
            let body = self.parse_block()?;
            self.expect(&Token::Dedent)?;
            Ok(body)
        } else {
            Ok(vec![self.parse_stmt()?])
        }
    }

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        match self.peek() {
            Some(Token::If) => self.parse_if(),
            Some(Token::For) => self.parse_for(),
            Some(Token::While) => self.parse_while(),
            Some(Token::Def) => self.parse_def(),
            Some(Token::Return) => {
                self.bump();
                if matches!(self.peek(), Some(Token::Newline) | Some(Token::Dedent) | None) {
                    Ok(Stmt::Return(None))
                } else {
                    Ok(Stmt::Return(Some(self.parse_expr()?)))
                }
            }
            Some(Token::Break) => {
                self.bump();
                Ok(Stmt::Break)
            }
            Some(Token::Continue) => {
                self.bump();
                Ok(Stmt::Continue)
            }
            Some(Token::Pass) => {
                self.bump();
                Ok(Stmt::Pass)
            }
            Some(Token::Import) => self.parse_import(),
            Some(Token::From) => self.parse_from(),
            Some(Token::Class) => self.parse_class(),
            Some(Token::Try) => self.parse_try(),
            Some(Token::Raise) => {
                self.bump();
                Ok(Stmt::Raise(self.parse_expr()?))
            }
            _ => self.parse_assign_or_expr(),
        }
    }

    fn parse_if(&mut self) -> Result<Stmt, String> {
        self.expect(&Token::If)?;
        let cond = self.parse_expr()?;
        self.expect(&Token::Colon)?;
        let body = self.parse_suite()?;
        let mut arms = vec![(cond, body)];
        while self.peek() == Some(&Token::Elif) {
            self.bump();
            let cond = self.parse_expr()?;
            self.expect(&Token::Colon)?;
            arms.push((cond, self.parse_suite()?));
        }
        let else_body = if self.eat(&Token::Else) {
            self.expect(&Token::Colon)?;
            self.parse_suite()?
        } else {
            Vec::new()
        };
        Ok(Stmt::If { arms, else_body })
    }

    fn parse_for(&mut self) -> Result<Stmt, String> {
        self.expect(&Token::For)?;
        let name = self.expect_ident()?;
        self.expect(&Token::In)?;
        let iter = self.parse_expr()?;
        self.expect(&Token::Colon)?;
        let body = self.parse_suite()?;
        Ok(Stmt::For { name, iter, body })
    }

    fn parse_while(&mut self) -> Result<Stmt, String> {
        self.expect(&Token::While)?;
        let cond = self.parse_expr()?;
        self.expect(&Token::Colon)?;
        let body = self.parse_suite()?;
        Ok(Stmt::While { cond, body })
    }

    fn parse_def(&mut self) -> Result<Stmt, String> {
        self.expect(&Token::Def)?;
        let name = self.expect_ident()?;
        self.expect(&Token::LParen)?;
        let mut params = Vec::new();
        if !self.eat(&Token::RParen) {
            loop {
                params.push(self.expect_ident()?);
                if self.eat(&Token::RParen) {
                    break;
                }
                self.expect(&Token::Comma)?;
            }
        }
        self.expect(&Token::Colon)?;
        let body = self.parse_suite()?;
        Ok(Stmt::Def { name, params, body })
    }

    fn parse_assign_or_expr(&mut self) -> Result<Stmt, String> {
        let expr = self.parse_expr()?;
        if self.eat(&Token::Assign) {
            let value = self.parse_expr()?;
            return match expr {
                Expr::Name(name) => Ok(Stmt::Assign { name, expr: value }),
                Expr::Index { base, index } => Ok(Stmt::SetIndex { base: *base, index: *index, expr: value }),
                Expr::Attr { base, name } => Ok(Stmt::SetAttr { base: *base, name, expr: value }),
                _ => Err(trf!("赋值的左边要是名字", "The left side of an assignment must be a name")),
            };
        }
        Ok(Stmt::Expr(expr))
    }

    fn parse_import(&mut self) -> Result<Stmt, String> {
        self.expect(&Token::Import)?;
        let mut names = vec![self.expect_ident()?];
        while self.eat(&Token::Comma) {
            names.push(self.expect_ident()?);
        }
        Ok(Stmt::ImportModules(names))
    }

    fn parse_from(&mut self) -> Result<Stmt, String> {
        self.expect(&Token::From)?;
        let module = self.expect_ident()?;
        self.expect(&Token::Import)?;
        let names = if self.eat(&Token::Op(BinOp::Mul)) {
            Vec::new()
        } else {
            let mut names = vec![self.expect_ident()?];
            while self.eat(&Token::Comma) {
                names.push(self.expect_ident()?);
            }
            names
        };
        Ok(Stmt::FromImport { module, names })
    }

    fn parse_class(&mut self) -> Result<Stmt, String> {
        self.expect(&Token::Class)?;
        let name = self.expect_ident()?;
        self.expect(&Token::Colon)?;
        let body = self.parse_suite()?;
        let mut methods = Vec::new();
        for stmt in body {
            if let Stmt::Def { name, params, body } = stmt {
                methods.push((name, FuncDef { params, body }));
            } else if matches!(stmt, Stmt::Pass) {
                continue;
            } else {
                return Err(trf!("类里面目前只能写方法", "A class body can only contain methods"));
            }
        }
        Ok(Stmt::Class { name, methods })
    }

    fn parse_try(&mut self) -> Result<Stmt, String> {
        self.expect(&Token::Try)?;
        self.expect(&Token::Colon)?;
        let body = self.parse_suite()?;
        self.expect(&Token::Except)?;
        let name = if self.eat(&Token::As) {
            Some(self.expect_ident()?)
        } else if matches!(self.peek(), Some(Token::Ident(_))) {
            let ident = self.expect_ident()?;
            if self.eat(&Token::As) {
                Some(self.expect_ident()?)
            } else {
                Some(ident)
            }
        } else {
            None
        };
        self.expect(&Token::Colon)?;
        let handler = self.parse_suite()?;
        Ok(Stmt::Try { body, name, handler })
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_and()?;
        while self.eat(&Token::Or) {
            left = Expr::Binary { op: "or", left: Box::new(left), right: Box::new(self.parse_and()?) };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_not()?;
        while self.eat(&Token::And) {
            left = Expr::Binary { op: "and", left: Box::new(left), right: Box::new(self.parse_not()?) };
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<Expr, String> {
        if self.eat(&Token::Not) {
            Ok(Expr::Unary { op: "not", expr: Box::new(self.parse_not()?) })
        } else {
            self.parse_cmp()
        }
    }

    fn parse_cmp(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_sum()?;
        loop {
            let op = match self.peek() {
                Some(Token::Op(BinOp::Eq)) => "==",
                Some(Token::Op(BinOp::Ne)) => "!=",
                Some(Token::Op(BinOp::Lt)) => "<",
                Some(Token::Op(BinOp::Gt)) => ">",
                Some(Token::Op(BinOp::Le)) => "<=",
                Some(Token::Op(BinOp::Ge)) => ">=",
                _ => break,
            };
            self.bump();
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(self.parse_sum()?) };
        }
        Ok(left)
    }

    fn parse_sum(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_term()?;
        loop {
            let op = match self.peek() {
                Some(Token::Op(BinOp::Add)) => "+",
                Some(Token::Op(BinOp::Sub)) => "-",
                _ => break,
            };
            self.bump();
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(self.parse_term()?) };
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Some(Token::Op(BinOp::Mul)) => "*",
                Some(Token::Op(BinOp::Div)) => "/",
                Some(Token::Op(BinOp::Floor)) => "//",
                Some(Token::Op(BinOp::Mod)) => "%",
                _ => break,
            };
            self.bump();
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(self.parse_unary()?) };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        if self.eat(&Token::Op(BinOp::Add)) {
            Ok(Expr::Unary { op: "+", expr: Box::new(self.parse_unary()?) })
        } else if self.eat(&Token::Op(BinOp::Sub)) {
            Ok(Expr::Unary { op: "-", expr: Box::new(self.parse_unary()?) })
        } else {
            self.parse_power()
        }
    }

    fn parse_power(&mut self) -> Result<Expr, String> {
        let left = self.parse_postfix()?;
        if self.eat(&Token::Op(BinOp::Pow)) {
            Ok(Expr::Binary { op: "**", left: Box::new(left), right: Box::new(self.parse_unary()?) })
        } else {
            Ok(left)
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.eat(&Token::Dot) {
                expr = Expr::Attr { base: Box::new(expr), name: self.expect_ident()? };
            } else if self.eat(&Token::LParen) {
                let args = self.parse_args()?;
                expr = Expr::Call { callee: Box::new(expr), args };
            } else if self.eat(&Token::LBrack) {
                let index = self.parse_expr()?;
                self.expect(&Token::RBrack)?;
                expr = Expr::Index { base: Box::new(expr), index: Box::new(index) };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_args(&mut self) -> Result<Vec<Expr>, String> {
        let mut args = Vec::new();
        if self.eat(&Token::RParen) {
            return Ok(args);
        }
        loop {
            args.push(self.parse_expr()?);
            if self.eat(&Token::RParen) {
                break;
            }
            self.expect(&Token::Comma)?;
        }
        Ok(args)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.bump() {
            Some(Token::Number(text)) => {
                if text.contains('.') || text.contains('e') || text.contains('E') {
                    let number = text.parse::<f64>().map_err(|_| trf!("数字写错了", "Bad number"))?;
                    Ok(Expr::Float(number))
                } else {
                    let number = text.parse::<i64>().map_err(|_| trf!("数字写错了", "Bad number"))?;
                    Ok(Expr::Int(number))
                }
            }
            Some(Token::String(text)) => Ok(Expr::Str(text)),
            Some(Token::True) => Ok(Expr::Bool(true)),
            Some(Token::False) => Ok(Expr::Bool(false)),
            Some(Token::NoneKw) => Ok(Expr::None),
            Some(Token::Ident(name)) => Ok(Expr::Name(name)),
            Some(Token::LParen) => {
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            Some(Token::LBrace) => {
                let mut pairs = Vec::new();
                if !self.eat(&Token::RBrace) {
                    loop {
                        let key = self.parse_expr()?;
                        self.expect(&Token::Colon)?;
                        let value = self.parse_expr()?;
                        pairs.push((key, value));
                        if self.eat(&Token::RBrace) {
                            break;
                        }
                        self.expect(&Token::Comma)?;
                        if self.eat(&Token::RBrace) {
                            break;
                        }
                    }
                }
                Ok(Expr::Dict(pairs))
            }
            Some(Token::LBrack) => {
                let mut items = Vec::new();
                if !self.eat(&Token::RBrack) {
                    loop {
                        items.push(self.parse_expr()?);
                        if self.eat(&Token::RBrack) {
                            break;
                        }
                        self.expect(&Token::Comma)?;
                    }
                }
                Ok(Expr::List(items))
            }
            _ => Err(trf!("脚本语法不对", "The script syntax is not valid")),
        }
    }

    fn expect_ident(&mut self) -> Result<String, String> {
        match self.bump() {
            Some(Token::Ident(name)) => Ok(name),
            _ => Err(trf!("这里要一个名字", "A name is required")),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Newline,
    Indent,
    Dedent,
    Ident(String),
    Number(String),
    String(String),
    Op(BinOp),
    Not,
    And,
    Or,
    LParen,
    RParen,
    LBrack,
    RBrack,
    LBrace,
    RBrace,
    Comma,
    Colon,
    Dot,
    Assign,
    If,
    Elif,
    Else,
    For,
    While,
    In,
    Def,
    Return,
    Break,
    Continue,
    Pass,
    Import,
    From,
    Class,
    Try,
    Except,
    Raise,
    As,
    True,
    False,
    NoneKw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Floor,
    Mod,
    Pow,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

fn tokenize(source: &str) -> Result<Vec<Token>, String> {
    let lines = logical_lines(source)?;
    let mut indents = vec![0usize];
    let mut out = Vec::new();
    for (indent, text) in lines {
        if indent > *indents.last().unwrap_or(&0) {
            indents.push(indent);
            out.push(Token::Indent);
        } else {
            while indent < *indents.last().unwrap_or(&0) {
                indents.pop();
                out.push(Token::Dedent);
            }
            if indent != *indents.last().unwrap_or(&0) {
                return Err(trf!("缩进对不上", "The indentation does not match"));
            }
        }
        lex_line(&text, &mut out)?;
        out.push(Token::Newline);
    }
    while indents.len() > 1 {
        indents.pop();
        out.push(Token::Dedent);
    }
    Ok(out)
}

fn logical_lines(source: &str) -> Result<Vec<(usize, String)>, String> {
    let chars: Vec<char> = source.chars().collect();
    let mut index = 0;
    let mut out = Vec::new();
    while index < chars.len() {
        let mut indent = 0usize;
        while index < chars.len() && matches!(chars[index], ' ' | '\t') {
            indent += if chars[index] == ' ' { 1 } else { 4 };
            index += 1;
        }
        if index >= chars.len() {
            break;
        }
        if chars[index] == '\r' {
            index += 1;
            continue;
        }
        if chars[index] == '\n' {
            index += 1;
            continue;
        }
        if chars[index] == '#' {
            while index < chars.len() && chars[index] != '\n' {
                index += 1;
            }
            continue;
        }
        let mut text = String::new();
        let mut paren = 0i32;
        let mut quote: Option<char> = None;
        loop {
            if index >= chars.len() {
                break;
            }
            let ch = chars[index];
            if let Some(mark) = quote {
                text.push(ch);
                index += 1;
                if ch == '\\' && index < chars.len() {
                    text.push(chars[index]);
                    index += 1;
                    continue;
                }
                if ch == mark {
                    quote = None;
                }
                continue;
            }
            if ch == '"' || ch == '\'' {
                quote = Some(ch);
                text.push(ch);
                index += 1;
                continue;
            }
            if ch == '#' && paren == 0 {
                while index < chars.len() && chars[index] != '\n' {
                    index += 1;
                }
                continue;
            }
            if matches!(ch, '(' | '[' | '{') {
                paren += 1;
                text.push(ch);
                index += 1;
                continue;
            }
            if matches!(ch, ')' | ']' | '}') {
                paren -= 1;
                text.push(ch);
                index += 1;
                continue;
            }
            if ch == '\\' && index + 1 < chars.len() && chars[index + 1] == '\n' {
                index += 2;
                continue;
            }
            if ch == '\n' {
                index += 1;
                if paren > 0 {
                    text.push(' ');
                    continue;
                }
                break;
            }
            if ch == '\r' {
                index += 1;
                continue;
            }
            text.push(ch);
            index += 1;
        }
        let text = text.trim().to_string();
        if !text.is_empty() {
            out.push((indent, text));
        }
    }
    Ok(out)
}

fn lex_line(text: &str, out: &mut Vec<Token>) -> Result<(), String> {
    let chars: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        if ch.is_whitespace() {
            index += 1;
            continue;
        }
        if ch == '"' || ch == '\'' {
            let (text, next) = lex_string(&chars, index)?;
            out.push(Token::String(text));
            index = next;
            continue;
        }
        if ch.is_ascii_digit() || (ch == '.' && chars.get(index + 1).is_some_and(|next| next.is_ascii_digit())) {
            let start = index;
            index += 1;
            while index < chars.len() && (chars[index].is_ascii_alphanumeric() || chars[index] == '.' || chars[index] == '+' || chars[index] == '-') {
                let prev = chars[index - 1];
                if (chars[index] == '+' || chars[index] == '-') && prev != 'e' && prev != 'E' {
                    break;
                }
                index += 1;
            }
            out.push(Token::Number(chars[start..index].iter().collect()));
            continue;
        }
        if ch.is_alphabetic() || ch == '_' {
            let start = index;
            index += 1;
            while index < chars.len() && (chars[index].is_alphanumeric() || chars[index] == '_') {
                index += 1;
            }
            let word: String = chars[start..index].iter().collect();
            out.push(keyword(&word));
            continue;
        }
        let two: String = chars[index..chars.len().min(index + 2)].iter().collect();
        if let Some(token) = two_char(&two) {
            out.push(token);
            index += 2;
            continue;
        }
        out.push(one_char(ch)?);
        index += 1;
    }
    Ok(())
}

fn lex_string(chars: &[char], start: usize) -> Result<(String, usize), String> {
    let quote = chars[start];
    let mut index = start + 1;
    let mut out = String::new();
    while index < chars.len() {
        let ch = chars[index];
        if ch == '\\' && index + 1 < chars.len() {
            let next = chars[index + 1];
            out.push(match next {
                'n' => '\n',
                't' => '\t',
                '\\' => '\\',
                '\'' => '\'',
                '"' => '"',
                other => other,
            });
            index += 2;
            continue;
        }
        if ch == quote {
            return Ok((out, index + 1));
        }
        out.push(ch);
        index += 1;
    }
    Err(trf!("字符串没有结束", "The string is not closed"))
}

fn keyword(word: &str) -> Token {
    match word {
        "if" => Token::If,
        "elif" => Token::Elif,
        "else" => Token::Else,
        "for" => Token::For,
        "while" => Token::While,
        "in" => Token::In,
        "def" => Token::Def,
        "return" => Token::Return,
        "break" => Token::Break,
        "continue" => Token::Continue,
        "pass" => Token::Pass,
        "import" => Token::Import,
        "from" => Token::From,
        "class" => Token::Class,
        "try" => Token::Try,
        "except" => Token::Except,
        "raise" => Token::Raise,
        "as" => Token::As,
        "and" => Token::And,
        "or" => Token::Or,
        "not" => Token::Not,
        "True" => Token::True,
        "False" => Token::False,
        "None" => Token::NoneKw,
        _ => Token::Ident(word.to_string()),
    }
}

fn two_char(text: &str) -> Option<Token> {
    Some(match text {
        "==" => Token::Op(BinOp::Eq),
        "!=" => Token::Op(BinOp::Ne),
        "<=" => Token::Op(BinOp::Le),
        ">=" => Token::Op(BinOp::Ge),
        "//" => Token::Op(BinOp::Floor),
        "**" => Token::Op(BinOp::Pow),
        _ => return None,
    })
}

fn one_char(ch: char) -> Result<Token, String> {
    Ok(match ch {
        '+' => Token::Op(BinOp::Add),
        '-' => Token::Op(BinOp::Sub),
        '*' => Token::Op(BinOp::Mul),
        '/' => Token::Op(BinOp::Div),
        '%' => Token::Op(BinOp::Mod),
        '<' => Token::Op(BinOp::Lt),
        '>' => Token::Op(BinOp::Gt),
        '(' => Token::LParen,
        ')' => Token::RParen,
        '[' => Token::LBrack,
        ']' => Token::RBrack,
        '{' => Token::LBrace,
        '}' => Token::RBrace,
        ',' => Token::Comma,
        ':' => Token::Colon,
        '.' => Token::Dot,
        '=' => Token::Assign,
        _ => return Err(trf!("脚本里有不认识的符号", "The script contains an unknown symbol")),
    })
}
