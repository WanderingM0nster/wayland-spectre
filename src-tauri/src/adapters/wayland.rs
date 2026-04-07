//! Wayland protocol adapter.
//!
//! Checks: F1 (startup race / zkde_screencast missing), F2 (wrong backend).
//! Layer: L3 (compositor protocols) and L2 (portal backend).
//!
//! Current implementation: subprocess via `wayland-info`.
//! TODO Session 2: replace with native wayland-client crate bindings —
//!   connect to $WAYLAND_DISPLAY, enumerate wl_registry globals directly.

use crate::domain::types::{Confidence, DiagnosticResult, Layer};
use std::process::Command;

// ── Protocol table ────────────────────────────────────────────────────────

/// Protocol, criticality (true = FAIL if absent, false = WARN), fix command.
const PROTOCOLS: &[(&str, bool, &str)] = &[
    (
        // CRITICAL — screen sharing cannot work without this.
        // Absent when Bug C prevents KWin from registering it on boot.
        "zkde_screencast_unstable_v1",
        true,
        "systemctl --user restart plasma-xdg-desktop-portal-kde xdg-desktop-portal",
    ),
    (
        // INFORMATIONAL — DRM explicit sync. Present on this system (confirmed in live run).
        // No user-space fix if absent — kernel/driver issue.
        "wp_linux_drm_syncobj_manager_v1",
        true,
        "",
    ),
    (
        // WARN, not FAIL — newer protocol (wayland-protocols 1.32+), not universally present.
        // KDE Plasma 6.3+ exposes it but it's not required for basic screen sharing.
        // Absent here due to Bug C, same root cause as zkde_screencast.
        "ext_image_capture_source_v1",
        false, // false = WARN if absent, not FAIL
        "systemctl --user restart plasma-xdg-desktop-portal-kde xdg-desktop-portal",
    ),
];

pub async fn check_wayland_protocols() -> Vec<DiagnosticResult> {
    let mut results = Vec::new();

    results.extend(check_screencast_backend());

    let wayland_globals = run_wayland_info();
    match wayland_globals {
        Err(e) => {
            results.push(DiagnosticResult::warn(
                Layer::L3,
                "wayland_info_available",
                format!(
                    "wayland-info not found or failed: {e}. \
                     Install with: sudo dnf install wayland-utils"
                ),
                None,
                Confidence::Medium,
            ));
        }
        Ok(globals) => {
            results.extend(check_protocols(&globals));
        }
    }

    results
}

/// Pure function: given a list of wayland-info output lines, produce
/// DiagnosticResults for each required protocol.
/// Extracted for unit testing without subprocess dependency.
pub(crate) fn check_protocols(globals: &[String]) -> Vec<DiagnosticResult> {
    let mut results = Vec::new();

    for (proto, is_critical, fix) in PROTOCOLS {
        let present = globals.iter().any(|g| g.contains(proto));

        if present {
            results.push(DiagnosticResult::pass(
                Layer::L3,
                *proto,
                format!("{proto} present in Wayland registry"),
            ));
        } else if *is_critical {
            // FAIL — screen sharing definitely broken without this
            let fix_opt = if fix.is_empty() { None } else { Some(fix.to_string()) };
            results.push(DiagnosticResult {
                layer: Layer::L3,
                check: proto.to_string(),
                status: crate::domain::types::CheckStatus::Fail,
                detail: format!(
                    "{proto} absent from Wayland registry — screen sharing will fail"
                ),
                fix: fix_opt,
                confidence: Confidence::High,
            });
        } else {
            // WARN — degraded but not necessarily broken
            let fix_opt = if fix.is_empty() { None } else { Some(fix.to_string()) };
            results.push(DiagnosticResult::warn(
                Layer::L3,
                *proto,
                format!(
                    "{proto} absent — newer protocol, not required for basic screen sharing \
                     but indicates portal registration issues (same root cause as zkde_screencast)"
                ),
                fix_opt,
                Confidence::Medium,
            ));
        }
    }

    results
}

// ── F2: Portal backend conflict ───────────────────────────────────────────

