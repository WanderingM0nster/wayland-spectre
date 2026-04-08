# AGENTS.md — wayland-spectre

AI coding context for agents working on this codebase.
Read this before making any changes.

---

## What this project is

`wayland-spectre` is a **diagnostic tool**, not a general utility.
Every design decision prioritises **clarity of failure diagnosis** over features.

It checks seven known failure modes for Wayland screen sharing on KDE Plasma / Bazzite,
presents results in a Tauri v2 + Svelte 5 GUI, and offers one-click conservative fixes.

Phase 1 (bash script at `~/.local/bin/wayland-screenshare-diag.sh`) is complete and is
the **ground truth** for what each check does. When in doubt about check behaviour,
consult the bash script.

---

## Target system — do not generalise away from this

| Property | Value |
|----------|-------|
| OS | Bazzite 43.x (Fedora 43 immutable / OSTree) |
| Kernel | 6.17.x |
| NVIDIA driver | 595.x open modules |
| KDE Plasma | 6.x, Wayland session |
| Display | Dell UP3214Q tiled 4K (two 1920×2160 panels, DP-4 + DP-5) |
| Username | <username> · Hostname: <hostname> |
| Package manager | rpm-ostree (not dnf for system packages) |
| JS package manager | pnpm (never npm or yarn) |

**This tool is Linux/Wayland/KDE-only.** Do not add Windows or macOS code paths.
All checks assume a systemd user session, D-Bus session bus, and PipeWire.

---

## Repository layout

```
wayland-spectre/
├── src/                            Svelte 5 + SvelteKit frontend (SPA, SSR disabled)
│   ├── routes/+page.svelte         Main pipeline view — entry point for UI changes
│   ├── routes/+layout.ts           ssr=false, prerender=false — do not change
│   └── lib/
│       ├── types.ts                TypeScript mirror of Rust types — keep in sync
│       ├── utils.ts                cn() helper only
│       ├── stores/diagnostic.svelte.ts   All Svelte 5 Runes state lives here
│       └── components/
│           ├── LayerRow.svelte     Expandable layer row with Fix buttons
│           ├── FixButton.svelte    Calls diagnostic.executeFix()
│           ├── SummaryBar.svelte   Pass/warn/fail counts + system info
│           └── CaptureTest.svelte  PipeWire end-to-end test panel
└── src-tauri/src/
    ├── main.rs                     CLI-or-GUI router (argv detection)
    ├── lib.rs                      Tauri builder + invoke_handler registration
    ├── commands.rs                 Tauri command handlers (thin wrappers)
    ├── cli.rs                      Headless CLI with owo-colors output
    ├── domain/
    │   ├── types.rs                Single source of truth for all types
    │   └── errors.rs               DiagnosticError enum
    └── adapters/                   One file per external system
        ├── nvidia.rs   L0          /proc/driver/nvidia, /sys/module/nvidia_drm
        ├── dbus.rs     L1          busctl, systemctl --user, portal version
        ├── wayland.rs  L2/L3       wayland-info subprocess → native crate in Session 2
        ├── pipewire.rs L4          pw-dump subprocess → libpipewire in Session 2
        ├── flatpak.rs  L5          flatpak permission-show
        ├── env.rs      L6          systemctl --user show-environment
        └── kwin.rs     L7          busctl KWin D-Bus, ~/.config/kwinrc
```

---

## Type contract — CRITICAL

`src/lib/types.ts` and `src-tauri/src/domain/types.rs` must stay **exactly in sync**.
JSON is the serialisation contract between Rust and Svelte.

- `CheckStatus` variants serialise as `"PASS"`, `"WARN"`, `"FAIL"`, `"SKIP"` (uppercase strings)
- `Layer` variants serialise as `"L0"` through `"L7"`
- `fix` field is `Option<String>` in Rust / `string | null` in TypeScript
- Never add a field on one side without adding it on the other

---

## Adding a new check

