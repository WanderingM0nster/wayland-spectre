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
        // WebKitGTK (the Tauri webview) tries to use the DMABuf renderer by default.
        // On NVIDIA + Wayland this causes a fatal "Error 71 (Protocol error)" crash
        // before the window opens. Detect NVIDIA and disable the DMABuf renderer
        // proactively so the app works out-of-the-box for NVIDIA users.
        //
        // This is a WebKitGTK bug, not a Wayland or NVIDIA bug — the workaround is
        // standard for any GTK app on NVIDIA/Wayland. Upstream: webkit bug #247452.
        if nvidia_present() {
            // Only set if not already overridden by the user's environment
            if std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
                std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
            }
        }
        run_gui();
    }
}

/// Returns true if the NVIDIA driver is loaded.
/// Fast path: just check for /proc/driver/nvidia/version — no subprocess needed.
fn nvidia_present() -> bool {
    std::path::Path::new("/proc/driver/nvidia/version").exists()
}
