//! CLI entry point.
//! Same adapter functions as the GUI — just different output formatting.
//! Uses owo-colors for terminal output (respects NO_COLOR / isatty).

use crate::adapters;
use crate::commands::generate_bug_report;
use crate::domain::types::CheckStatus;
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

    /// Output machine-readable JSON only (no colour, no formatting)
    #[arg(long)]
    json_only: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run all diagnostic checks (default)
    Check,
    /// Generate a bug report bundle (JSON + journal excerpts)
    Report,
}

/// Entry point for CLI mode. Returns an exit code (0 = all pass/warn, 1 = failures).
pub async fn run() -> i32 {
    let cli = Cli::parse();
    let use_color = supports_color::on(Stream::Stdout).is_some();

    match cli.command.unwrap_or(Commands::Check) {
        Commands::Check => run_checks(cli.json_only, use_color).await,
        Commands::Report => run_report().await,
    }
}

async fn run_checks(json_only: bool, use_color: bool) -> i32 {
    if !json_only {
        print_header(use_color);
    }

    // Run all adapters
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

    if json_only {
        // Minimal JSON output to stdout — same schema as GUI
        let pass = all_results.iter().filter(|r| r.status == CheckStatus::Pass).count();
        let warn = all_results.iter().filter(|r| r.status == CheckStatus::Warn).count();
        let fail = all_results.iter().filter(|r| r.status == CheckStatus::Fail).count();

        let output = serde_json::json!({
            "schema_version": "1",
            "results": all_results,
            "summary": { "pass": pass, "warn": warn, "fail": fail }
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
        return if fail > 0 { 1 } else { 0 };
    }

    // Human-readable output
    let mut fail_count = 0;
    let mut current_layer = String::new();

    for result in &all_results {
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

    print_summary(&all_results, use_color);
    if fail_count > 0 { 1 } else { 0 }
}

async fn run_report() -> i32 {
    println!("Generating bug report bundle…");
    match generate_bug_report().await {
        Ok(path) => {
            println!("Bug report saved: {path}");
            println!();
            println!("Contents:");
            // List the tarball entries for quick verification
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
    let title = "wayland-spectre";
    let subtitle = "Wayland screen sharing diagnostics · KDE Plasma / Bazzite";
    if use_color {
        println!("\n  {}{}", "wayland".white().bold(), "-spectre".red().bold());
        println!("  {}\n", subtitle.dimmed());
    } else {
        println!("\n  {title}");
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

