# KDE upstream comment — kwin_screencast_effect_active finding
# Target bugs: https://bugs.kde.org/show_bug.cgi?id=493277
#              https://bugs.kde.org/show_bug.cgi?id=503870
#
# Post to both bugs. The second bug (503870) is specifically about
# the TILE gap / wl_output split; cross-reference both.
#
# ── DRAFT COMMENT ────────────────────────────────────────────────────────

## New diagnostic finding: plugin loads but effect never activates

I've been running automated diagnostics against this bug and found a more
precise failure point that may help narrow down the root cause.

**System:** Ryzen 9 9950X3D, RTX 5090, Bazzite 43.20260406,
kernel 6.17.7-ba29.fc43.x86_64, NVIDIA driver 595.58.03 (open modules),
KDE Plasma 6.x Wayland. Display: Dell UP3214Q tiled 4K presenting as two
1920×2160 panels on DP-4 + DP-5.

### The distinction

KWin's `supportInformation` (via `busctl --user call org.kde.KWin /KWin
org.kde.KWin supportInformation`) has two separate sections:

```
Loaded Plugins:
kwin_screencast
kwin4_effect_blur
…

Loaded Effects:
blur
overview
…
```

On this system, `kwin_screencast` **is** present in `Loaded Plugins`
(the plugin was initialised by KWin at startup), but it is **absent**
from `Loaded Effects` (the effect never activated).

This is a finer-grained failure than simply "screencast plugin not loaded" —
it shows the plugin reaches KWin's plugin loader but fails at the effect
registration step.

### Hypothesis

The CRTC format mismatch between the two tile outputs (AB30 vs AB4H, visible
in Xid 51/69 and in `nvidia-bug-report` DRM output) occurs during KWin's
compositor startup sequence. Effect registration for `kwin_screencast`
apparently depends on compositor state that is not yet valid when the CRTC
mismatch is encountered, so the effect silently fails to register.

Consequence: `zkde_screencast_unstable_v1` and `ext_image_capture_source_v1`
are never advertised on the Wayland bus, which is what `wayland-info` and
`weston-info` report as missing.

### Corroborating evidence

```
journalctl --user -u plasma-kwin_wayland -b | grep -iE 'screencast|effect|crtc|format'
```

The boot journal shows the screencast plugin loading but no subsequent
"effect registered" log line. There is also no error line — the failure
is silent, which explains why it has been hard to diagnose from journals alone.

### Cross-reference

NVIDIA forum thread tracking the CRTC mismatch:
https://forums.developer.nvidia.com/t/331077

The `kwin_screencast_effect_active` check is implemented in
[wayland-spectre](https://forgejo.wanderingmonster.dev/WanderingMonster/wayland-spectre),
a diagnostic tool for this exact failure class — the check distinguishes
"plugin in Loaded Plugins" from "effect in Loaded Effects".

I'm happy to provide full `nvidia-bug-report.log.gz`, `supportInformation`
dump, or a `wayland-spectre` bug report tarball if that would help.
