//! 建表、画图、拟合、求交点和导出。不碰窗口，调用方自己决定要不要记入撤销。

use std::path::Path;

use sciplot_analysis::{
    intersect, FitAlgorithm, FitModel, FitOptions, IntersectMode, IntersectOptions, Weighting,
};
use sciplot_core::{
    apply_formula, parse_initials, unique_name, Axis, AxisScale, ColumnMode, Curve, CurveKind,
    CurveStyle, DataSource, GraphId, LegendPos, Project, Table, TableId, TablePlot3D,
};
use sciplot_plot::GlyphFont;
use serde_json::{json, Map, Value};

use crate::ops::{self, curve_input};
use crate::prefs::Prefs;

const VALUE_CAP: usize = 2000;

#[derive(Clone, Debug)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

#[derive(Clone, Debug, Default)]
pub struct ToolOutput {
    pub text: String,
    pub data: Value,
    pub png: Option<Vec<u8>>,
    pub svg: Option<String>,
    pub graph: Option<GraphId>,
    pub table: Option<TableId>,
    pub open_graph: bool,
    pub open_table: bool,
    /// 打开或保存之后，项目文件的路径。
    pub project_path: Option<String>,
    pub replaced_project: bool,
}

impl ToolOutput {
    fn new(text: impl Into<String>, data: Value) -> Self {
        let text = text.into();
        let shown = match serde_json::to_string_pretty(&data) {
            Ok(body) if !data.is_null() => format!("{text}\n{body}"),
            _ => text,
        };
        Self {
            text: shown,
            data,
            png: None,
            svg: None,
            graph: None,
            table: None,
            open_graph: false,
            open_table: false,
            project_path: None,
            replaced_project: false,
        }
    }
}

pub fn readonly(name: &str) -> bool {
    matches!(name, "get_project" | "render_graph" | "save_project")
}

pub fn list() -> Vec<ToolSpec> {
    vec![
        spec(
            "get_project",
            "列出当前项目的表格、列、图形、曲线和矩阵。List tables, columns, graphs, curves, and matrices.",
            props(&[]),
            &[],
        ),
        spec(
            "create_table",
            "新建表格并写入列。values 里可以是数字、null 或文本。role 可以是 x、y、z、xerr、yerr、label、none。Create a table. values may be numbers, null, or text.",
            props(&[
                ("name", string("表格名称。Table name.")),
                ("columns", json!({"type": "array", "description": "列。Columns.", "items": {"type": "object"}})),
            ]),
            &["name", "columns"],
        ),
        spec(
            "set_column_formula",
            "给一列写公式并立即计算。公式用 col(\"列名\")、col(1)、cell(\"列名\", 行号)、i。Set a column formula and evaluate it.",
            props(&[
                ("table", id_prop("表格名称或 id。Table name or id.")),
                ("column", id_prop("列名，或从 1 开始的序号。Column name or 1-based index.")),
                ("expr", string("公式。Formula.")),
            ]),
            &["table", "column", "expr"],
        ),
        spec(
            "rename_column",
            "重命名一列，并改写本表公式里的 col(\"旧名\") 和 cell(\"旧名\", …)。Rename a column and update formulas in that table.",
            props(&[
                ("table", id_prop("表格名称或 id。Table name or id.")),
                ("column", id_prop("列名，或从 1 开始的序号。Column name or 1-based index.")),
                ("name", string("新列名。New column name.")),
            ]),
            &["table", "column", "name"],
        ),
        spec(
            "plot_columns",
            "用表格的列画图。kind 可以是 line、scatter、line_symbol、steps、horizontal_steps、drop_lines、spline、bars、horizontal_bars、histogram、stacked_histogram、area、error_bars、box、pie、vector。向量可再给 vector_mode：dxdy、endpoint、angle。graph 省略时新建一张图。Plot columns. Omit graph to create one.",
            props(&[
                ("table", id_prop("表格名称或 id。Table name or id.")),
                ("x", id_prop("横轴列。默认是角色为 X 的列。X column. Defaults to the X-role column.")),
                ("y", json!({"type": "array", "description": "纵轴列，列名或从 1 开始的序号。Y columns, by name or 1-based index."})),
                ("kind", string("图形类型，默认 line。Plot kind, default line.")),
                ("graph", id_prop("已有图的名称或 id。省略则新建。Existing graph. Omit to create one.")),
                ("title", string("图层标题。Layer title.")),
                ("x_err", id_prop("x 误差列。可选。Optional x-error column.")),
                ("y_err", id_prop("y 误差列。可选。Optional y-error column.")),
                ("vector_mode", string("向量列的含义：dxdy、endpoint（终点 XY）或 angle（弧度角和模长）。Vector columns: dxdy, endpoint, or angle.")),
            ]),
            &["table", "y"],
        ),
        spec(
            "add_function_curve",
            "添加函数曲线。form 省略或 cartesian 时是 y=f(x)；parametric 时 expr 是 x(t)、y_expr 是 y(t)；polar 时 expr 是 r(θ)，θ 用弧度，可以用 theta、t 或 x。x_min/x_max 是 x、t 或 θ 的范围。Add a function curve: cartesian y=f(x), parametric, or polar.",
            props(&[
                ("expr", string("表达式。笛卡尔和极坐标用这一个；参数曲线把它当作 x(t)。Expression. For a parametric curve this is x(t).")),
                ("y_expr", string("参数曲线的 y(t)。Parametric y(t).")),
                ("form", string("cartesian、parametric 或 polar。默认 cartesian。")),
                ("x_min", json!({"type": "number"})),
                ("x_max", json!({"type": "number"})),
                ("samples", json!({"type": "integer", "description": "采样点数，默认 200。Sample count, default 200."})),
                ("graph", id_prop("已有图。省略则新建。Existing graph, or omit to create one.")),
                ("name", string("曲线名。默认用表达式。Curve name. Defaults to the expression.")),
            ]),
            &["expr", "x_min", "x_max"],
        ),
        spec(
            "set_graph_options",
            "改标题、坐标轴标题、范围、步长、刻度（linear、log10、reciprocal）、反向轴和图例。只改给出的字段；min/max/step 用 null 表示恢复自动。Set title, axis labels, range, step, scale, inversion, and legend.",
            props(&[
                ("graph", id_prop("图形名称或 id。Graph name or id.")),
                ("title", string("标题。Title.")),
                ("x_label", string("横轴标题。X axis label.")),
                ("y_label", string("纵轴标题。Y axis label.")),
                ("x_min", json!({"type": ["number", "null"]})),
                ("x_max", json!({"type": ["number", "null"]})),
                ("y_min", json!({"type": ["number", "null"]})),
                ("y_max", json!({"type": ["number", "null"]})),
                ("x_step", json!({"type": ["number", "null"]})),
                ("y_step", json!({"type": ["number", "null"]})),
                ("x_scale", string("linear、log10 或 reciprocal。")),
                ("y_scale", string("linear、log10 或 reciprocal。")),
                ("x_inverted", json!({"type": "boolean"})),
                ("y_inverted", json!({"type": "boolean"})),
                ("legend", json!({"type": "boolean"})),
                ("legend_position", string("top_right、top_left、bottom_right、bottom_left。")),
            ]),
            &["graph"],
        ),
        spec(
            "fit_curve",
            "拟合一条曲线，并把拟合函数画到同一张图上。model 可以是 linear、polynomial、exponential、exp_decay1、exp_decay2、exp_decay3、exp_growth、boltzmann、gaussian、lorentzian、multi_gaussian、multi_lorentzian、custom、plugin。Fit a curve and draw the fit on the same graph.",
            props(&[
                ("graph", id_prop("图形名称或 id。Graph name or id.")),
                ("curve", id_prop("曲线名，或从 1 开始的序号。Curve name or 1-based index.")),
                ("model", string("拟合模型。Fit model.")),
                ("degree", json!({"type": "integer", "description": "多项式阶数，默认 2。Polynomial degree, default 2."})),
                ("expr", string("自定义公式，例如 a*exp(-b*x)+c。Custom expression.")),
                ("initial", string("自定义参数初值，例如 a=1, b=0.5。Initial values.")),
                ("centers", json!({"type": "array", "items": {"type": "number"}, "description": "多峰的峰位。Peak centers."})),
                ("plugin", string("插件文件路径，model 为 plugin 时使用。Path to a fit plugin.")),
                ("weight", string("none、instrumental、statistical、dataset、direct。")),
                ("weight_column", string("dataset 或 direct 用的列名。Column used as errors or weights.")),
                ("algorithm", string("scaled_lm、unscaled_lm、nelder_mead。")),
                ("tolerance", json!({"type": "number"})),
                ("max_iterations", json!({"type": "integer"})),
                ("x_min", json!({"type": "number"})),
                ("x_max", json!({"type": "number"})),
            ]),
            &["graph", "curve"],
        ),
        spec(
            "plot_table_3d",
            "用表格里的一列画三维色带、三维柱或三维轨迹。kind 是 ribbon、bars 或 trajectory。Plot a table column as a 3D ribbon, bars, or trajectory.",
            props(&[
                ("table", id_prop("表格名称或 id。Table name or id.")),
                ("column", id_prop("作为高度的列。Column used as Z.")),
                ("kind", string("ribbon、bars 或 trajectory。")),
            ]),
            &["table", "column", "kind"],
        ),
        spec(
            "signal_columns",
            "对表格列做卷积、反卷积、互相关或自相关。kind 是 convolve、deconvolve、correlate、autocorrelate。Convolve, deconvolve, or correlate table columns.",
            props(&[
                ("table", id_prop("表格名称或 id。Table name or id.")),
                ("kind", string("convolve、deconvolve、correlate 或 autocorrelate。")),
                ("column_a", id_prop("第一列。First column.")),
                ("column_b", id_prop("第二列。自相关可以省略。Second column. Optional for autocorrelation.")),
            ]),
            &["table", "kind", "column_a"],
        ),
        spec(
            "find_intersections",
            "求两条曲线的交点，标在图上，并写入新表。smooth 为 true 时用样条或表达式求根。Find intersections, mark them, and write a table.",
            props(&[
                ("graph", id_prop("图形名称或 id。Graph name or id.")),
                ("curve_a", id_prop("第一条曲线。First curve.")),
                ("curve_b", id_prop("第二条曲线。Second curve.")),
                ("smooth", json!({"type": "boolean", "description": "平滑求根。默认 false，按折线求交。Smooth roots. Default false uses polylines."})),
                ("x_min", json!({"type": "number"})),
                ("x_max", json!({"type": "number"})),
                ("tolerance", json!({"type": "number", "description": "去重容差，默认 1e-8。Dedup tolerance, default 1e-8."})),
            ]),
            &["graph", "curve_a", "curve_b"],
        ),
        spec(
            "render_graph",
            "把图渲染成 PNG 或 SVG。format 默认 png。Render a graph to PNG or SVG.",
            props(&[
                ("graph", id_prop("图形名称或 id。Graph name or id.")),
                ("format", string("png 或 svg，默认 png。")),
                ("width", json!({"type": "number", "description": "像素宽，默认 1100。Width in pixels, default 1100."})),
                ("height", json!({"type": "number", "description": "像素高，默认 720。Height in pixels, default 720."})),
                ("path", string("可选，把文件写到这个路径。Optional file path.")),
            ]),
            &["graph"],
        ),
        spec(
            "open_project",
            "打开一个 .splot 项目，替换当前内存里的项目。Open a .splot file, replacing the in-memory project.",
            props(&[("path", string(".splot 路径。Path to a .splot file."))]),
            &["path"],
        ),
        spec(
            "save_project",
            "把当前项目存成 .splot。Save the current project as .splot.",
            props(&[("path", string(".splot 路径。窗口里可以省略，沿用当前文件。Path. Optional in the window."))]),
            &["path"],
        ),
    ]
}

