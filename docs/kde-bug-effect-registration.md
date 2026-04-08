# KDE Bug 518698 — FILED 2026-04-08
# https://bugs.kde.org/show_bug.cgi?id=518698
#
# KDE Bug — Final Draft (Goal 2, Session 6)
# Product:   KWin
# Component: effects
# Version:   6.6.3
# Severity:  major
#
# Summary (paste as bug title):
#   kwin_screencast effect fails to register when tiled 4K display (Dell
#   UP3214Q, two DP tiles) is the only connected output — connecting a
#   secondary monitor resolves the failure
#
# ── PASTE BELOW INTO https://bugs.kde.org/enter_bug.cgi ───────────────────

## Summary

On a system with an NVIDIA RTX 5090 and a tiled 4K display (Dell UP3214Q,
presented by the OS as two separate DP outputs), the `kwin_screencast` plugin
initialises and appears in **Loaded Plugins**, but the screencast **effect
silently fails to register** in **Loaded Effects** whenever the tiled display
is the **only** connected output.

**Connecting any secondary monitor resolves the failure immediately** — on the
next boot with an HDMI monitor attached, `screencast` appears in both Loaded
Plugins and Loaded Effects, and `zkde_screencast_unstable_v1` is correctly
advertised on the Wayland bus.

This produces a binary, reproducible delta:

| Display configuration | Loaded Effects contains `screencast`? | Screensharing works? |
|---|---|---|
| Tiled Dell UP3214Q only (DP-4 + DP-5) | ❌ No | ❌ No |
| Tiled Dell UP3214Q + HDMI-A-3 secondary | ✅ Yes | ✅ Yes |

No error is logged to journalctl in either case. The failure is completely
silent.

This bug is distinct from KDE bugs 493277 and 503870, which document the
underlying CRTC format mismatch (AB30 vs AB4H) on the tiled display. Those
bugs describe *why* the CRTC negotiation fails; this bug documents the
**downstream effect-registration failure and its secondary-monitor dependency**.


## Steps to Reproduce

**Failing configuration (tiled display only):**

1. Connect only the tiled Dell UP3214Q (or similar tiled display appearing
   as two DP outputs). Disconnect all other monitors.
2. Boot KDE Plasma 6.x Wayland session with NVIDIA GPU + open kernel modules.
3. Run:
   ```
   qdbus org.kde.KWin /KWin supportInformation | grep -A 20 "Loaded Plugins"
   qdbus org.kde.KWin /KWin supportInformation | grep -A 20 "Loaded Effects"
   ```
4. Observe `screencast` present in Loaded Plugins, absent from Loaded Effects.
5. Attempt any screen sharing application — all fail silently.
6. Confirm no CRTC error logged: `journalctl --user -u plasma-kwin_wayland -b`

**Working configuration (tiled display + secondary monitor):**

7. Connect any secondary display (tested: 2560×1440 HDMI-A-3 at 100Hz).
8. Reboot.
9. Repeat step 3.
10. Observe `screencast` now present in both Loaded Plugins and Loaded Effects.


## Expected Behaviour

When `kwin_screencast` appears in Loaded Plugins, it should register its
effect and appear in Loaded Effects regardless of how many outputs are
connected. Single-output and multi-output configurations should behave
identically.


## Actual Behaviour

With only the tiled 4K display connected (two DP outputs presenting as a
single logical display):

- `screencast` initialises (Loaded Plugins) ✓
- `screencast` effect registration silently fails (absent from Loaded Effects) ✗
- `zkde_screencast_unstable_v1` and `ext_image_capture_source_v1` never
  advertised on Wayland bus ✗
- No error in journalctl — failure is completely silent ✗

**Actual `supportInformation` excerpt — failing session (tiled display only):**

```
Loaded Plugins:
---------------
[...]
screencast              ← plugin initialised
[...]

Loaded Effects:
---------------
shakecursor
outputlocator
colorpicker
zoom
[...]
                        ← screencast ABSENT — effect registration failed silently
```

