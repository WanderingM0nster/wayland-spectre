# NVIDIA forum update — thread 331077
# https://forums.developer.nvidia.com/t/331077
# Session 4/5 findings — post as a reply to the original thread
#
# ── DRAFT POST ───────────────────────────────────────────────────────────

## Session 4 / Session 5 findings — precise failure point identified

**System:** unchanged — Ryzen 9 9950X3D, RTX 5090, Bazzite 43.20260406,
kernel 6.17.7-ba29.fc43.x86_64, NVIDIA driver 595.58.03 open modules,
KDE Plasma Wayland, Dell UP3214Q tiled 4K (DP-4 + DP-5, 1920×2160 each).

### New finding: kwin_screencast effect-not-activating failure mode

Since the last update I've added a diagnostic that reads the `Loaded Plugins`
and `Loaded Effects` sections of KWin's `supportInformation` separately.

**Result:** `kwin_screencast` appears in `Loaded Plugins` (the plugin was
loaded by KWin) but is **absent from `Loaded Effects`** (the effect never
activated). This is the precise failure point that explains why
`zkde_screencast_unstable_v1` is never advertised on the Wayland bus.

The effect registration silently fails at compositor startup — there is no
error in the journal, just the absence of a "registered" log line.

### Connection to the CRTC format mismatch

The timing is consistent with the CRTC format mismatch (AB30 vs AB4H)
reported in the original bug report. The two tile outputs on DP-4 and DP-5
are enumerated with different CRTC formats during the initial KMS setup,
and KWin's screencast effect registration depends on compositor state that
relies on all displays being correctly initialised at startup.

The `nvidia-bug-report.log` filed in this thread (Session 3) showed Xid 51
and Xid 69 on driver init. These coincide with the KWin boot window, which
is why a `systemctl --user restart plasma-kwin_wayland` does not fix the
issue — it restarts KWin but not the underlying NVIDIA/DRM state.

### What does and doesn't help

- `systemctl --user restart plasma-kwin_wayland` — **does not fix it**.
  KWin restarts but the CRTC format mismatch persists from the initial
  driver load; the screencast effect fails to register again.
- A full display power cycle (DP-4 and DP-5 off, then on, without reboot)
  — **sometimes fixes it transiently** by forcing CRTC re-enumeration.
- Reboot — **fixes it for a session** until the tiled panel's CRTC state
  causes the mismatch again on next cold boot.

### Cross-filed KDE bugs

- https://bugs.kde.org/show_bug.cgi?id=493277 (CRTC tiling format mismatch)
- https://bugs.kde.org/show_bug.cgi?id=503870 (TILE gap / wl_output split)

Both KDE bugs are aware of this NVIDIA forum thread. The `kwin_screencast_effect_active`
finding has been posted to both.

### Tooling

[wayland-spectre](https://forgejo.wanderingmonster.dev/WanderingMonster/wayland-spectre)
now has automated checks for all of the above. Happy to run any additional
diagnostics if that would help the driver team.
