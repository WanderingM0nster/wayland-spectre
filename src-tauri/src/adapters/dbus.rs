//! D-Bus adapter.
//!
//! Checks: F6 (zombie D-Bus portal session from previous unclean exit),
//!         portal service health, xdg-desktop-portal-kde registration.
//! Layer: L1 (D-Bus / portal session).
//!
//! Current implementation: subprocess via `busctl`.
//! TODO Session 2: replace with native zbus::Connection::session() probing.

use crate::domain::types::{Confidence, DiagnosticResult, Layer};
use std::process::Command;

pub async fn check_dbus_portal() -> Vec<DiagnosticResult> {
    let mut results = Vec::new();

    results.extend(check_portal_service_running());
    results.extend(check_kde_portal_running());
    results.extend(check_zombie_sessions());
    results.extend(check_portal_version());

    results
}

// ── Portal service health ─────────────────────────────────────────────────

fn check_portal_service_running() -> Vec<DiagnosticResult> {
    let status = systemctl_is_active("xdg-desktop-portal");
    match status.as_deref() {
        Some("active") => vec![DiagnosticResult::pass(
            Layer::L1,
            "xdg_desktop_portal_active",
            "xdg-desktop-portal.service is active",
        )],
        Some(s) => vec![DiagnosticResult::fail(
            Layer::L1,
            "xdg_desktop_portal_active",
            format!("xdg-desktop-portal.service is {s} — portal stack is down"),
            "systemctl --user restart xdg-desktop-portal",
            Confidence::High,
        )],
        None => vec![DiagnosticResult::skip(
            Layer::L1,
            "xdg_desktop_portal_active",
            "systemctl not available",
        )],
    }
}

fn check_kde_portal_running() -> Vec<DiagnosticResult> {
    let status = systemctl_is_active("plasma-xdg-desktop-portal-kde");
    match status.as_deref() {
        Some("active") => vec![DiagnosticResult::pass(
            Layer::L1,
            "kde_portal_active",
            "plasma-xdg-desktop-portal-kde.service is active",
        )],
        Some(s) => vec![DiagnosticResult::fail(
            Layer::L1,
            "kde_portal_active",
            format!("plasma-xdg-desktop-portal-kde.service is {s}"),
            "systemctl --user restart plasma-xdg-desktop-portal-kde",
            Confidence::High,
        )],
        None => vec![DiagnosticResult::skip(
            Layer::L1,
            "kde_portal_active",
            "systemctl not available",
        )],
    }
}

// ── F6: Zombie session check ──────────────────────────────────────────────

fn check_zombie_sessions() -> Vec<DiagnosticResult> {
    // A healthy KDE session has many portal-related bus names — the raw count
    // is not a reliable indicator (9+ is normal). Instead we look for specific
    // anomalies: duplicate primary service names, or extreme transient counts.
    let out = Command::new("busctl")
        .args(["--user", "list", "--no-pager"])
        .output();

    let Ok(o) = out else {
        return vec![DiagnosticResult::skip(
            Layer::L1,
            "zombie_portal_sessions",
            "busctl not available",
        )];
    };

    let names_output = String::from_utf8_lossy(&o.stdout);
    let issues = detect_zombie_sessions(&names_output);

    if issues.is_empty() {
        vec![DiagnosticResult::pass(
            Layer::L1,
            "zombie_portal_sessions",
            "Portal bus names look healthy — no duplicate services or session leaks",
        )]
    } else {
        vec![DiagnosticResult::warn(
            Layer::L1,
            "zombie_portal_sessions",
            format!(
                "Possible stale portal state: {}. \
                 If screen sharing fails after other fixes, restart the portal stack.",
                issues.join("; ")
            ),
            Some("systemctl --user restart xdg-desktop-portal plasma-xdg-desktop-portal-kde".to_string()),
            Confidence::Medium,
        )]
    }
}

