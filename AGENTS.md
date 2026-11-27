# SciPlot 代理说明

SciPlot 是用 Rust 写的科学绘图与数据分析程序，界面对齐 SciDAVis，界面语言是简体中文。目标平台是 Windows。仓库地址是 `pipidu/plot`。

## 提交规则

以后每次更改都直接提交并推送到 `main`。不要创建功能分支，不要开 Pull Request，也不要等用户确认提交、推送或合并。改完、测试通过后，自己 `git commit` 并 `git push origin main`。

如果工作开始时不在 `main` 上，先把改动并入 `main`，再在 `main` 上提交。自己留下的其他本地分支和 `origin` 上的对应分支，在内容已经进入 `main` 之后删掉。不要删除 `main`。

提交说明写清楚改了什么。不要把 `target/`、密钥或本机绝对路径写进仓库。

## 仓库布局

Cargo workspace，resolver 2，edition 2021，`rust-version` 1.81，许可证 MIT OR Apache-2.0。

| 路径 | 包名 | 库名 | 职责 |
| --- | --- | --- | --- |
| `crates/core` | `sciplot-core` | `sciplot_core` | 表格、矩阵、图形模型、公式、撤销、`.splot`、CSV。不依赖 GUI，也不依赖 analysis |
| `crates/analysis` | `sciplot-analysis` | `sciplot_analysis` | 拟合、微积分、插值、平滑、FFT、统计、求交点。不依赖 GUI |
| `crates/plot` | `sciplot-plot` | `sciplot_plot` | 坐标变换、图元、二维/三维绘制、SVG、PNG、EPS、PDF 和 JPEG/BMP/PBM/PGM/PPM/XBM/XPM。不依赖 analysis |
| `crates/script` | `sciplot-script` | `sciplot_script` | 程序内的 Python 解释器（字典、类、异常、math/statistics/random/json）和 `table()` / `graph()` / `newTable()` / `plot()` / `Fit`。不依赖 GUI，不是 CPython |
| `crates/app` | `sciplot` | 二进制 `sciplot` | egui 主窗口、菜单、对话框、字体 |
| `assets/fonts` |  |  | 内嵌中文字体 Droid Sans Fallback 及其许可证 |
| `crates/app/assets/icon.ico`、`icon.png` |  |  | 应用图标。Windows 下由 `crates/app/build.rs` 把 ICO 嵌入可执行文件；启动时用 PNG 设置窗口图标（Linux 任务栏和窗口标题也靠它） |

依赖方向只能是 `app → analysis / plot / core / script`，`script → core / analysis / plot`，`analysis → core`，`plot → core`。不要让 `core` 反过来依赖上面几层。求交点的算法放在 `crates/analysis/src/intersect.rs`，画到图上和写入表格的流程放在 `crates/app/src/ops.rs`。

## 常用命令

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p sciplot
cargo run -p sciplot -- --export-demo target/demo
cargo build -p sciplot --release --target x86_64-pc-windows-gnu
```

改完代码后至少跑 workspace 测试和上面的 clippy。动了界面时，用 `--export-demo` 检查中文和图形；有显示器时再打开窗口看菜单、表格和求交点。环境变量：

- `SCIPLOT_NO_DIALOG=1`：不弹系统文件对话框，改用程序内的路径输入。无桌面或没有 GTK 时用这个。
- `SCIPLOT_EXIT_FRAMES=N`：画完 N 帧后关闭窗口，便于自动截图。
- Linux 上若窗口起不来，先确认 `libxkbcommon-x11.so.0` 和 `libxcb-xkb.so.1` 能被加载。`rfd` 固定在 0.15.4，不要打开 gtk 特性。

Windows 交叉编译需要 `x86_64-w64-mingw32-gcc` 和 `x86_64-w64-mingw32-windres`。`build.rs` 在非 Windows 主机上会调用这两个带前缀的工具。`resource.o` 没有被引用的符号，所以除了 winres 生成的静态库，还要用 `cargo:rustc-link-arg` 把 `resource.o` 直接链进去，否则 exe 里没有图标。`main.rs` 在 Windows 上使用 `windows_subsystem = "windows"`，exe 不带控制台。

## 数据模型

`Project` 持有表格、矩阵、图形、备注、日志和文件夹。ID 是新类型：`TableId`、`MatrixId`、`GraphId`、`NoteId`、`FolderId`。对象上的 `folder` 为空表示根。读入项目后调用 `fix_ids()`，让分配器越过已有 ID，再 `sanitize_folders()`。

项目文件仍是 `FORMAT_VERSION = 1`，形状是 `{ "format", "project" }`。未压缩的 `.splot` 是 JSON。`.splot.gz` 是同一份 JSON 的 gzip。`.sciprj` / `.sciprj.gz` 有两条路：SciPlot 自己的 XML 可以往返；以 `SciDAVis x.y.z project file` 开头的官方项目可以读入表格、列公式、矩阵、备注、文件夹和曲线。官方文件没有单元格公式。打开官方 SciDAVis 或 Origin 文件后，保存另存为 `.splot` / `.splot.gz`，不会覆盖原文件。Origin（`.opj`、`.opju`、`.ogw`、`.ogg`、`.ogm`）只读：数值、文本、日期和 XML 里的曲线；二进制图窗口不还原，也不能写回 Origin。不认识的扩展名按 `.splot` 写，给原来的 HTTP/MCP 留兼容。数值里的空单元格用 `Option<f64>`，不要序列化 NaN。布尔字段如果默认是 true，用 `#[serde(default = "default_true")]`。枚举上不要写 `#[serde(default)]`。追加项目用 `append_project`，不要替换当前项目。删除文件夹只把子文件夹和对象交回父文件夹。

