//! 文本拟合插件。一个文件描述一个模型，放进 `fitPlugins` 目录后就能在拟合对话框里选用。
//!
//! ```text
//! # 注释
//! name: 饱和
//! expr: y0+a*(1-exp(-k*x))
//! initial: y0=0, a=1, k=0.1
//! ```
//!
//! `formula` 等同 `expr`，`params` 等同 `initial`。这不是 SciDAVis 的二进制或 XML `.fdf`，也不是 Python 插件。

use std::fs;
use std::path::Path;

use sciplot_core::{parameter_names, parse_initials, CompiledExpr};

#[derive(Clone, Debug, PartialEq)]
pub struct FitPlugin {
    pub name: String,
    pub expr: String,
    pub initial: Vec<(String, f64)>,
    pub source: String,
}

pub fn parse_fit_plugin(text: &str) -> Result<FitPlugin, String> {
    let mut name = String::new();
    let mut expr = String::new();
    let mut initial = String::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            return Err(sciplot_core::trf!(
                "插件行无法识别：{line}",
                "Unrecognized plugin line: {line}",
                line = line
            ));
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        match key.as_str() {
            "name" => name = value.to_string(),
            "expr" | "formula" => expr = value.to_string(),
            "initial" | "params" => initial = value.to_string(),
            other => {
                return Err(sciplot_core::trf!(
                    "插件里不认识的字段：{other}",
                    "Unknown plugin field: {other}",
                    other = other
                ));
            }
        }
    }
    if name.trim().is_empty() {
        return Err(sciplot_core::tr!("拟合插件需要 name", "A fit plugin needs a name").into());
    }
    if expr.trim().is_empty() {
        return Err(sciplot_core::tr!("拟合插件需要 expr", "A fit plugin needs an expr").into());
    }
    let names = parameter_names(&expr);
    if names.is_empty() {
        return Err(sciplot_core::tr!(
            "拟合插件的表达式里没有待拟合参数",
            "The plugin expression has no parameters to fit"
        )
        .into());
    }
    CompiledExpr::compile(&expr)?;
    let parsed = if initial.trim().is_empty() {
        Vec::new()
    } else {
        parse_initials(&initial)?
    };
    Ok(FitPlugin {
        name,
        expr,
        initial: parsed,
        source: String::new(),
    })
}

pub fn load_fit_plugins(dir: &Path) -> Vec<Result<FitPlugin, String>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    let mut paths: Vec<_> = entries.filter_map(|entry| entry.ok()).map(|entry| entry.path()).collect();
    paths.sort();
    let mut plugins = Vec::new();
    for path in paths {
        if !is_plugin_file(&path) {
            continue;
        }
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) => {
                plugins.push(Err(err.to_string()));
                continue;
            }
        };
        match parse_fit_plugin(&text) {
            Ok(mut plugin) => {
                plugin.source = path.display().to_string();
                plugins.push(Ok(plugin));
            }
            Err(err) => plugins.push(Err(format!("{}: {err}", path.display()))),
        }
    }
    plugins
}

fn is_plugin_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".txt") || lower.ends_with(".plugin") || lower.ends_with(".fit")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_text_compiles_and_a_directory_roundtrips() {
        let plugin = parse_fit_plugin(
            "\
# 饱和增长
name: 饱和
expr: y0+a*(1-exp(-k*x))
initial: y0=0, a=1, k=0.1
",
        )
        .unwrap();
        assert_eq!(plugin.name, "饱和");
        assert_eq!(plugin.initial.len(), 3);
        let err = parse_fit_plugin("name: 空\n").unwrap_err();
        assert!(err.contains("expr"), "{err}");

        let dir = std::env::temp_dir().join(format!("sciplot-plugins-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("line.plugin"), "name: 直线\nexpr: a*x+b\ninitial: a=1, b=0\n").unwrap();
        fs::write(dir.join("notes.md"), "not a plugin").unwrap();
        let loaded: Vec<_> = load_fit_plugins(&dir).into_iter().collect();
        assert_eq!(loaded.len(), 1, "{loaded:?}");
        assert_eq!(loaded[0].as_ref().unwrap().name, "直线");
        let _ = fs::remove_dir_all(&dir);
    }
}
