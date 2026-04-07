# wayland-spectre

Wayland screen sharing diagnostics for KDE Plasma / Bazzite. Phase 2 of the `wayland-screenshare-diag` project — a Tauri v2 + Svelte 5 GUI with a shared Rust diagnostic core.

## Prerequisites

```bash
rustup update stable
pnpm --version   # 9.x
node --version   # 22.x (via nvm or system)
```

Bazzite system deps (already present on the target system):
```bash
sudo dnf install wayland-utils   # for wayland-info
```

## Setup

```bash
# 1. Install JS deps
pnpm install

# 2. Add shadcn-svelte components
pnpm dlx shadcn-svelte@latest add button badge card progress separator

# 3. Development (GUI)
pnpm tauri dev

# 4. CLI mode (same binary)
cargo run --manifest-path src-tauri/Cargo.toml -- check
cargo run --manifest-path src-tauri/Cargo.toml -- check --json-only
cargo run --manifest-path src-tauri/Cargo.toml -- report
```

## Architecture

```
wayland-spectre/
├── src/                        Svelte 5 + SvelteKit frontend (SPA mode)
│   ├── routes/+page.svelte     Main pipeline view
│   └── lib/
│       ├── types.ts            TypeScript mirror of Rust domain types
│       ├── stores/diagnostic.svelte.ts   Svelte 5 Runes state
│       └── components/         LayerRow, FixButton, SummaryBar, CaptureTest
└── src-tauri/src/
    ├── main.rs                 CLI-or-GUI router
    ├── lib.rs                  Tauri builder
    ├── commands.rs             Tauri command handlers
    ├── cli.rs                  Headless CLI (owo-colors output)
    ├── domain/types.rs         DiagnosticResult, CheckStatus, etc.
    └── adapters/               One file per external system
        ├── nvidia.rs           L0: driver, modeset, explicit_sync
        ├── dbus.rs             L1: portal health, zombie sessions, portal version
        ├── wayland.rs          L2/L3: backend conflict, Wayland globals
        ├── pipewire.rs         L4: PipeWire graph, capture test
        ├── flatpak.rs          L5: permission store deny entries
        ├── env.rs              L6: systemd user environment
        └── kwin.rs             L7: screencast plugin, compositor type
```

## Session roadmap

| Session | Work |
|---------|------|
| 1 (this) | Scaffold, domain types, all adapters (subprocess-based), full Svelte UI |
| 2 | Replace wayland adapter with native `wayland-client` crate; replace dbus adapter with native `zbus`; live capture test via portal D-Bus |
| 3 | Bug report bundle polish, Forgejo push, AppImage packaging |

## Upstream bugs being tracked

| Bug | URL |
|-----|-----|
| xdg-desktop-portal #1953 | https://github.com/flatpak/xdg-desktop-portal/issues/1953 |
| KDE Bug 518650 | https://bugs.kde.org/show_bug.cgi?id=518650 |
| NVIDIA forum 331077 | https://forums.developer.nvidia.com/t/331077 |