撤销是整份项目的 JSON 快照，上限 40 步，接口是 `History::push`、`undo`、`redo`。界面上的修改走 `SciPlotApp::edit`，失败时恢复修改前的项目。

CSV 导入先按 UTF-8，失败再按 GBK。导出带 UTF-8 BOM。最近文件列表在用户主目录的 `.sciplot_recent.json`。默认样式和字体在 `.sciplot_prefs.json`，不写进项目。`font_path` 为空时按系统字体自动选择。这套默认样式可以另存为 `.spstyle`（`sciplot-style`），只含字体、字号、线型、符号、直方图箱数和颜色，不含语言、本机 API 和最近目录。它不是图形模板。

图形模板是 `.sptpl`（`sciplot-template`），JSON，不是 SciDAVis 的 `.qpt`。`GraphTemplate` 记下画布大小、图层几何、字体、颜色、坐标轴标题和文字标注，打开后是空图。曲线、曲线样式和刻度（步长、次刻度、刻度样式）不写入模板。

列公式和拟合表达式用 `crates/core/src/expr.rs` 里的表达式引擎，所有数都是 `f64`：`1/2` 等于 0.5，`1==1.0` 为真。不要换回 evalexpr：11.3.1 会截断整数除法，12 及以上是 AGPL。乘方是 `^`。`col("名字")`、`col("A")`、`col(1)`（从 1 开始）、`cell("A", 行号)`、`tablecol`、`i` 和 `row` 可用。`if()`、`?:` 和缩进的 if/elif/else 在 `crates/core/src/branch.rs`，只计算被选中的一边。以 `python:` 开头的列公式交给 `set_python_column_hook`，由 `sciplot-script` 逐行计算。套用列公式不会清掉已有的单元格公式。重算按依赖每个公式算一次，不是迭代到不动点。表格上方有公式栏。备注求值会先清掉线程里的列上下文。

## 绘图

不要引入 `egui_plot`。引擎先生成与后端无关的图元（`crates/plot/src/scene.rs` 的 `Prim`），再分别交给：

- `paint`：egui `Painter`，用于普通二维图的屏幕绘制
- `to_svg`：矢量导出，`font-family` 使用 `SVG_FONT`
- `to_png` / `to_rgba`：`tiny-skia` 光栅。三维图在屏幕上也走 `to_rgba`，再贴成纹理。egui 的 `convex_polygon` 会把很瘦的三角形拉成刺，所以曲面不要直接用 `Painter` 填三角形
- `export_scene` / `print_pdf`：EPS、PDF 和 JPEG、BMP、PBM、PGM、PPM、XBM、XPM。纸张、方向、DPI、JPEG 质量和透明背景从 `ExportOptions` 进去。不贴纸时 PNG 仍是 960×640 乘以 DPI/96，默认 DPI 144。EPS 和 PDF 的页始终用纸张点尺寸；文字用 `ttf-parser` 转成轮廓。透明背景会跳过铺满画布的白底，用于 PNG、XPM 和 SVG。PDF/EPS 里的图像标注写入像素；带透明通道的像素先合成到白底上。打印没有系统打印队列，`print_pdf` 把每张图写成 PDF 的一页