fn spec(name: &'static str, description: &'static str, properties: Map<String, Value>, required: &[&str]) -> ToolSpec {
    ToolSpec {
        name,
        description,
        input_schema: json!({
            "type": "object",
            "properties": properties,
            "required": required,
        }),
    }
}

fn props(fields: &[(&str, Value)]) -> Map<String, Value> {
    fields.iter().map(|(key, value)| ((*key).to_string(), value.clone())).collect()
}

fn string(description: &str) -> Value {
    json!({"type": "string", "description": description})
}

fn id_prop(description: &str) -> Value {
    json!({"description": description, "anyOf": [{"type": "string"}, {"type": "integer"}]})
}

pub fn dispatch(
    project: &mut Project,
    font: Option<&GlyphFont>,
    prefs: &Prefs,
    name: &str,
    args: &Value,
) -> Result<ToolOutput, String> {
    let empty = Map::new();
    let obj = if args.is_null() {
        &empty
    } else {
        args.as_object().ok_or_else(|| sciplot_core::tr!("参数必须是 JSON 对象", "Arguments must be a JSON object").to_string())?
    };
    match name {
        "get_project" => Ok(ToolOutput::new(sciplot_core::t("当前项目"), project_json(project))),
        "create_table" => create_table(project, obj),
        "set_column_formula" => set_column_formula(project, obj),
        "rename_column" => rename_column(project, obj),
        "plot_columns" => plot_columns(project, prefs, obj),
        "add_function_curve" => add_function_curve(project, prefs, obj),
        "set_graph_options" => set_graph_options(project, obj),
        "fit_curve" => fit_curve(project, prefs, obj),
        "plot_table_3d" => plot_table_3d(project, prefs, obj),
        "signal_columns" => signal_columns(project, prefs, obj),
        "find_intersections" => find_intersections(project, obj),
        "render_graph" => render_graph(project, font, obj),
        "open_project" => open_project(project, obj),
        "save_project" => save_project(project, obj),
        other => Err(sciplot_core::trf!("未知工具：{name}", "Unknown tool: {name}", name = other)),
    }
}

fn project_json(project: &Project) -> Value {
    json!({
        "name": project.name,
        "tables": project.tables.iter().map(table_json).collect::<Vec<_>>(),
        "graphs": project.graphs.iter().map(graph_json).collect::<Vec<_>>(),
        "matrices": project.matrices.iter().map(|matrix| json!({
            "id": matrix.id.raw(),
            "name": matrix.name,
            "rows": matrix.rows,
            "cols": matrix.cols,
        })).collect::<Vec<_>>(),
    })
}

fn table_json(table: &Table) -> Value {
    let rows = table.nrows();
    json!({
        "id": table.id.raw(),
        "name": table.name,
        "rows": rows,
        "columns": table.columns.iter().enumerate().map(|(index, column)| {
            let shown = rows.min(VALUE_CAP);
            let mut values = Vec::with_capacity(shown);
            for row in 0..shown {
                values.push(cell_json(column, row));
            }
            json!({
                "index": index + 1,
                "name": column.name,
                "role": mode_key(column.mode),
                "formula": column.formula,
                "values": values,
                "truncated": rows > VALUE_CAP,
            })
        }).collect::<Vec<_>>(),
    })
}

fn cell_json(column: &sciplot_core::Column, row: usize) -> Value {
    let text = column.texts.get(row).map(String::as_str).unwrap_or("");
    let number = column.numbers.get(row).copied().flatten();
    if !text.is_empty() {
        if let Some(number) = number {
            if text.parse::<f64>().ok() == Some(number) || sciplot_core::format_number(number) == text {
                return json!(number);
            }
        }
        return json!(text);
    }
    match number {
        Some(number) => json!(number),
        None => Value::Null,
    }
}

