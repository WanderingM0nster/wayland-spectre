// SPDX-License-Identifier: GPL-3.0-or-later
//! KWin adapter — Session 3.
//!
//! All D-Bus calls are now native zbus (no subprocess / busctl).
//! New checks derived from `supportInformation` parsing:
//!   - kwin_version              (L7) — KWin build version
//!   - kwin_render_backend       (L7) — EGL/GBM vs GLX (EGL required for screencasting)
//!   - kwin_screencast_loaded    (L7) — screencast plugin presence in supportInformation;
//!                                      absence directly explains L3 FAIL for
//!                                      zkde_screencast_unstable_v1
//!   - kwin_tiled_display        (L7) — ≥2 DP outputs → correlates with KDE bugs 493277/503870
//!
//! kwinrc file-read (kwin_screencast_plugin_kwinrc) unchanged.
//! Layer: L7

use crate::domain::types::{Confidence, DiagnosticResult, Layer};
use std::fs;
use std::path::PathBuf;
use zbus::{Connection, Proxy};

// ── Public entry point ─────────────────────────────────────────────────────

pub async fn check_kwin_plugins() -> Vec<DiagnosticResult> {
    let mut results = Vec::new();

    // kwinrc check: pure file read — no D-Bus needed
    results.extend(check_screencast_plugin_kwinrc());

    // Native D-Bus introspection
    let conn = match Connection::session().await {
        Ok(c) => c,
        Err(e) => {
            results.push(DiagnosticResult::fail(
                Layer::L7,
                "kwin_dbus_connect",
                format!("Cannot reach session bus for KWin checks: {e}"),
                "systemctl --user status dbus",
                Confidence::High,
            ));
            return results;
        }
    };

    match fetch_kwin_support_info(&conn).await {
        Err(e) => results.push(DiagnosticResult::fail(
            Layer::L7,
            "kwin_running",
            format!("KWin not responding on D-Bus: {e}"),
            "systemctl --user restart plasma-kwin_wayland",
            Confidence::High,
        )),
        Ok(info) => {
            results.push(DiagnosticResult::pass(
                Layer::L7,
                "kwin_running",
                "KWin responding on D-Bus (native zbus)",
            ));
            results.extend(analyse_support_info(&info));
        }
    }

    results
}

// ── D-Bus: fetch supportInformation ───────────────────────────────────────

async fn fetch_kwin_support_info(conn: &Connection) -> zbus::Result<String> {
    let proxy = Proxy::new(conn, "org.kde.KWin", "/KWin", "org.kde.KWin").await?;
    proxy
        .call::<_, _, (String,)>("supportInformation", &())
        .await
        .map(|(s,)| s)
}

// ── Pure analysis — all testable without D-Bus ────────────────────────────

/// Runs all supportInformation sub-checks. Exposed for unit tests.
pub(crate) fn analyse_support_info(info: &str) -> Vec<DiagnosticResult> {
    let mut out = Vec::new();
    out.extend(check_version(info));
    out.extend(check_render_backend(info));
    out.extend(check_screencast_plugin_info(info));
    out.extend(check_tiled_display(info));
    out
}

// ── KWin version ──────────────────────────────────────────────────────────

fn check_version(info: &str) -> Vec<DiagnosticResult> {
    for line in info.lines() {
        let lower = line.to_lowercase();
        if lower.starts_with("kwin version") || lower.starts_with("application version") {
            let version = line.splitn(2, ':').nth(1).unwrap_or("").trim().to_string();
            if !version.is_empty() {
                return vec![DiagnosticResult::pass(
                    Layer::L7,
                    "kwin_version",
                    format!("KWin {version}"),
                )];
            }
        }
    }
    vec![DiagnosticResult::warn(
        Layer::L7,
        "kwin_version",
        "KWin is running but version not found in supportInformation",
        None,
        Confidence::Low,
    )]
}

// ── Rendering backend (EGL/GBM vs GLX) ────────────────────────────────────