`Graph.arrangement` 用行列、对齐、边距和间距摆图层。`rows == 0` 且 `cols == 0` 时按接近正方形的网格，两层是左右而不是上下。新建、复制和插入图层都会重排。删除只在 `auto_rearrange` 为真时重排。单个图层的数值位置仍是归一化的 `x,y,w,h`。当前图层是界面上的 `active_layer`，不写进项目；图窗口左上角的数字按钮切换它。表格右键「面板」走 `Project::plot_panel`。

坐标可以是线性、以 10 为底的对数、倒数，并支持反向轴。刻度用 1-2-5。视图缩放和平移存在 `LayerView` 里，和坐标轴的 min/max 分开；平移和滚轮缩放同时改下、上、左、右四条轴。网格和标注跟曲线或标注绑定的那条轴。全是极坐标曲线的图层用极坐标网格和角度刻度。双击恢复视图。按住 Shift 拖拽只记录 X 区间，给求交点用。

纵轴中文标题按字竖排，不依赖旋转文本。纵轴的英文标题整行逆时针转 90 度，不把每个字母拆开。三维曲面按平均深度做画家算法排序，范围会缩进画布，避免裁切。鼠标拖拽改方位角和仰角，滚轮改 `zoom`。`SurfaceMode` 还有多边形、隐藏线和网格曲面；散点可以是圆点、十字、锥体或竖线。表格列可以画成色带、柱或轨迹（`SurfaceSource::Table`）。边长超过 160 的矩阵只在显示时抽稀。

## 分析

拟合在 `crates/analysis/src/fit.rs`。线性和多项式用正规方程加 LU，不要用 nalgebra 0.33 的 QR 解非方阵。指数、一到三阶衰减、指数增长、Boltzmann、高斯、洛伦兹、多峰和自定义函数走 `FitOptions`：默认权重为 1、缩放 Levenberg-Marquardt、80 次、容差 1e-12。算法还可以选不缩放的 Levenberg-Marquardt 和 Nelder-Mead。线性和多项式不看这个算法，只用加权最小二乘。自定义参数从 `parameter_names` 和 `parse_initials`（如 `a=1, b=0.5`）来。文本拟合插件放在主目录或程序旁边的 `fitPlugins`，由 `crates/analysis/src/plugin.rs` 读取，不是 `.fdf` 或 Python。`nalgebra` 停在 0.33：0.34 的 MSRV 更高，0.35 需要 edition 2024。

FFT 在 `crates/analysis/src/fft.rs`。原来的 `fft_spectrum` 仍是单边振幅和相位。`fft_filter` 保留低通和高通；带通和带阻用 `fft_filter_band`。`fft_forward` / `fft_inverse` 写出实部、虚部、振幅和相位，并接受归一化与零频移位。卷积、反卷积和信号相关在 `crates/analysis/src/signal.rs`，和 Pearson 相关不是一回事。备注里的「运行备注」仍是 `eval_note`，逐行公式，没有 `col()`。菜单「脚本」和「执行脚本」走 `crates/script`，是程序内的解释器，不是 CPython（没有嵌入 libpython：Windows 包是 mingw 交叉编译，再带三份标准库不现实，动态链接会要求用户另装 Python；RustPython 的工具链高于声明的 1.81）。脚本有字典、类、异常，以及 math、statistics、random、json。不认识的 import 会报错。脚本里的 `/` 是真除法，`//` 才向下取整。

Savitzky-Golay 按窗口内每个偏移单独算系数，这样边缘也能保住阶数不超过窗口的多项式。不要用反射填充，那会破坏直线。

求交点：

