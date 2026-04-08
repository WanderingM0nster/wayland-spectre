## wayland-spectre v0.4.0 Infernoheart: update

Quick update on the Wayland screensharing diagnostic work.

### What's new in v0.4.0

- **Bug report redaction**: the "Bug report" button now has a "Redact" checkbox
  (on by default) that replaces your hostname, username, and home paths with
  safe placeholders before saving. Makes it easier to share reports publicly
  without leaking personal info.

- **KWin Journal: current boot only**: the journal panel now filters to the
  current boot, so you only see entries from this session rather than a mix
  of multiple boots.

- **"Copy cmd" button on fix actions**: every fix suggestion now has a
  "Copy cmd" button alongside "Fix it", so you can paste and run the command
  yourself in a terminal instead of letting the tool execute it directly.
  Useful if you want to review what it does first, or if you prefer to run
  fixes in a specific terminal context.

### Upstream bug status

The three critical bugs we identified are all filed and awaiting upstream fixes:

1. **CRTC pixel format mismatch** (NVIDIA RTX 5090 + tiled Dell UP3214Q):
   NVIDIA forum 331077, KDE bugs 493277 and 503870. NVIDIA moderator has
   filed an internal bug. No patch yet.

2. **Portal ELOOP** (xdg-desktop-portal #1953): root cause and fix identified,
   PR open upstream. The `O_RDONLY|O_NOFOLLOW` on `/proc/<pid>/root` needs to
   be `O_PATH|O_NOFOLLOW`.

3. **kwin_screencast effect absent** (KDE bug 518698): filed with a clean
   reproducer. The kwin_screencast effect fails to register when only the
   tiled display is connected; adding any secondary monitor resolves it.

### KWin tiled display MR

We posted `drm_info` output and KWin journal data to
[KWin MR !1174](https://invent.kde.org/plasma/kwin/-/merge_requests/1174)
(the tiled display support MR by Zamundaaa). Our Dell UP3214Q + RTX 5090
is one of the few real tiled display setups that can test this. Waiting for
confirmation that the branch is ready for NVIDIA + MST testing.

### Download

AppImage available on the
[GitHub releases page](https://github.com/WanderingM0nster/wayland-spectre/releases).

If you have a Bazzite + NVIDIA setup and screensharing is broken, give
wayland-spectre a try. The diagnostic output has been directly useful in
filing the bugs above.
