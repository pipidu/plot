//! SciPlot 主程序。
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod dialogs;
mod fonts;
mod i18n;
mod ops;

use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--export-demo") {
        let dir = args.get(1).map(Path::new).unwrap_or_else(|| Path::new("."));
        if let Err(err) = export_demo(dir) {
            eprintln!("{err}");
            std::process::exit(1);
        }
        return;
    }
    if let Err(err) = app::run() {
        eprintln!("SciPlot 启动失败：{err}");
        std::process::exit(1);
    }
}

fn export_demo(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    let mut project = sciplot_core::demo_project();
    ops::enrich_demo(&mut project);
    let font_bytes = fonts::load_bytes();
    let font = sciplot_plot::GlyphFont::from_bytes(&font_bytes);
    for graph in &project.graphs {
        let scene = sciplot_plot::render(&project, graph, &[], 1100.0, 720.0, 1.0);
        let stem = graph.name.replace(['/', '\\', ' '], "_");
        let svg = sciplot_plot::to_svg(&scene);
        std::fs::write(dir.join(format!("{stem}.svg")), svg).map_err(|err| err.to_string())?;
        let png = sciplot_plot::to_png(&scene, font.as_ref()).map_err(|err| err.to_string())?;
        std::fs::write(dir.join(format!("{stem}.png")), png).map_err(|err| err.to_string())?;
        println!("已导出 {stem}");
    }
    Ok(())
}