1. Decide which layer it belongs to (L0–L7)
2. Add it to the appropriate adapter file in `src-tauri/src/adapters/`
3. Use `DiagnosticResult::pass()`, `::fail()`, `::warn()`, or `::skip()` constructors
4. The `fix` field must be a **complete, pasteable shell command** or `None`
5. Fix commands must be **conservative** — restarts only, never destructive operations
6. Never require root/sudo — all checks run as the normal user
7. No new check needs a UI change — `LayerRow` renders any result automatically

---

## Fix command safety rules

`commands.rs::execute_fix()` enforces these at runtime, but follow them at authoring time too:

- Single command only — no `|`, `;`, `&`, `>`, `<`, `$`, `` ` ``
- Allowed: `systemctl --user restart ...`, `flatpak permission-reset ...`, `busctl --user call ...`
- Never: `rm`, `mv`, file edits, anything requiring sudo
- The fix is run verbatim via `std::process::Command` — no shell interpolation

---

## Svelte 5 Runes patterns used in this project

```typescript
// State in .svelte.ts modules — always use getters for reactivity
let _value = $state<T>(initial)
export const store = { get value() { return _value } }

// Derived with logic
const computed = $derived.by(() => { ... })

// Props in components
let { propA, propB }: Props = $props()

// Reactive conditions
const isActive = $derived(something === 'active')
```

**Never use:**
- Svelte 4 `$: ` reactive statements
- `writable()` / `readable()` stores from `svelte/store`
- `export let` for component props (use `$props()`)
- `on:click` event syntax (use `onclick={handler}`)

---

## Tailwind 4 notes

This project uses Tailwind 4 (via `@tailwindcss/vite`).

- No `tailwind.config.js` — configuration is in `src/app.css` via `@theme inline { }`
- CSS variables for status colours: `--color-status-pass`, `--color-status-warn`, `--color-status-fail`, `--color-status-skip`
- Use `bg-status-pass/10` opacity syntax, not arbitrary values like `bg-[hsl(...)]`
- The design is dark-only — no light mode toggle needed

---

## Development commands

```bash
# GUI development
pnpm tauri dev

# Type-check frontend only
pnpm check

# CLI mode (no GUI, same binary)
cargo run --manifest-path src-tauri/Cargo.toml -- check
cargo run --manifest-path src-tauri/Cargo.toml -- check --json-only
cargo run --manifest-path src-tauri/Cargo.toml -- report

# Build release
pnpm tauri build
```

---

## Session 2 TODO (native crate adapters)

These are explicitly deferred — do not implement unless asked:

- `adapters/wayland.rs` — replace `wayland-info` subprocess with native `wayland-client` crate:
  connect to `$WAYLAND_DISPLAY`, enumerate `wl_registry` globals, check for
  `zkde_screencast_unstable_v1`, `wp_linux_drm_syncobj_manager_v1`, `ext_image_capture_source_v1`

- `adapters/dbus.rs` — replace `busctl` subprocess with native `zbus::Connection::session()`:
  `connection.call_method()` to introspect portal, enumerate session names for zombie detection

- `adapters/pipewire.rs::run_capture_test()` — replace `pw-dump` with a proper
  xdg-desktop-portal `org.freedesktop.portal.ScreenCast.CreateSession` D-Bus call

---

## Known upstream bugs (context for checks)

| Bug | Affects | Check |
|-----|---------|-------|
| xdg-desktop-portal #1953 | portal 1.20.3: O_RDONLY\|O_NOFOLLOW on /proc/<pid>/root → ELOOP | `dbus.rs::check_portal_version()` |
| KDE Bug 518650
  ✓ KDE bug 518698 — kwin_screencast effect absent from Loaded Effects (tiled display only) — https://bugs.kde.org/show_bug.cgi?id=518698 | KWin best-effort portal registration timing | `wayland.rs` zkde_screencast check |
| NVIDIA forum 331077 | KWin pageflip timeout on tiled display, Xid 51/69 | `nvidia.rs::check_explicit_sync()` |
| KDE bug 493277 | CRTC format mismatch AB30 vs AB4H on tiled panels → screencast plugin init failure | `kwin.rs::check_tiled_display()` + `wayland.rs::bug_d_screencast_globals` |
| KDE bug 503870 | Tile gap / wl_output split on tiled display | `kwin.rs::check_tiled_display()` |
| **Bug D** (synthesised) | KWin NOT advertising `zkde_screencast_unstable_v1` or `ext_image_capture_source_v1` — confirmed root cause on test system (tiled Dell UP3214Q + NVIDIA open 595.x). Both globals absent → `bug_d_screencast_globals` L3 FAIL | `wayland.rs::build_checks()` |

---

## Distribution packaging (Session 3)

AppImage and .deb are the supported bundle targets (`pnpm tauri build`).

### Icons
The repo ships a single `src-tauri/icons/icon.png`. Before a public release
run `pnpm tauri icon src-tauri/icons/icon.png` to generate the full size set
(32×32, 128×128, etc.) that Tauri embeds into the AppImage and .deb.

### Building on Bazzite / Fedora
```bash
# System deps (already on Bazzite 43.x)
sudo dnf install wayland-utils dbus-devel webkit2gtk4.1-devel

