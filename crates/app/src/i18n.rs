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
pub fn scripting() -> &'static str {
    sciplot_core::t("脚本")
}

const FORMULA_HELP_ZH: &str = "\
表达式规则：用普通的算式写列公式和拟合公式。

运算：+  -  *  /  %  ^（乘方）和括号。
求值器是 SciPlot 自己的表达式引擎，所有数都是小数。1/2 等于 0.5，1 和 1.0 相等。

函数：sin cos tan asin acos atan atan2 sinh cosh tanh
      exp ln log（以 10 为底）log10 log2 sqrt abs
      floor ceil round min max hypot pow cbrt sign

常数：pi、e。函数曲线里自变量是 x。参数曲线用 t，x(t) 和 y(t) 在「函数曲线」里分别填写。极坐标用 theta、t 或 x 表示弧度角。曲面公式里是 x 和 y。

重算按列之间的依赖排一次序，每个公式只算一遍，不迭代到不动点。右边的列可以读到左边已经算完的单元格公式。有环时按列号、行号各算一次，并记一条警告。套用列公式不会清掉这一列已经有的单元格公式。表格上方的公式栏直接改当前格：以 = 开头的是单元格公式，否则写入数值并清掉这一格自己的公式。

列公式还可以用：
  col(\"列名\")、col(\"A\")（按列名；没有同名列时才按字母列标）
  col(1)（从 1 开始的列位置，不看列名）
  cell(\"A\", 行号)
  i 或 row：从 1 开始的当前行号
  tablecol(\"表名\", \"列名\") 或 tablecol(\"表名\", 1)：另一张表同一行的数

某一格可以有自己的公式，计算时盖过这一列的公式。表格工具栏或右键菜单里的「公式编辑模式」会在格子里显示公式。以 = 开头的文本在离开格子时也会当成公式。

例如：sin(col(\"X\")) * 2

列公式可以分支。if(条件, 为真时, 为假时) 和 条件 ? 为真时 : 为假时 只计算选中的一边。也可以写成缩进的 if / elif / else。1/2 仍然是 0.5。

以 python: 开头的列公式用程序内的 Python 子集逐行计算，可以用 col()、cell()、tablecol()、i 和 row。

备注有两种算法。「运行备注」仍是逐行公式：`名字 = 表达式` 会记住这个数，空行、# 和 // 会跳过。这里不能用 col()。先重算表格再运行备注时，col()、cell() 和 tablecol() 不会读到上一张表的最后一行，而是报错。「执行脚本」运行程序内的解释器，可以建表、画图、拟合和导出。这不是 CPython。
";

const FORMULA_HELP_EN: &str = "\
Expressions: write column formulas and fit formulas as ordinary arithmetic.

Operators: +  -  *  /  %  ^ (power) and parentheses.
The evaluator is SciPlot's own expression engine. Every number is a decimal, so 1/2 is 0.5 and 1 equals 1.0.

Functions: sin cos tan asin acos atan atan2 sinh cosh tanh
           exp ln log (base 10) log10 log2 sqrt abs
           floor ceil round min max hypot pow cbrt sign

Constants: pi and e. Function curves use x. A parametric curve uses t; enter x(t) and y(t) in the function dialog. A polar curve uses theta, t, or x for the angle in radians. Surfaces use x and y.

Recalculation orders formulas by their column dependencies and evaluates each one once. It does not iterate to a fixed point. A column on the right can read cell formulas to its left that were just computed. A cycle is evaluated once, in column and row order, and recorded as a warning. Applying a column formula keeps that column's existing cell formulas. The formula bar above the table edits the current cell: text that starts with = is a cell formula; otherwise the value is written and that cell's own formula is cleared.

Column formulas can also use:
  col(\"name\"), col(\"A\") (the column of that name; a letter only when no column has it)
  col(1) (the column in that position, starting at 1, ignoring the name)
  cell(\"A\", row)
  i or row: the current row number, starting at 1
  tablecol(\"table\", \"column\") or tablecol(\"table\", 1): the same row of another table

A cell can have its own formula, which overrides the column formula. Formula edit mode, on the table toolbar or the context menu, shows those formulas in the cells. Text that starts with = is also treated as a formula when you leave the cell.

Example: sin(col(\"X\")) * 2

Column formulas can branch. if(condition, when true, when false) and condition ? when true : when false evaluate only the chosen side. An indented if / elif / else block works too. 1/2 is still 0.5.

A column formula that starts with python: is evaluated row by row by the built-in Python subset. It can use col(), cell(), tablecol(), i, and row.

A note has two runners. Run note still evaluates one formula per line: `name = expression` remembers the number, and blank lines, #, and // are skipped. That runner cannot use col(). After a table recalculation, col(), cell(), and tablecol() in a note do not read the previous table's last row; they report an error. Run script executes the built-in interpreter, which can create tables, plot, fit, and export. It is not CPython.
";

pub fn script_help() -> String {
    match sciplot_core::language() {
        sciplot_core::Language::Zh => SCRIPT_HELP_ZH.to_string(),
        sciplot_core::Language::En => SCRIPT_HELP_EN.to_string(),
        other => crate::help_text::script(other).to_string(),
    }
}

const SCRIPT_HELP_ZH: &str = "\
脚本菜单和备注里的「执行脚本」使用程序内的解释器，不是 CPython。\n\
没有嵌入 CPython：Windows 发行包在 Linux 上用 mingw 交叉编译，PyO3 需要 libpython；再为三个平台带上标准库既放不进发行包，也会要求用户另外安装 Python。RustPython 的工具链高于本仓库声明的 1.81。\n\
可以写 if/elif/else、for、while、def、列表、字典、类、try/except，以及 table()/graph()/newTable()/plot()/Fit。\n\
import math、statistics、random、json 和 from sciplot import * 会装上这些名字。不认识的 import 会报错。\n\
脚本里的 / 是真除法，// 是向下取整。列公式的 1/2 也是 0.5。\n\
newTable(名字, 列数, 行数)。setCell 和 setColName 的序号从 1 开始。\n\
常用方法还有列名、公式、importASCII、exportASCII、坐标轴标题、saveProject，以及 tableList/graphList。";

const SCRIPT_HELP_EN: &str = "\
The Scripting menu and Run script in a note use the built-in interpreter, not CPython.\n\
CPython is not embedded. The Windows package is cross-compiled with mingw, and PyO3 needs libpython. Shipping a standard library for three platforms would not fit the release, and linking dynamically would make users install Python themselves. RustPython's toolchain is newer than this repo's declared 1.81.\n\
You can write if/elif/else, for, while, def, lists, dicts, classes, try/except, and table()/graph()/newTable()/plot()/Fit.\n\
import math, statistics, random, json, and from sciplot import * install those names. An unknown import is an error.\n\
In a script, / is true division and // floors. A column formula's 1/2 is also 0.5.\n\
newTable(name, columns, rows). setCell and setColName are numbered from 1.\n\
Other common methods cover column names, formulas, importASCII, exportASCII, axis titles, saveProject, and tableList/graphList.";

pub fn formula_help() -> String {
    match sciplot_core::language() {
        sciplot_core::Language::Zh => FORMULA_HELP_ZH.to_string(),
        sciplot_core::Language::En => FORMULA_HELP_EN.to_string(),
        other => crate::help_text::formula(other).to_string(),
    }
}

const ABOUT_ZH: &str = "\
科学绘图软件。用 Rust 和 egui 编写，界面对齐 SciDAVis。

表格、矩阵、多种二维图、三维曲面、拟合、微积分、插值、平滑、FFT，
以及曲线/折线求交点（可限制 X 区间，也可与水平线、竖直线求交）。

项目可以存成 .splot 或压缩的 .splot.gz。SciPlot 自己的 .sciprj 可以往返；官方 SciDAVis 项目也能打开，保存时另存为 .splot，不会覆盖原文件。Origin 可以读入数值、文本和日期列，不能写回去。图形可以导出 PNG、SVG 和 PDF。透明背景对 PNG、XPM 和 SVG 生效。EPS 和 PDF 里的图像标注带上像素。

发布页有 Windows、Linux，以及未签名、未公证的 macOS 程序（Apple Silicon，和未在 Intel 硬件上运行过的 x86_64 包）。没有 Apple Developer 证书。
";

const ABOUT_EN: &str = "\
Scientific plotting, written in Rust and egui, with a layout close to SciDAVis.

Tables, matrices, many two-dimensional plots, three-dimensional surfaces, fitting,
calculus, interpolation, smoothing, FFT, and intersections of curves or polylines
(an X range, a horizontal line, or a vertical line).

A project can be saved as .splot or compressed .splot.gz. SciPlot's own .sciprj round-trips. Official SciDAVis projects open too; saving writes a .splot and does not overwrite the original. Origin files can be read, including text and date columns, and cannot be written back. Graphs export to PNG, SVG, and PDF. A transparent background applies to PNG, XPM, and SVG. Image annotations in EPS and PDF keep their pixels.

Releases include Windows, Linux, and an unsigned, unnotarized macOS program (Apple Silicon, plus an x86_64 package that was not run on Intel hardware). There is no Apple Developer certificate.
";

pub fn about() -> String {
    match sciplot_core::language() {
        sciplot_core::Language::Zh => ABOUT_ZH.to_string(),
        sciplot_core::Language::En => ABOUT_EN.to_string(),
        other => crate::help_text::about(other).to_string(),
    }
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
    match sciplot_core::language() {
        sciplot_core::Language::Zh => AI_HELP_ZH.to_string(),
        sciplot_core::Language::En => AI_HELP_EN.to_string(),
        other => crate::help_text::ai(other).to_string(),
    }
}
