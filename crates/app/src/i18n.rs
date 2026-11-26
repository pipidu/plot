//! 界面文案。当前只有简体中文，字符串集中在这里，方便以后加英文。

pub const APP_TITLE: &str = "SciPlot";

pub const FILE: &str = "文件";
pub const EDIT: &str = "编辑";
pub const SETTINGS: &str = "设置";
pub const WINDOW: &str = "窗口";
pub const HELP: &str = "帮助";

pub const FORMULA_HELP: &str = "\
列公式和拟合表达式使用 evalexpr 语法。

运算：+  -  *  /  %  ^（乘方）和括号。
整数相除会截断，需要小数时写成 1.0/2。

函数：sin cos tan asin acos atan atan2 sinh cosh tanh
      exp ln log（以 10 为底）log10 log2 sqrt abs
      floor ceil round min max hypot pow cbrt sign

常数：pi、e。函数曲线里自变量是 x，参数曲线里是 t，曲面公式里是 x 和 y。

列公式还可以用：
  col(\"列名\")、col(\"A\")、col(1)（从 1 开始的列号）
  cell(\"A\", 行号)
  i 或 row：从 1 开始的当前行号

例如：sin(col(\"X\")) * 2
";

pub const ABOUT: &str = "\
SciPlot 0.1
用 Rust 和 egui 编写的科学绘图程序，界面对齐 SciDAVis。

表格、矩阵、多种二维图、三维曲面、拟合、微积分、插值、平滑、FFT，
以及曲线/折线求交点（可限制 X 区间，也可与水平线、竖直线求交）。

项目文件是 .splot。图形可以导出 PNG 和 SVG。
";
