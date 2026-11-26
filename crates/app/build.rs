fn main() {
    println!("cargo:rerun-if-env-changed=SCIPLOT_BUILD");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        let mut res = winres::WindowsResource::new();
        if std::env::var("HOST").ok().is_some_and(|host| !host.contains("windows")) {
            res.set_windres_path("x86_64-w64-mingw32-windres");
            res.set_ar_path("x86_64-w64-mingw32-ar");
        }
        res.set_icon("assets/icon.ico");
        let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".into());
        let file_version = if version.matches('.').count() >= 3 { version.clone() } else { format!("{version}.0") };
        res.set("ProductName", "SciPlot");
        res.set("FileDescription", "SciPlot scientific plotting");
        res.set("LegalCopyright", "MIT OR Apache-2.0");
        res.set("ProductVersion", &version);
        res.set("FileVersion", &file_version);
        if let Err(err) = res.compile() {
            println!("cargo:warning=failed to embed Windows icon: {err}");
        } else if let Ok(out) = std::env::var("OUT_DIR") {
            // windres 的 .o 没有被引用的符号，静态库不会被链接器拉进来。
            println!("cargo:rustc-link-arg={out}/resource.o");
        }
    }
}
