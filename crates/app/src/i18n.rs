//! 界面里较长的说明。菜单和对话框的短句在显示时按当前语言翻译。

pub const APP_TITLE: &str = "SciPlot";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 工作流编译时写入 `SCIPLOT_BUILD`，本地构建为空。
pub fn current_version() -> String {
    match option_env!("SCIPLOT_BUILD") {
        Some(build) if !build.is_empty() => format!("{VERSION}-b{build}"),
        _ => VERSION.to_string(),
    }
}
pub const REPO_URL: &str = "https://github.com/pipidu/plot";
pub const RELEASES_URL: &str = "https://github.com/pipidu/plot/releases";
pub const RELEASES_API: &str = "https://api.github.com/repos/pipidu/plot/releases/latest";

pub fn file() -> &'static str {
    sciplot_core::t("文件")
}
pub fn edit() -> &'static str {
    sciplot_core::t("编辑")
}
pub fn settings() -> &'static str {
    sciplot_core::t("设置")
}
pub fn window() -> &'static str {
    sciplot_core::t("窗口")
}
pub fn help() -> &'static str {
    sciplot_core::t("帮助")
}

const FORMULA_HELP_ZH: &str = "\
表达式规则：用普通的算式写列公式和拟合公式。

运算：+  -  *  /  %  ^（乘方）和括号。
两个整数相除只会得到整数，需要小数时写成 1.0/2。

函数：sin cos tan asin acos atan atan2 sinh cosh tanh
      exp ln log（以 10 为底）log10 log2 sqrt abs
      floor ceil round min max hypot pow cbrt sign

常数：pi、e。函数曲线里自变量是 x，参数曲线里是 t，曲面公式里是 x 和 y。

列公式还可以用：
  col(\"列名\")、col(\"A\")（按列名；没有同名列时才按字母列标）
  col(1)（从 1 开始的列位置，不看列名）
  cell(\"A\", 行号)
  i 或 row：从 1 开始的当前行号

例如：sin(col(\"X\")) * 2
";

const FORMULA_HELP_EN: &str = "\
Expressions: write column formulas and fit formulas as ordinary arithmetic.

Operators: +  -  *  /  %  ^ (power) and parentheses.
Dividing two integers truncates. Write 1.0/2 when you need a fraction.

Functions: sin cos tan asin acos atan atan2 sinh cosh tanh
           exp ln log (base 10) log10 log2 sqrt abs
           floor ceil round min max hypot pow cbrt sign

Constants: pi and e. Function curves use x, parametric curves use t, and surfaces use x and y.

Column formulas can also use:
  col(\"name\"), col(\"A\") (the column of that name; a letter only when no column has it)
  col(1) (the column in that position, starting at 1, ignoring the name)
  cell(\"A\", row)
  i or row: the current row number, starting at 1

Example: sin(col(\"X\")) * 2
";

pub fn formula_help() -> &'static str {
    sciplot_core::translate(FORMULA_HELP_ZH, FORMULA_HELP_EN)
}

const ABOUT_ZH: &str = "\
科学绘图软件。用 Rust 和 egui 编写，界面对齐 SciDAVis。

表格、矩阵、多种二维图、三维曲面、拟合、微积分、插值、平滑、FFT，
以及曲线/折线求交点（可限制 X 区间，也可与水平线、竖直线求交）。

项目文件是 .splot。图形可以导出 PNG 和 SVG。

发布页有 Windows 程序，也有需要图形界面的 Linux 程序。Windows 是主要使用的系统。
";

const ABOUT_EN: &str = "\
Scientific plotting, written in Rust and egui, with a layout close to SciDAVis.

Tables, matrices, many two-dimensional plots, three-dimensional surfaces, fitting,
calculus, interpolation, smoothing, FFT, and intersections of curves or polylines
(an X range, a horizontal line, or a vertical line).

Projects are .splot files. Graphs export to PNG and SVG.

Releases include a Windows program and a Linux program that needs a graphical session. Windows is the main platform.
";

pub fn about() -> &'static str {
    sciplot_core::translate(ABOUT_ZH, ABOUT_EN)
}
