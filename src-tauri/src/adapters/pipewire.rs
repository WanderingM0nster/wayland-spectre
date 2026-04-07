//! PipeWire adapter.
//!
//! Checks: PipeWire service health, screencast node presence.
//! Layer: L4 (PipeWire graph).
//!
//! Current implementation: subprocess via `pw-dump` (JSON graph dump).
//! TODO Session 2: replace with native libpipewire bindings via pw-sys or pipewire-rs.

use crate::domain::types::{CaptureTestResult, Confidence, DiagnosticResult, Layer};
use std::process::Command;

pub async fn check_pipewire() -> Vec<DiagnosticResult> {
    let mut results = Vec::new();

    results.extend(check_pipewire_running());
    results.extend(check_wireplumber_running());
    results.extend(check_pipewire_graph());

    results
}

// ── Service health ────────────────────────────────────────────────────────

fn check_pipewire_running() -> Vec<DiagnosticResult> {
    let status = systemctl_is_active("pipewire");
    match status.as_deref() {
        Some("active") => vec![DiagnosticResult::pass(
            Layer::L4,
            "pipewire_active",
            "pipewire.service is active",
        )],
        Some(s) => vec![DiagnosticResult::fail(
            Layer::L4,
            "pipewire_active",
            format!("pipewire.service is {s}"),
            "systemctl --user start pipewire",
            Confidence::High,
        )],
        None => vec![DiagnosticResult::skip(
            Layer::L4,
            "pipewire_active",
            "systemctl not available",
        )],
    }
}

fn check_wireplumber_running() -> Vec<DiagnosticResult> {
    let status = systemctl_is_active("wireplumber");
    match status.as_deref() {
        Some("active") => vec![DiagnosticResult::pass(
            Layer::L4,
            "wireplumber_active",
            "wireplumber.service is active",
        )],
        Some(s) => vec![DiagnosticResult::fail(
            Layer::L4,
            "wireplumber_active",
            format!("wireplumber.service is {s}"),
            "systemctl --user start wireplumber",
            Confidence::High,
        )],
        None => vec![DiagnosticResult::skip(
            Layer::L4,
            "wireplumber_active",
            "systemctl not available",
        )],
    }
}

// ── PipeWire graph ────────────────────────────────────────────────────────

fn check_pipewire_graph() -> Vec<DiagnosticResult> {
    let out = Command::new("pw-dump").output();

    let Ok(o) = out else {
        return vec![DiagnosticResult::skip(
            Layer::L4,
            "pipewire_graph",
            "pw-dump not available",
        )];
    };

    if !o.status.success() {
        return vec![DiagnosticResult::warn(
            Layer::L4,
            "pipewire_graph",
            "pw-dump failed — PipeWire may not be running or PIPEWIRE_REMOTE not set",
            Some("systemctl --user restart pipewire pipewire-pulse".into()),
            Confidence::Medium,
        )];
    }

    let graph: serde_json::Value = match serde_json::from_slice(&o.stdout) {
        Ok(v) => v,
        Err(_) => {
            return vec![DiagnosticResult::warn(
                Layer::L4,
                "pipewire_graph",
                "pw-dump output could not be parsed",
                None,
                Confidence::Low,
            )]
        }
    };

    let nodes = match graph.as_array() {
        Some(arr) => arr,
        None => return vec![],
    };

    // Look for screencast-related nodes
    let screencast_nodes: Vec<&serde_json::Value> = nodes
        .iter()
        .filter(|n| {
            let type_str = n["type"].as_str().unwrap_or("");
            let media_class = n["info"]["props"]["media.class"].as_str().unwrap_or("");
            let media_role = n["info"]["props"]["media.role"].as_str().unwrap_or("");
            type_str == "PipeWire:Interface:Node"
                && (media_class.contains("Video/Source")
                    || media_role.contains("Screen")
                    || media_role.contains("screencast"))
        })
        .collect();

    if screencast_nodes.is_empty() {
        vec![DiagnosticResult::pass(
            Layer::L4,
            "pipewire_graph",
            "PipeWire graph healthy — no stale screencast nodes (expected when idle)",
        )]
    } else {
        let count = screencast_nodes.len();
        vec![DiagnosticResult::pass(
            Layer::L4,
            "pipewire_graph",
            format!("{count} screencast-related PipeWire node(s) active"),
        )]
    }
}

// ── Capture test ──────────────────────────────────────────────────────────

/// End-to-end capture test: attempts to get a screencast node via pw-dump
/// and returns its metadata. Phase 2 will use a proper portal D-Bus call.
pub async fn run_capture_test() -> anyhow::Result<CaptureTestResult> {
    let out = Command::new("pw-dump")
        .output()
        .map_err(|e| anyhow::anyhow!("pw-dump not found: {e}"))?;

    if !out.status.success() {
        return Ok(CaptureTestResult {
            success: false,
            node_id: None,
            width: None,
            height: None,
            format: None,
            error: Some(
                "pw-dump failed — PipeWire not running or PIPEWIRE_REMOTE not set".into(),
            ),
        });
    }

    let graph: serde_json::Value = serde_json::from_slice(&out.stdout)
        .map_err(|e| anyhow::anyhow!("pw-dump parse error: {e}"))?;

    let nodes = graph.as_array().cloned().unwrap_or_default();

    // Find a Video/Source node — the most likely screencast candidate
    let candidate = nodes.iter().find(|n| {
        n["info"]["props"]["media.class"]
            .as_str()
            .map(|c| c.contains("Video/Source"))
            .unwrap_or(false)
    });

    match candidate {
        Some(node) => {
            let id = node["id"].as_u64().map(|n| n as u32);
            // Try to extract format from the params
            let format = node["info"]["params"]["EnumFormat"]
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|f| f["mediaSubtype"].as_str().map(|s| s.to_string()))
                .or_else(|| {
                    node["info"]["props"]["video.format"]
                        .as_str()
                        .map(|s| s.to_string())
                });

            // Note: this is a PASSIVE check — we found an existing Video/Source node
            // in the PipeWire graph. It does NOT test the full portal path (which is
            // what's broken when zkde_screencast_unstable_v1 is absent).
            // Session 2 will replace this with an active portal CreateSession call.
            Ok(CaptureTestResult {
                success: true,
                node_id: id,
                width: node["info"]["props"]["video.width"]
                    .as_u64()
                    .map(|n| n as u32),
                height: node["info"]["props"]["video.height"]
                    .as_u64()
                    .map(|n| n as u32),
                format,
                error: None,
            })
        }
        None => Ok(CaptureTestResult {
            success: false,
            node_id: None,
            width: None,
            height: None,
            format: None,
            error: Some(
                "No Video/Source PipeWire node found — PipeWire graph is healthy but \
                 no active screencast session exists. This is expected when screen sharing \
                 is not currently active. The real test is whether zkde_screencast_unstable_v1 \
                 appears in the Wayland registry (see L3)."
                    .into(),
            ),
        }),
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn systemctl_is_active(unit: &str) -> Option<String> {
    Command::new("systemctl")
        .args(["--user", "is-active", unit])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}
