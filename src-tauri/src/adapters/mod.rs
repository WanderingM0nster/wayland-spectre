//! Adapters — one file per external system being probed.
//! Each adapter exposes an async `check_*()` function returning
//! `Vec<DiagnosticResult>`. They share no state.
//!
//! Current strategy: subprocess-based (matching Phase 1 bash script).
//! Session 2 will replace wayland + dbus with native crate bindings.

pub mod dbus;
pub mod env;
pub mod flatpak;
pub mod kwin;
pub mod nvidia;
pub mod pipewire;
pub mod wayland;