/// Returns `(has_egl, has_gbm, has_glx)` from supportInformation text.
/// Scans only lines that reference backend/platform/compositor/opengl
/// to avoid false positives from unrelated mentions.
/// Exported for unit tests.
pub(crate) fn parse_render_backend(info: &str) -> (bool, bool, bool) {
    let mut has_egl = false;
    let mut has_gbm = false;
    let mut has_glx = false;

    for line in info.lines() {
        let upper = line.to_uppercase();
        let is_backend_line = upper.contains("BACKEND")
            || upper.contains("PLATFORM")
            || upper.contains("OPENGL")
            || upper.contains("COMPOSITOR");

        if is_backend_line {
            if upper.contains("EGL") {
                has_egl = true;
            }
            if upper.contains("GLX") {
                has_glx = true;
            }
        }
        // GBM can appear on platform lines or standalone
        if upper.contains("GBM") {
            has_gbm = true;
        }
    }

    (has_egl, has_gbm, has_glx)
}

fn check_render_backend(info: &str) -> Vec<DiagnosticResult> {
    let (has_egl, has_gbm, has_glx) = parse_render_backend(info);

    if has_glx {
        return vec![DiagnosticResult::fail(
            Layer::L7,
            "kwin_render_backend",
            "KWin GLX backend detected — screencasting requires EGL/GBM. \
             Verify nvidia-drm.modeset=1 and that __EGL_VENDOR_LIBRARY_FILENAMES \
             is not overridden in the environment.",
            "Add __EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/10_nvidia.json \
             to /etc/environment then reboot",
            Confidence::High,
        )];
    }
    if has_egl {
        return vec![DiagnosticResult::pass(
            Layer::L7,
            "kwin_render_backend",
            format!(
                "KWin EGL backend confirmed{}",
                if has_gbm { " (GBM)" } else { "" }
            ),
        )];
    }
    vec![DiagnosticResult::warn(
        Layer::L7,
        "kwin_render_backend",
        "KWin rendering backend not identifiable from supportInformation",
        None,
        Confidence::Low,
    )]
}

// ── Screencast plugin ──────────────────────────────────────────────────────

/// Returns true if the screencast plugin appears in supportInformation text.
/// Case-insensitive. Exported for unit tests.
pub(crate) fn screencast_in_support_info(info: &str) -> bool {
    info.to_lowercase().contains("screencast")
}

fn check_screencast_plugin_info(info: &str) -> Vec<DiagnosticResult> {
    if screencast_in_support_info(info) {
        vec![DiagnosticResult::pass(
            Layer::L7,
            "kwin_screencast_loaded",
            "KWin supportInformation references screencast plugin — plugin initialised",
        )]
    } else {
        vec![DiagnosticResult::fail(
            Layer::L7,
            "kwin_screencast_loaded",
            "KWin supportInformation has no screencast reference — plugin likely failed to \
             initialise. This directly explains the L3 FAIL: zkde_screencast_unstable_v1 \
             not advertised. Common causes: NVIDIA driver / tiled display init failure \
             (KDE bugs 493277, 503870), or plugin crash at startup.",
            "systemctl --user restart plasma-kwin_wayland",
            Confidence::High,
        )]
    }
}

// ── Tiled display (KDE bugs 493277 + 503870) ──────────────────────────────

/// Count "DP-N" style output references in supportInformation text.
/// KWin lists each tile of a tiled display as a separate DP-N output.
/// Exported for unit tests.
pub(crate) fn count_dp_outputs(info: &str) -> usize {
    let mut count = 0;
    let mut rest = info;
    while let Some(pos) = rest.find("DP-") {
        if rest[pos + 3..]
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
        {
            count += 1;
        }
        rest = &rest[pos + 3..];
    }
    count
}

