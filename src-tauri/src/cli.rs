// SPDX-License-Identifier: GPL-3.0-or-later
//! CLI entry point.
//! Same adapter functions as the GUI — just different output formatting.
//! Uses owo-colors for terminal output (respects NO_COLOR / isatty).
//!
//! Session 5 changes:
//!   - `--json-only` moved from top-level Cli to the `check` subcommand,
//!     so the natural form is:  check --json-only
//!   - `--layer <LAYER>` added to `check` — runs only the nominated layer.
//!     Accepts L0..L7 (case-insensitive).  All adapters still run in parallel;
//!     results are filtered before output so timings stay consistent.
//!
//! Session 6 changes:
//!   - `--check <LAYER>` top-level shorthand:
//!       wayland-spectre --check L3   ≡   wayland-spectre check --layer L3
//!     Faster to type for common single-layer spot-checks.
//!   - Colour-coded layer headers: each of the eight layers gets its own
//!     terminal colour, making layer boundaries instantly scannable.
//!     Plain text mode (NO_COLOR / non-tty) falls back to "[L3 protocols]".

use crate::adapters;
use crate::commands::generate_bug_report;
use crate::domain::types::{CheckStatus, Layer, SessionType};
use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;
use supports_color::Stream;

#[derive(Parser, Debug)]
#[command(
    name = "wayland-spectre",
    about = "Wayland screen sharing diagnostics for KDE Plasma / Bazzite",
    version
)]
struct Cli {
    /// Shorthand: --check <LAYER>  ≡  check --layer <LAYER>
    ///
    /// Runs only the nominated layer without typing the `check` subcommand.
    /// Example:  wayland-spectre --check L3
    /// Example:  wayland-spectre --check L7
    #[arg(long, value_name = "LAYER")]
    check: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run all diagnostic checks (default)
    Check {
        /// Output machine-readable JSON only (no colour, no formatting)
        #[arg(long)]
        json_only: bool,

        /// Run checks for a single layer only (L0–L7).
        /// Example: check --layer L3
        #[arg(long, value_name = "LAYER")]
        layer: Option<String>,
    },
    /// Generate a bug report bundle (JSON + journal excerpts)
    Report {
        /// Replace hostname, username, and home paths with safe placeholders
        #[arg(long)]
        redact: bool,
    },
}

/// Entry point for CLI mode. Returns an exit code (0 = all pass/warn, 1 = failures).
pub async fn run() -> i32 {
    let cli = Cli::parse();
    let use_color = supports_color::on(Stream::Stdout).is_some();

    // --check <LAYER> and a subcommand are mutually exclusive
    if cli.check.is_some() && cli.command.is_some() {
        eprintln!("error: --check cannot be combined with a subcommand");
        eprintln!("  use:  wayland-spectre --check L3");
        eprintln!("   or:  wayland-spectre check --layer L3");
        return 2;
    }

    // --check L3 shorthand  →  check --layer L3
    if let Some(layer) = cli.check {
        return run_checks(false, Some(layer), use_color).await;
    }

    match cli.command.unwrap_or(Commands::Check { json_only: false, layer: None }) {
        Commands::Check { json_only, layer } => run_checks(json_only, layer, use_color).await,
        Commands::Report { redact } => run_report(redact).await,
    }
}

