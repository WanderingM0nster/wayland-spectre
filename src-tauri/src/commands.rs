//! Tauri command handlers.
//! Each command is thin — it delegates to adapters and domain logic,
//! then serialises the result for the Svelte frontend.

use crate::adapters;
use crate::domain::types::{CaptureTestResult, DiagnosticReport, DiagnosticSummary, SystemInfo, CheckStatus};
use chrono::Utc;
use std::process::Command;

/// Run all diagnostic checks and return the full report.
/// This is the primary command — the Svelte store calls this on mount
/// and after every fix.
#[tauri::command]
pub async fn run_diagnostics() -> Result<DiagnosticReport, String> {
    let system = gather_system_info();

    // Run all adapters concurrently via tokio::join!
    let (wayland_results, dbus_results, pipewire_results, flatpak_results, nvidia_results, env_results, kwin_results) =
        tokio::join!(
            adapters::wayland::check_wayland_protocols(),
            adapters::dbus::check_dbus_portal(),
            adapters::pipewire::check_pipewire(),
            adapters::flatpak::check_flatpak_permissions(),
            adapters::nvidia::check_nvidia(),
            adapters::env::check_environment(),
            adapters::kwin::check_kwin_plugins(),
        );

    let mut results = Vec::new();
    results.extend(nvidia_results);
    results.extend(dbus_results);
    results.extend(wayland_results); // includes portal backend check
    results.extend(pipewire_results);
    results.extend(flatpak_results);
    results.extend(env_results);
    results.extend(kwin_results);

    let summary = DiagnosticSummary {
        pass: results.iter().filter(|r| r.status == CheckStatus::Pass).count(),
        warn: results.iter().filter(|r| r.status == CheckStatus::Warn).count(),
        fail: results.iter().filter(|r| r.status == CheckStatus::Fail).count(),
    };

    // Write JSON report to /tmp for continuity with the bash script
    let epoch = Utc::now().timestamp();
    let report = DiagnosticReport {
        schema_version: "1".into(),
        system,
        results,
        summary,
    };
    let json_path = format!("/tmp/screenshare-diag-{epoch}.json");
    if let Ok(json) = serde_json::to_string_pretty(&report) {
        let _ = std::fs::write(&json_path, json);
    }

    Ok(report)
}

