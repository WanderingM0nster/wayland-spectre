// SPDX-License-Identifier: GPL-3.0-or-later
//! KWin adapter — Session 4.
//!
//! New in this session:
//!   - `screencast_in_section(info, section_name)` — parse a named section of
//!     supportInformation instead of doing a global substring search.
//!   - `kwin_screencast_effect_active` (L7) — distinguishes between the plugin
//!     appearing in "Loaded Plugins" (plugin was loaded) vs. appearing in
//!     "Loaded Effects" (effect is actually running).
//!     On our system: plugin is in Loaded Plugins but NOT in Loaded Effects —
//!     CRTC format mismatch (AB30 vs AB4H) on the tiled Dell UP3214Q prevents
//!     effect registration at compositor startup. This is the precise failure
//!     point that explains why zkde_screencast_unstable_v1 is never advertised.
//!   - `generate_bug_report` now captures the full KWin boot journal and a
//!     targeted effect-startup extract (commands.rs change).
//!
//! kwinrc check and tiled-display check: unchanged from Session 3.
//! Layer: L7

use crate::domain::types::{Confidence, DiagnosticResult, Layer, SessionType};
use std::fs;
use std::path::PathBuf;
use zbus::{Connection, Proxy};

// ── Public entry point ─────────────────────────────────────────────────────

pub async fn check_kwin_plugins(session: SessionType) -> Vec<DiagnosticResult> {
    let mut results = Vec::new();

    // kwinrc check: pure file read — no D-Bus needed. Runs on X11 too: a
    // disabled plugin key matters the moment the user logs back into Wayland.
    results.extend(check_screencast_plugin_kwinrc());

    // Native D-Bus introspection — org.kde.KWin is on the bus for both
    // kwin_wayland and kwin_x11.
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
                if session == SessionType::X11 {
                    "KWin responding on D-Bus (kwin_x11 — X11 session)"
                } else {
                    "KWin responding on D-Bus (native zbus)"
                },
            ));
            results.extend(analyse_support_info(&info, session));
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
///
/// v0.4.1: takes the session type. On X11 the compositor is kwin_x11 —
/// the Wayland-screencast-specific checks (render backend EGL requirement,
/// screencast plugin/effect) report SKIP; version and tiled-display
/// detection still apply.
pub(crate) fn analyse_support_info(info: &str, session: SessionType) -> Vec<DiagnosticResult> {
    let mut out = Vec::new();
    out.extend(check_version(info));
    out.extend(check_render_backend(info, session));
    out.extend(check_screencast_plugin_info(info, session));
    out.extend(check_screencast_effect_active(info, session));
    out.extend(check_tiled_display(info));
    out
}

// ── Section-aware parsing ──────────────────────────────────────────────────

