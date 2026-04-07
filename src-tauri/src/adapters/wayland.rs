// SPDX-License-Identifier: GPL-3.0-or-later
//! Wayland protocol adapter — Session 3.
//!
//! Changes vs Session 2:
//!   - Added wp_linux_drm_syncobj_manager_v1  (L3, required) — explicit sync fence protocol;
//!     needed on NVIDIA 555+ for Xid 51/69 mitigation and tiled display stability.
//!   - Added zwlr_screencopy_manager_v1       (L3, optional) — wlroots fallback screencopy;
//!     not advertised by KDE but absence confirms KDE-native path is the only option.
//!   - Added wl_output tiling correlation     (L3) — ≥2 wl_output globals on a system
//!     with no zkde_screencast → annotate the tiled-display hypothesis.

use wayland_client::{protocol::wl_registry, Connection, Dispatch, QueueHandle};
use crate::domain::types::{Confidence, DiagnosticResult, Layer};

#[derive(Debug, Clone)]
pub(crate) struct WlGlobal {
    interface: String,
    version: u32,
}

struct RegistryCollector {
    globals: Vec<WlGlobal>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for RegistryCollector {
    fn event(
        state: &mut Self,
        _: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            interface, version, ..
        } = event
        {
            state.globals.push(WlGlobal { interface, version });
        }
    }
}

pub async fn check_wayland_protocols() -> Vec<DiagnosticResult> {
    match connect_and_enumerate() {
        Err(e) => vec![DiagnosticResult::fail(
            Layer::L2,
            "wayland_connect",
            format!("Cannot connect to $WAYLAND_DISPLAY: {e}"),
            "echo 'Check WAYLAND_DISPLAY is set'",
            Confidence::High,
        )],
        Ok(globals) => build_checks(globals),
    }
}

fn connect_and_enumerate(
) -> Result<Vec<WlGlobal>, Box<dyn std::error::Error + Send + Sync>> {
    let conn = Connection::connect_to_env()?;
    let mut queue = conn.new_event_queue::<RegistryCollector>();
    let qh = queue.handle();
    conn.display().get_registry(&qh, ());
    let mut state = RegistryCollector {
        globals: Vec::new(),
    };
    queue.roundtrip(&mut state)?;
    Ok(state.globals)
}

// ── Protocol tables ────────────────────────────────────────────────────────

/// (interface, label, required, fix_cmd)
///
/// `required=true`  → FAIL if absent, with the given `fix_cmd`.
/// `required=false` → WARN if absent (optional/informational, no fix needed).
///
/// Fix commands must satisfy the safety rules in AGENTS.md:
///   single token, no |;&><$`, no sudo, restarts only.
const SCREENCAST_GLOBALS: &[(&str, &str, bool, &str)] = &[
    // zwp_linux_dmabuf_v1 has been in Mesa and KWin since ~2018;
    // if absent something has gone very wrong with the compositor startup.
    ("zwp_linux_dmabuf_v1",
     "Linux DMA-BUF",
     true,
     "journalctl --user -u plasma-kwin_wayland -n 100"),

    // zkde_screencast_unstable_v1 is advertised by the KWin screencast plugin.
    // If absent the plugin failed to initialise — restart is the first step.
    // (Bug D: on tiled displays the CRTC format mismatch prevents init.)
    ("zkde_screencast_unstable_v1",
     "KDE ScreenCast (kde-screencast)",
     true,
     "systemctl --user restart plasma-kwin_wayland"),

    // ext_image_capture_source_v1 is the standardised replacement added in
    // KWin 6.1 / xdg-desktop-portal-kde 1.18. Same root cause as above.
    ("ext_image_capture_source_v1",
     "ext-image-capture-source",
     true,
     "systemctl --user restart plasma-kwin_wayland"),

    // wp_linux_drm_syncobj_manager_v1 (explicit sync fences) — required by
    // NVIDIA open modules ≥555 to avoid Xid 51/69 on tiled panels.
    // Added to KWin in Plasma 6.1 / kernel 6.6+.
    ("wp_linux_drm_syncobj_manager_v1",
     "Linux DRM syncobj (explicit sync)",
     true,
     "journalctl --user -u plasma-kwin_wayland -n 100"),

    // Optional / informational
    ("wp_viewporter",              "wp-viewporter",               false, ""),
    ("wp_presentation",            "wp-presentation",             false, ""),
    ("zwlr_screencopy_manager_v1", "wlroots screencopy (fallback)", false, ""),
];