fn graph_json(graph: &sciplot_core::Graph) -> Value {
    json!({
        "id": graph.id.raw(),
        "name": graph.name,
        "surface": graph.surface.is_some(),
        "layers": graph.layers.iter().enumerate().map(|(index, layer)| json!({
            "index": index + 1,
            "title": layer.title,
            "x_axis": axis_json(&layer.x_axis),
            "y_axis": axis_json(&layer.y_axis),
            "legend": {
                "visible": layer.legend.visible,
                "position": legend_key(layer.legend.position),
            },
            "curves": layer.curves.iter().enumerate().map(|(index, curve)| json!({
                "index": index + 1,
                "name": curve.name,
                "kind": kind_key(curve.kind),
                "visible": curve.visible,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

fn axis_json(axis: &Axis) -> Value {
    json!({
        "label": axis.label,
        "scale": scale_key(axis.scale),
        "min": axis.min,
        "max": axis.max,
        "step": axis.step,
        "inverted": axis.inverted,
    })
}

fn create_table(project: &mut Project, args: &Map<String, Value>) -> Result<ToolOutput, String> {
    let name = req_str(args, "name")?;
    let columns = args
        .get("columns")
        .and_then(Value::as_array)
        .ok_or_else(|| sciplot_core::tr!("缺少参数 columns", "Missing argument columns").to_string())?;
    let id = project.new_table(&name);
    {
        let table = project.table_mut(id).ok_or_else(|| sciplot_core::t("找不到表格").to_string())?;
        if !columns.is_empty() {
            table.columns.clear();
        }
        for (index, column) in columns.iter().enumerate() {
            let column = column.as_object().ok_or_else(|| sciplot_core::tr!("columns 里的每一项都要是对象", "Each column must be an object").to_string())?;
            let fallback = format!("C{}", index + 1);
            let column_name = column.get("name").and_then(Value::as_str).unwrap_or(fallback.as_str());
            let mode = match column.get("role").and_then(Value::as_str) {
                Some(role) => parse_mode(role)?,
                None if index == 0 => ColumnMode::X,
                None => ColumnMode::Y,
            };
            let at = table.add_column(column_name, mode);
            if let Some(values) = column.get("values").and_then(Value::as_array) {
                for (row, value) in values.iter().enumerate() {
                    write_cell(&mut table.columns[at], row, value)?;
                }
            }
        }
    }
    let table = project.table(id).ok_or_else(|| sciplot_core::t("找不到表格").to_string())?;
    let mut output = ToolOutput::new(
        sciplot_core::trf!(
            "已新建表格「{name}」，共 {n} 列。",
            "Created table \"{name}\" with {n} columns.",
            name = table.name,
            n = table.columns.len()
        ),
        json!({"id": id.raw(), "name": table.name}),
    );
    output.table = Some(id);
    output.open_table = true;
    Ok(output)
}

fn write_cell(column: &mut sciplot_core::Column, row: usize, value: &Value) -> Result<(), String> {
    match value {
        Value::Null => {
            column.resize(column.len().max(row + 1));
            column.texts[row].clear();
            column.numbers[row] = None;
            Ok(())
        }
        Value::Number(number) => {
            let number = number.as_f64().ok_or_else(|| sciplot_core::tr!("单元格不是有限数值", "A cell is not a finite number").to_string())?;
            if !number.is_finite() {
                return Err(sciplot_core::tr!("单元格不是有限数值", "A cell is not a finite number").into());
            }
            column.resize(column.len().max(row + 1));
            column.set_text(row, &sciplot_core::format_number(number));
            Ok(())
        }
        Value::String(text) => {
            column.set_text(row, text);
            Ok(())
        }
        Value::Bool(flag) => {
            column.set_text(row, if *flag { "1" } else { "0" });
            Ok(())
        }
        _ => Err(sciplot_core::tr!("单元格只能是数字、文本或 null", "A cell must be a number, text, or null").into()),
    }
}

fn set_column_formula(project: &mut Project, args: &Map<String, Value>) -> Result<ToolOutput, String> {
    let table_id = find_table(project, req(args, "table")?)?;
    let expr = req_str(args, "expr")?;
    let column = {
        let table = project.table(table_id).ok_or_else(|| sciplot_core::t("找不到表格").to_string())?;
        find_column(table, req(args, "column")?)?
    };
    let evaluated = {
        let table = project.table_mut(table_id).ok_or_else(|| sciplot_core::t("找不到表格").to_string())?;
        apply_formula(table, column, &expr)?
    };
    let written = evaluated.values.iter().filter(|value| value.is_some()).count();
    let name = project.table(table_id).and_then(|table| table.columns.get(column)).map(|column| column.name.clone()).unwrap_or_default();
    Ok(ToolOutput::new(
        sciplot_core::trf!(
            "已计算列「{name}」，写入 {written} 个数值。",
            "Evaluated column \"{name}\" and wrote {written} numbers.",
            name = name,
            written = written
        ),
        json!({"table_id": table_id.raw(), "column": column + 1, "written": written}),
    ))
}

fn rename_column(project: &mut Project, args: &Map<String, Value>) -> Result<ToolOutput, String> {
    let table_id = find_table(project, req(args, "table")?)?;
    let column = {
        let table = project.table(table_id).ok_or_else(|| sciplot_core::t("找不到表格").to_string())?;
        find_column(table, req(args, "column")?)?
    };
    let name = req_str(args, "name")?;
    project.rename_table_column(table_id, column, &name)?;
    Ok(ToolOutput::new(
        sciplot_core::trf!("已把列改名为「{name}」。", "Renamed the column to \"{name}\".", name = name),
        json!({"table_id": table_id.raw(), "column": column + 1, "name": name}),
    ))
}

fn plot_columns(project: &mut Project, prefs: &Prefs, args: &Map<String, Value>) -> Result<ToolOutput, String> {
    let table_id = find_table(project, req(args, "table")?)?;
    let kind = match args.get("kind").and_then(Value::as_str) {
        Some(kind) => parse_kind(kind)?,
        None => CurveKind::Line,
    };
    let (x, y_cols, y_names, x_label, x_err, y_err, table_name) = {
        let table = project.table(table_id).ok_or_else(|| sciplot_core::t("找不到表格").to_string())?;
        let x = match args.get("x") {
            Some(value) => find_column(table, value)?,
            None => table
                .columns
                .iter()
                .position(|column| column.mode == ColumnMode::X)
                .unwrap_or(0),
        };
        let y_values = args
            .get("y")
            .and_then(Value::as_array)
            .ok_or_else(|| sciplot_core::tr!("缺少参数 y", "Missing argument y").to_string())?;
        if y_values.is_empty() && kind != CurveKind::Vector {
            return Err(sciplot_core::tr!("至少指定一列 Y", "Name at least one Y column").into());
        }
        let mut y_cols = Vec::new();
        for value in y_values {
            y_cols.push(find_column(table, value)?);
        }
        let y_names: Vec<String> = y_cols
            .iter()
            .map(|index| {
                table
                    .columns
                    .get(*index)
                    .map(|column| column.name.clone())
                    .unwrap_or_else(|| format!("Y{}", index + 1))
            })
            .collect();
        let x_label = table.columns.get(x).map(|column| column.name.clone()).unwrap_or_else(|| "x".into());
        let x_err = optional_column(table, args.get("x_err"))?;
        let y_err = optional_column(table, args.get("y_err"))?;
        (x, y_cols, y_names, x_label, x_err, y_err, table.name.clone())
    };
    let existing = args.get("graph").filter(|value| !value.is_null());
    let created = existing.is_none();
    let graph_id = if let Some(value) = existing {
        find_graph(project, value)?
    } else {
        let y_name = y_names.first().cloned().unwrap_or_else(|| kind.label().to_string());
        let base = format!("{y_name}-{}", kind.label());
        project.new_graph(&base)
    };
    let title = args.get("title").and_then(Value::as_str).map(str::to_string);
    let before = project.graph(graph_id).and_then(|graph| graph.layers.first()).map(|layer| layer.curves.len()).unwrap_or(0);
    if kind == CurveKind::Vector {
        let mode = parse_vector_mode(args.get("vector_mode").and_then(Value::as_str).unwrap_or("dxdy"))?;
        {
            let graph = project.graph_mut(graph_id).ok_or_else(|| sciplot_core::t("找不到图形").to_string())?;
            if graph.layers.is_empty() {
                graph.layers.push(sciplot_core::Layer::default());
            }
            let graph_name = graph.name.clone();
            let layer = &mut graph.layers[0];
            if let Some(title) = &title {
                layer.title = title.clone();
            } else if created && layer.title.is_empty() {
                layer.title = graph_name;
            }
        }
        project.add_curves_with(graph_id, 0, table_id, kind, mode)?;
    } else {
        let graph = project.graph_mut(graph_id).ok_or_else(|| sciplot_core::t("找不到图形").to_string())?;
        if graph.layers.is_empty() {
            graph.layers.push(sciplot_core::Layer::default());
        }
        let graph_name = graph.name.clone();
        let layer = &mut graph.layers[0];
        if created {
            if layer.x_axis.label.is_empty() || layer.x_axis.label == "x" {
                layer.x_axis.label = x_label;
            }
            if y_names.len() == 1 && (layer.y_axis.label.is_empty() || layer.y_axis.label == "y") {
                layer.y_axis.label = y_names[0].clone();
            }
            if let Some(title) = &title {
                layer.title = title.clone();
            } else if layer.title.is_empty() {
                layer.title = graph_name;
            }
        } else if let Some(title) = &title {
            layer.title = title.clone();
        }
        for (y, name) in y_cols.into_iter().zip(y_names) {
            let style = CurveStyle::from_index(layer.curves.len());
            let mut curve = Curve {
                name,
                kind,
                style,
                source: DataSource::Columns {
                    table: table_id,
                    x,
                    y,
                    x_err,
                    y_err,
                    dx: None,
                    dy: None,
                },
                visible: true,
                ..Curve::default()
            };
            if x_err.is_some() {
                curve.error.x = sciplot_core::ErrorSource::Column;
            }
            if y_err.is_some() {
                curve.error.y = sciplot_core::ErrorSource::Column;
            }
            layer.curves.push(curve);
        }
    }
    if created {
        prefs.apply_new_graph(project, graph_id);
    } else {
        prefs.apply_curves(project, graph_id, 0, before);
    }
    let graph_name = project.graph(graph_id).map(|graph| graph.name.clone()).unwrap_or_default();
    let curves = project.graph(graph_id).and_then(|graph| graph.layers.first()).map(|layer| layer.curves.len()).unwrap_or(0);
    let mut output = ToolOutput::new(
        sciplot_core::trf!(
            "已把 {table_name} 画到「{graph_name}」，图上现有 {curves} 条曲线。",
            "Plotted {table_name} on \"{graph_name}\". The graph now has {curves} curves.",
            table_name = table_name,
            graph_name = graph_name,
            curves = curves
        ),
        json!({"graph_id": graph_id.raw(), "name": graph_name, "curves": curves}),
    );
    output.graph = Some(graph_id);
    output.open_graph = true;
    Ok(output)
}

fn optional_column(table: &Table, value: Option<&Value>) -> Result<Option<usize>, String> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(value) => Ok(Some(find_column(table, value)?)),
    }
}

fn add_function_curve(project: &mut Project, prefs: &Prefs, args: &Map<String, Value>) -> Result<ToolOutput, String> {
    let expr = req_str(args, "expr")?;
    sciplot_core::CompiledExpr::compile(&expr)?;
    let form = args.get("form").and_then(Value::as_str).unwrap_or("cartesian");
    let y_expr = args.get("y_expr").and_then(Value::as_str).unwrap_or("").to_string();
    if matches!(form, "parametric" | "参数" | "参数曲线") {
        if y_expr.trim().is_empty() {
            return Err(sciplot_core::tr!("参数曲线需要 y_expr", "A parametric curve needs y_expr").into());
        }
        sciplot_core::CompiledExpr::compile(&y_expr)?;
    }
    let x_min = req_f64(args, "x_min")?;
    let x_max = req_f64(args, "x_max")?;
    if !x_min.is_finite() || !x_max.is_finite() || x_max <= x_min {
        return Err(sciplot_core::tr!("x_min 必须小于 x_max", "x_min must be less than x_max").into());
    }
    let samples = args.get("samples").and_then(Value::as_u64).unwrap_or(200).clamp(8, 5000) as usize;
    let curve_name = args.get("name").and_then(Value::as_str).unwrap_or(expr.as_str()).to_string();
    let created = args.get("graph").map(|value| value.is_null()).unwrap_or(true);
    let graph_id = if created {
        project.new_graph(&curve_name)
    } else {
        find_graph(project, req(args, "graph")?)?
    };
    let before = project.graph(graph_id).and_then(|graph| graph.layers.first()).map(|layer| layer.curves.len()).unwrap_or(0);
    {
        let graph = project.graph_mut(graph_id).ok_or_else(|| sciplot_core::t("找不到图形").to_string())?;
        if graph.layers.is_empty() {
            graph.layers.push(sciplot_core::Layer::default());
        }
        let layer = &mut graph.layers[0];
        if created && layer.title.is_empty() {
            layer.title = curve_name.clone();
        }
        let style = CurveStyle::from_index(layer.curves.len());
        let (kind, source) = if matches!(form, "parametric" | "参数" | "参数曲线") {
            (
                CurveKind::Parametric,
                DataSource::Parametric {
                    x_expr: expr.clone(),
                    y_expr: y_expr.clone(),
                    t_min: x_min,
                    t_max: x_max,
                    samples,
                },
            )
        } else if matches!(form, "polar" | "极坐标") {
            (
                CurveKind::Polar,
                DataSource::Polar {
                    expr: expr.clone(),
                    theta_min: x_min,
                    theta_max: x_max,
                    samples,
                },
            )
        } else {
            (
                CurveKind::Function,
                DataSource::Function {
                    expr: expr.clone(),
                    x_min,
                    x_max,
                    samples,
                },
            )
        };
        layer.curves.push(Curve {
            name: curve_name.clone(),
            kind,
            style,
            source,
            visible: true,
            ..Curve::default()
        });
    }
    if created {
        prefs.apply_new_graph(project, graph_id);
    } else {
        prefs.apply_curves(project, graph_id, 0, before);
    }
    let graph_name = project.graph(graph_id).map(|graph| graph.name.clone()).unwrap_or_default();
    let mut output = ToolOutput::new(
        sciplot_core::trf!(
            "已添加函数曲线「{name}」。",
            "Added function curve \"{name}\".",
            name = curve_name
        ),
        json!({"graph_id": graph_id.raw(), "name": graph_name}),
    );
    output.graph = Some(graph_id);
    output.open_graph = true;
    Ok(output)
}

fn set_graph_options(project: &mut Project, args: &Map<String, Value>) -> Result<ToolOutput, String> {
    let graph_id = find_graph(project, req(args, "graph")?)?;
    {
        let graph = project.graph_mut(graph_id).ok_or_else(|| sciplot_core::t("找不到图形").to_string())?;
        let layer = graph.layers.first_mut().ok_or_else(|| sciplot_core::t("找不到图层").to_string())?;
        if let Some(title) = args.get("title").and_then(Value::as_str) {
            layer.title = title.to_string();
        }
        if let Some(label) = args.get("x_label").and_then(Value::as_str) {
            layer.x_axis.label = label.to_string();
        }
        if let Some(label) = args.get("y_label").and_then(Value::as_str) {
            layer.y_axis.label = label.to_string();
        }
        apply_bound(&mut layer.x_axis.min, args.get("x_min"))?;
        apply_bound(&mut layer.x_axis.max, args.get("x_max"))?;
        apply_bound(&mut layer.y_axis.min, args.get("y_min"))?;
        apply_bound(&mut layer.y_axis.max, args.get("y_max"))?;
        apply_bound(&mut layer.x_axis.step, args.get("x_step"))?;
        apply_bound(&mut layer.y_axis.step, args.get("y_step"))?;
        if let Some(scale) = args.get("x_scale").and_then(Value::as_str) {
            layer.x_axis.scale = parse_scale(scale)?;
        }
        if let Some(scale) = args.get("y_scale").and_then(Value::as_str) {
            layer.y_axis.scale = parse_scale(scale)?;
        }
        if let Some(flag) = args.get("x_inverted").and_then(Value::as_bool) {
            layer.x_axis.inverted = flag;
        }
        if let Some(flag) = args.get("y_inverted").and_then(Value::as_bool) {
            layer.y_axis.inverted = flag;
        }
        if let Some(label) = args.get("x_top_label").and_then(Value::as_str) {
            layer.x_top.label = label.to_string();
            layer.show_top = true;
        }
        if let Some(label) = args.get("y_right_label").and_then(Value::as_str) {
            layer.y_right.label = label.to_string();
            layer.show_right = true;
        }
        apply_bound(&mut layer.y_right.min, args.get("y_right_min"))?;
        apply_bound(&mut layer.y_right.max, args.get("y_right_max"))?;
        if let Some(side) = args.get("curve_y_side").and_then(Value::as_str) {
            let index = args.get("curve").and_then(Value::as_u64).unwrap_or(1).saturating_sub(1) as usize;
            let curve = layer.curves.get_mut(index).ok_or_else(|| sciplot_core::t("找不到曲线").to_string())?;
            curve.axes.y = if matches!(side, "right" | "右" | "y_right") {
                sciplot_core::YAxisSide::Right
            } else {
                sciplot_core::YAxisSide::Left
            };
            if curve.axes.y == sciplot_core::YAxisSide::Right {
                layer.show_right = true;
            }
        }
        if let Some(side) = args.get("curve_x_side").and_then(Value::as_str) {
            let index = args.get("curve").and_then(Value::as_u64).unwrap_or(1).saturating_sub(1) as usize;
            let curve = layer.curves.get_mut(index).ok_or_else(|| sciplot_core::t("找不到曲线").to_string())?;
            curve.axes.x = if matches!(side, "top" | "上" | "x_top") {
                sciplot_core::XAxisSide::Top
            } else {
                sciplot_core::XAxisSide::Bottom
            };
            if curve.axes.x == sciplot_core::XAxisSide::Top {
                layer.show_top = true;
            }
        }
        if let Some(flag) = args.get("legend").and_then(Value::as_bool) {
            layer.legend.visible = flag;
        }
        if let Some(position) = args.get("legend_position").and_then(Value::as_str) {
            layer.legend.position = parse_legend(position)?;
        }
    }
    let name = project.graph(graph_id).map(|graph| graph.name.clone()).unwrap_or_default();
    let mut output = ToolOutput::new(
        sciplot_core::trf!("已更新图形「{name}」。", "Updated graph \"{name}\".", name = name),
        json!({"graph_id": graph_id.raw(), "name": name}),
    );
    output.graph = Some(graph_id);
    output.open_graph = true;
    Ok(output)
}

fn apply_bound(slot: &mut Option<f64>, value: Option<&Value>) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.is_null() {
        *slot = None;
        return Ok(());
    }
    let number = value.as_f64().ok_or_else(|| sciplot_core::tr!("范围和步长必须是数值或 null", "Range and step must be a number or null").to_string())?;
    if !number.is_finite() {
        return Err(sciplot_core::tr!("范围和步长必须是有限数值", "Range and step must be finite").into());
    }
    *slot = Some(number);
    Ok(())
}

fn fit_curve(project: &mut Project, prefs: &Prefs, args: &Map<String, Value>) -> Result<ToolOutput, String> {
    let graph_id = find_graph(project, req(args, "graph")?)?;
    let curve = {
        let graph = project.graph(graph_id).ok_or_else(|| sciplot_core::t("找不到图形").to_string())?;
        let layer = graph.layers.first().ok_or_else(|| sciplot_core::t("找不到图层").to_string())?;
        find_curve(layer, req(args, "curve")?)?
    };
    let model_name = args.get("model").and_then(Value::as_str).unwrap_or("linear");
    let model = parse_model(model_name, args)?;
    let mut options = parse_fit_options(args)?;
    if matches!(model_name.trim().to_ascii_lowercase().as_str(), "plugin" | "插件") {
        if let Some(path) = args.get("plugin").and_then(Value::as_str) {
            if let Ok(text) = std::fs::read_to_string(path) {
                if let Ok(plugin) = sciplot_analysis::parse_fit_plugin(&text) {
                    options.plugin_name = Some(plugin.name);
                }
            }
        }
    }
    if matches!(options.weighting, Weighting::Dataset | Weighting::Direct) {
        let column = args
            .get("weight_column")
            .and_then(Value::as_str)
            .ok_or_else(|| sciplot_core::tr!("请给出 weight_column", "weight_column is required").to_string())?;
        options.column = ops::weight_column(project, graph_id, 0, curve, column)?;
    }
    let x_min = args.get("x_min").and_then(Value::as_f64);
    let x_max = args.get("x_max").and_then(Value::as_f64);
    let before = project.graph(graph_id).and_then(|graph| graph.layers.first()).map(|layer| layer.curves.len()).unwrap_or(0);
    let report = ops::fit_on_graph(project, graph_id, 0, curve, &model, x_min, x_max, &options)?;
    prefs.apply_curves(project, graph_id, 0, before);
    let mut output = ToolOutput::new(report.clone(), json!({"graph_id": graph_id.raw(), "report": report}));
    output.graph = Some(graph_id);
    output.open_graph = true;
    Ok(output)
}

fn find_intersections(project: &mut Project, args: &Map<String, Value>) -> Result<ToolOutput, String> {
    let graph_id = find_graph(project, req(args, "graph")?)?;
    let (index_a, index_b) = {
        let graph = project.graph(graph_id).ok_or_else(|| sciplot_core::t("找不到图形").to_string())?;
        let layer = graph.layers.first().ok_or_else(|| sciplot_core::t("找不到图层").to_string())?;
        (find_curve(layer, req(args, "curve_a")?)?, find_curve(layer, req(args, "curve_b")?)?)
    };
    let (input_a, input_b, name_a, name_b) = {
        let graph = project.graph(graph_id).ok_or_else(|| sciplot_core::t("找不到图形").to_string())?;
        let layer = graph.layers.first().ok_or_else(|| sciplot_core::t("找不到图层").to_string())?;
        let curve_a = layer.curves.get(index_a).ok_or_else(|| sciplot_core::t("找不到曲线").to_string())?;
        let curve_b = layer.curves.get(index_b).ok_or_else(|| sciplot_core::t("找不到曲线").to_string())?;
        (
            curve_input(project, curve_a)?,
            curve_input(project, curve_b)?,
            curve_a.name.clone(),
            curve_b.name.clone(),
        )
    };
    let smooth = args.get("smooth").and_then(Value::as_bool).unwrap_or(false);
    let result = intersect(
        &input_a,
        &input_b,
        &IntersectOptions {
            mode: if smooth { IntersectMode::Smooth } else { IntersectMode::Piecewise },
            x_min: args.get("x_min").and_then(Value::as_f64),
            x_max: args.get("x_max").and_then(Value::as_f64),
            tolerance: args.get("tolerance").and_then(Value::as_f64).unwrap_or(1e-8),
        },
    );
    let base = sciplot_core::trf!("交点_{a}_{b}", "intersections_{a}_{b}", a = name_a, b = name_b);
    let names: Vec<String> = project.tables.iter().map(|table| table.name.clone()).collect();
    let table_name = unique_name(&names, &base);
    let count = ops::apply_intersection(project, graph_id, 0, &table_name, &result)?;
    let table_id = project.tables.iter().find(|table| table.name == table_name).map(|table| table.id);
    let mut output = ToolOutput::new(
        sciplot_core::trf!(
            "求得 {count} 个交点。",
            "Found {count} intersections.",
            count = count
        ),
        json!({
            "count": count,
            "note": result.note,
            "graph_id": graph_id.raw(),
            "table_id": table_id.map(|id| id.raw()),
            "table": if count == 0 { Value::Null } else { json!(table_name) },
        }),
    );
    output.graph = Some(graph_id);
    output.open_graph = true;
    output.table = table_id;
    output.open_table = table_id.is_some();
    Ok(output)
}

fn render_graph(project: &Project, font: Option<&GlyphFont>, args: &Map<String, Value>) -> Result<ToolOutput, String> {
    let graph_id = find_graph(project, req(args, "graph")?)?;
    let graph = project.graph(graph_id).ok_or_else(|| sciplot_core::t("找不到图形").to_string())?;
    let format = args.get("format").and_then(Value::as_str).unwrap_or("png").to_ascii_lowercase();
    let width = args.get("width").and_then(Value::as_f64).unwrap_or(1100.0).clamp(64.0, 4096.0) as f32;
    let height = args.get("height").and_then(Value::as_f64).unwrap_or(720.0).clamp(64.0, 4096.0) as f32;
    let scene = sciplot_plot::render(project, graph, &[], width, height, 1.0);
    let name = graph.name.clone();
    let notes = scene.notes.clone();
    let mut output = ToolOutput::new(
        sciplot_core::trf!("已渲染「{name}」。", "Rendered \"{name}\".", name = name),
        json!({"graph_id": graph_id.raw(), "name": name, "notes": notes, "format": format.clone()}),
    );
    output.graph = Some(graph_id);
    match format.as_str() {
        "png" => {
            let png = sciplot_plot::to_png(&scene, font)?;
            if let Some(path) = args.get("path").and_then(Value::as_str) {
                write_bytes(Path::new(path), &png)?;
            }
            output.png = Some(png);
        }
        "svg" => {
            let svg = sciplot_plot::to_svg(&scene);
            if let Some(path) = args.get("path").and_then(Value::as_str) {
                write_bytes(Path::new(path), svg.as_bytes())?;
            }
            output.svg = Some(svg);
        }
        other => {
            return Err(sciplot_core::trf!(
                "图片格式用 png 或 svg，不能是 {format}",
                "Image format must be png or svg, not {format}",
                format = other
            ));
        }
    }
    Ok(output)
}

fn open_project(project: &mut Project, args: &Map<String, Value>) -> Result<ToolOutput, String> {
    let path = req_str(args, "path")?;
    let text = std::fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let loaded = Project::from_json(&text)?;
    let name = loaded.name.clone();
    *project = loaded;
    let mut output = ToolOutput::new(
        sciplot_core::trf!("已打开项目 {path}", "Opened project {path}", path = path),
        json!({"name": name, "path": path}),
    );
    output.project_path = Some(path);
    output.replaced_project = true;
    Ok(output)
}

fn save_project(project: &Project, args: &Map<String, Value>) -> Result<ToolOutput, String> {
    let path = req_str(args, "path")?;
    let text = project.to_json()?;
    write_bytes(Path::new(&path), text.as_bytes())?;
    let mut output = ToolOutput::new(
        sciplot_core::trf!("已保存项目到 {path}", "Saved the project to {path}", path = path),
        json!({"path": path}),
    );
    output.project_path = Some(path);
    Ok(output)
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
    }
    std::fs::write(path, bytes).map_err(|err| err.to_string())
}

fn req<'a>(args: &'a Map<String, Value>, key: &str) -> Result<&'a Value, String> {
    args.get(key).filter(|value| !value.is_null()).ok_or_else(|| sciplot_core::trf!("缺少参数 {name}", "Missing argument {name}", name = key))
}

fn req_str(args: &Map<String, Value>, key: &str) -> Result<String, String> {
    req(args, key)?
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .ok_or_else(|| sciplot_core::trf!("参数 {name} 必须是非空文本", "Argument {name} must be non-empty text", name = key))
}

fn req_f64(args: &Map<String, Value>, key: &str) -> Result<f64, String> {
    req(args, key)?
        .as_f64()
        .filter(|value| value.is_finite())
        .ok_or_else(|| sciplot_core::trf!("参数 {name} 必须是有限数值", "Argument {name} must be a finite number", name = key))
}

fn find_table(project: &Project, key: &Value) -> Result<TableId, String> {
    if let Some(name) = key.as_str() {
        let matches: Vec<_> = project.tables.iter().filter(|table| table.name == name).map(|table| table.id).collect();
        if matches.len() > 1 {
            return Err(sciplot_core::trf!(
                "有多张名为「{name}」的表格，请改用 id",
                "More than one table is named \"{name}\". Use its id.",
                name = name
            ));
        }
        if let Some(id) = matches.first() {
            return Ok(*id);
        }
        if let Some(index) = parse_positive(name) {
            if let Some(table) = project.tables.iter().find(|table| table.id.raw() == index as u64) {
                return Ok(table.id);
            }
        }
        return Err(sciplot_core::trf!("找不到名为「{name}」的表格", "No table named \"{name}\"", name = name));
    }
    if let Some(id) = json_u64(key) {
        return project
            .tables
            .iter()
            .find(|table| table.id.raw() == id)
            .map(|table| table.id)
            .ok_or_else(|| sciplot_core::trf!("找不到 id 为 {id} 的表格", "No table with id {id}", id = id));
    }
    Err(sciplot_core::tr!("表格要用名称或 id", "A table is named or given by id").into())
}

fn find_graph(project: &Project, key: &Value) -> Result<GraphId, String> {
    if let Some(name) = key.as_str() {
        let matches: Vec<_> = project.graphs.iter().filter(|graph| graph.name == name).map(|graph| graph.id).collect();
        if matches.len() > 1 {
            return Err(sciplot_core::trf!(
                "有多张名为「{name}」的图形，请改用 id",
                "More than one graph is named \"{name}\". Use its id.",
                name = name
            ));
        }
        if let Some(id) = matches.first() {
            return Ok(*id);
        }
        if let Some(index) = parse_positive(name) {
            if let Some(graph) = project.graphs.iter().find(|graph| graph.id.raw() == index as u64) {
                return Ok(graph.id);
            }
        }
        return Err(sciplot_core::trf!("找不到名为「{name}」的图形", "No graph named \"{name}\"", name = name));
    }
    if let Some(id) = json_u64(key) {
        return project
            .graphs
            .iter()
            .find(|graph| graph.id.raw() == id)
            .map(|graph| graph.id)
            .ok_or_else(|| sciplot_core::trf!("找不到 id 为 {id} 的图形", "No graph with id {id}", id = id));
    }
    Err(sciplot_core::tr!("图形要用名称或 id", "A graph is named or given by id").into())
}

fn find_column(table: &Table, key: &Value) -> Result<usize, String> {
    if let Some(name) = key.as_str() {
        if let Some(index) = table.columns.iter().position(|column| column.name == name) {
            return Ok(index);
        }
        if let Some(index) = parse_positive(name) {
            if index <= table.columns.len() {
                return Ok(index - 1);
            }
        }
        return Err(sciplot_core::trf!("找不到列「{name}」", "Column \"{name}\" was not found", name = name));
    }
    if let Some(index) = json_u64(key) {
        if index == 0 || index as usize > table.columns.len() {
            return Err(sciplot_core::tr!("列序号从 1 开始", "Column numbers start at 1").into());
        }
        return Ok(index as usize - 1);
    }
    Err(sciplot_core::tr!("列要用列名或从 1 开始的序号", "A column is named or given by a 1-based index").into())
}

fn find_curve(layer: &sciplot_core::Layer, key: &Value) -> Result<usize, String> {
    if let Some(name) = key.as_str() {
        if let Some(index) = layer.curves.iter().position(|curve| curve.name == name) {
            return Ok(index);
        }
        if let Some(index) = parse_positive(name) {
            if index <= layer.curves.len() {
                return Ok(index - 1);
            }
        }
        return Err(sciplot_core::trf!("找不到曲线「{name}」", "Curve \"{name}\" was not found", name = name));
    }
    if let Some(index) = json_u64(key) {
        if index == 0 || index as usize > layer.curves.len() {
            return Err(sciplot_core::tr!("曲线序号从 1 开始", "Curve numbers start at 1").into());
        }
        return Ok(index as usize - 1);
    }
    Err(sciplot_core::tr!("曲线要用名称或从 1 开始的序号", "A curve is named or given by a 1-based index").into())
}

fn parse_positive(text: &str) -> Option<usize> {
    text.parse::<usize>().ok().filter(|index| *index > 0)
}

fn json_u64(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| value.as_i64().filter(|number| *number > 0).map(|number| number as u64))
}

fn parse_mode(text: &str) -> Result<ColumnMode, String> {
    let folded = text.trim().to_ascii_lowercase().replace(['_', '-', ' '], "");
    let mode = match folded.as_str() {
        "x" => ColumnMode::X,
        "y" => ColumnMode::Y,
        "z" => ColumnMode::Z,
        "xerr" | "xerror" => ColumnMode::XErr,
        "yerr" | "yerror" => ColumnMode::YErr,
        "label" => ColumnMode::Label,
        "none" => ColumnMode::None,
        _ => {
            return ColumnMode::all()
                .iter()
                .copied()
                .find(|mode| mode.label().eq_ignore_ascii_case(text.trim()))
                .ok_or_else(|| sciplot_core::trf!("不认识的列角色：{name}", "Unknown column role: {name}", name = text));
        }
    };
    Ok(mode)
}

fn parse_vector_mode(text: &str) -> Result<sciplot_core::VectorMode, String> {
    match text {
        "dxdy" | "dx_dy" | "dx" | "向量场" => Ok(sciplot_core::VectorMode::DxDy),
        "endpoint" | "end" | "xyxy" | "终点" => Ok(sciplot_core::VectorMode::EndPoint),
        "angle" | "xyam" | "angle_magnitude" | "角度" => Ok(sciplot_core::VectorMode::AngleMagnitude),
        other => Err(sciplot_core::trf!(
            "不认识的向量形式：{other}",
            "Unknown vector mode: {other}",
            other = other
        )),
    }
}

fn parse_kind(text: &str) -> Result<CurveKind, String> {
    let folded = text.trim().to_ascii_lowercase().replace([' ', '-'], "_");
    let kind = match folded.as_str() {
        "line" | "折线" => CurveKind::Line,
        "scatter" | "散点" => CurveKind::Scatter,
        "line_symbol" | "linesymbol" | "点线" => CurveKind::LineSymbol,
        "steps" | "阶梯" => CurveKind::Steps,
        "bars" | "bar" | "柱状" | "柱形" => CurveKind::Bars,
        "horizontal_bars" | "horizontal" | "条形" => CurveKind::HorizontalBars,
        "histogram" | "直方图" => CurveKind::Histogram,
        "area" | "面积" => CurveKind::Area,
        "error_bars" | "errorbars" | "误差棒" => CurveKind::ErrorBars,
        "box" | "boxplot" | "箱线" => CurveKind::BoxPlot,
        "pie" | "饼图" => CurveKind::Pie,
        "vector" | "向量场" => CurveKind::Vector,
        "drop_lines" | "droplines" | "垂直落线" | "落线" => CurveKind::DropLines,
        "spline" | "样条" => CurveKind::Spline,
        "horizontal_steps" | "水平阶梯" => CurveKind::HorizontalSteps,
        "stacked_histogram" | "stacked" | "堆积直方图" => CurveKind::StackedHistogram,
        "polar" | "极坐标" => CurveKind::Polar,
        "parametric" | "参数" => CurveKind::Parametric,
        "contour" | "等高线" => CurveKind::Contour,
        "heatmap" | "热力图" => CurveKind::Heatmap,
        "function" | "函数" => CurveKind::Function,
        _ => {
            return Err(sciplot_core::trf!("不认识的图形类型：{kind}", "Unknown plot kind: {kind}", kind = text));
        }
    };
    Ok(kind)
}

fn parse_scale(text: &str) -> Result<AxisScale, String> {
    match text.trim().to_ascii_lowercase().as_str() {
        "linear" | "线性" => Ok(AxisScale::Linear),
        "log" | "log10" | "对数" => Ok(AxisScale::Log10),
        "reciprocal" | "倒数" => Ok(AxisScale::Reciprocal),
        other => Err(sciplot_core::trf!("不认识的坐标刻度：{name}", "Unknown axis scale: {name}", name = other)),
    }
}

fn parse_legend(text: &str) -> Result<LegendPos, String> {
    match text.trim().to_ascii_lowercase().replace([' ', '-'], "_").as_str() {
        "top_right" => Ok(LegendPos::TopRight),
        "top_left" => Ok(LegendPos::TopLeft),
        "bottom_right" => Ok(LegendPos::BottomRight),
        "bottom_left" => Ok(LegendPos::BottomLeft),
        other => Err(sciplot_core::trf!("不认识的图例位置：{name}", "Unknown legend position: {name}", name = other)),
    }
}

fn plot_table_3d(project: &mut Project, prefs: &Prefs, args: &Map<String, Value>) -> Result<ToolOutput, String> {
    let table = find_table(project, req(args, "table")?)?;
    let column = {
        let table_ref = project.table(table).ok_or_else(|| sciplot_core::t("找不到表格").to_string())?;
        find_column(table_ref, req(args, "column")?)?
    };
    let kind = match args.get("kind").and_then(Value::as_str).unwrap_or("ribbon").trim().to_ascii_lowercase().as_str() {
        "ribbon" | "色带" | "三维色带" => TablePlot3D::Ribbon,
        "bars" | "bar" | "柱" | "三维柱" => TablePlot3D::Bars,
        "trajectory" | "line" | "轨迹" | "三维轨迹" => TablePlot3D::Trajectory,
        other => {
            return Err(sciplot_core::trf!("不认识的三维类型：{kind}", "Unknown 3D kind: {kind}", kind = other));
        }
    };
    let graph = project.plot_table_surface(table, column, kind)?;
    prefs.apply_new_graph(project, graph);
    let name = project.graph(graph).map(|graph| graph.name.clone()).unwrap_or_default();
    let mut output = ToolOutput::new(
        sciplot_core::trf!("已绘制{kind}「{name}」。", "Plotted {kind} \"{name}\".", kind = kind.label(), name = name),
        json!({"graph_id": graph.raw(), "name": name}),
    );
    output.graph = Some(graph);
    output.open_graph = true;
    Ok(output)
}

fn signal_columns(project: &mut Project, prefs: &Prefs, args: &Map<String, Value>) -> Result<ToolOutput, String> {
    let table = find_table(project, req(args, "table")?)?;
    let kind = args.get("kind").and_then(Value::as_str).unwrap_or("correlate");
    let column_a = {
        let table_ref = project.table(table).ok_or_else(|| sciplot_core::t("找不到表格").to_string())?;
        find_column(table_ref, req(args, "column_a")?)?
    };
    let column_b = match args.get("column_b") {
        Some(value) if !value.is_null() => {
            let table_ref = project.table(table).ok_or_else(|| sciplot_core::t("找不到表格").to_string())?;
            Some(find_column(table_ref, value)?)
        }
        _ => None,
    };
    let created = ops::signal_table(project, table, kind, column_a, column_b)?;
    let graph = project.plot_table(created, CurveKind::Line)?;
    prefs.apply_new_graph(project, graph);
    let name = project.table(created).map(|table| table.name.clone()).unwrap_or_default();
    let mut output = ToolOutput::new(
        sciplot_core::trf!("已计算「{name}」。", "Computed \"{name}\".", name = name),
        json!({"table_id": created.raw(), "graph_id": graph.raw(), "name": name}),
    );
    output.table = Some(created);
    output.graph = Some(graph);
    output.open_table = true;
    output.open_graph = true;
    Ok(output)
}

fn parse_fit_options(args: &Map<String, Value>) -> Result<FitOptions, String> {
    let weighting = match args.get("weight").and_then(Value::as_str).unwrap_or("none").trim().to_ascii_lowercase().as_str() {
        "none" | "" | "不加权" => Weighting::None,
        "instrumental" | "仪器" => Weighting::Instrumental,
        "statistical" | "统计" => Weighting::Statistical,
        "dataset" | "误差列" => Weighting::Dataset,
        "direct" | "直接" => Weighting::Direct,
        other => {
            return Err(sciplot_core::trf!("不认识的权重：{name}", "Unknown weighting: {name}", name = other));
        }
    };
    let algorithm = match args.get("algorithm").and_then(Value::as_str).unwrap_or("scaled_lm").trim().to_ascii_lowercase().as_str()
    {
        "scaled_lm" | "scaled" | "levenberg" => FitAlgorithm::ScaledLm,
        "unscaled_lm" | "unscaled" => FitAlgorithm::UnscaledLm,
        "nelder_mead" | "nelder" | "simplex" => FitAlgorithm::NelderMead,
        other => {
            return Err(sciplot_core::trf!("不认识的拟合算法：{name}", "Unknown fit algorithm: {name}", name = other));
        }
    };
    let tolerance = args.get("tolerance").and_then(Value::as_f64).unwrap_or(1e-12);
    let max_iterations = args.get("max_iterations").and_then(Value::as_u64).unwrap_or(80) as usize;
    Ok(FitOptions {
        weighting,
        algorithm,
        tolerance,
        max_iterations: max_iterations.max(1),
        ..FitOptions::default()
    })
}

fn number_list(args: &Map<String, Value>, key: &str) -> Result<Vec<f64>, String> {
    let Some(value) = args.get(key) else {
        return Err(sciplot_core::tr!("多峰拟合需要 centers", "A multi-peak fit needs centers").into());
    };
    let list = value.as_array().ok_or_else(|| sciplot_core::tr!("centers 必须是数组", "centers must be an array").to_string())?;
    let numbers: Vec<f64> = list.iter().filter_map(Value::as_f64).filter(|value| value.is_finite()).collect();
    if numbers.is_empty() {
        return Err(sciplot_core::tr!("centers 里没有有限数值", "centers has no finite numbers").into());
    }
    Ok(numbers)
}

fn parse_model(name: &str, args: &Map<String, Value>) -> Result<FitModel, String> {
    match name.trim().to_ascii_lowercase().as_str() {
        "linear" | "线性" => Ok(FitModel::Linear),
        "polynomial" | "poly" | "多项式" => {
            let degree = args.get("degree").and_then(Value::as_u64).unwrap_or(2) as usize;
            if !(1..=8).contains(&degree) {
                return Err(sciplot_core::tr!("多项式阶数要在 1 到 8 之间", "Polynomial degree must be from 1 to 8").into());
            }
            Ok(FitModel::Polynomial(degree))
        }
        "exponential" | "指数" => Ok(FitModel::Exponential),
        "exp_decay1" | "decay1" | "一阶指数衰减" => Ok(FitModel::ExpDecay1),
        "exp_decay2" | "decay2" | "二阶指数衰减" => Ok(FitModel::ExpDecay2),
        "exp_decay3" | "decay3" | "三阶指数衰减" => Ok(FitModel::ExpDecay3),
        "exp_growth" | "growth" | "指数增长" => Ok(FitModel::ExpGrowth),
        "boltzmann" | "玻尔兹曼" => Ok(FitModel::Boltzmann),
        "gaussian" | "高斯" => Ok(FitModel::Gaussian),
        "lorentzian" | "洛伦兹" => Ok(FitModel::Lorentzian),
        "multi_gaussian" | "multigaussian" | "多峰高斯" => Ok(FitModel::MultiPeak {
            gaussian: true,
            centers: number_list(args, "centers")?,
        }),
        "multi_lorentzian" | "multilorentzian" | "多峰洛伦兹" => Ok(FitModel::MultiPeak {
            gaussian: false,
            centers: number_list(args, "centers")?,
        }),
        "plugin" | "插件" => {
            let path = args
                .get("plugin")
                .and_then(Value::as_str)
                .ok_or_else(|| sciplot_core::tr!("插件拟合需要 plugin 路径", "A plugin fit needs a plugin path").to_string())?;
            let text = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
            let plugin = sciplot_analysis::parse_fit_plugin(&text)?;
            Ok(FitModel::Custom {
                expr: plugin.expr,
                initial: plugin.initial,
            })
        }
        "custom" | "自定义" => {
            let expr = args
                .get("expr")
                .and_then(Value::as_str)
                .ok_or_else(|| sciplot_core::tr!("自定义拟合需要 expr", "A custom fit needs expr").to_string())?;
            let initial = parse_initials(args.get("initial").and_then(Value::as_str).unwrap_or(""))?;
            Ok(FitModel::Custom { expr: expr.to_string(), initial })
        }
        other => Err(sciplot_core::trf!("不认识的拟合模型：{name}", "Unknown fit model: {name}", name = other)),
    }
}

fn mode_key(mode: ColumnMode) -> &'static str {
    match mode {
        ColumnMode::X => "x",
        ColumnMode::Y => "y",
        ColumnMode::Z => "z",
        ColumnMode::XErr => "xerr",
        ColumnMode::YErr => "yerr",
        ColumnMode::Label => "label",
        ColumnMode::None => "none",
    }
}

fn kind_key(kind: CurveKind) -> &'static str {
    match kind {
        CurveKind::Line => "line",
        CurveKind::Scatter => "scatter",
        CurveKind::LineSymbol => "line_symbol",
        CurveKind::Steps => "steps",
        CurveKind::Bars => "bars",
        CurveKind::HorizontalBars => "horizontal_bars",
        CurveKind::Histogram => "histogram",
        CurveKind::Area => "area",
        CurveKind::ErrorBars => "error_bars",
        CurveKind::BoxPlot => "box",
        CurveKind::Pie => "pie",
        CurveKind::Vector => "vector",
        CurveKind::DropLines => "drop_lines",
        CurveKind::Spline => "spline",
        CurveKind::HorizontalSteps => "horizontal_steps",
        CurveKind::StackedHistogram => "stacked_histogram",
        CurveKind::Polar => "polar",
        CurveKind::Contour => "contour",
        CurveKind::Heatmap => "heatmap",
        CurveKind::Function => "function",
        CurveKind::Parametric => "parametric",
    }
}

