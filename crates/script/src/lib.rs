//! 程序内的 Python 解释器和 SciDAVis 风格对象接口。
//!
//! 窗口、命令行和 MCP 进程启动时调用 [`install`]，列公式里的 `python:` 才会执行。
//! 这不是 CPython。没有嵌入解释器的原因写在 `engine` 模块说明里。
//! 脚本可以使用字典、类、异常，以及 math、statistics、random、json。不认识的 import 会报错。

mod engine;

pub use engine::{execute, python_column, ScriptOutput};

pub fn install() {
    sciplot_core::set_python_column_hook(Some(python_column));
}

#[cfg(test)]
mod tests {
    use super::*;
    use sciplot_core::{apply_formula, ColumnMode, CurveKind, DataSource, Table, TableId};

    #[test]
    fn script_drives_tables_graphs_fit_convolution_layout_and_export() {
        install();
        let mut project = sciplot_core::Project::new("脚本");
        let out = execute(
            &mut project,
            r#"
import math
from sciplot import *
t = newTable("示例", 2, 4)
t.setColName(1, "x")
t.setColName(2, "y")
for i in range(1, 5):
    t.setCell(1, i, i)
    t.setCell(2, i, i * i)
g = plot(t, "line")
g.setTitle("平方")
print(g.name())
print(1 / 2)
print(1 // 2)
if 0:
    print(1 / 0)
else:
    print("ok")
def add(a, b):
    return a + b
print(add(2, 3))
n = 0
while n < 3:
    n = n + 1
print(n)
"#,
        )
        .unwrap();
        assert!(out.output.contains("平方"), "{}", out.output);
        assert!(out.output.contains("0.5"), "{}", out.output);
        assert!(out.output.contains("ok"), "{}", out.output);
        assert!(out.output.contains('\n'), "{}", out.output);
        let lines: Vec<_> = out.output.lines().collect();
        assert!(lines.contains(&"0"), "{lines:?}");
        assert!(lines.contains(&"5"), "{lines:?}");
        assert!(lines.contains(&"3"), "{lines:?}");
        let table = project.tables.iter().find(|table| table.name == "示例").unwrap();
        assert_eq!(table.columns[0].numbers[3], Some(4.0));
        assert_eq!(table.columns[1].numbers[3], Some(16.0));
        assert!(project.graphs.iter().any(|graph| graph.name == "平方"));

        let out = execute(
            &mut project,
            r#"
u = newTable("直线", 2, 3)
u.setCell(1, 1, 0)
u.setCell(2, 1, 1)
u.setCell(1, 2, 1)
u.setCell(2, 2, 3)
u.setCell(1, 3, 2)
u.setCell(2, 3, 5)
h = plot(u, "scatter")
f = Fit(h, "linear")
f.fit()
print(f.r2())
"#,
        )
        .unwrap();
        let r2: f64 = out.output.trim().parse().expect(&out.output);
        assert!(r2 > 0.99, "{}", out.output);
        assert!(project.graphs.iter().any(|graph| {
            graph.layers.iter().any(|layer| {
                layer.curves.iter().any(|curve| {
                    matches!(curve.kind, CurveKind::Function)
                        && matches!(curve.source, DataSource::Function { .. })
                })
            })
        }));

        let out = execute(&mut project, "print(convolve([1, 2], [3, 4]))\n").unwrap();
        assert_eq!(out.output.trim(), "[3, 10, 8]");
        execute(&mut project, "print(len(deconvolve(convolve([1, 2], [3, 4]), [3, 4])))\n").unwrap();
        execute(&mut project, "print(len(correlate([1, 2, 3], [1, 2, 3])))\n").unwrap();

        execute(&mut project, "graph(\"平方\").arrangeLayers(2, 2)\n").unwrap();
        assert_eq!(project.graphs.iter().find(|graph| graph.name == "平方").unwrap().layers.len(), 4);

        let dir = std::env::temp_dir().join(format!("sciplot-script-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let svg = dir.join("out.svg");
        let pdf = dir.join("out.pdf");
        let source = format!(
            "graph(\"平方\").exportImage(\"{}\")\nprintPdf(graph(\"平方\"), \"{}\")\n",
            svg.display(),
            pdf.display()
        );
        execute(&mut project, &source).unwrap();
        let svg_text = std::fs::read_to_string(&svg).unwrap();
        assert!(svg_text.contains("<svg"), "{svg_text}");
        let pdf_bytes = std::fs::read(&pdf).unwrap();
        assert!(pdf_bytes.starts_with(b"%PDF"));
        std::fs::remove_dir_all(&dir).ok();

        let mut column_table = Table::new(TableId(1), "列");
        column_table.columns.clear();
        column_table.add_numeric_column("A", ColumnMode::X, &[1.0, 2.0, 3.0]);
        column_table.add_column("B", ColumnMode::Y);
        apply_formula(&mut column_table, 1, "python:\ncol(\"A\") * 2\n").unwrap();
        assert_eq!(column_table.columns[1].numbers, vec![Some(2.0), Some(4.0), Some(6.0)]);

        let folder = execute(&mut project, "folder = newFolder(\"记录\")\nprint(folder.name())\n").unwrap();
        assert!(folder.output.contains("记录"));
        assert!(project.folders.iter().any(|item| item.name == "记录"));
    }

    #[test]
    fn python_has_dicts_classes_exceptions_imports_and_object_api() {
        install();
        let mut project = sciplot_core::Project::new("api");
        let out = execute(
            &mut project,
            r#"
import math
from statistics import mean
import json
d = {"a": 1, "b": 2}
d["c"] = d["a"] + 3
print(d["c"])
print(len(d))
print(mean([1, 2, 3]))
print(math.sqrt(9))
print(json.loads(json.dumps({"ok": 1}))["ok"])
class Point:
    def __init__(self, x):
        self.x = x
    def doubled(self):
        self.x = self.x * 2
        return self.x
p = Point(4)
print(p.doubled())
print(p.x)
try:
    raise "boom"
except as err:
    print(err)
t = newTable("测量", 2, 3)
t.setColName(1, "x")
t.setCell(1, 1, 1)
t.setCell(1, 2, 2)
t.setCell(1, 3, 3)
t.setFormula(2, "col(\"x\")*10")
print(t.colName(1))
print(t.cell(2, 2))
print(t.nr())
g = plot(t, "line")
g.setXTitle("时间")
g.setYTitle("读数")
print(g.layerCount())
print(len(tableList()))
"#,
        )
        .unwrap();
        let lines: Vec<&str> = out.output.lines().collect();
        assert_eq!(lines[0], "4");
        assert_eq!(lines[1], "3");
        assert_eq!(lines[2], "2");
        assert_eq!(lines[3], "3");
        assert_eq!(lines[4], "1");
        assert_eq!(lines[5], "8");
        assert_eq!(lines[6], "8");
        assert_eq!(lines[7], "boom");
        assert_eq!(lines[8], "x");
        assert_eq!(lines[9], "20");
        assert_eq!(lines[10], "3");
        assert_eq!(lines[11], "1");
        assert_eq!(lines[12], "1");
        let graph = project.graphs.iter().find(|graph| graph.layers.iter().any(|layer| layer.x_axis.label == "时间")).unwrap();
        assert_eq!(graph.layers[0].y_axis.label, "读数");
        assert!(execute(&mut project, "import os\n").is_err());
        let dir = std::env::temp_dir().join(format!("sciplot-script-api-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let csv = dir.join("t.csv");
        let project_path = dir.join("p.splot");
        let source = format!(
            "table(\"测量\").exportASCII(\"{}\")\nsaveProject(\"{}\")\n",
            csv.display(),
            project_path.display()
        );
        execute(&mut project, &source).unwrap();
        assert!(std::fs::read_to_string(&csv).unwrap().contains("x"));
        assert!(std::fs::read_to_string(&project_path).unwrap().contains("测量"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