fn check_tiled_display(info: &str) -> Vec<DiagnosticResult> {
    match count_dp_outputs(info) {
        0 => vec![], // HDMI/VGA only — no tiled display concern
        1 => vec![DiagnosticResult::pass(
            Layer::L7,
            "kwin_tiled_display",
            "Single DP output — no tiled display split detected",
        )],
        n => vec![DiagnosticResult::warn(
            Layer::L7,
            "kwin_tiled_display",
            format!(
                "{n} DP outputs detected — tiled 4K display (e.g. Dell UP3214Q) appears as \
                 two DP tiles. KDE bugs 493277 + 503870: CRTC format mismatch (AB30 vs AB4H) \
                 on tiled panels prevents KWin screencast plugin from advertising \
                 zkde_screencast_unstable_v1 and ext_image_capture_source_v1. \
                 This is the probable root cause of the L3 FAILs. \
                 Tracked at NVIDIA forum 331077."
            ),
            Some("xdg-open https://bugs.kde.org/show_bug.cgi?id=493277".into()),
            Confidence::High,
        )],
    }
}

// ── kwinrc: screencast plugin key ──────────────────────────────────────────

fn check_screencast_plugin_kwinrc() -> Vec<DiagnosticResult> {
    let Some(content) = read_kwinrc() else {
        return vec![DiagnosticResult::skip(
            Layer::L7,
            "kwin_screencast_plugin_kwinrc",
            "kwinrc not found — cannot check plugin key",
        )];
    };

    let disabled = content.lines().any(|l| {
        // KDE used the typo "screencasl" in some older versions
        (l.starts_with("screencaslPlugin=") || l.starts_with("screencastPlugin="))
            && l.ends_with("false")
    });

    if disabled {
        vec![DiagnosticResult::fail(
            Layer::L7,
            "kwin_screencast_plugin_kwinrc",
            "KWin screencast plugin explicitly disabled in kwinrc — all screen sharing fails",
            "busctl --user call org.kde.KWin /Plugins org.kde.KWin.Plugins loadPlugin 's' screencast",
            Confidence::High,
        )]
    } else {
        vec![DiagnosticResult::pass(
            Layer::L7,
            "kwin_screencast_plugin_kwinrc",
            "KWin screencast plugin not explicitly disabled in kwinrc",
        )]
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn read_kwinrc() -> Option<String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    fs::read_to_string(PathBuf::from(home).join(".config/kwinrc")).ok()
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::CheckStatus;

    fn run(info: &str) -> Vec<DiagnosticResult> {
        analyse_support_info(info)
    }

    fn status(results: &[DiagnosticResult], check: &str) -> CheckStatus {
        results
            .iter()
            .find(|r| r.check == check)
            .unwrap_or_else(|| panic!("check '{check}' not found"))
            .status
            .clone()
    }

    // ── Version ──────────────────────────────────────────────────────────

    #[test]
    fn version_extracted_from_kwin_line() {
        let r = run("KWin version: 6.2.5\nsome other line\n");
        assert_eq!(status(&r, "kwin_version"), CheckStatus::Pass);
        assert!(r.iter().find(|x| x.check == "kwin_version").unwrap().detail.contains("6.2.5"));
    }

    #[test]
    fn version_extracted_from_application_line() {
        let r = run("Application version: 6.3.0\nQt version: 6.8\n");
        assert_eq!(status(&r, "kwin_version"), CheckStatus::Pass);
    }

    #[test]
    fn version_missing_produces_warn() {
        let r = run("nothing useful here\n");
        assert_eq!(status(&r, "kwin_version"), CheckStatus::Warn);
    }

    #[test]
    fn version_case_insensitive() {
        let r = run("KWIN VERSION: 6.1.0\n");
        assert_eq!(status(&r, "kwin_version"), CheckStatus::Pass);
    }

    // ── Render backend ────────────────────────────────────────────────────

    #[test]
    fn egl_gbm_backend_passes() {
        let info = "Hardware Backend: Native EGL\nPlatform: GBM\n";
        let (egl, gbm, glx) = parse_render_backend(info);
        assert!(egl, "EGL expected");
        assert!(gbm, "GBM expected");
        assert!(!glx, "GLX should be absent");
        assert_eq!(status(&run(info), "kwin_render_backend"), CheckStatus::Pass);
    }

    #[test]
    fn glx_backend_fails() {
        let info = "Hardware Backend: GLX\nOpenGL platform: GLX\n";
        assert_eq!(status(&run(info), "kwin_render_backend"), CheckStatus::Fail);
    }

    #[test]
    fn no_backend_hints_warns() {
        let info = "KWin version: 6.0.0\n";
        assert_eq!(status(&run(info), "kwin_render_backend"), CheckStatus::Warn);
    }

    #[test]
    fn egl_without_gbm_still_passes() {
        let info = "OpenGL platform: EGL\n";
        let (egl, gbm, _) = parse_render_backend(info);
        assert!(egl);
        assert!(!gbm);
        assert_eq!(status(&run(info), "kwin_render_backend"), CheckStatus::Pass);
    }

    // ── Screencast plugin ─────────────────────────────────────────────────

    #[test]
    fn screencast_present_passes() {
        let info = "Loaded Effects: screencast, blur, overview\n";
        assert!(screencast_in_support_info(info));
        assert_eq!(status(&run(info), "kwin_screencast_loaded"), CheckStatus::Pass);
    }

    #[test]
    fn screencast_absent_fails() {
        let info = "Loaded Effects: blur, overview, slideback\n";
        assert!(!screencast_in_support_info(info));
        assert_eq!(status(&run(info), "kwin_screencast_loaded"), CheckStatus::Fail);
    }

    #[test]
    fn screencast_case_insensitive() {
        assert!(screencast_in_support_info("ScreenCast: loaded\n"));
        assert!(screencast_in_support_info("SCREENCAST plugin active\n"));
        assert!(screencast_in_support_info("Plugin: kwin_screencast enabled\n"));
    }

    #[test]
    fn screencast_fail_mentions_l3() {
        let r = run("Loaded Effects: blur\n");
        let d = &r.iter().find(|x| x.check == "kwin_screencast_loaded").unwrap().detail;
        assert!(d.contains("L3") || d.contains("zkde_screencast"));
    }

    // ── Tiled display ─────────────────────────────────────────────────────

    #[test]
    fn two_dp_outputs_warns() {
        let info = "Output DP-4: 1920x2160\nOutput DP-5: 1920x2160\n";
        assert_eq!(count_dp_outputs(info), 2);
        assert_eq!(status(&run(info), "kwin_tiled_display"), CheckStatus::Warn);
    }

    #[test]
    fn single_dp_passes() {
        let info = "Output DP-1: 2560x1440\n";
        assert_eq!(count_dp_outputs(info), 1);
        assert_eq!(status(&run(info), "kwin_tiled_display"), CheckStatus::Pass);
    }

    #[test]
    fn no_dp_outputs_no_tiled_check() {
        let info = "Output HDMI-1: 1920x1080\n";
        assert_eq!(count_dp_outputs(info), 0);
        assert!(run(info).iter().all(|r| r.check != "kwin_tiled_display"));
    }

    #[test]
    fn four_dp_outputs_detected() {
        let info = "DP-1: mode\nDP-2: mode\nDP-3: mode\nDP-4: mode\n";
        assert_eq!(count_dp_outputs(info), 4);
    }

    #[test]
    fn dp_prefix_without_digit_not_counted() {
        // "DP-" must be followed by a digit
        let info = "DP-Link: present\nDP-CEC: off\nDP-4: active\n";
        assert_eq!(count_dp_outputs(info), 1);
    }

    #[test]
    fn tiled_display_warn_mentions_kde_bugs() {
        let r = run("Output DP-4: x\nOutput DP-5: x\n");
        let d = &r.iter().find(|x| x.check == "kwin_tiled_display").unwrap().detail;
        assert!(d.contains("493277") || d.contains("503870"));
    }
}
