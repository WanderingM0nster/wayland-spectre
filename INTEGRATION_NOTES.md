# Session 2 — Integration notes

## Cargo.toml  (src-tauri/Cargo.toml)

```toml
# Uncomment / add:
wayland-client = "0.31"          # was already stubbed
zbus = { version = "5", default-features = false, features = ["tokio"] }
futures-util = "0.3"
```

`default-features = false` drops the async-io I/O thread; we're in Tauri v2 / tokio,
so the tokio feature is the only one needed.

`zbus::zvariant` is re-exported inside zbus — no separate zvariant dep.

---

## commands.rs — async signatures

`dbus.rs` and `pipewire.rs` are now `async`.
If the Tauri command handlers that call them aren't already `async fn`, add `async`:

```rust
#[tauri::command]
async fn run_diagnostics() -> Result<Vec<CheckResult>, String> {
    let mut results = Vec::new();

    // wayland.rs is sync — run in spawn_blocking to keep command handler non-blocking
    let wl = tokio::task::spawn_blocking(check_wayland).await
        .map_err(|e| e.to_string())?;
    results.extend(wl);

    results.extend(check_dbus().await);
    results.extend(check_pipewire().await);
    // … other adapters
    Ok(results)
}
```

---

## domain/types.rs — CheckResult fields

The new adapters assume this shape (adjust to match actuals):

```rust
pub struct CheckResult {
    pub id:     String,
    pub layer:  u8,          // 0–7
    pub name:   String,
    pub status: CheckStatus,
    pub detail: String,
    pub fix:    Option<FixAction>,
}

pub struct FixAction {
    pub label:   String,
    pub command: Vec<String>,
}

#[derive(PartialEq)]
pub enum CheckStatus { Pass, Warn, Fail, Skip }
```

If `layer` is a `Layer` enum instead of `u8`, s/`layer: 1,`/`layer: Layer::L1,`/ etc.

---

## Expected test delta

All 25 existing unit tests should continue to pass — the new files add:

| File          | New tests | What they test                              |
|---------------|-----------|---------------------------------------------|
| wayland.rs    | +5        | build_checks() logic, no Wayland display    |
| dbus.rs       | +5        | version classification, XML zombie count    |
| pipewire.rs   | +5        | path construction, ELOOP detection, codes   |

Total: ~40 unit tests after Session 2.

---

## Expected diagnostic change on test system

**Before (Session 1):**
- L3 `zkde_screencast_unstable_v1` → FAIL  (wayland-info subprocess failing)
- L3 `ext_image_capture_source_v1` → FAIL  (same)
- L4 `pipewire_*`                  → PASS  (pw-dump found old node = false positive)

**After (Session 2), assuming Bug C still present:**
- L3 `zkde_screencast_unstable_v1` → PASS  (direct wl_registry roundtrip)
- L3 `ext_image_capture_source_v1` → PASS  (same)
- L4 `portal_create_session`       → FAIL  (CreateSession triggers ELOOP; correctly reported)
- L4 `pipewire_socket`             → PASS  (socket exists)

Net: 22→24 PASS, 2→1 FAIL (the real Bug C, now properly attributed to L4).
