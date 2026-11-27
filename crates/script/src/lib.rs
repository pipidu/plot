//! 程序内的 Python 子集和 SciDAVis 风格对象接口。
//!
//! 窗口、命令行和 MCP 进程启动时调用 [`install`]，列公式里的 `python:` 才会执行。
//! 这不是 CPython：没有类、字典、异常对象和任意 import。

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
}
