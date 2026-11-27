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
    match parse_args(&args) {
        Command::ExportDemo(dir) => {
            if let Err(err) = export_demo(&dir) {
                eprintln!("{err}");
                std::process::exit(1);
            }
        }
        Command::Mcp { project } => {
            if let Err(err) = api::serve(project.as_deref()) {
                eprintln!("{err}");
                std::process::exit(1);
            }
        }
        Command::Gui { project } => {
            if let Err(err) = app::run(project) {
                eprintln!(
                    "{}",
                    sciplot_core::trf!("SciPlot 启动失败：{err}", "SciPlot failed to start: {err}", err = err)
                );
                std::process::exit(1);
            }
        }
        Command::Help => println!("{}", usage()),
        Command::Error(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Command {
    ExportDemo(std::path::PathBuf),
    Mcp { project: Option<String> },
    Gui { project: Option<std::path::PathBuf> },
    Help,
    Error(String),
}

fn usage() -> String {
    sciplot_core::translate(
        "用法：\n  sciplot [文件.splot]\n  sciplot --export-demo [目录]\n  sciplot --mcp [--project 文件.splot]\n  sciplot --help\n",
        "Usage:\n  sciplot [file.splot]\n  sciplot --export-demo [directory]\n  sciplot --mcp [--project file.splot]\n  sciplot --help\n",
    )
    .to_string()
}

fn parse_args(args: &[String]) -> Command {
    if args.is_empty() {
        return Command::Gui { project: None };
    }
    match args[0].as_str() {
        "--help" | "-h" => Command::Help,
        "--export-demo" => {
            if args.len() > 2 {
                return Command::Error(format!("{}\n{}", sciplot_core::t("参数太多"), usage()));
            }
            let dir = args.get(1).map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from("."));
            Command::ExportDemo(dir)
        }
        "--mcp" => Command::Mcp {
            project: mcp_project_arg(&args[1..]),
        },
        other if other.starts_with('-') => Command::Error(format!(
            "{}\n{}",
            sciplot_core::trf!("不认识的参数 {}", "Unknown argument {}", other),
            usage()
        )),
        _ => {
            if args.len() != 1 {
                return Command::Error(format!("{}\n{}", sciplot_core::t("参数太多"), usage()));
            }
            Command::Gui {
                project: Some(std::path::PathBuf::from(&args[0])),
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn command_line_opens_a_project_file() {
        assert_eq!(parse_args(&[]), Command::Gui { project: None });
        assert_eq!(
            parse_args(&args(&["实验.splot"])),
            Command::Gui {
                project: Some(std::path::PathBuf::from("实验.splot"))
            }
        );
        assert!(matches!(parse_args(&args(&["实验.splot", "多余"])), Command::Error(_)));
        assert!(matches!(parse_args(&args(&["--help"])), Command::Help));
        assert_eq!(
            parse_args(&args(&["--export-demo", "out"])),
            Command::ExportDemo(std::path::PathBuf::from("out"))
        );
        assert_eq!(
            parse_args(&args(&["--mcp", "--project", "实验.splot"])),
            Command::Mcp {
                project: Some("实验.splot".into())
            }
        );
        assert!(matches!(parse_args(&args(&["--nope"])), Command::Error(_)));
    }
}
