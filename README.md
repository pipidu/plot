# SciPlot

用 Rust 编写的科学绘图与数据分析程序，界面对齐 [SciDAVis](https://scidavis.sourceforge.net/)，原生支持中文。在表格、矩阵、多种二维图、三维曲面和拟合分析之外，可以求曲线或折线的交点。

## 构建

需要 Rust 1.81 或更新的稳定版。

```bash
cargo run -p sciplot
cargo test
```

启动后会打开示例项目：正弦与余弦已经算出交点，标在图上，并写入表格「交点_sin_cos」。

不打开窗口、只导出示例图：

```bash
cargo run -p sciplot -- --export-demo target/demo
```

## Windows

程序面向 Windows。在 Linux 上可以交叉编译：

```bash
rustup target add x86_64-pc-windows-gnu
sudo apt install mingw-w64
cargo build -p sciplot --release --target x86_64-pc-windows-gnu
```

生成的 `sciplot.exe` 不带控制台窗口。中文字体优先使用 `C:\Windows\Fonts\msyh.ttc`（微软雅黑），找不到时使用程序内嵌的 Droid Sans Fallback，所以一台没有中文字体的机器也能显示中文。

## 主要功能

- 项目树、可停靠的表格 / 矩阵 / 图形 / 备注窗口，撤销与重做，`.splot` 项目文件，最近文件。
- 表格：增删行列，列角色（X/Y/Z/误差/标签），数值、文本和日期，列公式，排序，统计，CSV 导入导出（UTF-8 与 GBK）。
- 矩阵：公式填充、转置、求逆，与表格互转。
- 二维图：折线、散点、点线、阶梯、柱状、条形、直方图、面积、误差棒、箱线、饼图、向量场、热力图、等高线、函数曲线。
- 坐标轴可以是线性、对数或倒数，支持网格、图例、标题、曲线样式、文本和箭头标注。滚轮缩放，拖拽平移，双击恢复，Shift 拖拽选择 X 区间。
- 三维曲面、线框和散点，鼠标拖拽旋转。
- 分析：线性、多项式、指数、高斯、洛伦兹和自定义拟合，微分、积分、插值、平滑、FFT、滤波、相关。
- 导出 PNG（可设 DPI）、SVG，或复制到剪贴板。

## 求交点

菜单「分析 → 求交点…」。两条输入各自可以是数据曲线、函数曲线、拟合曲线、水平线 `y=c` 或竖直线 `x=c`。

- 分段模式把曲线看成折线，先用网格丢掉不相交的线段，再精确求交。曲线不必按 X 单调。
- 平滑模式对函数表达式直接求根；数据如果能按 X 排成严格递增，则先做三次样条，再用 Brent 法收束。
- 可以手填 X 区间，也可以先在图上按住 Shift 拖出区间，对话框会带上这个范围。
- 交点用标记画在图上，同时写入新表「交点_A_B」，并记入底部日志。

## 公式

列公式和拟合表达式用 evalexpr。`sin(col("X"))*2`、`col("A")`、`cell("A", 行号)`、`i`（从 1 开始的行号）都可以。整数除法会截断，需要小数时写成 `1.0/2`。帮助菜单里有完整说明。

## 快捷键

| 按键 | 作用 |
| --- | --- |
| Ctrl+N | 新建项目 |
| Ctrl+O | 打开 |
| Ctrl+S | 保存 |
| Ctrl+Z | 撤销 |
| Ctrl+Y | 重做 |
| Delete | 删除选中的标注 |

## 目录

- `crates/core`：表格、矩阵、项目、公式、CSV。
- `crates/analysis`：拟合、微积分、插值、平滑、FFT、统计、交点。
- `crates/plot`：坐标变换、图元、SVG 和 PNG。
- `crates/app`：egui 界面。