fn scale_key(scale: AxisScale) -> &'static str {
    match scale {
        AxisScale::Linear => "linear",
        AxisScale::Log10 => "log10",
        AxisScale::Reciprocal => "reciprocal",
    }
}

fn legend_key(position: LegendPos) -> &'static str {
    match position {
        LegendPos::TopRight => "top_right",
        LegendPos::TopLeft => "top_left",
        LegendPos::BottomRight => "bottom_right",
        LegendPos::BottomLeft => "bottom_left",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(project: &mut Project, name: &str, args: Value) -> ToolOutput {
        let prefs = Prefs::default();
        dispatch(project, None, &prefs, name, &args).unwrap_or_else(|err| panic!("{name}: {err}"))
    }

    #[test]
    fn create_plot_render_and_rename() {
        let mut project = Project::new("p");
        let created = run(
            &mut project,
            "create_table",
            json!({
                "name": "实验",
                "columns": [
                    {"name": "时间", "values": [1, 2, 3, 4, 5]},
                    {"name": "读数", "values": [2, 4, 6, 8, 10]}
                ]
            }),
        );
        assert!(created.open_table);
        let plotted = run(
            &mut project,
            "plot_columns",
            json!({"table": "实验", "x": "时间", "y": ["读数"], "kind": "line"}),
        );
        assert!(plotted.open_graph);
        let graph = plotted.graph.unwrap();
        let rendered = dispatch(
            &mut project,
            None,
            &Prefs::default(),
            "render_graph",
            &json!({"graph": graph.raw(), "format": "png", "width": 480, "height": 320}),
        )
        .unwrap();
        let png = rendered.png.expect("png");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        run(&mut project, "rename_column", json!({"table": "实验", "column": 1, "name": "t"}));
        assert_eq!(project.tables[0].columns[0].name, "t");
        let err = dispatch(
            &mut project,
            None,
            &Prefs::default(),
            "rename_column",
            &json!({"table": "实验", "column": "t", "name": "读数"}),
        )
        .unwrap_err();
        assert!(err.contains("同名"), "{err}");
        run(
            &mut project,
            "set_column_formula",
            json!({"table": "实验", "column": "读数", "expr": "col(\"t\")*3"}),
        );
        assert_eq!(project.tables[0].columns[1].numbers[0], Some(3.0));
    }
}