async fn run_checks(json_only: bool, layer_filter: Option<String>, use_color: bool) -> i32 {
    if !json_only {
        print_header(use_color);
    }

    // Detected once per run, threaded into the session-dependent adapters
    let session = SessionType::detect();

    // Run all adapters — always concurrent; filter afterwards
    let (wayland, dbus, pipewire, flatpak, nvidia, env, kwin) = tokio::join!(
        adapters::wayland::check_wayland_protocols(session),
        adapters::dbus::check_dbus_portal(),
        adapters::pipewire::check_pipewire(),
        adapters::flatpak::check_flatpak_permissions(),
        adapters::nvidia::check_nvidia(),
        adapters::env::check_environment(session),
        adapters::kwin::check_kwin_plugins(session),
    );

    let mut all_results = Vec::new();
    all_results.extend(nvidia);
    all_results.extend(dbus);
    all_results.extend(wayland);
    all_results.extend(pipewire);
    all_results.extend(flatpak);
    all_results.extend(env);
    all_results.extend(kwin);

    // Apply --layer / --check filter if requested
    let filtered: Vec<_> = if let Some(ref wanted) = layer_filter {
        let wanted_upper = wanted.to_uppercase();
        let target = parse_layer_filter(&wanted_upper);
        if target.is_none() {
            eprintln!("Unknown layer '{}'. Valid values: L0 L1 L2 L3 L4 L5 L6 L7", wanted);
            return 2;
        }
        all_results
            .into_iter()
            .filter(|r| Some(&r.layer) == target.as_ref())
            .collect()
    } else {
        all_results
    };

    if json_only {
        let pass = filtered.iter().filter(|r| r.status == CheckStatus::Pass).count();
        let warn = filtered.iter().filter(|r| r.status == CheckStatus::Warn).count();
        let fail = filtered.iter().filter(|r| r.status == CheckStatus::Fail).count();
        let skip = filtered.iter().filter(|r| r.status == CheckStatus::Skip).count();
        let output = serde_json::json!({
            "schema_version": "1",
            "layer_filter": layer_filter,
            "session_type": session,
            "results": filtered,
            "summary": { "pass": pass, "warn": warn, "fail": fail, "skip": skip }
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
        // Skips never affect the exit code — an all-skip X11 run exits 0
        return if fail > 0 { 1 } else { 0 };
    }

    // Human-readable output
    if session != SessionType::Wayland {
        let note = match session {
            SessionType::X11 =>
                "X11 session detected — Wayland-specific checks are reported as SKIP",
            _ =>
                "Session type unknown (XDG_SESSION_TYPE unset) — all checks run",
        };
        if use_color {
            println!("  {}", note.dimmed());
        } else {
            println!("  {note}");
        }
    }
    if let Some(ref lf) = layer_filter {
        if use_color {
            println!("  {}", format!("layer filter: {lf}").dimmed());
        } else {
            println!("  layer filter: {lf}");
        }
    }

    let mut fail_count = 0;
    let mut current_layer = String::new();

    for result in &filtered {
        let layer_str = format!("{:?}", result.layer);
        if layer_str != current_layer {
            current_layer = layer_str.clone();
            println!();
            println!("  {}", format_layer_header(&layer_str, use_color));
        }

        let (prefix, coloured_status) = match result.status {
            CheckStatus::Pass => {
                let s = if use_color { "PASS".green().to_string() } else { "PASS".into() };
                ("  ✓ ", s)
            }
            CheckStatus::Warn => {
                let s = if use_color { "WARN".yellow().to_string() } else { "WARN".into() };
                ("  ⚠ ", s)
            }
            CheckStatus::Fail => {
                fail_count += 1;
                let s = if use_color { "FAIL".red().bold().to_string() } else { "FAIL".into() };
                ("  ✗ ", s)
            }
            CheckStatus::Skip => {
                let s = if use_color { "SKIP".dimmed().to_string() } else { "SKIP".into() };
                ("  – ", s)
            }
        };

        println!("{prefix}[{coloured_status}] {} — {}", result.check, result.detail);

        if result.status == CheckStatus::Fail || result.status == CheckStatus::Warn {
            if let Some(fix) = &result.fix {
                if use_color {
                    println!("        fix: {}", fix.cyan());
                } else {
                    println!("        fix: {fix}");
                }
            }
        }
    }

    print_summary(&filtered, use_color);
    if fail_count > 0 { 1 } else { 0 }
}

/// Colour-coded layer header with short label.
fn format_layer_header(layer_str: &str, use_color: bool) -> String {
    let label = layer_label(layer_str);
    let full = format!("{layer_str}  {label}");
    if !use_color {
        return format!("[{full}]");
    }
    match layer_str {
        "L0" => full.bright_blue().bold().to_string(),
        "L1" => full.cyan().bold().to_string(),
        "L2" => full.magenta().bold().to_string(),
        "L3" => full.yellow().bold().to_string(),
        "L4" => full.green().bold().to_string(),
        "L5" => full.bright_cyan().bold().to_string(),
        "L6" => full.white().bold().to_string(),
        "L7" => full.bright_red().bold().to_string(),
        _    => full.bold().to_string(),
    }
}

/// Short human-readable label for each layer.
fn layer_label(layer_str: &str) -> &'static str {
    match layer_str {
        "L0" => "hardware / gpu",
        "L1" => "d-bus / portal session",
        "L2" => "compositor connection",
        "L3" => "wayland protocols",
        "L4" => "pipewire",
        "L5" => "flatpak permissions",
        "L6" => "environment",
        "L7" => "kwin plugins",
        _    => "",
    }
}

/// Parse a string like "L3" into a `Layer` variant.
fn parse_layer_filter(s: &str) -> Option<Layer> {
    match s {
        "L0" => Some(Layer::L0),
        "L1" => Some(Layer::L1),
        "L2" => Some(Layer::L2),
        "L3" => Some(Layer::L3),
        "L4" => Some(Layer::L4),
        "L5" => Some(Layer::L5),
        "L6" => Some(Layer::L6),
        "L7" => Some(Layer::L7),
        _    => None,
    }
}

async fn run_report(redact: bool) -> i32 {
    if redact {
        println!("Generating bug report bundle (redacted)…");
    } else {
        println!("Generating bug report bundle…");
    }
    match generate_bug_report(redact).await {
        Ok(path) => {
            println!("Bug report saved: {path}");
            println!();
            println!("Contents:");
            if let Ok(o) = std::process::Command::new("tar")
                .args(["--list", "--file", &path])
                .output()
            {
                let entries = String::from_utf8_lossy(&o.stdout);
                for line in entries.lines().filter(|l| !l.trim().is_empty()) {
                    println!("  {line}");
                }
            }
            println!();
            println!("Next steps:");
            println!("  • Attach {path} to NVIDIA forum thread:");
            println!("    https://forums.developer.nvidia.com/t/331077");
            println!("  • Update Bazzite community thread:");
            println!("    https://universal-blue.discourse.group/t/11901");
            0
        }
        Err(e) => {
            eprintln!("Error: {e}");
            1
        }
    }
}

fn print_header(use_color: bool) {
    let subtitle = "Wayland screen sharing diagnostics · KDE Plasma / Bazzite";
    if use_color {
        println!("\n  {}{}", "wayland".white().bold(), "-spectre".red().bold());
        println!("  {}\n", subtitle.dimmed());
    } else {
        println!("\n  wayland-spectre");
        println!("  {subtitle}\n");
    }
}

fn print_summary(results: &[crate::domain::types::DiagnosticResult], use_color: bool) {
    let pass = results.iter().filter(|r| r.status == CheckStatus::Pass).count();
    let warn = results.iter().filter(|r| r.status == CheckStatus::Warn).count();
    let fail = results.iter().filter(|r| r.status == CheckStatus::Fail).count();
    let skip = results.iter().filter(|r| r.status == CheckStatus::Skip).count();

    println!();
    if use_color {
        println!(
            "  {} pass  {} warn  {} fail  {} skip",
            pass.to_string().green().bold(),
            warn.to_string().yellow().bold(),
            fail.to_string().red().bold(),
            skip.to_string().dimmed(),
        );
    } else {
        println!("  {pass} pass  {warn} warn  {fail} fail  {skip} skip");
    }
    println!();
}
