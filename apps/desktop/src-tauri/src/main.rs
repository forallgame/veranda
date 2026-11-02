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
    padu_desktop_lib::run()
}