- 折线与折线：网格哈希剪枝，再用叉积求线段交点。共线重叠返回两个端点。按容差去重，默认 `1e-8`。曲线不必按 X 单调。
- 平滑模式：表达式直接构造 `h(x)=f(x)-g(x)`；数据在 X 严格递增时先做三次样条。变号区间用 Brent 精化。
- 两条都是水平线或都是竖直线时，记平行或无限重叠，不要假装求出一个点。
- X 区间是闭区间。结果写入表「交点_A_B」，列是序号、X、Y、线段A、线段B。平滑求根没有线段索引时，单元格写成「—」。
- 图上的交点是 `AnnotationKind::Marker`。新标注的 id 先为 0，再由 `reassign_zero_ids` 分配。不要在分配前把 0 改成别的数。

## 界面

文案集中在 `crates/app/src/i18n.rs`。菜单栏是：文件、编辑、脚本、设置、窗口、帮助。表格、矩阵、绘图、分析和格式不要再堆回菜单栏，放在 `crates/app/src/context.rs` 的右键菜单里。

- 表格表头和单元格：插入或删除这一列、这一行，公式、排序、统计、绘制为、导入导出 CSV。行号上是行操作。
- 矩阵：公式填充、转置、求逆、转成表格、热力图、等高线、三维图。
- 项目树：分组标题上新建，每一项上打开、绘制、删除。文件夹可以新建、重命名和删除。点文件夹只改当前文件夹，不把项目标成已修改。「含子文件夹」决定过滤时是否带上子文件夹里的对象。对象可以移到当前文件夹。
- 图上：先命中标注，再按线段距离命中曲线（`nearest_curve`，12 像素）。曲线菜单改属性、显隐、删除、拟合和求交点。空白处是坐标轴、图例、标题、标注、加曲线和导出。图层菜单里有新建、复制、插入其他图、排列和删除。三维图右键切换显示样式，也可以打开三维设置。多峰拟合点选峰位时不平移视图，Esc 取消。
- 文件菜单：导出图形、导出全部图形、打印、打印全部图形、保存为模板、打开模板。设置菜单：导出默认样式、打开默认样式。这些对话框和 `crates/app/src/api/tools.rs` 里的同名工具调用同一套函数。

平移、框选 X 区间、拖标注和旋转三维图只认鼠标左键。右键打开菜单，不改变视图。菜单项点下去要 `close_menu()`。

「设置 → 默认样式与字体」改字体文件、标题和坐标轴字号、线宽、线型、符号。保存后立刻 `fonts::install`。新建图形时套用整套样式；往已有图追加曲线时只改新曲线。已打开的示例图不要在启动时重套样式。导出的 `.spstyle` 只含这些视觉字段；打开时保留本机的语言、API 和最近目录。

界面字体启动时按顺序找 Windows 的微软雅黑（`msyh.ttc` 等），再找系统里的 Noto Sans、文泉驿、Droid，最后用 `include_bytes!` 的 `assets/fonts/DroidSansFallbackFull.ttf`。图上的拉丁字母按 Times New Roman、Liberation Serif 这条顺序，中文按宋体、Noto Serif CJK、思源宋体、文鼎明体，找不到就用同一份内嵌 Droid。设置里的字体文件如果填了，界面和图都改用这一个文件；留空才走上面的自动顺序。TTC 要先抽出一个 TTF，`ab_glyph` 读不了字体集合。界面字体同时注册为 egui 的 Proportional 和 Monospace。图使用名为 `sciplot-plot` 的字体族，拉丁文件在前、中文文件在后，缺字时 egui 会落到后一个。SVG 的 `font-family` 写成 Times New Roman、Liberation Serif、SimSun、宋体这一串，不用双引号，避免撑破 XML 属性。

界面语言在 `sciplot_core::Language`，默认简体中文，存在 `.sciplot_prefs.json` 的 `language` 字段，不写进 `.splot`。除了简体中文和英语，还有繁体中文、德语、法语、俄语、西班牙语、捷克语和巴西葡萄牙语。短句用 `t("中文")` 或 `tr!` / `trf!`，英文对照在 `crates/core/src/catalog.rs`，其余语言的菜单短句在 `crates/core/src/lang_extra.rs`。没有对照的短句先用英语，再用中文。状态栏的带参数句子在 `crates/core/src/lang_formats.rs`，七种语言按中文格式串填孔，没有收录时留中文，不再改用英文。公式说明、脚本说明、关于和 AI 说明也有这七种语言的译文。`format!` 的格式串必须是字面量，带参数的句子用 `trf!`。测试默认语言是中文，改成别的语言后要改回来，否则同一线程上后面的测试会读到那种语言。示例项目的名字在创建时翻译；查找示例表和图时同时认中文名和英文名。

