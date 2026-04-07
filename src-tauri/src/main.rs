// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use wayland_spectre_lib::{cli, run_gui};

fn main() {
    // If any CLI flags are present (--json-only, --check, --help, etc.)
    // we run in headless CLI mode. Otherwise we launch the Tauri GUI.
    //
    // We detect CLI mode by checking if any arg beyond the binary name exists
    // and if it looks like a flag rather than a Tauri internal arg.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let is_cli = args.iter().any(|a| a.starts_with('-') || a == "check" || a == "report");

    if is_cli {
        // Block on the async CLI runner
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let code = rt.block_on(cli::run());
        std::process::exit(code);
    } else {
        run_gui();
    }
}
