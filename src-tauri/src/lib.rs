pub mod cli;
pub mod commands;
pub mod domain;
pub mod adapters;

use commands::{execute_fix, generate_bug_report, run_capture_test, run_diagnostics};

/// Launch the Tauri GUI window.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run_gui() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            run_diagnostics,
            execute_fix,
            run_capture_test,
            generate_bug_report,
        ])
        .run(tauri::generate_context!())
        .expect("error while running wayland-spectre");
}
