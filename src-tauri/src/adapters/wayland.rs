// SPDX-License-Identifier: GPL-3.0-or-later
use wayland_client::{protocol::wl_registry, Connection, Dispatch, QueueHandle};
use crate::domain::types::{Confidence, DiagnosticResult, Layer};

#[derive(Debug, Clone)]
struct WlGlobal { interface: String, version: u32 }

struct RegistryCollector { globals: Vec<WlGlobal> }

impl Dispatch<wl_registry::WlRegistry, ()> for RegistryCollector {
    fn event(state: &mut Self, _: &wl_registry::WlRegistry, event: wl_registry::Event,
             _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let wl_registry::Event::Global { interface, version, .. } = event {
            state.globals.push(WlGlobal { interface, version });
        }
    }
}

pub async fn check_wayland_protocols() -> Vec<DiagnosticResult> {
    match connect_and_enumerate() {
        Err(e) => vec![DiagnosticResult::fail(Layer::L2, "wayland_connect",
            format!("Cannot connect to $WAYLAND_DISPLAY: {e}"),
            "echo 'Check WAYLAND_DISPLAY is set'", Confidence::High)],
        Ok(globals) => build_checks(globals),
    }
}

fn connect_and_enumerate() -> Result<Vec<WlGlobal>, Box<dyn std::error::Error + Send + Sync>> {
    let conn = Connection::connect_to_env()?;
    let mut queue = conn.new_event_queue::<RegistryCollector>();
    let qh = queue.handle();
    conn.display().get_registry(&qh, ());
    let mut state = RegistryCollector { globals: Vec::new() };
    queue.roundtrip(&mut state)?;
    Ok(state.globals)
}

const SCREENCAST_GLOBALS: &[(&str, &str, bool)] = &[
    ("zwp_linux_dmabuf_v1",         "Linux DMA-BUF",            true),
    ("zkde_screencast_unstable_v1", "KDE ScreenCast protocol",  true),
    ("ext_image_capture_source_v1", "ext-image-capture-source", true),
    ("wp_viewporter",               "wp-viewporter",            false),
    ("wp_presentation",             "wp-presentation",          false),
];

fn build_checks(globals: Vec<WlGlobal>) -> Vec<DiagnosticResult> {
    let mut out = Vec::new();
    let has_kde = globals.iter().any(|g| g.interface.starts_with("org_kde_") || g.interface.starts_with("kde_"));
    let has_xdg = globals.iter().any(|g| g.interface == "xdg_wm_base");
    let has_drm = globals.iter().any(|g| g.interface == "wl_drm");

    if has_kde && has_xdg {
        out.push(DiagnosticResult::pass(Layer::L2, "wayland_backend",
            format!("KWin/Wayland confirmed — {} globals, wl_drm={}", globals.len(), has_drm)));
    } else if has_drm && !has_kde {
        out.push(DiagnosticResult::warn(Layer::L2, "wayland_backend",
            "wl_drm present without KDE globals — possible XWayland compositor",
            None, Confidence::Medium));
    } else {
        out.push(DiagnosticResult::warn(Layer::L2, "wayland_backend",
            format!("Compositor type unclear — {} globals", globals.len()),
            None, Confidence::Low));
    }

    let n_outputs = globals.iter().filter(|g| g.interface == "wl_output").count();
    if n_outputs > 0 {
        out.push(DiagnosticResult::pass(Layer::L3, "wl_output_count",
            format!("{n_outputs} output(s) advertised")));
    } else {
        out.push(DiagnosticResult::fail(Layer::L3, "wl_output_count",
            "No wl_output globals", "echo 'Check display connection'", Confidence::High));
    }

    for (iface, label, required) in SCREENCAST_GLOBALS {
        match globals.iter().find(|g| g.interface == *iface) {
            Some(g) => out.push(DiagnosticResult::pass(Layer::L3, *iface,
                format!("{label} advertised at version {}", g.version))),
            None if *required => out.push(DiagnosticResult::fail(Layer::L3, *iface,
                format!("{iface} not advertised by compositor"),
                "echo 'Update KDE Plasma / KWin'", Confidence::High)),
            None => out.push(DiagnosticResult::warn(Layer::L3, *iface,
                format!("{iface} not advertised (optional)"), None, Confidence::Low)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::CheckStatus;
    fn g(ifaces: &[&str]) -> Vec<WlGlobal> {
        ifaces.iter().map(|i| WlGlobal { interface: i.to_string(), version: 1 }).collect()
    }
    #[test]
    fn kde_full_set_no_failures() {
        let r = build_checks(g(&["wl_compositor","xdg_wm_base","org_kde_plasma_window_management",
            "zwp_linux_dmabuf_v1","zkde_screencast_unstable_v1","ext_image_capture_source_v1","wl_output"]));
        assert!(r.iter().all(|x| x.status != CheckStatus::Fail));
    }
    #[test]
    fn missing_required_globals_fail() {
        let r = build_checks(g(&["wl_compositor","xdg_wm_base","org_kde_plasma_window_management","wl_output"]));
        assert_eq!(r.iter().find(|x| x.check == "zkde_screencast_unstable_v1").unwrap().status, CheckStatus::Fail);
    }
    #[test]
    fn xwayland_warns() {
        let r = build_checks(g(&["wl_compositor","wl_drm","xdg_wm_base","wl_output"]));
        assert_eq!(r.iter().find(|x| x.check == "wayland_backend").unwrap().status, CheckStatus::Warn);
    }
    #[test]
    fn no_outputs_fails() {
        let r = build_checks(g(&["wl_compositor","xdg_wm_base","org_kde_plasma_window_management"]));
        assert_eq!(r.iter().find(|x| x.check == "wl_output_count").unwrap().status, CheckStatus::Fail);
    }
}
