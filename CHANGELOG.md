# Changelog

All notable changes to wayland-spectre. Release names follow the
alphabetical Gelatinous-Cube sequence; patch releases keep their minor
version's name.

## v0.4.1 "Infernoheart" — 2026-07-27

Patch release: run cleanly on X11, stop reporting known false positives.

- **X11 support**: session type (Wayland / X11 / unknown) is detected once per
  run from `XDG_SESSION_TYPE` with `WAYLAND_DISPLAY`/`DISPLAY` fallback. On X11
  the Wayland-specific checks (L2/L3, `WAYLAND_DISPLAY`, KWin screencast
  plugin/effect, EGL backend) report SKIP — not applicable, not broken. The GUI
  shows an explanatory banner; misleading Wayland-only fix commands are
  suppressed.
- **False-positive audit** (upstream-confirmed by KDE/portal developers):
  `zkde_screencast_unstable_v1` absence → SKIP (permission-gated, never visible
  to unprivileged clients); `kwin_screencast_loaded` absence → WARN
  (inconclusive listing); portal ScreenCast version query failure → WARN.
  `bug_d_screencast_globals` → WARN (its zkde half is never observable, so the
  synthesis is a correlation, not proof).
  `kwin_screencast_effect_active` (L7) and the live portal CreateSession probe
  (L4) remain the authoritative signals.
- **CLI**: summaries (JSON and human) now include the `skip` count; exit code
  stays 0 when the only non-passing results are skips.
- **Packaging**: desktop entry now sets `Categories=System;Monitor;` (was
  empty — app landed in "Lost and Found").
- Retroactive CHANGELOG; README states X11 behaviour; frontend type-check
  fixes (`@types/node`, `WithElementRef`).

## v0.4.0 "Infernoheart" — 2026-04-08

- Bug-report redaction: hostname/username/home-path scrubbing (CLI `--redact`,
  GUI checkbox, on by default).
- KWin journal capture restricted to current boot; full boot journal plus
  targeted effect-startup extracts in bug bundles.
- "Copy cmd" button alongside every "Fix it" button.
- xdg-desktop-portal version check flags the ELOOP-affected 1.20.x series.
- AppImageHub catalogue descriptor.

## v0.3.0 "Frostcrystal" — 2026-04-08

- KWin Journal panel auto-expands when `kwin_screencast_effect_active` FAILs.
- CLI `--check <LAYER>` shorthand and colour-coded layer headers.
- GUI zoom controls (Ctrl+= / Ctrl+- / Ctrl+0).
- Two-step confirmation for destructive fix commands; fix-button layout fixes.
- NVIDIA WebKit rendering workaround; multiline detail rendering.

## v0.2.0 "Celestialradiance" — 2026-04-07

- CLI `--layer <LAYER>` filter.
- KWin Journal live-tail panel.
- `kwin_screencast_effect_active` (L7): distinguishes plugin-loaded from
  effect-activated via KWin supportInformation sections.
- AppImage and .deb packaging.

## v0.1.0 "Abyssalreaver" — 2026-04-07

- First release: Tauri v2 + Svelte 5 + Rust port of the Phase-1 bash
  diagnostic. One binary, GUI and headless CLI (`check`, `report`).
- Eight diagnostic layers (L0–L7): NVIDIA driver, D-Bus/portal, compositor,
  Wayland protocols, PipeWire, Flatpak permissions, environment, KWin.
- Native adapters (wayland-client, zbus) — no subprocess probes for core
  checks; live portal CreateSession capture test.
- One-click conservative fixes with runtime safety filter; tiled-display
  (Dell UP3214Q) correlation checks; "Bug D" synthesis.