**Actual `supportInformation` excerpt — working session (tiled + HDMI):**

```
Loaded Plugins:
---------------
[...]
screencast              ← plugin initialised
[...]

Loaded Effects:
---------------
[...]
screencast              ← effect registered correctly
[...]

Currently Active Effects:
[...]
screencast              ← effect active
```


## Hypothesis

The screencast effect registration path likely depends on successful CRTC
setup across outputs at startup. The tiled Dell UP3214Q triggers a CRTC
pixel format negotiation failure (AB30 vs AB4H, documented in KDE bugs
493277 + 503870) when it is the only connected output. The presence of a
secondary monitor appears to change the order or outcome of CRTC format
negotiation in a way that allows the tiled output commits to succeed.

The critical point for this bug is that the **effect registration failure
is silent** — no warning or error is emitted when a plugin initialises but
its effect does not register. This makes the failure extremely difficult to
diagnose without tooling such as wayland-spectre.

Two potential fixes:
1. **Logging**: Emit a warning when a plugin appears in Loaded Plugins but
   its effect is absent from Loaded Effects at startup.
2. **Decoupling**: Investigate whether screencast effect registration can
   proceed independently of CRTC format negotiation — screencasting does
   not require direct scanout and may not need to wait on per-output format
   commits.


## System Information

```
KWin version:    6.6.3
Qt Version:      6.10.2
Operation Mode:  Wayland
LogicalOutput:   DRM backend, Atomic Mode Setting on GPU 0: true

GPU (primary):   NVIDIA RTX 5090
                 Driver: 595.58.03, open kernel modules
                 /dev/dri/card2, renderD129
GPU (secondary): AMD (Ryzen 9 9950X3D iGPU)
                 /dev/dri/card1

Display (failing config):
  DP-4  1920×2160@59.99  Dell UP3214Q tile 1
  DP-5  1920×2160@59.99  Dell UP3214Q tile 2

Display (working config, adds):
  HDMI-A-3  2560×1440@99.95  secondary monitor

OS:     Bazzite bazzite-nvidia-open:stable, image 43.20260406
Kernel: 6.17.7-ba29.fc43.x86_64
```


## Journal — Failing Session (tiled display only, relevant excerpt)

```
Apr 08 HH:MM:SS <hostname> kwin_wayland[<pid>]: No backend specified, automatically choosing drm
Apr 08 HH:MM:SS <hostname> kwin_wayland[<pid>]: Failed to register with host portal
  QDBusError("org.freedesktop.portal.Error.Failed",
  "Could not register app ID: Unable to open /proc/3450/root")
```

Note: the portal registration error is the separate `O_RDONLY|O_NOFOLLOW`
ELOOP bug filed as xdg-desktop-portal#1953. **No CRTC or effect error is
logged** — the screencast effect registration failure is completely silent.


## Attachments

Attach the wayland-spectre bug report tarball, which includes:
- Full structured JSON diagnostic (all 8 layers, pass/warn/fail per check)
- Full KWin boot journal
- Targeted screencast/effect journal excerpt

Generate with:
```bash
wayland-spectre report
```

Source: https://forgejo.wanderingmonster.dev/WanderingMonster/wayland-spectre


## Cross-References

- **KDE bug 493277** — tiled display Night Light / CRTC interaction (same hardware)
- **KDE bug 503870** — CRTC format mismatch AB30 vs AB4H on tiled display (same hardware)
- **NVIDIA forum 331077** — upstream NVIDIA tracking of RTX 5090 CRTC format mismatch
- **xdg-desktop-portal #1953** — O_RDONLY|O_NOFOLLOW → ELOOP on portal startup (separate)
- **KDE bug 518650** — KWin should create `zkde_screencast_unstable_v1` regardless
  of portal registration success (related hardening, filed separately)
