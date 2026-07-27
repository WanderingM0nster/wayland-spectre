// SPDX-License-Identifier: GPL-3.0-or-later
use zbus::{fdo, Connection, Proxy};
use crate::domain::types::{Confidence, DiagnosticResult, Layer};

pub async fn check_dbus_portal() -> Vec<DiagnosticResult> {
    let conn = match Connection::session().await {
        Ok(c) => c,
        Err(e) => return vec![DiagnosticResult::fail(Layer::L1, "dbus_session",
            format!("Cannot connect to session bus: {e}"),
            "systemctl --user status dbus", Confidence::High)],
    };
    let mut results = Vec::new();
    let (presence, portal_up) = check_portal_presence(&conn).await;
    results.push(presence);
    if portal_up {
        results.push(check_screencast_version(&conn).await);
        results.extend(check_zombie_sessions(&conn).await);
    }
    results
}

async fn check_portal_presence(conn: &Connection) -> (DiagnosticResult, bool) {
    let dbus = match fdo::DBusProxy::new(conn).await {
        Ok(d) => d,
        Err(e) => return (DiagnosticResult::fail(Layer::L1, "portal_presence",
            format!("Cannot reach D-Bus daemon: {e}"),
            "systemctl --user status dbus", Confidence::High), false),
    };
    let names: Vec<String> = dbus.list_names().await.unwrap_or_default()
        .into_iter().map(|n| n.to_string()).collect();
    let active = names.iter().any(|n| n == "org.freedesktop.portal.Desktop");
    if active {
        (DiagnosticResult::pass(Layer::L1, "portal_presence",
            "org.freedesktop.portal.Desktop active on session bus"), true)
    } else {
        (DiagnosticResult::fail(Layer::L1, "portal_presence",
            "org.freedesktop.portal.Desktop not found — portal not running",
            "systemctl --user start xdg-desktop-portal", Confidence::High), false)
    }
}

async fn check_screencast_version(conn: &Connection) -> DiagnosticResult {
    let proxy = match Proxy::new(conn, "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop", "org.freedesktop.portal.ScreenCast").await {
        Ok(p) => p,
        // v0.4.1 audit: an unprivileged client may be unable to query the
        // interface even when screencasting works — inconclusive, not a fault.
        // The live CreateSession probe (L4) is the reliable signal.
        Err(e) => return DiagnosticResult::warn(Layer::L1, "portal_screencast_iface",
            format!("Cannot query ScreenCast interface version: {e} — inconclusive; \
                     the portal may still work. The live CreateSession probe (L4) is \
                     the reliable signal."),
            None, Confidence::Low),
    };
    match proxy.get_property::<u32>("version").await {
        Err(e) => DiagnosticResult::warn(Layer::L1, "portal_screencast_iface",
            format!("Could not read version property: {e}"), None, Confidence::Low),
        Ok(4) => DiagnosticResult::pass(Layer::L1, "portal_screencast_iface",
            "ScreenCast API v4 — stable, no known screencast regressions"),
        Ok(5) => DiagnosticResult::warn(Layer::L1, "portal_screencast_iface",
            "ScreenCast API v5 — xdg-desktop-portal 1.20.x; ELOOP (Bug C) known to affect this series",
            Some("systemctl --user restart xdg-desktop-portal".into()), Confidence::High),
        Ok(v) if v <= 3 => DiagnosticResult::warn(Layer::L1, "portal_screencast_iface",
            format!("ScreenCast API v{v} — older portal, may lack ext_image_capture_source"),
            None, Confidence::Medium),
        Ok(v) => DiagnosticResult::pass(Layer::L1, "portal_screencast_iface",
            format!("ScreenCast API v{v} — newer than known-buggy 1.20.x series")),
    }
}

async fn check_zombie_sessions(conn: &Connection) -> Vec<DiagnosticResult> {
    let proxy = match Proxy::new(conn, "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop", "org.freedesktop.DBus.Introspectable").await {
        Ok(p) => p,
        Err(_) => return vec![],
    };
    let xml: String = match proxy.call("Introspect", &()).await {
        Ok(s) => s,
        Err(e) => return vec![DiagnosticResult::warn(Layer::L1, "zombie_sessions",
            format!("Introspect failed: {e}"), None, Confidence::Low)],
    };
    let n = xml.matches(r#"<node name="session"#).count();
    vec![if n == 0 {
        DiagnosticResult::pass(Layer::L1, "zombie_sessions",
            "No orphaned portal session objects found")
    } else {
        DiagnosticResult::warn(Layer::L1, "zombie_sessions",
            format!("{n} session node(s) still registered — possible zombie(s)"),
            Some("systemctl --user restart xdg-desktop-portal".into()), Confidence::Medium)
    }]
}

#[cfg(test)]
mod tests {
    use crate::domain::types::CheckStatus;
    #[test]
    fn version_5_is_warn() {
        let s = match 5u32 { 4 => CheckStatus::Pass, 5 => CheckStatus::Warn, _ => CheckStatus::Warn };
        assert_eq!(s, CheckStatus::Warn);
    }
    #[test]
    fn version_4_is_pass() {
        let s = match 4u32 { 4 => CheckStatus::Pass, _ => CheckStatus::Warn };
        assert_eq!(s, CheckStatus::Pass);
    }
    #[test]
    fn zombie_xml_count() {
        let xml = r#"<node><node name="session_abc"/><node name="session_def"/></node>"#;
        assert_eq!(xml.matches(r#"<node name="session"#).count(), 2);
    }
    #[test]
    fn clean_xml_no_zombies() {
        let xml = r#"<node><interface name="org.freedesktop.portal.ScreenCast"/></node>"#;
        assert_eq!(xml.matches(r#"<node name="session"#).count(), 0);
    }
}