/// Search for `needle` (case-insensitive, hardcoded to "screencast") within
/// the named section of KWin supportInformation.
///
/// KWin's `supportInformation` uses sections separated by blank lines:
///
/// ```text
/// Loaded Plugins:
/// kwin4_effect_blur
/// kwin_screencast
/// kwin4_effect_overview
///
/// Loaded Effects:
/// blur
/// overview
/// ```
///
/// This function:
/// 1. Locates the line `"{section_name}:"` (case-insensitive)
/// 2. Reads subsequent lines until the first blank line (section separator)
/// 3. Returns whether any of those lines contain "screencast"
///
/// Returns `false` if the named section header is not found in `info`.
/// Exported for unit tests.
pub(crate) fn screencast_in_section(info: &str, section_name: &str) -> bool {
    let lower = info.to_lowercase();
    let header = format!("{}:", section_name.to_lowercase());

    let Some(section_start) = lower.find(&header) else {
        return false;
    };
    let after = &lower[section_start + header.len()..];

    // Read lines until the first blank line (KWin section separator).
    // `past_first` guards against stopping immediately if the header is
    // followed by a bare newline before the first content line.
    let mut past_first = false;
    for line in after.lines() {
        let t = line.trim();
        if t.is_empty() {
            if past_first {
                break; // end of section
            }
        } else {
            past_first = true;
            if t.contains("screencast") {
                return true;
            }
        }
    }
    false
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

fn check_render_backend(info: &str, session: SessionType) -> Vec<DiagnosticResult> {
    // GLX is the normal backend under kwin_x11 — the EGL/GBM requirement
    // only applies to the kwin_wayland screencast path (v0.4.1).
    if session == SessionType::X11 {
        return vec![DiagnosticResult::skip(
            Layer::L7,
            "kwin_render_backend",
            "X11 session — EGL/GBM requirement applies to the kwin_wayland screencast path",
        )];
    }

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

// ── Screencast plugin — present in "Loaded Plugins" ───────────────────────

/// Returns true if the screencast plugin appears in the "Loaded Plugins"
/// section of supportInformation, or (as a fallback for older/unusual KWin
/// output that lacks section headers) anywhere in the text.
/// Exported for unit tests.
pub(crate) fn screencast_in_support_info(info: &str) -> bool {
    // Prefer section-aware search. If "Loaded Plugins:" is absent (old KWin
    // format or truncated output), fall back to a global substring search.
    if info.to_lowercase().contains("loaded plugins:") {
        screencast_in_section(info, "Loaded Plugins")
    } else {
        info.to_lowercase().contains("screencast")
    }
}

fn check_screencast_plugin_info(info: &str, session: SessionType) -> Vec<DiagnosticResult> {
    if session == SessionType::X11 {
        return vec![DiagnosticResult::skip(
            Layer::L7,
            "kwin_screencast_loaded",
            "X11 session — kwin_screencast is a kwin_wayland plugin; not expected under kwin_x11",
        )];
    }

    if screencast_in_support_info(info) {
        vec![DiagnosticResult::pass(
            Layer::L7,
            "kwin_screencast_loaded",
            "KWin screencast plugin present in 'Loaded Plugins' — plugin initialised by KWin",
        )]
    } else {
        // v0.4.1 false-positive audit: upstream confirmed this listing is not
        // reliable for the screencast plugin — it can be absent on systems
        // where screen sharing works. Inconclusive, not a fault; the check
        // that actually distinguishes plugin-loaded from effect-activated is
        // kwin_screencast_effect_active (L7), which keeps its FAIL weight.
        vec![DiagnosticResult::warn(
            Layer::L7,
            "kwin_screencast_loaded",
            "KWin screencast plugin not listed in 'Loaded Plugins' — inconclusive: this \
             listing is unreliable for the screencast plugin and can be absent on systems \
             where screen sharing works (upstream-confirmed). See \
             kwin_screencast_effect_active (L7) for the signal that distinguishes \
             plugin-loaded from effect-activated.",
            None,
            Confidence::Low,
        )]
    }
}

// ── Screencast effect — actually active in "Loaded Effects" ───────────────

/// Returns true if the screencast effect appears in the "Loaded Effects"
/// section of supportInformation.
/// Exported for unit tests.
pub(crate) fn screencast_effect_active(info: &str) -> bool {
    screencast_in_section(info, "Loaded Effects")
}

/// NEW in Session 4.
/// Distinguishes between:
///   - Plugin present in "Loaded Plugins" (kwin_screencast_loaded)
///   - Effect *active* in "Loaded Effects" (this check)
///
/// On arctic: plugin loads (present in Loaded Plugins) but the effect never
/// activates (absent from Loaded Effects) because the CRTC format mismatch
/// (AB30 vs AB4H) on the tiled Dell UP3214Q prevents KWin from registering
/// the effect at compositor startup. This is why zkde_screencast_unstable_v1
/// is never advertised on the Wayland bus.
///
/// To inspect startup errors:
///   journalctl --user -u plasma-kwin_wayland -b | grep -iE 'screencast|effect|crtc|format'
///
/// On kwin_wayland --replace: there is no non-destructive effect-reload path
/// in KDE Plasma 6. kwin_wayland --replace requires D-Bus takeover and is
/// not supported when KWin is managed by plasma-kwin_wayland.service.
/// The least-disruptive option remains:
///   systemctl --user restart plasma-kwin_wayland
fn check_screencast_effect_active(info: &str, session: SessionType) -> Vec<DiagnosticResult> {
    if session == SessionType::X11 {
        return vec![DiagnosticResult::skip(
            Layer::L7,
            "kwin_screencast_effect_active",
            "X11 session — the screencast effect only exists under kwin_wayland",
        )];
    }

    // Skip if "Loaded Effects:" section is absent — we can't distinguish
    if !info.to_lowercase().contains("loaded effects:") {
        return vec![DiagnosticResult::skip(
            Layer::L7,
            "kwin_screencast_effect_active",
            "Cannot determine effect activation state: 'Loaded Effects' section absent \
             from supportInformation (older KWin format or truncated output)",
        )];
    }

    if screencast_effect_active(info) {
        return vec![DiagnosticResult::pass(
            Layer::L7,
            "kwin_screencast_effect_active",
            "KWin screencast effect confirmed active in 'Loaded Effects'",
        )];
    }

    // Effect is not active. Distinguish plugin-loaded-but-stuck from
    // plugin-not-loaded-at-all to give a more precise failure message.
    let plugin_loaded = screencast_in_support_info(info);

    let detail = if plugin_loaded {
        "KWin screencast plugin is present in 'Loaded Plugins' but absent from \
         'Loaded Effects' — effect failed to activate at compositor startup. \
         This is the precise failure point explaining why zkde_screencast_unstable_v1 \
         and ext_image_capture_source_v1 are never advertised on the Wayland bus. \
         Root cause: CRTC format mismatch (AB30 vs AB4H) on tiled Dell UP3214Q \
         prevents KWin from registering the screencast effect (NVIDIA forum 331077, \
         KDE bugs 493277 + 503870). \
         To inspect startup errors: \
         journalctl --user -u plasma-kwin_wayland -b | grep -iE 'screencast|effect|crtc|format'. \
         Note: kwin_wayland --replace is not available in KDE Plasma 6 when KWin \
         is managed by plasma-kwin_wayland.service — only a service restart is viable."
    } else {
        "KWin screencast effect absent from 'Loaded Effects'. \
         Plugin also absent from 'Loaded Plugins' — screencast plugin likely disabled, \
         missing from the KWin plugin path, or crashed before reaching effect registration."
    };

    vec![DiagnosticResult::fail(
        Layer::L7,
        "kwin_screencast_effect_active",
        detail,
        "systemctl --user restart plasma-kwin_wayland",
        Confidence::High,
    )]
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
        analyse_support_info(info, SessionType::Wayland)
    }

    fn run_x11(info: &str) -> Vec<DiagnosticResult> {
        analyse_support_info(info, SessionType::X11)
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

    // ── Section parser ────────────────────────────────────────────────────

    #[test]
    fn section_parser_finds_screencast_in_plugins_section() {
        let info = "Loaded Plugins:\nkwin4_effect_blur\nkwin_screencast\nkwin4_effect_overview\n\nLoaded Effects:\nblur\noverview\n";
        assert!(screencast_in_section(info, "Loaded Plugins"), "should find in Loaded Plugins");
        assert!(!screencast_in_section(info, "Loaded Effects"), "should not find in Loaded Effects");
    }

    #[test]
    fn section_parser_finds_screencast_in_effects_section() {
        let info = "Loaded Plugins:\nkwin4_effect_blur\nkwin_screencast\n\nLoaded Effects:\nblur\nscreencast\noverview\n";
        assert!(screencast_in_section(info, "Loaded Plugins"));
        assert!(screencast_in_section(info, "Loaded Effects"));
    }

    #[test]
    fn section_parser_returns_false_for_missing_section() {
        let info = "KWin version: 6.0.0\nsome text mentioning screencast here\n";
        assert!(!screencast_in_section(info, "Loaded Plugins"),
            "should not find section header");
    }

    #[test]
    fn section_parser_handles_inline_section_content() {
        // KWin may put content on the same line as the header
        let info = "Loaded Effects: screencast blur overview\n";
        assert!(screencast_in_section(info, "Loaded Effects"));
    }

    #[test]
    fn section_parser_case_insensitive() {
        let info = "LOADED PLUGINS:\nkwin_SCREENCAST\n\n";
        assert!(screencast_in_section(info, "Loaded Plugins"));
    }

    // ── Screencast plugin (kwin_screencast_loaded) ─────────────────────

    #[test]
    fn screencast_present_passes() {
        // No "Loaded Plugins:" section → falls back to global search
        let info = "Loaded Effects: screencast, blur, overview\n";
        assert!(screencast_in_support_info(info));
        assert_eq!(status(&run(info), "kwin_screencast_loaded"), CheckStatus::Pass);
    }

    #[test]
    fn screencast_absent_warns_inconclusive() {
        // v0.4.1 audit: absence from 'Loaded Plugins' is unreliable — WARN, not FAIL
        let info = "Loaded Effects: blur, overview, slideback\n";
        assert!(!screencast_in_support_info(info));
        let r = run(info);
        assert_eq!(status(&r, "kwin_screencast_loaded"), CheckStatus::Warn);
        let res = r.iter().find(|x| x.check == "kwin_screencast_loaded").unwrap();
        assert!(res.fix.is_none(), "inconclusive warn must not offer a fix");
    }

    #[test]
    fn screencast_case_insensitive() {
        assert!(screencast_in_support_info("ScreenCast: loaded\n"));
        assert!(screencast_in_support_info("SCREENCAST plugin active\n"));
        assert!(screencast_in_support_info("Plugin: kwin_screencast enabled\n"));
    }

    #[test]
    fn screencast_warn_points_at_effect_active_signal() {
        let r = run("Loaded Effects: blur\n");
        let d = &r.iter().find(|x| x.check == "kwin_screencast_loaded").unwrap().detail;
        assert!(d.contains("inconclusive"), "should frame absence as inconclusive");
        assert!(d.contains("kwin_screencast_effect_active"),
            "should point at the reliable L7 signal");
    }

    #[test]
    fn screencast_loaded_uses_section_when_available() {
        // "Loaded Plugins:" section present, screencast NOT in it
        // Even though "screencast" appears elsewhere in text
        let info = "Loaded Plugins:\nkwin4_effect_blur\nkwin4_effect_overview\n\n\
                    Some other section mentioning screencast configuration\n";
        // Section-aware: looks in "Loaded Plugins:" section only → not found
        assert!(!screencast_in_section(info, "Loaded Plugins"));
        // screencast_in_support_info now uses section-aware path
        assert!(!screencast_in_support_info(info),
            "should not report plugin as loaded when absent from Loaded Plugins section");
        assert_eq!(status(&run(info), "kwin_screencast_loaded"), CheckStatus::Warn);
    }

    // ── Screencast effect active (kwin_screencast_effect_active) — NEW ──

    #[test]
    fn effect_active_pass_when_in_loaded_effects() {
        let info = "Loaded Plugins:\nkwin_screencast\n\nLoaded Effects:\nscreencast\nblur\n";
        assert_eq!(status(&run(info), "kwin_screencast_effect_active"), CheckStatus::Pass);
    }

    #[test]
    fn effect_active_fail_when_plugin_loaded_but_effect_missing() {
        // This is the exact failure mode on arctic: plugin in Loaded Plugins,
        // absent from Loaded Effects — CRTC mismatch prevents effect registration.
        let info = "Loaded Plugins:\nkwin_screencast\nkwin4_effect_blur\n\n\
                    Loaded Effects:\nblur\noverview\n";
        assert_eq!(status(&run(info), "kwin_screencast_loaded"), CheckStatus::Pass);
        assert_eq!(status(&run(info), "kwin_screencast_effect_active"), CheckStatus::Fail);
    }

    #[test]
    fn effect_active_fail_mentions_crtc_and_restart_note() {
        let info = "Loaded Plugins:\nkwin_screencast\n\nLoaded Effects:\nblur\n";
        let r = run(info);
        let d = &r.iter().find(|x| x.check == "kwin_screencast_effect_active").unwrap().detail;
        assert!(d.contains("CRTC") || d.contains("crtc") || d.contains("493277"),
            "detail should mention CRTC mismatch or KDE bug");
        assert!(d.contains("restart") || d.contains("kwin_wayland"),
            "detail should mention restart path");
    }

    #[test]
    fn effect_active_skips_when_no_loaded_effects_section() {
        // Older KWin format or truncated output — can't determine effect state
        let info = "KWin version: 6.0.0\nsome text\n";
        assert_eq!(status(&run(info), "kwin_screencast_effect_active"), CheckStatus::Skip);
    }

    #[test]
    fn effect_active_different_detail_when_plugin_also_absent() {
        // Neither loaded: different diagnosis than "plugin-loaded-but-stuck"
        let info = "Loaded Plugins:\nkwin4_effect_blur\n\nLoaded Effects:\nblur\n";
        let r = run(info);
        let result = r.iter().find(|x| x.check == "kwin_screencast_effect_active").unwrap();
        assert_eq!(result.status, CheckStatus::Fail);
        // Detail should NOT mention CRTC mismatch (wrong diagnosis for this case)
        assert!(!result.detail.contains("CRTC"),
            "CRTC diagnosis should only appear when plugin is loaded but effect is not");
    }

    #[test]
    fn effect_active_check_independent_of_screencast_loaded_check() {
        // Both PASS
        let info_both = "Loaded Plugins:\nkwin_screencast\n\nLoaded Effects:\nscreencast\n";
        assert_eq!(status(&run(info_both), "kwin_screencast_loaded"), CheckStatus::Pass);
        assert_eq!(status(&run(info_both), "kwin_screencast_effect_active"), CheckStatus::Pass);

        // Plugin loaded, effect not active (the arctic failure mode)
        let info_stuck = "Loaded Plugins:\nkwin_screencast\n\nLoaded Effects:\nblur\n";
        assert_eq!(status(&run(info_stuck), "kwin_screencast_loaded"), CheckStatus::Pass);
        assert_eq!(status(&run(info_stuck), "kwin_screencast_effect_active"), CheckStatus::Fail);

        // Plugin not listed → loaded check is inconclusive (WARN, v0.4.1 audit),
        // but the effect check keeps its FAIL weight — it is the real signal.
        let info_none = "Loaded Plugins:\nkwin4_effect_blur\n\nLoaded Effects:\nblur\n";
        assert_eq!(status(&run(info_none), "kwin_screencast_loaded"), CheckStatus::Warn);
        assert_eq!(status(&run(info_none), "kwin_screencast_effect_active"), CheckStatus::Fail);
    }

    // ── v0.4.1: X11 session behaviour ─────────────────────────────────────

    #[test]
    fn x11_render_backend_skips_even_with_glx() {
        // GLX is normal under kwin_x11 — must not FAIL there
        let info = "Hardware Backend: GLX\nOpenGL platform: GLX\n";
        let r = run_x11(info);
        assert_eq!(status(&r, "kwin_render_backend"), CheckStatus::Skip);
        let res = r.iter().find(|x| x.check == "kwin_render_backend").unwrap();
        assert!(res.fix.is_none());
        assert!(res.detail.contains("X11 session"));
    }

    #[test]
    fn x11_screencast_checks_skip() {
        // No screencast plugin/effect anywhere — expected under kwin_x11
        let info = "Loaded Plugins:\nkwin4_effect_blur\n\nLoaded Effects:\nblur\n";
        let r = run_x11(info);
        assert_eq!(status(&r, "kwin_screencast_loaded"), CheckStatus::Skip);
        assert_eq!(status(&r, "kwin_screencast_effect_active"), CheckStatus::Skip);
        for check in ["kwin_screencast_loaded", "kwin_screencast_effect_active"] {
            let res = r.iter().find(|x| x.check == check).unwrap();
            assert!(res.fix.is_none(), "{check} must not offer a fix on X11");
        }
    }

    #[test]
    fn x11_version_and_tiled_display_still_run() {
        let info = "KWin version: 6.6.3\nOutput DP-4: 1920x2160\nOutput DP-5: 1920x2160\n";
        let r = run_x11(info);
        assert_eq!(status(&r, "kwin_version"), CheckStatus::Pass);
        // Tiled-panel topology is real hardware info regardless of session type
        assert_eq!(status(&r, "kwin_tiled_display"), CheckStatus::Warn);
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