fn check_screencast_backend() -> Vec<DiagnosticResult> {
    let mut results = Vec::new();

    let out = Command::new("busctl")
        .args([
            "--user",
            "introspect",
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.ScreenCast",
        ])
        .output();

    match out {
        Err(e) => {
            results.push(DiagnosticResult::skip(
                Layer::L2,
                "screencast_backend",
                format!("busctl not available: {e}"),
            ));
        }
        Ok(o) if !o.status.success() => {
            results.push(DiagnosticResult::fail(
                Layer::L2,
                "screencast_backend",
                "ScreenCast interface not introspectable — portal may not be running",
                "systemctl --user restart xdg-desktop-portal xdg-desktop-portal-kde",
                Confidence::High,
            ));
        }
        Ok(o) => {
            let output = String::from_utf8_lossy(&o.stdout);
            if output.contains("CreateSession") {
                results.push(DiagnosticResult::pass(
                    Layer::L2,
                    "screencast_backend",
                    "ScreenCast interface introspectable on session bus",
                ));
            } else {
                results.push(DiagnosticResult::fail(
                    Layer::L2,
                    "screencast_backend",
                    "ScreenCast interface present but incomplete — wrong backend may have won",
                    "systemctl --user restart plasma-xdg-desktop-portal-kde xdg-desktop-portal",
                    Confidence::Medium,
                ));
            }
        }
    }

    let gtk_portal = Command::new("systemctl")
        .args(["--user", "is-active", "xdg-desktop-portal-gtk"])
        .output();

    if let Ok(o) = gtk_portal {
        let active = String::from_utf8_lossy(&o.stdout).trim() == "active";
        if active {
            results.push(DiagnosticResult::warn(
                Layer::L2,
                "gtk_portal_conflict",
                "xdg-desktop-portal-gtk is active on a KDE session — may steal ScreenCast interface",
                Some("systemctl --user stop xdg-desktop-portal-gtk".to_string()),
                Confidence::Medium,
            ));
        } else {
            results.push(DiagnosticResult::pass(
                Layer::L2,
                "gtk_portal_conflict",
                "xdg-desktop-portal-gtk not active — no backend conflict",
            ));
        }
    }

    results
}

// ── Subprocess helper ─────────────────────────────────────────────────────

fn run_wayland_info() -> Result<Vec<String>, String> {
    let out = Command::new("wayland-info")
        .output()
        .map_err(|e| e.to_string())?;

    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }

    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.to_string())
        .collect())
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::CheckStatus;

    fn globals_with(protocols: &[&str]) -> Vec<String> {
        protocols.iter().map(|p| format!("  interface: {p}  version: 1")).collect()
    }

    #[test]
    fn all_protocols_present_all_pass() {
        let globals = globals_with(&[
            "zkde_screencast_unstable_v1",
            "wp_linux_drm_syncobj_manager_v1",
            "ext_image_capture_source_v1",
        ]);
        let results = check_protocols(&globals);
        assert!(
            results.iter().all(|r| r.status == CheckStatus::Pass),
            "all present → all PASS: {results:?}"
        );
    }

    #[test]
    fn missing_zkde_is_fail() {
        // zkde_screencast absent → FAIL (critical protocol)
        let globals = globals_with(&[
            "wp_linux_drm_syncobj_manager_v1",
            "ext_image_capture_source_v1",
        ]);
        let results = check_protocols(&globals);
        let zkde = results.iter().find(|r| r.check == "zkde_screencast_unstable_v1");
        assert!(zkde.is_some(), "zkde result missing entirely");
        assert_eq!(zkde.unwrap().status, CheckStatus::Fail, "missing zkde should be FAIL");
    }

    #[test]
    fn missing_ext_image_is_warn_not_fail() {
        // ext_image_capture_source_v1 absent → WARN (newer optional protocol)
        let globals = globals_with(&[
            "zkde_screencast_unstable_v1",
            "wp_linux_drm_syncobj_manager_v1",
        ]);
        let results = check_protocols(&globals);
        let ext = results.iter().find(|r| r.check == "ext_image_capture_source_v1");
        assert!(ext.is_some(), "ext_image result missing entirely");
        assert_eq!(ext.unwrap().status, CheckStatus::Warn,
            "missing ext_image should be WARN, not FAIL");
    }

    #[test]
    fn missing_syncobj_is_fail_no_fix() {
        // wp_linux_drm_syncobj missing → FAIL but no fix command (driver issue)
        let globals = globals_with(&[
            "zkde_screencast_unstable_v1",
            "ext_image_capture_source_v1",
        ]);
        let results = check_protocols(&globals);
        let sync = results.iter().find(|r| r.check == "wp_linux_drm_syncobj_manager_v1");
        assert!(sync.is_some());
        let sync = sync.unwrap();
        assert_eq!(sync.status, CheckStatus::Fail);
        assert!(sync.fix.is_none(), "syncobj missing should have no fix command");
    }

    #[test]
    fn protocol_detected_by_substring() {
        // wayland-info output includes the name as part of a longer line
        let globals = vec![
            "  interface: zkde_screencast_unstable_v1  version: 1".to_string(),
            "  interface: wl_compositor  version: 6".to_string(),
        ];
        let results = check_protocols(&globals);
        let zkde = results.iter().find(|r| r.check == "zkde_screencast_unstable_v1").unwrap();
        assert_eq!(zkde.status, CheckStatus::Pass);
    }

    #[test]
    fn empty_globals_all_missing() {
        let results = check_protocols(&[]);
        assert!(results.iter().all(|r| r.status != CheckStatus::Pass),
            "empty globals should have no PASSes");
    }

    #[test]
    fn zkde_fix_command_is_portal_restart() {
        let globals = globals_with(&[]);
        let results = check_protocols(&globals);
        let zkde = results.iter().find(|r| r.check == "zkde_screencast_unstable_v1").unwrap();
        let fix = zkde.fix.as_deref().unwrap_or("");
        assert!(fix.contains("restart"), "zkde fix should be a restart command");
        assert!(fix.contains("plasma-xdg-desktop-portal-kde"));
    }
}
