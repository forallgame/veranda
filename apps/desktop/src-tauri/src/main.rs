// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Elevated helper mode: `padu-desktop --hosts-apply <payload>` writes the hosts file
    // and exits without touching the GUI.
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 3 && args[1] == "--hosts-apply" {
        let hosts_file = padu_core::Platform::current().hosts_file();
        let payload = std::path::PathBuf::from(&args[2]);
        let code = match padu_core::hosts::apply_payload(&hosts_file, &payload) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("{e:#}");
                1
            }
        };
        let _ = std::fs::remove_file(&payload);
        std::process::exit(code);
    }
    // WebKitGTK's DMA-BUF renderer causes stuttering scroll and flicker on many Linux GPUs
    // (especially on Wayland). Opting out is the usual fix for Tauri apps; users can override.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        // SAFETY: called before any other thread exists (start of main).
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }
    padu_desktop_lib::run()
}
