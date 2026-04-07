//! Flatpak adapter.
//!
//! Checks: F4 (stale "deny" in Flatpak permission store).
//! Layer: L5 (Flatpak permissions).
//!
//! The permission store lives at ~/.local/share/xdg-permission-store/
//! Stale "deny" entries block screen sharing silently.

use crate::domain::types::{Confidence, DiagnosticResult, Layer};
use std::path::PathBuf;

pub async fn check_flatpak_permissions() -> Vec<DiagnosticResult> {
    let mut results = Vec::new();

    results.extend(check_permission_store());
    results.extend(check_screencast_denies());

    results
}

// ── Permission store ──────────────────────────────────────────────────────

fn check_permission_store() -> Vec<DiagnosticResult> {
    let store_path = permission_store_path();

    if !store_path.exists() {
        return vec![DiagnosticResult::pass(
            Layer::L5,
            "flatpak_permission_store",
            "No Flatpak permission store found — no stale denies possible",
        )];
    }

    vec![DiagnosticResult::pass(
        Layer::L5,
        "flatpak_permission_store",
        format!("Permission store found at {}", store_path.display()),
    )]
}

fn check_screencast_denies() -> Vec<DiagnosticResult> {
    // The screencast table lives in the permission store database.
    // We check via flatpak permission-show if available.
    use std::process::Command;

    let out = Command::new("flatpak")
        .args(["permission-show", "screencast"])
        .output();

    match out {
        Err(_) => {
            // flatpak not installed — check doesn't apply
            vec![DiagnosticResult::skip(
                Layer::L5,
                "flatpak_screencast_denies",
                "flatpak not installed — permission store check skipped",
            )]
        }
        Ok(o) => {
            let output = String::from_utf8_lossy(&o.stdout).to_string();
            let stderr = String::from_utf8_lossy(&o.stderr).to_string();

            // No output / "No permissions" = clean
            if !o.status.success() || output.trim().is_empty() || stderr.contains("No permissions") {
                return vec![DiagnosticResult::pass(
                    Layer::L5,
                    "flatpak_screencast_denies",
                    "No Flatpak screencast permissions in store (clean state)",
                )];
            }

            // Parse deny entries: lines containing "no" or "deny"
            let deny_lines: Vec<&str> = output
                .lines()
                .filter(|l| {
                    let lower = l.to_lowercase();
                    lower.contains("no") || lower.contains("deny")
                })
                .collect();

            if deny_lines.is_empty() {
                vec![DiagnosticResult::pass(
                    Layer::L5,
                    "flatpak_screencast_denies",
                    "No stale deny entries in Flatpak screencast permission store",
                )]
            } else {
                let apps: Vec<&str> = deny_lines
                    .iter()
                    .filter_map(|l| l.split_whitespace().next())
                    .collect();
                let app_list = apps.join(", ");

                vec![DiagnosticResult::warn(
                    Layer::L5,
                    "flatpak_screencast_denies",
                    format!(
                        "Stale screencast deny entries found for: {app_list}. \
                         These will silently reject screen share requests."
                    ),
                    Some(format!(
                        "flatpak permission-reset screencast{}",
                        if apps.is_empty() { String::new() } else { format!(" {}", apps[0]) }
                    )),
                    Confidence::High,
                )]
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn permission_store_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    PathBuf::from(home)
        .join(".local/share/xdg-permission-store")
}
