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

const AI_HELP_ZH: &str = "\
让 AI 在 SciPlot 里画图有两种用法。

1. 窗口开着时，本机会在 127.0.0.1 上开一个 API，只接受带令牌的请求。令牌和端口写在主目录的 .sciplot_api.json，退出时删除。可以在「设置 → 默认样式与字体」里关掉，或改端口。AI 的改动记在撤销里。

2. 不打开窗口时，用命令行：
   sciplot --mcp
   sciplot --mcp --project 实验.splot
   窗口开着时，--mcp 会把调用转给窗口；没开时就在这个进程里改项目。带了 --project 才会在每次改动后写回文件。

Claude Desktop 或 Cursor 的配置：

{
  \"mcpServers\": {
    \"sciplot\": {
      \"command\": \"sciplot\",
      \"args\": [\"--mcp\"]
    }
  }
}

Windows 上把 command 写成 sciplot.exe 的完整路径。标准输出只写 MCP 协议，不要把日志重定向到标准输出。
";

const AI_HELP_EN: &str = "\
There are two ways for an AI to plot in SciPlot.

1. While the window is open, SciPlot listens on 127.0.0.1 and accepts only requests that carry the token. The token and port are written to .sciplot_api.json in your home directory and removed on exit. Turn it off, or change the port, under Settings → Default style and fonts. Changes from the AI can be undone.

2. Without a window:
   sciplot --mcp
   sciplot --mcp --project experiment.splot
   If a window is open, --mcp forwards calls to it. Otherwise it edits a project in this process. A --project file is written back after each change.

Claude Desktop or Cursor configuration:

{
  \"mcpServers\": {
    \"sciplot\": {
      \"command\": \"sciplot\",
      \"args\": [\"--mcp\"]
    }
  }
}

On Windows, set command to the full path of sciplot.exe. Standard output is reserved for the MCP protocol; do not redirect logs there.
";

pub fn ai_help() -> String {
    sciplot_core::translate(AI_HELP_ZH, AI_HELP_EN).to_string()
}
