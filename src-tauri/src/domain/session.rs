// SPDX-License-Identifier: GPL-3.0-or-later
//! Session-type detection — v0.4.1.
//!
//! Detects whether the tool is running under a Wayland or X11 session so the
//! Wayland-specific adapters (wayland.rs, env.rs, kwin.rs) can report SKIP
//! instead of FAIL on X11 (AppImageHub review: the app must present as
//! not-applicable on X11, not as broken).
//!
//! `XDG_SESSION_TYPE` is the primary signal; it is empty in some login paths,
//! so presence of `WAYLAND_DISPLAY` / `DISPLAY` is the fallback.
//! `WAYLAND_DISPLAY` wins over `DISPLAY` because Wayland sessions almost
//! always export both (XWayland sets `DISPLAY`).
//!
//! Detection runs **once** per diagnostic run and the result is passed by
//! value into the adapters that need it — no global, no OnceLock, so the
//! X11 code paths stay unit-testable.

use crate::domain::types::SessionType;

impl SessionType {
    /// Reads the process environment. Called once per diagnostic run.
    pub fn detect() -> Self {
        Self::classify(
            std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
            std::env::var_os("WAYLAND_DISPLAY").is_some(),
            std::env::var_os("DISPLAY").is_some(),
        )
    }

    /// Pure classification — unit-testable without touching the environment.
    pub(crate) fn classify(
        xdg_session_type: Option<&str>,
        has_wayland_display: bool,
        has_display: bool,
    ) -> Self {
        match xdg_session_type.map(str::trim) {
            Some("wayland") => Self::Wayland,
            Some("x11") => Self::X11,
            // Unset, empty, "tty", or anything unexpected → env-var fallback
            _ if has_wayland_display => Self::Wayland,
            _ if has_display => Self::X11,
            _ => Self::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdg_wayland_wins() {
        assert_eq!(
            SessionType::classify(Some("wayland"), false, false),
            SessionType::Wayland
        );
    }

    #[test]
    fn xdg_x11_wins_even_with_wayland_display() {
        // Explicit XDG_SESSION_TYPE=x11 outranks a stray WAYLAND_DISPLAY
        assert_eq!(
            SessionType::classify(Some("x11"), true, true),
            SessionType::X11
        );
    }

    #[test]
    fn unset_falls_back_to_wayland_display() {
        assert_eq!(
            SessionType::classify(None, true, true),
            SessionType::Wayland
        );
    }

    #[test]
    fn unset_falls_back_to_display_when_no_wayland() {
        assert_eq!(SessionType::classify(None, false, true), SessionType::X11);
    }

    #[test]
    fn empty_string_treated_as_unset() {
        assert_eq!(SessionType::classify(Some(""), true, false), SessionType::Wayland);
        assert_eq!(SessionType::classify(Some(""), false, true), SessionType::X11);
    }

    #[test]
    fn tty_session_uses_fallback() {
        // SSH login shells report XDG_SESSION_TYPE=tty
        assert_eq!(SessionType::classify(Some("tty"), false, true), SessionType::X11);
        assert_eq!(SessionType::classify(Some("tty"), false, false), SessionType::Unknown);
    }

    #[test]
    fn whitespace_trimmed() {
        assert_eq!(
            SessionType::classify(Some(" wayland "), false, false),
            SessionType::Wayland
        );
    }

    #[test]
    fn nothing_set_is_unknown() {
        assert_eq!(SessionType::classify(None, false, false), SessionType::Unknown);
    }

    #[test]
    fn serialises_uppercase() {
        assert_eq!(serde_json::to_string(&SessionType::Wayland).unwrap(), "\"WAYLAND\"");
        assert_eq!(serde_json::to_string(&SessionType::X11).unwrap(), "\"X11\"");
        assert_eq!(serde_json::to_string(&SessionType::Unknown).unwrap(), "\"UNKNOWN\"");
    }
}
