// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Elevated helper mode: `veranda-desktop --hosts-apply <payload>` writes the hosts file
    // and exits without touching the GUI.
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 3 && args[1] == "--hosts-apply" {
        let hosts_file = veranda_core::Platform::current().hosts_file();
        let payload = std::path::PathBuf::from(&args[2]);
        let code = match veranda_core::hosts::apply_payload(&hosts_file, &payload) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("{e:#}");
                1
            }
        };
        let _ = std::fs::remove_file(&payload);
        std::process::exit(code);
    }
    // WebKitGTK's DMA-BUF (GPU) renderer is on by default; `general.hardware_acceleration = false`
    // opts out for drivers that flicker. An explicit env var always wins.
    // Rasterise page content on the CPU but keep GPU compositing: scrolling stays at 60 fps
    // (measured) while the GPU-accelerated Skia raster path, which crashes WebKitGTK's web process
    // on some Intel/Mesa drivers, is not used. An explicit env var always wins.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_SKIA_ENABLE_CPU_RENDERING").is_none() {
        // SAFETY: called before any other thread exists (start of main).
        unsafe { std::env::set_var("WEBKIT_SKIA_ENABLE_CPU_RENDERING", "1") };
    }
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        let gpu = veranda_core::Veranda::open()
            .map(|p| p.config.general.hardware_acceleration)
            .unwrap_or(true);
        if !gpu {
            // SAFETY: called before any other thread exists (start of main).
            unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
        }
    }
    veranda_desktop_lib::run()
}