/// Pure function: analyse busctl list output for zombie indicators.
/// Returns a list of human-readable issue descriptions (empty = healthy).
/// Extracted for unit testing without subprocess dependency.
pub(crate) fn detect_zombie_sessions(busctl_output: &str) -> Vec<String> {
    let mut issues = Vec::new();

    // Count lines claiming the primary portal service names.
    // busctl list shows one row per D-Bus name; duplicates = stale instances.
    let desktop_portal_owners = busctl_output
        .lines()
        .filter(|l| {
            let trimmed = l.trim_start();
            trimmed.starts_with("org.freedesktop.portal.Desktop")
                && !trimmed.starts_with("org.freedesktop.portal.Desktop.")
        })
        .count();

    let kde_portal_owners = busctl_output
        .lines()
        .filter(|l| {
            let trimmed = l.trim_start();
            trimmed.starts_with("org.kde.xdg-desktop-portal-kde")
                || trimmed.starts_with("org.kde.portal.KDE")
        })
        .count();

    // Only flag transient names if extremely high — normal KDE sessions
    // have 20–80; >150 suggests accumulated session leaks.
    let transient_count = busctl_output
        .lines()
        .filter(|l| l.trim_start().starts_with(':'))
        .count();

    if desktop_portal_owners > 1 {
        issues.push(format!(
            "org.freedesktop.portal.Desktop has {desktop_portal_owners} owners (expected 1)"
        ));
    }
    if kde_portal_owners > 1 {
        issues.push(format!(
            "KDE portal service has {kde_portal_owners} owners (expected 1)"
        ));
    }
    if transient_count > 150 {
        issues.push(format!(
            "{transient_count} transient bus names — unusually high, possible session leak"
        ));
    }

    issues
}

// ── Portal version ────────────────────────────────────────────────────────

fn check_portal_version() -> Vec<DiagnosticResult> {
    // Bug C: xdg-desktop-portal 1.20.0–1.20.3 opens /proc/<pid>/root with
    // O_RDONLY|O_NOFOLLOW. That path is a kernel magic symlink; O_NOFOLLOW
    // always returns ELOOP. KWin screencast registration fails on every boot.
    // Fix: O_PATH (upstream PR merged, targeting 1.20.4+).
    //
    // IMPORTANT: versions BEFORE 1.20 (e.g. 1.18.x) do NOT have this bug.
    // They use a different code path. Only flag 1.20.0–1.20.3.
    let version = get_portal_version();
    match version {
        None => vec![DiagnosticResult::skip(
            Layer::L1,
            "portal_version",
            "Could not determine xdg-desktop-portal version",
        )],
        Some(v) => {
            if is_portal_version_buggy(&v) {
                vec![DiagnosticResult::warn(
                    Layer::L1,
                    "portal_version",
                    format!(
                        "xdg-desktop-portal {v} has Bug C: O_RDONLY|O_NOFOLLOW on \
                         /proc/<pid>/root causes ELOOP. KWin screencast registration \
                         may fail on every boot. \
                         Upstream fix: github.com/flatpak/xdg-desktop-portal/issues/1953"
                    ),
                    Some("rpm-ostree upgrade".to_string()),
                    Confidence::High,
                )]
            } else {
                vec![DiagnosticResult::pass(
                    Layer::L1,
                    "portal_version",
                    format!("xdg-desktop-portal {v} — not in Bug C range (1.20.0–1.20.3)"),
                )]
            }
        }
    }
}

/// Pure function: returns true iff this version string is in the Bug C range.
/// Extracted for unit testing.
pub(crate) fn is_portal_version_buggy(version: &str) -> bool {
    // Strip rpm epoch prefix ("2:1.20.3" → "1.20.3")
    let after_epoch = version.split(':').last().unwrap_or(version);
    // Strip distro release suffix ("1.20.3-3.fc43" → "1.20.3")
    let clean = after_epoch.split('-').next().unwrap_or(after_epoch);

    let parts: Vec<u32> = clean
        .split('.')
        .filter_map(|p| p.parse().ok())
        .collect();

    // Bug C is ONLY in 1.20.0, 1.20.1, 1.20.2, 1.20.3
    matches!(parts.as_slice(), [1, 20, patch, ..] if *patch <= 3)
}

