<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

<div align="center">
<img src="src-tauri/icons/icon.png" width="80" alt="wayland-spectre icon" />

# wayland-spectre

**Diagnose Wayland screen sharing failures on KDE Plasma + NVIDIA in one command.**

[![GPL-3.0-or-later](https://img.shields.io/badge/licence-GPL--3.0--or--later-blue)](LICENSE)
[![Bazzite](https://img.shields.io/badge/target-Bazzite%20%2F%20KDE%20Plasma-8A2BE2)](https://bazzite.gg)
[![NVIDIA](https://img.shields.io/badge/GPU-NVIDIA%20open%20modules-76b900)](https://github.com/NVIDIA/open-gpu-kernel-modules)

</div>

---

## Who is this for?

You are on **KDE Plasma on Wayland**, you have an **NVIDIA GPU** (RTX 30/40/50 series, open kernel modules), and **screen sharing doesn't work** — or works sometimes and silently fails other times. You might be on [Bazzite](https://bazzite.gg), Fedora Kinoite, or another immutable desktop.

The failure shows up as:

- OBS, Discord, Teams, Zoom, Pipewire-based capture: **black screen or no sources available**
- `xdg-desktop-portal` chooser appears but nothing streams
- The portal CreateSession call hangs or returns an error with no useful message
- Screen sharing worked on X11 but not on Wayland
- You have a **tiled or multi-tile 4K display** (e.g. Dell UP3214Q, some LG 4K panels) and sharing is completely broken

There are eight independent layers between your GPU and a working screencast, and any one of them can silently fail. Reading journal logs from all of them is tedious and requires knowing what to look for.

**wayland-spectre runs all the checks at once and tells you exactly which layer is broken and why.**

---

## What it checks

```
  wayland-spectre
  Wayland screen sharing diagnostics · KDE Plasma / Bazzite

  L0  ✓ [PASS] nvidia_driver_loaded — NVIDIA driver 595.58.03 loaded (open modules)
      ✓ [PASS] nvidia_open_modules — nvidia-open confirmed — required for Wayland DMA-BUF
      ✓ [PASS] nvidia_drm_modeset — nvidia-drm.modeset=1 — kernel modesetting enabled
      ✓ [PASS] nvidia_explicit_sync — Driver ≥570 — explicit sync on by default

  L1  ✓ [PASS] portal_presence — org.freedesktop.portal.Desktop active on session bus
      ⚠ [WARN] portal_screencast_iface — ScreenCast API v5 — xdg-desktop-portal 1.20.x
      ⚠ [WARN] zombie_sessions — 1 orphaned portal session object found

  L3  ✗ [FAIL] zkde_screencast_unstable_v1 — not advertised by compositor
              fix: systemctl --user restart plasma-kwin_wayland
      ✗ [FAIL] ext_image_capture_source_v1 — not advertised by compositor

  L4  ✓ [PASS] pipewire_socket — PipeWire socket present
      ✓ [PASS] portal_create_session — Session created and closed — Bug C not present

  L7  ✓ [PASS] kwin_running — KWin responding on D-Bus
      ✓ [PASS] kwin_version — KWin 6.2.5
      ✓ [PASS] kwin_render_backend — KWin EGL backend confirmed (GBM)
      ✗ [FAIL] kwin_screencast_loaded — supportInformation has no screencast reference
              fix: systemctl --user restart plasma-kwin_wayland
      ⚠ [WARN] kwin_tiled_display — 2 DP outputs detected — tiled 4K display.
              KDE bugs 493277 + 503870: CRTC format mismatch (AB30 vs AB4H) on
              tiled panels prevents zkde_screencast_unstable_v1 advertisement.
              See https://bugs.kde.org/show_bug.cgi?id=493277

  23 pass  3 warn  3 fail
```

Each failing check includes the exact `fix:` command to paste — no searching, no guessing.

### The eight diagnostic layers

| Layer | What it checks |
|-------|----------------|
| L0 | NVIDIA driver loaded, open modules, `nvidia-drm.modeset=1`, explicit sync fences |
| L1 | D-Bus session, `org.freedesktop.portal.Desktop` alive, ScreenCast API version, zombie sessions |
| L2 | Wayland compositor type (KWin vs XWayland vs unknown) |
| L3 | Wayland protocol globals: `zkde_screencast_unstable_v1`, `ext_image_capture_source_v1`, DMA-BUF, DRM syncobj |
| L4 | PipeWire socket, portal `CreateSession` live probe (detects the ELOOP / Bug C deadlock) |
| L5 | Flatpak permission-store entries that deny screencasting |
| L6 | Required environment variables (`XDG_RUNTIME_DIR`, `WAYLAND_DISPLAY`, `DBUS_SESSION_BUS_ADDRESS`) |
| L7 | KWin: screencast plugin loaded, EGL/GBM rendering backend, tiled display correlation |

---

## Usage

### CLI (no GUI needed)

```bash
# Run all checks — human-readable colour output
cargo run --manifest-path src-tauri/Cargo.toml -- check

# Machine-readable JSON (pipe to jq, save for bug reports)
cargo run --manifest-path src-tauri/Cargo.toml -- --json-only check

# Bundle a full bug report (JSON + journal excerpts → .tar.gz)
cargo run --manifest-path src-tauri/Cargo.toml -- report
```

### GUI (Tauri)

```bash
pnpm install
pnpm tauri dev
```

---

## Build / install

**Requirements:** Rust stable (≥1.82), Node 22, pnpm 9, KDE Plasma on Wayland.

```bash
git clone https://forgejo.wanderingmonster.dev/WanderingMonster/wayland-spectre
cd wayland-spectre
pnpm install
cargo build --manifest-path src-tauri/Cargo.toml --release
```

Bazzite system dependencies (already present on the target system):

```bash
sudo dnf install wayland-utils
```

---

## Bugs being tracked

The diagnostics are built around real open bugs. If wayland-spectre points you at one of these, there is an active upstream thread to follow or comment on:

| Bug | Summary |
|-----|---------|
| [NVIDIA forum 331077](https://forums.developer.nvidia.com/t/331077) | `zkde_screencast_unstable_v1` not advertised on tiled 4K + NVIDIA open modules |
| [KDE bug 493277](https://bugs.kde.org/show_bug.cgi?id=493277) | CRTC tiling format mismatch (AB30 vs AB4H) prevents KWin screencast plugin init |
| [KDE bug 503870](https://bugs.kde.org/show_bug.cgi?id=503870) | Tile gap / wl_output split causes KWin protocol advertisement regression |
| [xdg-desktop-portal #1953](https://github.com/flatpak/xdg-desktop-portal/issues/1953) | ScreenCast API v5 / ELOOP deadlock (Bug C) in portal 1.20.x |

---

## Licence

[GPL-3.0-or-later](LICENSE) — same licence as the parent `wayland-screenshare-diag` project.