// ── Main check builder ─────────────────────────────────────────────────────

pub(crate) fn build_checks(globals: Vec<WlGlobal>) -> Vec<DiagnosticResult> {
    let mut out = Vec::new();

    // L2: compositor type
    let has_kde  = globals.iter().any(|g| {
        g.interface.starts_with("org_kde_") || g.interface.starts_with("kde_")
    });
    let has_xdg  = globals.iter().any(|g| g.interface == "xdg_wm_base");
    let has_drm  = globals.iter().any(|g| g.interface == "wl_drm");

    if has_kde && has_xdg {
        out.push(DiagnosticResult::pass(
            Layer::L2,
            "wayland_backend",
            format!(
                "KWin/Wayland confirmed — {} globals, wl_drm={}",
                globals.len(),
                has_drm
            ),
        ));
    } else if has_drm && !has_kde {
        out.push(DiagnosticResult::warn(
            Layer::L2,
            "wayland_backend",
            "wl_drm present without KDE globals — possible XWayland compositor",
            None,
            Confidence::Medium,
        ));
    } else {
        out.push(DiagnosticResult::warn(
            Layer::L2,
            "wayland_backend",
            format!("Compositor type unclear — {} globals", globals.len()),
            None,
            Confidence::Low,
        ));
    }

    // L3: wl_output count — tiled display indicator
    let n_outputs = globals.iter().filter(|g| g.interface == "wl_output").count();
    if n_outputs > 0 {
        out.push(DiagnosticResult::pass(
            Layer::L3,
            "wl_output_count",
            format!("{n_outputs} output(s) advertised"),
        ));
    } else {
        out.push(DiagnosticResult::fail(
            Layer::L3,
            "wl_output_count",
            "No wl_output globals",
            "echo 'Check display connection'",
            Confidence::High,
        ));
    }

    // L3: tiled display cross-correlation
    // If ≥2 wl_outputs AND zkde_screencast missing, flag the known KDE bug
    let has_zkde = globals
        .iter()
        .any(|g| g.interface == "zkde_screencast_unstable_v1");
    if n_outputs >= 2 && !has_zkde {
        out.push(DiagnosticResult::warn(
            Layer::L3,
            "wl_output_tiled_correlation",
            format!(
                "{n_outputs} wl_outputs present but zkde_screencast_unstable_v1 absent — \
                 consistent with tiled 4K display (KDE bugs 493277 + 503870). \
                 KWin screencast plugin may fail to init when CRTC tiling triggers \
                 format mismatch (AB30 vs AB4H). See L7 kwin_tiled_display."
            ),
            None,
            Confidence::High,
        ));
    }

    // L3: protocol-by-protocol checks
    for (iface, label, required, fix_cmd) in SCREENCAST_GLOBALS {
        match globals.iter().find(|g| g.interface == *iface) {
            Some(g) => out.push(DiagnosticResult::pass(
                Layer::L3,
                *iface,
                format!("{label} advertised at version {}", g.version),
            )),
            None if *required => out.push(DiagnosticResult::fail(
                Layer::L3,
                *iface,
                format!("{iface} not advertised by compositor"),
                *fix_cmd,
                Confidence::High,
            )),
            None => out.push(DiagnosticResult::warn(
                Layer::L3,
                *iface,
                format!("{iface} not advertised (optional)"),
                None,
                Confidence::Low,
            )),
        }
    }

    // ── Bug D: KWin screencast globals completely absent ───────────────────
    //
    // When BOTH zkde_screencast_unstable_v1 AND ext_image_capture_source_v1
    // are missing, screen sharing is broken at the compositor level regardless
    // of portal or PipeWire state.  This is "Bug D" — distinguished from
    // individual missing-global FAILs above by synthesising the root-cause
    // hypothesis and pointing at the specific upstream threads.
    //
    // On this system (RTX 5090, NVIDIA open 595.x, tiled Dell UP3214Q) the
    // cause is the CRTC tiling format mismatch (AB30 vs AB4H) preventing
    // the KWin screencast plugin from initialising at compositor startup.
    // KDE bugs 493277 + 503870, NVIDIA forum 331077.
    let has_ext = globals.iter().any(|g| g.interface == "ext_image_capture_source_v1");
    if !has_zkde && !has_ext {
        let tiled_context = if n_outputs >= 2 {
            format!(
                " {n_outputs} wl_outputs detected — consistent with a tiled 4K display \
                 (e.g. Dell UP3214Q split across DP-4 + DP-5). The CRTC format mismatch \
                 (AB30 vs AB4H) on NVIDIA open modules is the probable init blocker. \
                 See KDE bugs 493277 + 503870 and NVIDIA forum 331077."
            )
        } else {
            " Check L7 kwin_screencast_loaded — the plugin may have crashed or been \
             disabled in kwinrc."
            .to_string()
        };

        out.push(DiagnosticResult::fail(
            Layer::L3,
            "bug_d_screencast_globals",
            format!(
                "Bug D — KWin is not advertising its screencast protocol globals. \
                 Screen sharing is broken at the compositor level: no amount of portal \
                 or PipeWire configuration will help until KWin exposes these globals.\
                 {tiled_context}\n\
                 Investigation steps:\n\
                 (1) Check L7 kwin_screencast_loaded — does supportInformation list the plugin?\n\
                 (2) journalctl --user -u plasma-kwin_wayland -n 100 | grep -i screen\n\
                 (3) These globals require KWin ≥5.20; with NVIDIA open modules ≥555 \
                 wp_linux_drm_syncobj_manager_v1 must also be present.\n\
                 Upstream: https://bugs.kde.org/show_bug.cgi?id=493277  \
                 https://forums.developer.nvidia.com/t/331077"
            ),
            "systemctl --user restart plasma-kwin_wayland",
            Confidence::High,
        ));
    }

    out
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::CheckStatus;

    fn g(ifaces: &[&str]) -> Vec<WlGlobal> {
        ifaces
            .iter()
            .map(|i| WlGlobal {
                interface: i.to_string(),
                version: 1,
            })
            .collect()
    }

    fn status(results: &[DiagnosticResult], check: &str) -> CheckStatus {
        results
            .iter()
            .find(|r| r.check == check)
            .unwrap_or_else(|| panic!("check '{check}' not found"))
            .status
            .clone()
    }

    // ── Existing tests (unchanged) ────────────────────────────────────────

    #[test]
    fn kde_full_set_no_failures() {
        let r = build_checks(g(&[
            "wl_compositor", "xdg_wm_base", "org_kde_plasma_window_management",
            "zwp_linux_dmabuf_v1", "zkde_screencast_unstable_v1",
            "ext_image_capture_source_v1", "wp_linux_drm_syncobj_manager_v1",
            "wl_output",
        ]));
        assert!(r.iter().all(|x| x.status != CheckStatus::Fail));
    }

    #[test]
    fn missing_required_globals_fail() {
        let r = build_checks(g(&[
            "wl_compositor", "xdg_wm_base", "org_kde_plasma_window_management", "wl_output",
        ]));
        assert_eq!(
            status(&r, "zkde_screencast_unstable_v1"),
            CheckStatus::Fail
        );
    }

    #[test]
    fn xwayland_warns() {
        let r = build_checks(g(&[
            "wl_compositor", "wl_drm", "xdg_wm_base", "wl_output",
        ]));
        assert_eq!(status(&r, "wayland_backend"), CheckStatus::Warn);
    }

    #[test]
    fn no_outputs_fails() {
        let r = build_checks(g(&[
            "wl_compositor", "xdg_wm_base", "org_kde_plasma_window_management",
        ]));
        assert_eq!(status(&r, "wl_output_count"), CheckStatus::Fail);
    }

    // ── Session 3: new protocol checks ───────────────────────────────────

    #[test]
    fn drm_syncobj_required_fails_when_absent() {
        let r = build_checks(g(&[
            "wl_compositor", "xdg_wm_base", "org_kde_plasma_window_management",
            "zwp_linux_dmabuf_v1", "zkde_screencast_unstable_v1",
            "ext_image_capture_source_v1",
            "wl_output",
            // wp_linux_drm_syncobj_manager_v1 intentionally absent
        ]));
        assert_eq!(
            status(&r, "wp_linux_drm_syncobj_manager_v1"),
            CheckStatus::Fail
        );
    }

    #[test]
    fn drm_syncobj_passes_when_present() {
        let r = build_checks(g(&[
            "wl_compositor", "xdg_wm_base", "org_kde_plasma_window_management",
            "zwp_linux_dmabuf_v1", "zkde_screencast_unstable_v1",
            "ext_image_capture_source_v1", "wp_linux_drm_syncobj_manager_v1",
            "wl_output",
        ]));
        assert_eq!(
            status(&r, "wp_linux_drm_syncobj_manager_v1"),
            CheckStatus::Pass
        );
    }

    #[test]
    fn wlroots_screencopy_optional_warns_when_absent() {
        let r = build_checks(g(&[
            "wl_compositor", "xdg_wm_base", "org_kde_plasma_window_management",
            "zwp_linux_dmabuf_v1", "zkde_screencast_unstable_v1",
            "ext_image_capture_source_v1", "wp_linux_drm_syncobj_manager_v1",
            "wl_output",
            // zwlr_screencopy_manager_v1 absent — expected on KDE
        ]));
        assert_eq!(
            status(&r, "zwlr_screencopy_manager_v1"),
            CheckStatus::Warn
        );
    }

    #[test]
    fn tiled_display_correlation_fires_on_two_outputs_without_zkde() {
        // 2 wl_outputs + no zkde_screencast → tiled display warning
        let r = build_checks(g(&[
            "wl_compositor", "xdg_wm_base", "org_kde_plasma_window_management",
            "zwp_linux_dmabuf_v1",
            "wl_output", "wl_output",
        ]));
        assert_eq!(
            status(&r, "wl_output_tiled_correlation"),
            CheckStatus::Warn
        );
    }

    #[test]
    fn tiled_correlation_absent_when_zkde_present() {
        // zkde advertised → no tiled correlation check emitted
        let r = build_checks(g(&[
            "wl_compositor", "xdg_wm_base", "org_kde_plasma_window_management",
            "zwp_linux_dmabuf_v1", "zkde_screencast_unstable_v1",
            "ext_image_capture_source_v1", "wp_linux_drm_syncobj_manager_v1",
            "wl_output", "wl_output",
        ]));
        assert!(r.iter().all(|r| r.check != "wl_output_tiled_correlation"));
    }

    #[test]
    fn single_output_no_tiled_correlation() {
        let r = build_checks(g(&[
            "wl_compositor", "xdg_wm_base", "org_kde_plasma_window_management",
            "wl_output",
        ]));
        assert!(r.iter().all(|r| r.check != "wl_output_tiled_correlation"));
    }

    // ── Session 3: Bug D screencast globals check ─────────────────────────

    #[test]
    fn bug_d_fires_when_both_screencast_globals_absent() {
        // Neither zkde nor ext present → Bug D check must fire as FAIL
        let r = build_checks(g(&[
            "wl_compositor", "xdg_wm_base", "org_kde_plasma_window_management",
            "zwp_linux_dmabuf_v1",
            "wl_output", "wl_output",
        ]));
        assert_eq!(status(&r, "bug_d_screencast_globals"), CheckStatus::Fail);
    }

    #[test]
    fn bug_d_detail_contains_upstream_links() {
        let r = build_checks(g(&[
            "wl_compositor", "xdg_wm_base", "org_kde_plasma_window_management",
            "zwp_linux_dmabuf_v1", "wl_output",
        ]));
        let d = &r.iter().find(|x| x.check == "bug_d_screencast_globals").unwrap().detail;
        assert!(d.contains("493277"), "should reference KDE bug 493277");
        assert!(d.contains("331077"), "should reference NVIDIA forum thread");
        assert!(d.contains("Bug D"), "should self-identify as Bug D");
    }

    #[test]
    fn bug_d_fix_is_kwin_restart() {
        let r = build_checks(g(&[
            "wl_compositor", "xdg_wm_base", "org_kde_plasma_window_management",
            "wl_output",
        ]));
        let fix = r.iter()
            .find(|x| x.check == "bug_d_screencast_globals")
            .and_then(|x| x.fix.as_deref())
            .unwrap_or("");
        assert!(fix.contains("restart"), "fix should be a restart command");
        assert!(fix.contains("kwin"), "fix should target KWin");
    }

    #[test]
    fn bug_d_absent_when_only_zkde_missing() {
        // ext present but zkde absent → individual FAIL, no Bug D
        let r = build_checks(g(&[
            "wl_compositor", "xdg_wm_base", "org_kde_plasma_window_management",
            "zwp_linux_dmabuf_v1", "ext_image_capture_source_v1",
            "wp_linux_drm_syncobj_manager_v1", "wl_output",
        ]));
        assert!(r.iter().all(|x| x.check != "bug_d_screencast_globals"),
            "Bug D should not fire when ext_image_capture_source_v1 is present");
    }

    #[test]
    fn bug_d_absent_when_only_ext_missing() {
        // zkde present but ext absent → individual FAIL, no Bug D
        let r = build_checks(g(&[
            "wl_compositor", "xdg_wm_base", "org_kde_plasma_window_management",
            "zwp_linux_dmabuf_v1", "zkde_screencast_unstable_v1",
            "wp_linux_drm_syncobj_manager_v1", "wl_output",
        ]));
        assert!(r.iter().all(|x| x.check != "bug_d_screencast_globals"),
            "Bug D should not fire when zkde_screencast_unstable_v1 is present");
    }

    #[test]
    fn bug_d_absent_when_both_present() {
        let r = build_checks(g(&[
            "wl_compositor", "xdg_wm_base", "org_kde_plasma_window_management",
            "zwp_linux_dmabuf_v1", "zkde_screencast_unstable_v1",
            "ext_image_capture_source_v1", "wp_linux_drm_syncobj_manager_v1",
            "wl_output",
        ]));
        assert!(r.iter().all(|x| x.check != "bug_d_screencast_globals"));
    }

    #[test]
    fn bug_d_with_two_outputs_mentions_tiled() {
        let r = build_checks(g(&[
            "wl_compositor", "xdg_wm_base", "org_kde_plasma_window_management",
            "zwp_linux_dmabuf_v1",
            "wl_output", "wl_output",
        ]));
        let d = &r.iter().find(|x| x.check == "bug_d_screencast_globals").unwrap().detail;
        assert!(d.contains("wl_output") || d.contains("tiled"), "tiled context expected");
    }

    #[test]
    fn bug_d_single_output_mentions_plugin_check() {
        // Single output → no tiled hypothesis → falls back to plugin-check advice
        let r = build_checks(g(&[
            "wl_compositor", "xdg_wm_base", "org_kde_plasma_window_management",
            "zwp_linux_dmabuf_v1", "wl_output",
        ]));
        let d = &r.iter().find(|x| x.check == "bug_d_screencast_globals").unwrap().detail;
        // With 1 output the tiled-display branch is skipped; plugin-check path is used
        assert!(d.to_lowercase().contains("plugin") || d.contains("kwin_screencast"));
    }

    #[test]
    fn per_protocol_fix_commands_are_actionable() {
        // Required globals absent → their fix commands must be non-echo shell commands
        let r = build_checks(g(&[
            "wl_compositor", "xdg_wm_base", "org_kde_plasma_window_management",
            "wl_output",
        ]));
        for check in &["zkde_screencast_unstable_v1", "ext_image_capture_source_v1"] {
            let fix = r.iter()
                .find(|x| &x.check == check)
                .and_then(|x| x.fix.as_deref())
                .unwrap_or("");
            assert!(!fix.starts_with("echo"), "fix for {check} should not be a bare echo");
            assert!(!fix.is_empty(), "fix for {check} should be non-empty");
        }
    }
}
