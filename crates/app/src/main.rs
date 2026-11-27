//! SciPlot 主程序。
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod api;
mod app;
mod context;
mod dialogs;
mod fonts;
mod i18n;
mod ops;
mod prefs;
mod update;

use std::path::Path;

fn main() {
    sciplot_core::set_language(prefs::load().language);
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--export-demo") {
        let dir = args.get(1).map(Path::new).unwrap_or_else(|| Path::new("."));
        if let Err(err) = export_demo(dir) {
            eprintln!("{err}");
            std::process::exit(1);
        }
        return;
    }
    if args.first().map(String::as_str) == Some("--mcp") {
        let project = mcp_project_arg(&args[1..]);
        if let Err(err) = api::serve(project.as_deref()) {
            eprintln!("{err}");
            std::process::exit(1);
        }
        return;
    }
    if let Err(err) = app::run() {
        eprintln!("{}", sciplot_core::trf!("SciPlot 启动失败：{err}", "SciPlot failed to start: {err}", err = err));
        std::process::exit(1);
    }
}

fn mcp_project_arg(args: &[String]) -> Option<String> {
    let mut project = None;
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--project" {
            project = args.get(index + 1).cloned();
            index += 2;
            continue;
        }
        eprintln!(
            "{}",
            sciplot_core::trf!(
                "不认识的参数 {}。用法：sciplot --mcp [--project 文件.splot]",
                "Unknown argument {}. Usage: sciplot --mcp [--project file.splot]",
                args[index]
            )
        );
        index += 1;
    }
    project
}

fn export_demo(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    let mut project = sciplot_core::demo_project();
    ops::enrich_demo(&mut project);
    let loaded = fonts::load(None);
    let font = fonts::plot_glyphs(&loaded);
    for graph in &project.graphs {
        let scene = sciplot_plot::render(&project, graph, &[], 1100.0, 720.0, 1.0);
        let stem = graph.name.replace(['/', '\\', ' '], "_");
        let svg = sciplot_plot::to_svg(&scene);
        std::fs::write(dir.join(format!("{stem}.svg")), svg).map_err(|err| err.to_string())?;
        let png = sciplot_plot::to_png(&scene, font.as_ref()).map_err(|err| err.to_string())?;
        std::fs::write(dir.join(format!("{stem}.png")), png).map_err(|err| err.to_string())?;
        println!("{}", sciplot_core::trf!("已导出 {stem}", "Exported {stem}", stem = stem));
    }
    Ok(())
}
