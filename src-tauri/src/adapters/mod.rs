//! Adapters — one file per external system being probed.
//! Each adapter exposes an async `check_*()` function returning
//! `Vec<DiagnosticResult>`. They share no state.
//!
//! Session 3: kwin.rs converted from subprocess (busctl) to native zbus.
//! All seven adapters now use only native crate bindings — no subprocess calls.

pub mod dbus;
pub mod env;
pub mod flatpak;
pub mod kwin;
pub mod nvidia;
pub mod pipewire;
pub mod wayland;
