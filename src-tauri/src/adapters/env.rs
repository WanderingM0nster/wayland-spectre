//! Environment adapter.
//!
//! Checks: F3 (WAYLAND_DISPLAY / XDG_CURRENT_DESKTOP missing from systemd env).
//! Layer: L6 (Environment variables).
//!
//! KWin sets these in the systemd user environment on login.
//! If missing, portal backends cannot connect to the Wayland compositor.

use crate::domain::types::{Confidence, DiagnosticResult, Layer};
use std::process::Command;

pub async fn check_environment() -> Vec<DiagnosticResult> {
    let mut results = Vec::new();

    // Get both the process environment AND the systemd user environment.
    // They can differ — portal services inherit systemd's env, not the shell's.
    let systemd_env = get_systemd_env();

    results.extend(check_wayland_display(&systemd_env));
    results.extend(check_xdg_desktop(&systemd_env));
    results.extend(check_dbus_session(&systemd_env));
    results.extend(check_xdg_runtime_dir(&systemd_env));

    results
}

fn check_wayland_display(systemd_env: &[String]) -> Vec<DiagnosticResult> {
    let in_process = std::env::var("WAYLAND_DISPLAY").is_ok();
    let in_systemd = systemd_env.iter().any(|e| e.starts_with("WAYLAND_DISPLAY="));

    match (in_process, in_systemd) {
        (true, true) => vec![DiagnosticResult::pass(
            Layer::L6,
            "WAYLAND_DISPLAY",
            "WAYLAND_DISPLAY set in both process env and systemd user env",
        )],
        (true, false) => vec![DiagnosticResult::warn(
            Layer::L6,
            "WAYLAND_DISPLAY",
            "WAYLAND_DISPLAY set in shell but NOT in systemd user environment. \
             Portal services launched by systemd won't see the Wayland socket.",
            Some(
                "systemctl --user import-environment WAYLAND_DISPLAY XDG_CURRENT_DESKTOP"
                    .into(),
            ),
            Confidence::High,
        )],
        (false, true) => vec![DiagnosticResult::pass(
            Layer::L6,
            "WAYLAND_DISPLAY",
            "WAYLAND_DISPLAY set in systemd user env (tool running without X/Wayland context)",
        )],
        (false, false) => vec![DiagnosticResult::fail(
            Layer::L6,
            "WAYLAND_DISPLAY",
            "WAYLAND_DISPLAY not set anywhere — not running in a Wayland session?",
            "systemctl --user import-environment WAYLAND_DISPLAY",
            Confidence::Medium,
        )],
    }
}

fn check_xdg_desktop(systemd_env: &[String]) -> Vec<DiagnosticResult> {
    let in_process = std::env::var("XDG_CURRENT_DESKTOP")
        .map(|v| v.to_lowercase().contains("kde"))
        .unwrap_or(false);
    let in_systemd = systemd_env
        .iter()
        .any(|e| e.starts_with("XDG_CURRENT_DESKTOP=") && e.to_lowercase().contains("kde"));

    match (in_process, in_systemd) {
        (true, true) => vec![DiagnosticResult::pass(
            Layer::L6,
            "XDG_CURRENT_DESKTOP",
            "XDG_CURRENT_DESKTOP=KDE in both process env and systemd user env",
        )],
        (_, false) => vec![DiagnosticResult::warn(
            Layer::L6,
            "XDG_CURRENT_DESKTOP",
            "XDG_CURRENT_DESKTOP not set to KDE in systemd user environment. \
             Portal services may choose wrong backend.",
            Some(
                "systemctl --user import-environment XDG_CURRENT_DESKTOP".into(),
            ),
            Confidence::High,
        )],
        (false, true) => vec![DiagnosticResult::pass(
            Layer::L6,
            "XDG_CURRENT_DESKTOP",
            "XDG_CURRENT_DESKTOP=KDE in systemd env",
        )],
    }
}

fn check_dbus_session(systemd_env: &[String]) -> Vec<DiagnosticResult> {
    let in_systemd = systemd_env
        .iter()
        .any(|e| e.starts_with("DBUS_SESSION_BUS_ADDRESS="));

    if in_systemd {
        vec![DiagnosticResult::pass(
            Layer::L6,
            "DBUS_SESSION_BUS_ADDRESS",
            "DBUS_SESSION_BUS_ADDRESS present in systemd user env",
        )]
    } else {
        vec![DiagnosticResult::warn(
            Layer::L6,
            "DBUS_SESSION_BUS_ADDRESS",
            "DBUS_SESSION_BUS_ADDRESS missing from systemd user env — \
             portal D-Bus calls may fail",
            Some("systemctl --user import-environment DBUS_SESSION_BUS_ADDRESS".into()),
            Confidence::Medium,
        )]
    }
}

fn check_xdg_runtime_dir(systemd_env: &[String]) -> Vec<DiagnosticResult> {
    let in_systemd = systemd_env
        .iter()
        .any(|e| e.starts_with("XDG_RUNTIME_DIR="));

    if in_systemd {
        vec![DiagnosticResult::pass(
            Layer::L6,
            "XDG_RUNTIME_DIR",
            "XDG_RUNTIME_DIR present in systemd user env",
        )]
    } else {
        vec![DiagnosticResult::warn(
            Layer::L6,
            "XDG_RUNTIME_DIR",
            "XDG_RUNTIME_DIR missing from systemd user env",
            Some("systemctl --user import-environment XDG_RUNTIME_DIR".into()),
            Confidence::Medium,
        )]
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

/// Returns the systemd user environment as a list of "KEY=value" strings.
fn get_systemd_env() -> Vec<String> {
    Command::new("systemctl")
        .args(["--user", "show-environment"])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|l| l.to_string())
                .collect()
        })
        .unwrap_or_default()
}
