//! KWin adapter.
//!
//! Checks: F7 (KWin screencast plugin disabled in kwinrc).
//! Layer: L7 (KWin plugins).
//!
//! All checks are native — reads kwinrc directly.
//! Also verifies KWin is actually running as the Wayland compositor.

use crate::domain::types::{Confidence, DiagnosticResult, Layer};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub async fn check_kwin_plugins() -> Vec<DiagnosticResult> {
    let mut results = Vec::new();

    results.extend(check_kwin_running());
    results.extend(check_screencast_plugin());
    results.extend(check_kwin_support_info());

    results
}

// ── KWin running ──────────────────────────────────────────────────────────

fn check_kwin_running() -> Vec<DiagnosticResult> {
    let out = Command::new("busctl")
        .args([
            "--user",
            "call",
            "org.kde.KWin",
            "/KWin",
            "org.kde.KWin",
            "supportInformation",
        ])
        .output();

    match out {
        Err(_) => vec![DiagnosticResult::skip(
            Layer::L7,
            "kwin_running",
            "busctl not available — cannot query KWin",
        )],
        Ok(o) if !o.status.success() => {
            let err = String::from_utf8_lossy(&o.stderr);
            vec![DiagnosticResult::fail(
                Layer::L7,
                "kwin_running",
                format!("KWin not responding on D-Bus: {err}"),
                "systemctl --user restart plasma-kwin_wayland",
                Confidence::High,
            )]
        }
        Ok(_) => vec![DiagnosticResult::pass(
            Layer::L7,
            "kwin_running",
            "KWin responding on D-Bus",
        )],
    }
}

// ── Screencast plugin ─────────────────────────────────────────────────────

fn check_screencast_plugin() -> Vec<DiagnosticResult> {
    let kwinrc = read_kwinrc();

    let Some(content) = kwinrc else {
        return vec![DiagnosticResult::skip(
            Layer::L7,
            "kwin_screencast_plugin",
            "kwinrc not found",
        )];
    };

    // Parse the [Plugins] section for screencaslPlugin=false
    // (note: KDE uses the typo "screencasl" in older versions, "screencast" in newer)
    let plugin_disabled = content.lines().any(|l| {
        (l.starts_with("screencaslPlugin=") || l.starts_with("screencastPlugin="))
            && l.ends_with("false")
    });

    if plugin_disabled {
        vec![DiagnosticResult::fail(
            Layer::L7,
            "kwin_screencast_plugin",
            "KWin screencast plugin is disabled in kwinrc — all screen sharing will fail",
            "busctl --user call org.kde.KWin /Plugins org.kde.KWin.Plugins loadPlugin 's' screencast",
            Confidence::High,
        )]
    } else {
        vec![DiagnosticResult::pass(
            Layer::L7,
            "kwin_screencast_plugin",
            "KWin screencast plugin not explicitly disabled in kwinrc",
        )]
    }
}

// ── KWin support info — extract compositor/OpenGL context ────────────────

fn check_kwin_support_info() -> Vec<DiagnosticResult> {
    let out = Command::new("busctl")
        .args([
            "--user",
            "call",
            "org.kde.KWin",
            "/KWin",
            "org.kde.KWin",
            "supportInformation",
        ])
        .output();

    let Ok(o) = out else {
        return vec![];
    };
    if !o.status.success() {
        return vec![];
    }

    let info = String::from_utf8_lossy(&o.stdout);

    // Check for EGL vs GLX context (EGL required for Wayland DMA-BUF)
    let has_egl = info.contains("EGL") && !info.contains("GLX");
    let compositor_type = if info.contains("Wayland") {
        "Wayland"
    } else if info.contains("X11") {
        "X11"
    } else {
        "Unknown"
    };

    let mut results = Vec::new();

    if compositor_type == "Wayland" {
        results.push(DiagnosticResult::pass(
            Layer::L7,
            "kwin_compositor_wayland",
            "KWin is running as Wayland compositor",
        ));
    } else {
        results.push(DiagnosticResult::warn(
            Layer::L7,
            "kwin_compositor_wayland",
            format!("KWin compositor type: {compositor_type} — expected Wayland"),
            None,
            Confidence::Medium,
        ));
    }

    if has_egl {
        results.push(DiagnosticResult::pass(
            Layer::L7,
            "kwin_egl_context",
            "KWin using EGL context (required for DMA-BUF screen capture)",
        ));
    }

    results
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn kwinrc_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    PathBuf::from(home).join(".config/kwinrc")
}

fn read_kwinrc() -> Option<String> {
    fs::read_to_string(kwinrc_path()).ok()
}