fn get_portal_version() -> Option<String> {
    // Try rpm first (Bazzite/Fedora)
    if let Ok(o) = Command::new("rpm")
        .args(["-q", "--queryformat", "%{VERSION}", "xdg-desktop-portal"])
        .output()
    {
        if o.status.success() {
            let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !v.is_empty() && !v.contains("not installed") {
                return Some(v);
            }
        }
    }
    // Try dpkg (Ubuntu/Debian)
    if let Ok(o) = Command::new("dpkg-query")
        .args(["-W", "-f=${Version}", "xdg-desktop-portal"])
        .output()
    {
        if o.status.success() {
            let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn systemctl_is_active(unit: &str) -> Option<String> {
    Command::new("systemctl")
        .args(["--user", "is-active", unit])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Portal version ────────────────────────────────────────────────────

    #[test]
    fn buggy_versions_flagged() {
        for v in &["1.20.0", "1.20.1", "1.20.2", "1.20.3"] {
            assert!(is_portal_version_buggy(v), "{v} should be flagged as buggy");
        }
    }

    #[test]
    fn fixed_versions_clear() {
        for v in &["1.20.4", "1.20.5", "1.22.0", "1.22.1", "2.0.0"] {
            assert!(!is_portal_version_buggy(v), "{v} should be safe");
        }
    }

    #[test]
    fn pre_1_20_series_not_buggy() {
        // 1.18.x used a different code path — Bug C does not exist there
        for v in &["1.18.0", "1.18.4", "1.16.0", "1.14.3"] {
            assert!(!is_portal_version_buggy(v), "{v} is pre-1.20 and should be safe");
        }
    }

    #[test]
    fn rpm_epoch_prefix_stripped() {
        assert!(is_portal_version_buggy("2:1.20.3"));
        assert!(!is_portal_version_buggy("2:1.20.4"));
    }

    #[test]
    fn fedora_release_suffix_stripped() {
        assert!(is_portal_version_buggy("1.20.3-3.fc43"));
        assert!(!is_portal_version_buggy("1.20.4-1.fc43"));
    }

    #[test]
    fn garbage_version_not_buggy() {
        assert!(!is_portal_version_buggy(""));
        assert!(!is_portal_version_buggy("unknown"));
        assert!(!is_portal_version_buggy("not installed"));
    }

    // ── Zombie session detection ──────────────────────────────────────────

    #[test]
    fn healthy_session_no_issues() {
        // Realistic KDE session — many names, all healthy
        let output = "\
org.freedesktop.DBus                     - -     - - -
org.freedesktop.portal.Desktop           - -     - - -
org.freedesktop.portal.Documents         - -     - - -
org.freedesktop.portal.Flatpak           - -     - - -
org.freedesktop.portal.FileChooser       - -     - - -
org.freedesktop.portal.OpenURI           - -     - - -
org.kde.KWin                             - -     - - -
org.kde.xdg-desktop-portal-kde          - -     - - -
org.freedesktop.portal.Desktop.kde       - -     - - -
:1.0   0  - - - -
:1.1   1  - - - -
:1.2   2  - - - -
:1.3   3  - - - -
:1.10  10 - - - -
";
        let issues = detect_zombie_sessions(output);
        assert!(issues.is_empty(), "healthy session raised false alarm: {issues:?}");
    }

    #[test]
    fn duplicate_portal_desktop_flagged() {
        let output = "\
org.freedesktop.portal.Desktop   - - - - -
org.freedesktop.portal.Desktop   - - - - -
:1.0  0 - - - -
";
        let issues = detect_zombie_sessions(output);
        assert!(!issues.is_empty(), "duplicate portal.Desktop should be flagged");
        assert!(issues.iter().any(|i| i.contains("portal.Desktop")));
    }

    #[test]
    fn subdomain_portal_names_not_double_counted() {
        // org.freedesktop.portal.Desktop.kde must NOT count as a second
        // org.freedesktop.portal.Desktop owner
        let output = "\
org.freedesktop.portal.Desktop       - - - - -
org.freedesktop.portal.Desktop.kde   - - - - -
";
        let issues = detect_zombie_sessions(output);
        assert!(issues.is_empty(), "subdomain name caused false positive: {issues:?}");
    }

    #[test]
    fn extreme_transient_count_flagged() {
        let mut output = String::from("org.freedesktop.portal.Desktop   - - - - -\n");
        for i in 0..200 {
            output.push_str(&format!(":1.{i}  {i} - - - -\n"));
        }
        let issues = detect_zombie_sessions(&output);
        assert!(issues.iter().any(|i| i.contains("transient")),
            "200 transient names should be flagged");
    }

    #[test]
    fn normal_transient_count_ok() {
        let mut output = String::from("org.freedesktop.portal.Desktop   - - - - -\n");
        for i in 0..80 {
            output.push_str(&format!(":1.{i}  {i} - - - -\n"));
        }
        let issues = detect_zombie_sessions(&output);
        assert!(issues.is_empty(), "80 transient names raised false alarm: {issues:?}");
    }
}