发布用 `.github/workflows/windows.yml`。`release` 任务仍编译 Windows 和 Linux，并创建 `v0.2.1-bN`。不要给这个任务加 `needs`，否则 macOS 失败会挡住已有的包。`macos` 任务在 `macos-latest` 上打未签名、未公证的 Apple Silicon 包，并交叉编译一个没有在 Intel 硬件上运行过的 x86_64 包。`attach-macos` 只上传 `.tar.gz`，重算包含 macOS 的 `SHA256.txt`，再改写发布说明。没有 Apple Developer 证书，不能签名或公证。不要在本机创建标签或 GitHub Release。

停靠用 `egui_dock` 0.16。`DockState` 和 `TabViewer` 不要同时可变借用：先 `mem::replace` 取出 dock，画完再放回去。因此 `open_tab` 只把标签放进 `pending_tabs`，等 dock 放回来再 `flush_tabs`。在表格或图的右键菜单里直接改 `self.dock` 会被这一帧的换回盖掉。菜单项被点击后调用 `ui.close_menu()`，否则对话框打开时菜单会留在画面上。

`egui` / `eframe` / `egui_extras` 停在 0.31.1。这个版本里 `ViewportInfo::close_requested` 是方法，`Shape::rect_stroke` 需要 `StrokeKind`，`Stroke` 可以实现成 `PathStroke`。浮点字面量传给 `f32` 参数时写成 `1.0_f32`。

表格表头只显示列名。列角色不是「无」、而且和列名不同时，才在后面加上角色，例如 `sin (Y)`。列名从列的右键菜单「重命名此列…」或双击表头来改。改名会改写本表公式里的 `col("旧名")` 和 `cell("旧名", …)`，曲线仍按列序号引用。

## AI 接入

`crates/app/src/api/tools.rs` 是建表、画图、拟合、求交点和渲染的共用入口。窗口开着时，`api/http.rs` 在 `127.0.0.1` 上提供这些工具，默认端口 8765，被占用就顺延。令牌和端口写在用户主目录的 `.sciplot_api.json`（测试可设 `SCIPLOT_HOME`），退出时按 pid 删掉。请求要带 `Authorization: Bearer <token>`。HTTP 线程把调用送进 channel，`SciPlotApp::update` 里执行；会改项目的调用走 `edit`，可以撤销。`get_project`、`render_graph`、`save_project` 不进撤销栈。

`sciplot --mcp [--project 文件.splot]` 是 stdio 上的 JSON-RPC，给 Claude Desktop、Cursor 用。stdout 只能写协议消息，日志写 stderr。每次 `tools/call` 先看 `.sciplot_api.json`，窗口在就转过去；不在就在本进程里调用同一套工具。带了 `--project` 时，本进程的改动会写回这个文件。Windows 的 exe 不带控制台，但父进程用管道启动时会继承标准输入输出。

## 示例项目

`demo_project()` 包含：

- 表「三角函数」：X、sin、cos，约 200 个点，范围 0 到 2π
- 图「正弦与余弦」
- 表「水果销量」：中文分类的柱状图
- 矩阵「波面」以及对应的三维曲面和热力图
- 备注「使用说明」

`ops::enrich_demo` 再用平滑模式求 sin 与 cos 的交点。启动和 `--export-demo` 都会走这一步。回归时至少确认有两个交点，X 接近 π/4 和 5π/4。

## 改代码时别踩的坑

- 多项式拟合先 `clone` 正规矩阵再 `lu()`，因为 `lu()` 会吃掉矩阵，后面还要用它求逆来算标准误差。
- `HashMap::new` 在这里不是 const，`thread_local` 里不要写成 `const { RefCell::new(HashMap::new()) }`。
- 裁剪或命中检测里，一个标注映射失败要用 `continue`，不要用 `?` 放弃其余标注。
- 关闭前提示保存时，如果还没路径，`save_current` 会改成路径对话框。这时退出对话框必须让出，不能再把自己写回去。
- clippy 以 `-D warnings` 为准。绘图函数参数多是允许的，用函数上的 `#[allow(clippy::too_many_arguments)]`，不要为了消警告把调用点拆乱。