/// Execute a fix command (always a safe, non-destructive systemctl restart
/// or similar). Returns the combined stdout+stderr for display in the UI.
///
/// Fix commands come from the `fix` field of DiagnosticResult —
/// they are authored in the adapter files and should never be destructive.
#[tauri::command]
pub async fn execute_fix(fix_command: String) -> Result<String, String> {
    // Safety: refuse anything with pipes, redirects, or semicolons.
    // All legitimate fix commands are single-word systemctl/busctl calls.
    if fix_command.contains(['|', ';', '&', '>', '<', '$', '`']) {
        return Err(format!("Refused unsafe fix command: {fix_command}"));
    }

    let parts: Vec<&str> = fix_command.split_whitespace().collect();
    if parts.is_empty() {
        return Err("Empty fix command".into());
    }

    let output = Command::new(parts[0])
        .args(&parts[1..])
        .output()
        .map_err(|e| format!("Failed to run fix: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}").trim().to_string();

    if output.status.success() {
        Ok(if combined.is_empty() { "Done.".into() } else { combined })
    } else {
        Err(format!("Command exited {}: {combined}", output.status))
    }
}

/// Attempt an end-to-end PipeWire screencast capture via pw-dump.
/// Phase 2 (Session 2) will replace this with a native PipeWire node request.
#[tauri::command]
pub async fn run_capture_test() -> Result<CaptureTestResult, String> {
    adapters::pipewire::run_capture_test()
        .await
        .map_err(|e| e.to_string())
}

/// Bundle a bug report: JSON diagnostics + recent journal excerpts + system info.
/// Writes to /tmp/wayland-spectre-bugreport-<epoch>.tar.gz and returns the path.
#[tauri::command]
pub async fn generate_bug_report() -> Result<String, String> {
    let epoch = Utc::now().timestamp();
    let dir = format!("/tmp/wayland-spectre-bugreport-{epoch}");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    // 1. Run fresh diagnostics and save JSON
    let report = run_diagnostics().await?;
    let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    std::fs::write(format!("{dir}/diagnostics.json"), &json).map_err(|e| e.to_string())?;

    // 2. Human-readable SUMMARY.txt — failures and warnings only, for easy
    //    copy-paste into bug tracker comments (NVIDIA forum, KDE Bugzilla)
    let summary_text = build_summary_text(&report);
    std::fs::write(format!("{dir}/SUMMARY.txt"), summary_text).map_err(|e| e.to_string())?;

    // 3. Capture relevant journal units
    for unit in &[
        "xdg-desktop-portal",
        "xdg-desktop-portal-kde",
        "plasma-kwin_wayland",
        "pipewire",
    ] {
        let out = Command::new("journalctl")
            .args(["--user", "-u", unit, "-n", "200", "--no-pager"])
            .output();
        if let Ok(o) = out {
            let _ = std::fs::write(format!("{dir}/journal-{unit}.log"), o.stdout);
        }
    }

    // 4. KWin supportInformation — the primary source for L7 checks.
    //    busctl prints the response as:  s "actual content here"
    //    We strip the leading `s "` and trailing `"` for readability.
    if let Ok(o) = Command::new("busctl")
        .args([
            "--user", "call",
            "org.kde.KWin", "/KWin", "org.kde.KWin",
            "supportInformation",
        ])
        .output()
    {
        let raw = String::from_utf8_lossy(&o.stdout);
        // busctl wraps the string: `s "…"` — unwrap it if present
        let content = if raw.trim_start().starts_with("s \"") {
            raw.trim_start()
                .trim_start_matches("s \"")
                .trim_end()
                .trim_end_matches('"')
                .replace("\\n", "\n")
                .replace("\\t", "\t")
        } else {
            raw.into_owned()
        };
        let _ = std::fs::write(format!("{dir}/kwin-support-info.txt"), content);
    }

    // 5. NVIDIA driver info — useful for NVIDIA forum reports
    if let Ok(content) = std::fs::read_to_string("/proc/driver/nvidia/version") {
        let _ = std::fs::write(format!("{dir}/nvidia-driver-version.txt"), content);
    }
    if let Ok(o) = Command::new("nvidia-smi").args(["--query-gpu=name,driver_version,vbios_version,pci.bus_id", "--format=csv"]).output() {
        let _ = std::fs::write(format!("{dir}/nvidia-smi.txt"), o.stdout);
    }

    // 6. wayland-info if available
    if let Ok(o) = Command::new("wayland-info").output() {
        let _ = std::fs::write(format!("{dir}/wayland-info.txt"), o.stdout);
    }

    // 7. wl-info alternative (some distros ship this instead)
    if let Ok(o) = Command::new("wl-info").output() {
        let _ = std::fs::write(format!("{dir}/wl-info.txt"), o.stdout);
    }

    // 8. os-release for image / kernel context
    if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
        let _ = std::fs::write(format!("{dir}/os-release.txt"), content);
    }

    // 9. Tar it up
    let tarball = format!("/tmp/wayland-spectre-bugreport-{epoch}.tar.gz");
    let status = Command::new("tar")
        .args(["-czf", &tarball, "-C", "/tmp", &format!("wayland-spectre-bugreport-{epoch}")])
        .status()
        .map_err(|e| e.to_string())?;

    if status.success() {
        let _ = std::fs::remove_dir_all(&dir);
        Ok(tarball)
    } else {
        Err(format!("tar failed, raw dir at: {dir}"))
    }
}

