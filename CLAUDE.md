# wayland-spectre / KDE Wayland — Session 7 brief

---

## Project context

This project operates on two parallel tracks:

**Track A — wayland-spectre tool**
A Tauri v2 + Svelte 5 + Rust diagnostic tool that identifies why Wayland
screensharing fails on KDE Plasma / Bazzite. Built specifically to diagnose
and document the Critical-severity bugs below. The tool is the instrument;
upstream bug filings are the goal.

**Track B — Broader KDE Wayland contribution tier list**
A prioritised list of KDE/Wayland issues to contribute to, ranging from
testing existing MRs (days) to implementing protocols (months). The Critical
bugs that were at the top of this list have been addressed by Track A work
and no longer appear in the active tier list.

---

## Critical bugs — status

These were rated Critical at project start. Now addressed at diagnostic +
filing level; awaiting upstream fixes.

### Critical 1 — Screensharing broken: CRTC pixel format mismatch (AB30 vs AB4H)
- **Root cause:** NVIDIA RTX 5090 open modules negotiate format AB30 for tiled
  Dell UP3214Q (DP-4 + DP-5) but KWin expects AB4H. CRTC setup fails at
  compositor startup → kwin_screencast effect never registers → all screensharing
  silently broken.
- **Filed:** NVIDIA forum 331077 (latest post /26), KDE bug 493277, KDE bug 503870
- **Status:** Open. NVIDIA moderator amrits filing internal bug. No patch yet.
- **Key finding (Session 6):** Failure is conditional — adding any secondary
  HDMI monitor resolves it on next boot (kwin_screencast effect registers
  correctly). This is the precise reproducer in KDE bug 518698.

### Critical 2 — Portal registration ELOOP (O_RDONLY|O_NOFOLLOW on magic symlink)
- **Root cause:** xdg-desktop-portal opens /proc/<pid>/root with
  O_RDONLY|O_NOFOLLOW. Kernel magic symlinks always return ELOOP with
  O_NOFOLLOW regardless of destination. Fix: O_PATH|O_NOFOLLOW.
- **Filed:** xdg-desktop-portal #1953 with root cause + fix identified
- **Status:** Open. Root cause and fix documented, awaiting upstream merge.

### Major bug 3 — kwin_screencast effect absent from Loaded Effects (midway)
- **Root cause:** Effect registration is conditional on display configuration
  at startup. Tiled display only → effect fails silently. Secondary monitor
  present → effect registers correctly.
- **Filed:** KDE bug 518698 (Session 6) — precise secondary-monitor dependency
  documented as reproducer.
- **Status:** Just filed. Awaiting response from Zamundaaa (KDE/KWin dev).
  Clean reproducer makes this highly actionable.
- **Hardening proposal:** KDE bug 518650 — KWin should create
  zkde_screencast_unstable_v1 regardless of portal registration success.

---

## Broader contribution tier list — current state

The Critical bugs above are gone from this list (addressed). Remaining work:

### Tier 1 — Quick wins: test/review existing MRs (days)
| # | Issue | Severity | Notes |
|---|-------|----------|-------|
| 1 | Multi-monitor TILE/MST | Medium | **Test MR !1174** (work/tiled-displays branch) — directly relevant to UP3214Q tiled display. You are one of very few people who can test this meaningfully. May overlap with Bug A fix. **Phase 1 done (Session 7):** drm_info + journal posted to [MR !1174 comment](https://invent.kde.org/plasma/kwin/-/merge_requests/1174#note_1466367). Phase 2 (build + test branch) pending Zamundaaa response. ★★★★★ |
| 2 | Per-app keyboard layouts | Medium/Plasma | Review & test MR !5963 ★★★★☆ |
| 3 | Graphic tablet multi-strip | Info/Plasma | Test MR #3353, report results ★★★★☆ |

### Tier 2 — Quick wins: documentation/guides (days–week)
| # | Issue | Severity | Notes |
|---|-------|----------|-------|
| 4 | Clipboard & X11 migration | Medium | Write xclip→wl-copy/wl-paste/xkill alternatives guide ★★★★☆ |
| 5 | Global hotkeys & automation | High | Write compat shim docs + portal adoption guide ★★★☆☆ |

### Tier 3 — Medium effort: testing + write-up (1–2 weeks)
| # | Issue | Severity | Notes |
|---|-------|----------|-------|
| 6 | Color management & HDR | High | Test & document HDR + ICC on Bazzite KDE. Mesa Vulkan path nearly works, your Bazzite setup is ideal. ★★★☆☆ |
| 7 | Accessibility gaps | Medium/Plasma | Survey AT tool compat + document workarounds ★★★☆☆ |

### Tier 4 — Harder: coding required (weeks–months)
| # | Issue | Severity | Notes |
|---|-------|----------|-------|
| 8 | Konsole/window activation | Info/Plasma | Implement xdg-activation in Konsole CLI open path ★★★☆☆ |
| 9 | Session restore | High/Plasma | Implement ext-session-management in KWin ★★☆☆☆ |
| 10 | Remote desktop/headless RDP | High/Plasma | Prototype KWin virtual outputs ★★☆☆☆ |
| 11 | Compositor fragmentation | High | Audit protocol extension coverage matrix ★★☆☆☆ |

---

## wayland-spectre tool — current state

**Version:** 0.3.0 Frostcrystal Gelatinous Cube (released Session 6)
**Forgejo:** https://forgejo.wanderingmonster.dev/WanderingMonster/wayland-spectre
**GitHub mirror:** https://github.com/WanderingM0nster/wayland-spectre
**AppImageHub PR:** https://github.com/AppImage/appimage.github.io/pull/3733 (open, Session 6)
**Bazzite thread:** https://universal-blue.discourse.group/t/11901

**Diagnostic results (tiled display only, no secondary monitor):**
- 63 tests passing, 0 failing
- 4 confirmed FAILs — all root-caused to Critical Bug 1:
  - L3: zkde_screencast_unstable_v1 not advertised
  - L3: ext_image_capture_source_v1 not advertised
  - L3: bug_d_screencast_globals (synthesised)
  - L7: kwin_screencast_effect_active — plugin loaded, effect absent

**Session 6 additions:**
- KwinJournal auto-expands (amber badge) when kwin_screencast_effect_active FAIL
- `--check <LAYER>` CLI shorthand; colour-coded layer headers
- FixButton two-step confirmation for destructive commands (restart compositor)
- AppImage built and distributed; GitHub mirror live; AppImageHub PR open
- README TL;DR for non-technical users

---

## System profile
- OS: Bazzite bazzite-nvidia-open:stable, image 43.20260406, Fedora 43 immutable
- Kernel: 6.17.7-ba29.fc43.x86_64
- GPU: RTX 5090 (NVIDIA driver 595.58.03 open modules, card2/renderD129) + AMD iGPU (card1)
- CPU: Ryzen 9 9950X3D
- Display: Dell UP3214Q tiled 4K — DP-4 + DP-5 (1920×2160 each)
  Secondary HDMI-A-3 (2560×1440@100Hz) resolves kwin_screencast_effect_active FAIL
- Desktop: KDE Plasma 6.6.3, Wayland session

## Working directory & tokens
- Working dir: <WORKDIR>/
- Forgejo token: <FORGEJO_TOKEN>
- GitHub token (WanderingM0nster): <GITHUB_TOKEN>
- GitHub remote already added as 'github' in local repo
- Files.zip pattern (flat): unzip -o files.zip -d files-extracted; cp files-extracted/<file> $REPO/<dest>
- Single-file changes: commit directly via Forgejo API token (no zip needed)
- After each session: git pull --ff-only && git push github main --tags

## Build
- Full: NO_STRIP=1 pnpm tauri build  (Fedora 43 RELR workaround)
- Dev:  pnpm tauri dev
- Test: cargo t
- CLI:  src-tauri/target/release/wayland-spectre

---

## Session 7 proposed goals

### Immediate (check first)
1. **AppImageHub PR #3733** — check CI result; fix if failed; confirm merge status
2. **KDE bug 518698** — check for Zamundaaa response; reply if needed
3. **Tier 1 #1 — MR !1174** — ~~test multi-monitor TILE/MST MR on UP3214Q setup~~
   Phase 1 done: drm_info + journal [posted](https://invent.kde.org/plasma/kwin/-/merge_requests/1174#note_1466367) 2026-04-08. Phase 2 (build + test) awaiting Zamundaaa response.

### wayland-spectre v0.4.0 Infernoheart
Candidate features (decide at session start):
- Hostname redaction option in `generate_bug_report` — sanitize before public filing
- Add check for xdg-desktop-portal version (Bug B ELOOP affects specific versions)
- KwinJournal: filter to current boot only (multiple boots currently visible)
- "Copy fix command" button — let user paste + run manually instead of auto-exec
- Bazzite community thread update post for Session 6 findings

### Release naming sequence
  v0.4.0 Infernoheart  ← next (fiery, intense, dramatic)
  v0.5.0 Lumigrotto
  v0.6.0 Mysticweaver
  ...

---

## Always
- GPL-3.0-or-later licence header in all new source files
- Sanitize hostname (arctic), username (<username>), local paths before public commits
- Artwork committed to docs/releases/ BEFORE creating tag
- Both Forgejo AND GitHub releases need AppImage attached
- Rich/coloured terminal output (red/amber/green) for scripts
