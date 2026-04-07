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

use crate::adapters;
use crate::commands::generate_bug_report;
use crate::domain::types::{CheckStatus, Layer};
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
    Report,
}

/// Entry point for CLI mode. Returns an exit code (0 = all pass/warn, 1 = failures).
pub async fn run() -> i32 {
    let cli = Cli::parse();
    let use_color = supports_color::on(Stream::Stdout).is_some();

    match cli.command.unwrap_or(Commands::Check { json_only: false, layer: None }) {
        Commands::Check { json_only, layer } => run_checks(json_only, layer, use_color).await,
        Commands::Report => run_report().await,
    }
}

async fn run_checks(json_only: bool, layer_filter: Option<String>, use_color: bool) -> i32 {
    if !json_only {
        print_header(use_color);
    }

    // Run all adapters — always concurrent; filter afterwards
    let (wayland, dbus, pipewire, flatpak, nvidia, env, kwin) = tokio::join!(
        adapters::wayland::check_wayland_protocols(),
        adapters::dbus::check_dbus_portal(),
        adapters::pipewire::check_pipewire(),
        adapters::flatpak::check_flatpak_permissions(),
        adapters::nvidia::check_nvidia(),
        adapters::env::check_environment(),
        adapters::kwin::check_kwin_plugins(),
    );

    let mut all_results = Vec::new();
    all_results.extend(nvidia);
    all_results.extend(dbus);
    all_results.extend(wayland);
    all_results.extend(pipewire);
    all_results.extend(flatpak);
    all_results.extend(env);
    all_results.extend(kwin);

    // Apply --layer filter if requested
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
        // Minimal JSON output to stdout — same schema as GUI
        let pass = filtered.iter().filter(|r| r.status == CheckStatus::Pass).count();
        let warn = filtered.iter().filter(|r| r.status == CheckStatus::Warn).count();
        let fail = filtered.iter().filter(|r| r.status == CheckStatus::Fail).count();

        let output = serde_json::json!({
            "schema_version": "1",
            "layer_filter": layer_filter,
            "results": filtered,
            "summary": { "pass": pass, "warn": warn, "fail": fail }
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
        return if fail > 0 { 1 } else { 0 };
    }

    // Human-readable output
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
            if use_color {
                println!("\n  {}", layer_str.bold().dimmed());
            } else {
                println!("\n  [{layer_str}]");
            }
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

async fn run_report() -> i32 {
    println!("Generating bug report bundle…");
    match generate_bug_report().await {
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

    println!();
    if use_color {
        println!(
            "  {} pass  {} warn  {} fail",
            pass.to_string().green().bold(),
            warn.to_string().yellow().bold(),
            fail.to_string().red().bold(),
        );
    } else {
        println!("  {pass} pass  {warn} warn  {fail} fail");
    }
    println!();
}