# Tauri downloads linuxdeploy automatically — requires internet first build
pnpm tauri build

# AppImage location after build:
# src-tauri/target/release/bundle/appimage/wayland-spectre_*.AppImage
```

### Test the AppImage before committing
```bash
./wayland-spectre_*.AppImage -- check          # must exit 0 or 1 (no crash)
./wayland-spectre_*.AppImage -- check --json-only | jq .summary
./wayland-spectre_*.AppImage -- report         # must produce a .tar.gz
```

---

## Files that must not be changed without explicit instruction

| File | Reason |
|------|--------|
| `src/routes/+layout.ts` | `ssr=false` is required for Tauri SPA mode |
| `src-tauri/src/domain/types.rs` | JSON contract — changes break the frontend |
| `src/lib/types.ts` | Must mirror `types.rs` exactly |
| `src-tauri/src/commands.rs::execute_fix()` | Safety filter — do not weaken |

---

## Bug report bundle contents (Session 3)

`cargo run -- report` (or `AppImage -- report`) produces a `.tar.gz` containing:

| File | Source |
|------|--------|
| `diagnostics.json` | Full structured report (all checks) |
| `SUMMARY.txt` | Human-readable FAILs + WARNs for copy-paste into bug trackers |
| `journal-plasma-kwin_wayland.log` | Last 200 lines |
| `journal-xdg-desktop-portal.log` | Last 200 lines |
| `journal-xdg-desktop-portal-kde.log` | Last 200 lines |
| `journal-pipewire.log` | Last 200 lines |
| `kwin-support-info.txt` | KWin `supportInformation` D-Bus call (raw) |
| `nvidia-driver-version.txt` | `/proc/driver/nvidia/version` |
| `nvidia-smi.txt` | `nvidia-smi` GPU/driver query |
| `wayland-info.txt` | `wayland-info` globals dump (if installed) |
| `os-release.txt` | `/etc/os-release` (Bazzite image tag) |

Attach the `.tar.gz` to NVIDIA forum thread 331077 and Bazzite community thread 11901.

---

## Output locations

- JSON diagnostic report: `/tmp/screenshare-diag-<epoch>.json` (same path as bash script)
- Bug report bundle: `/tmp/wayland-spectre-bugreport-<epoch>.tar.gz`
- Creations folder: `/path/to/Creations/`
- Forgejo: `https://forgejo.wanderingmonster.dev/WanderingMonster`
- NVIDIA forum thread: `https://forums.developer.nvidia.com/t/331077`
- Bazzite community thread: `https://universal-blue.discourse.group/t/11901`