/// Build a human-readable SUMMARY.txt for copy-pasting into bug reports.
fn build_summary_text(report: &crate::domain::types::DiagnosticReport) -> String {
    let mut out = String::new();

    out.push_str("wayland-spectre diagnostic report\n");
    out.push_str(&"=".repeat(50));
    out.push('\n');
    out.push_str(&format!("Generated : {}\n", report.system.generated_at));
    out.push_str(&format!("Hostname  : {}\n", report.system.hostname));
    out.push_str(&format!("Kernel    : {}\n", report.system.kernel));
    if let Some(ref d) = report.system.nvidia_driver {
        out.push_str(&format!("NVIDIA    : {d}\n"));
    }
    if let Some(ref b) = report.system.bazzite_image {
        out.push_str(&format!("Bazzite   : {b}\n"));
    }
    out.push('\n');

    let fails: Vec<_> = report.results.iter().filter(|r| r.status == CheckStatus::Fail).collect();
    let warns: Vec<_> = report.results.iter().filter(|r| r.status == CheckStatus::Warn).collect();

    out.push_str(&format!(
        "Summary: {} pass  {} warn  {} fail\n\n",
        report.summary.pass, report.summary.warn, report.summary.fail
    ));

    if !fails.is_empty() {
        out.push_str("FAILURES\n");
        out.push_str(&"-".repeat(40));
        out.push('\n');
        for r in &fails {
            out.push_str(&format!("[{:?}] {}: {}\n", r.layer, r.check, r.detail));
            if let Some(ref fix) = r.fix {
                out.push_str(&format!("  fix: {fix}\n"));
            }
            out.push('\n');
        }
    }

    if !warns.is_empty() {
        out.push_str("WARNINGS\n");
        out.push_str(&"-".repeat(40));
        out.push('\n');
        for r in &warns {
            out.push_str(&format!("[{:?}] {}: {}\n", r.layer, r.check, r.detail));
            if let Some(ref fix) = r.fix {
                out.push_str(&format!("  fix: {fix}\n"));
            }
            out.push('\n');
        }
    }

    out.push_str(&"-".repeat(50));
    out.push('\n');
    out.push_str("Generated by wayland-spectre — https://forgejo.wanderingmonster.dev/WanderingMonster/wayland-spectre\n");
    out
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn gather_system_info() -> SystemInfo {
    let hostname = std::fs::read_to_string("/etc/hostname")
        .unwrap_or_default()
        .trim()
        .to_string();

    let kernel = Command::new("uname")
        .arg("-r")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    let nvidia_driver = read_nvidia_driver_version();
    let bazzite_image = read_bazzite_image();

    SystemInfo {
        generated_at: Utc::now().to_rfc3339(),
        hostname,
        kernel,
        nvidia_driver,
        bazzite_image,
    }
}

fn read_nvidia_driver_version() -> Option<String> {
    // Try /proc/driver/nvidia/version first (most reliable)
    if let Ok(content) = std::fs::read_to_string("/proc/driver/nvidia/version") {
        if let Some(line) = content.lines().next() {
            // "NVRM version: NVIDIA UNIX Open Kernel Module for x86_64  595.58.03 ..."
            if let Some(ver) = line.split_whitespace().find(|s| {
                s.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
                    && s.contains('.')
            }) {
                return Some(ver.to_string());
            }
        }
    }
    // Fallback: nvidia-smi
    Command::new("nvidia-smi")
        .args(["--query-gpu=driver_version", "--format=csv,noheader"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

fn read_bazzite_image() -> Option<String> {
    // Bazzite exposes image tag via /etc/os-release IMAGE_VERSION or via rpm-ostree
    if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if line.starts_with("IMAGE_VERSION=") {
                return Some(line.trim_start_matches("IMAGE_VERSION=").trim_matches('"').to_string());
            }
        }
    }
    // Fallback: rpm-ostree status
    Command::new("rpm-ostree")
        .args(["status", "--json"])
        .output()
        .ok()
        .and_then(|o| serde_json::from_slice::<serde_json::Value>(&o.stdout).ok())
        .and_then(|v| {
            v["deployments"][0]["version"]
                .as_str()
                .map(|s| s.to_string())
        })
}

