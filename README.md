<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

<div align="center">
<img src="src-tauri/icons/icon.png" width="80" alt="wayland-spectre icon" />

# wayland-spectre

**Diagnose Wayland screen sharing failures on KDE Plasma + NVIDIA in one command.**

[![GPL-3.0-or-later](https://img.shields.io/badge/licence-GPL--3.0--or--later-blue)](LICENSE)
[![Bazzite](https://img.shields.io/badge/target-Bazzite%20%2F%20KDE%20Plasma-8A2BE2)](https://bazzite.gg)
[![NVIDIA](https://img.shields.io/badge/GPU-NVIDIA%20open%20modules-76b900)](https://github.com/NVIDIA/open-gpu-kernel-modules)
[![GitHub mirror](https://img.shields.io/badge/mirror-GitHub-181717?logo=github)](https://github.com/WanderingM0nster/wayland-spectre)

</div>

![wayland-spectre screenshot](https://forgejo.wanderingmonster.dev/WanderingMonster/wayland-spectre/raw/branch/main/docs/screenshot.jpg)

---

## TL;DR — Screen sharing is broken. What do I do?

> *Someone sent you here, or you found this tool and just want to know if your
> screen sharing can be fixed. Here's the short version.*

**1. Download and run the app**

Download the latest `wayland-spectre_*.AppImage` from the
[Releases page](https://forgejo.wanderingmonster.dev/WanderingMonster/wayland-spectre/releases),
then open a terminal and run:

```bash
chmod +x wayland-spectre_*.AppImage
./wayland-spectre_*.AppImage
```

The app opens, runs all checks automatically, and shows you a colour-coded
list of what is working (✓ green), needs attention (⚠ amber), or is broken
(✗ red).

**2. Try the suggested fixes**

Any red ✗ row has a **Fix** button next to it. Click it. The app will apply
the fix and re-run the checks. If the row turns green, that problem is solved.

**3. Test whether screen sharing now works**

Scroll to the bottom and click **▶ Run test**. If it says the test passed,
screen sharing should work — try it in your app (OBS, Discord, etc.).

**4. If it's still broken — send a report**

Click **⬇ Bug report** in the top-right corner. It saves a file to `/tmp/`
with a name like `wayland-spectre-bugreport-1234567890.tar.gz`.

Post in one of these places and attach that file:

- **Bazzite users:** [Bazzite community thread](https://universal-blue.discourse.group/t/11901)
- **KDE Plasma bugs:** [KDE bug tracker](https://bugs.kde.org) → product: KWin
- **NVIDIA GPU issues:** [NVIDIA developer forum](https://forums.developer.nvidia.com/t/331077)

Or click **⎘ Copy summary** and paste the text directly into a forum post
without attaching a file.

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

### Runs on X11 too

The tool detects the session type at startup and also runs on **X11**. The
Wayland-specific checks (compositor connection, Wayland protocols, KWin
screencast plugin) are reported as **SKIP — not applicable** there rather than
as failures, with a banner explaining why; the portal, PipeWire, Flatpak, and
driver checks run normally. The CLI exits 0 on X11 when skips are the only
non-passing results.

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

Fix buttons marked **Fix it ⚠** will restart your Wayland compositor and end your session — you will need to log back in. These show a confirmation step before executing.

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
wayland-spectre check

# Single layer only (faster for spot-checks)
wayland-spectre --check L7
wayland-spectre check --layer L3

# Machine-readable JSON (pipe to jq, save for bug reports)
wayland-spectre check --json-only

# Bundle a full bug report (JSON + journal excerpts → .tar.gz)
wayland-spectre report
```

Layer reference: L0 GPU/NVIDIA · L1 D-Bus/portal · L2 portal backend · L3 Wayland protocols · L4 PipeWire · L5 Flatpak · L6 environment · L7 KWin plugins

### GUI (Tauri)

```bash
pnpm install
pnpm tauri dev
```

The GUI runs all checks automatically on launch and presents results as an
expandable layer pipeline. Several additional tools are available below the
main results:

**Live Capture Test** — attempts to open a real PipeWire screencast node via
`xdg-desktop-portal`. Confirms end-to-end whether screensharing actually works
on your system right now, beyond what the passive checks can tell you. Run it
after fixing any reported failures to verify the fix worked.

**KWin Journal panel** — displays the last 80 lines of the
`plasma-kwin_wayland` systemd journal. Quick-filter chips (`screencast`,
`effect`, `crtc`, `error`, `format`) let you isolate the specific startup log
lines that explain a failure. Auto-expands when `kwin_screencast_effect_active`
is FAIL so the evidence surfaces without hunting for it.

**Copy summary** — copies a plain-text `SUMMARY.txt` to the clipboard: system
info, all failures and warnings with fix commands, and a footer. Paste it
directly into a forum post or bug comment without downloading anything.

**⬇ Bug report** — generates a `.tar.gz` bundle at `/tmp/` containing the full
JSON diagnostic, `SUMMARY.txt`, KWin boot journal, targeted startup log
excerpts, `kwin-support-info.txt`, and NVIDIA driver info. Attach to KDE
Bugzilla, the NVIDIA forum, or the Bazzite community thread.

**Zoom controls** (header, `−` / `150%` / `+`) — scales the entire UI.
Default is 150% for legibility on HiDPI displays. `Ctrl+=` / `Ctrl+-` /
`Ctrl+0` also work from the keyboard.

---

## Build / install

### AppImage (recommended — runs on any x86-64 Linux)

```bash
# Build
git clone https://forgejo.wanderingmonster.dev/WanderingMonster/wayland-spectre
cd wayland-spectre
pnpm install
pnpm tauri build                # produces src-tauri/target/release/bundle/appimage/

# Install (move to a directory on your PATH)
cp src-tauri/target/release/bundle/appimage/wayland-spectre_*.AppImage \
   ~/.local/bin/wayland-spectre.AppImage
chmod +x ~/.local/bin/wayland-spectre.AppImage

# Run
wayland-spectre.AppImage               # GUI
wayland-spectre.AppImage -- check      # CLI (colour output)
wayland-spectre.AppImage -- report     # Bug report bundle → /tmp/*.tar.gz
```

> **Note:** Tauri downloads `linuxdeploy` automatically during `pnpm tauri build`.
> Requires an internet connection on the build machine the first time.

### .deb (Ubuntu / Debian / Bazzite)

```bash
pnpm tauri build
sudo dpkg -i src-tauri/target/release/bundle/deb/wayland-spectre_*.deb
```

### Cargo only (CLI, no GUI)

**Requirements:** Rust stable ≥1.82 (no Node, no pnpm).

```bash
git clone https://forgejo.wanderingmonster.dev/WanderingMonster/wayland-spectre
cd wayland-spectre
cargo build --manifest-path src-tauri/Cargo.toml --release
# Binary: src-tauri/target/release/wayland-spectre
```

### Multi-size icons for distribution builds

The repo ships a single `src-tauri/icons/icon.png`. Before a public release,
regenerate the full icon set:

```bash
pnpm tauri icon src-tauri/icons/icon.png   # writes 32x32.png, 128x128.png, etc.
```

Bazzite system dependencies (already present on Bazzite 43.x):

```bash
# Fedora / Bazzite (if building from source on a fresh machine)
sudo dnf install wayland-utils dbus-devel
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
| [KDE bug 518650](https://bugs.kde.org/show_bug.cgi?id=518650) | KWin should create `zkde_screencast_unstable_v1` regardless of portal registration success |
| [KDE bug 518698](https://bugs.kde.org/show_bug.cgi?id=518698) | kwin_screencast effect absent from Loaded Effects when tiled display is the only output — secondary monitor resolves failure |
| Bug D (`bug_d_screencast_globals`) | KWin not advertising `zkde_screencast_unstable_v1` **or** `ext_image_capture_source_v1` — confirmed root cause on tiled 4K + NVIDIA open modules; synthesised at L3 with root-cause hypothesis and upstream links |

---

## Licence

[GPL-3.0-or-later](LICENSE) — same licence as the parent `wayland-screenshare-diag` project.

