# Faro Distribution & Packaging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn Faro into a self-installing, tray-controlled, auto-updating app distributed as prebuilt Windows + macOS installers via GitHub Releases, so colleagues install it with no commands and no manual file editing.

**Architecture:** A new native-Rust module registers Claude Code hooks at launch (replacing `install-windows.ps1`); a system tray gives the chromeless overlay a control surface; `tauri-plugin-autostart` and `tauri-plugin-updater` add login-start and self-update; a GitHub Actions matrix builds both platforms and publishes signed updater artifacts to Releases.

**Tech Stack:** Tauri 2, Rust (axum), React 19 + TypeScript, `serde_json` (`preserve_order`), `dirs`, `tauri-plugin-autostart`, `tauri-plugin-updater`, `tauri-apps/tauri-action` (GitHub Actions).

## Global Constraints

- **Platforms:** Windows 10/11 and macOS (universal binary, Intel + Apple Silicon).
- **OS code signing:** NONE for now; deferring it must require only adding certs/secrets later, never re-architecting.
- **Updater signing:** REQUIRED, separate from OS signing — a free minisign (ed25519) keypair; public key committed in `tauri.conf.json`, private key + password only as GitHub Actions secrets.
- **Distribution repo:** `stefff94/faro`. Release trigger: pushing a `v*` git tag.
- **Updater endpoint:** `https://github.com/stefff94/faro/releases/latest/download/latest.json`.
- **Hook command stability:** the registered hook command points at `<claude-home>/hooks/agent-monitor-report.sh` (outside the app bundle) so app updates never invalidate it.
- **No silent settings corruption:** malformed `settings.json` aborts the write with no change; every write is preceded by a `settings.json.faro-bak` backup; output is UTF-8 without BOM; existing key order preserved.
- **The 7 hook events:** `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `Notification`, `Stop`, `StopFailure`, `SessionEnd`.
- **Faro idempotency marker:** a Faro hook group is any group whose command contains the substring `agent-monitor-report`; re-registration drops existing Faro groups and re-adds exactly one fresh group per event, preserving all non-Faro groups.
- **Reporter delivery:** the reporter script is embedded in the binary via `include_str!` (dev/prod parity) and written out at launch — chosen over `bundle.resources` because `resource_dir()` is not reliably populated in `tauri dev`. This satisfies the spec's "reporter ships inside the app, written to a stable path".

---

### Task 1: Hook-registration core (pure Rust, TDD)

Self-contained module with the merge logic and the filesystem install routine. This is the only task with rich unit tests; everything downstream calls into it.

**Files:**
- Create: `src-tauri/src/hooks_install.rs`
- Modify: `src-tauri/Cargo.toml` (deps)
- Modify: `src-tauri/src/lib.rs:1-8` (add `pub mod hooks_install;`)

**Interfaces:**
- Produces:
  - `pub const FARO_MARK: &str` = `"agent-monitor-report"`
  - `pub const EVENTS: [&str; 7]`
  - `pub fn merge_faro_hooks(settings: serde_json::Value, command: &str) -> serde_json::Value`
  - `pub fn claude_home() -> Option<std::path::PathBuf>`
  - `pub struct InstallReport { pub registered: bool, pub backup_made: bool, pub error: Option<String> }`
  - `pub fn install_hooks(claude_home: &std::path::Path) -> InstallReport`

- [ ] **Step 1: Add dependencies**

In `src-tauri/Cargo.toml`, replace the `serde_json = "1"` line and add `dirs` so the `[dependencies]` block reads:

```toml
[dependencies]
axum = "0.7"
tokio = { version = "1", features = ["full"] }
tauri = { version = "2", features = ["macos-private-api"] }
tauri-plugin-opener = "2"
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1", features = ["preserve_order"] }
dirs = "5"
```

(`tempfile = "3"` is already in `[dev-dependencies]`.)

- [ ] **Step 2: Register the module**

In `src-tauri/src/lib.rs`, add to the module list at the top (after `pub mod git;`):

```rust
pub mod hooks_install;
```

- [ ] **Step 3: Write the failing tests**

Create `src-tauri/src/hooks_install.rs` with ONLY the test module first (it will not compile yet — that is the failing state):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const CMD: &str = "bash \"/home/u/.claude/hooks/agent-monitor-report.sh\"";

    #[test]
    fn registers_all_seven_events() {
        let out = merge_faro_hooks(json!({}), CMD);
        let hooks = out["hooks"].as_object().unwrap();
        for evt in EVENTS {
            let arr = hooks[evt].as_array().unwrap();
            assert_eq!(arr.len(), 1, "event {evt} should have one group");
            assert_eq!(arr[0]["hooks"][0]["command"], CMD);
            assert_eq!(arr[0]["hooks"][0]["type"], "command");
        }
    }

    #[test]
    fn idempotent_no_duplicate_faro_groups() {
        let once = merge_faro_hooks(json!({}), CMD);
        let twice = merge_faro_hooks(once.clone(), CMD);
        assert_eq!(once, twice);
        for evt in EVENTS {
            assert_eq!(twice["hooks"][evt].as_array().unwrap().len(), 1);
        }
    }

    #[test]
    fn preserves_non_faro_groups() {
        let existing = json!({
            "hooks": { "PreToolUse": [
                { "hooks": [ { "type": "command", "command": "/usr/bin/other-tool" } ] }
            ] }
        });
        let out = merge_faro_hooks(existing, CMD);
        let arr = out["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["hooks"][0]["command"], "/usr/bin/other-tool");
        assert!(arr[1]["hooks"][0]["command"].as_str().unwrap().contains(FARO_MARK));
    }

    #[test]
    fn replaces_stale_faro_group_path() {
        let existing = json!({
            "hooks": { "Stop": [
                { "hooks": [ { "type": "command", "command": "bash \"/old/agent-monitor-report.sh\"" } ] }
            ] }
        });
        let out = merge_faro_hooks(existing, CMD);
        let arr = out["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["hooks"][0]["command"], CMD);
    }

    #[test]
    fn preserves_other_top_level_keys() {
        let out = merge_faro_hooks(json!({ "model": "opus", "hooks": {} }), CMD);
        assert_eq!(out["model"], "opus");
    }

    #[test]
    fn coerces_non_object_settings() {
        let out = merge_faro_hooks(json!(null), CMD);
        assert!(out["hooks"].is_object());
    }

    #[test]
    fn install_creates_script_and_settings() {
        let tmp = tempfile::tempdir().unwrap();
        let rep = install_hooks(tmp.path());
        assert!(rep.registered, "error: {:?}", rep.error);
        assert!(tmp.path().join("hooks/agent-monitor-report.sh").exists());
        let raw = std::fs::read_to_string(tmp.path().join("settings.json")).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(settings["hooks"]["Stop"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn install_aborts_on_malformed_settings() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("settings.json"), "{ not json").unwrap();
        let rep = install_hooks(tmp.path());
        assert!(!rep.registered);
        assert!(rep.error.is_some());
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("settings.json")).unwrap(),
            "{ not json"
        );
    }

    #[test]
    fn install_makes_backup_when_settings_exists() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("settings.json"), "{\"model\":\"opus\"}").unwrap();
        let rep = install_hooks(tmp.path());
        assert!(rep.registered);
        assert!(rep.backup_made);
        assert!(tmp.path().join("settings.json.faro-bak").exists());
    }

    #[test]
    fn install_command_uses_forward_slashes_and_bash() {
        let tmp = tempfile::tempdir().unwrap();
        install_hooks(tmp.path());
        let raw = std::fs::read_to_string(tmp.path().join("settings.json")).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let cmd = settings["hooks"]["Stop"][0]["hooks"][0]["command"].as_str().unwrap();
        assert!(cmd.starts_with("bash \""));
        assert!(!cmd.contains('\\'));
        assert!(cmd.contains("/hooks/agent-monitor-report.sh"));
    }
}
```

- [ ] **Step 4: Run tests to verify they fail**

Run: `cd src-tauri && cargo test hooks_install`
Expected: FAIL — compile errors (`merge_faro_hooks`, `install_hooks`, `EVENTS`, `FARO_MARK`, `InstallReport` not found).

- [ ] **Step 5: Write the implementation**

Prepend the implementation ABOVE the test module in `src-tauri/src/hooks_install.rs`:

```rust
//! Native, cross-platform registration of Faro's Claude Code hooks.
//! Mirrors the behaviour of the retired hooks/install-windows.ps1 in Rust.

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

/// Substring identifying a Faro hook group (any path/form).
pub const FARO_MARK: &str = "agent-monitor-report";

/// The Claude Code hook events Faro registers.
pub const EVENTS: [&str; 7] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "Notification",
    "Stop",
    "StopFailure",
    "SessionEnd",
];

/// The reporter script, embedded at compile time so it ships with the binary
/// and is identical in `tauri dev` and packaged builds.
pub const REPORTER_BODY: &str = include_str!("../../hooks/agent-monitor-report.sh");

const REPORTER_NAME: &str = "agent-monitor-report.sh";

#[derive(Debug)]
pub struct InstallReport {
    pub registered: bool,
    pub backup_made: bool,
    pub error: Option<String>,
}

/// Resolve the Claude home dir: `FARO_CLAUDE_HOME` override, else `~/.claude`.
pub fn claude_home() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("FARO_CLAUDE_HOME") {
        return Some(PathBuf::from(p));
    }
    dirs::home_dir().map(|h| h.join(".claude"))
}

fn is_faro_group(grp: &Value) -> bool {
    grp.get("hooks")
        .and_then(|h| h.as_array())
        .map(|arr| {
            arr.iter().any(|hh| {
                hh.get("command")
                    .and_then(|c| c.as_str())
                    .map(|s| s.contains(FARO_MARK))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// Merge one fresh Faro group per event into `settings`, dropping any existing
/// Faro group and preserving every other group and top-level key.
pub fn merge_faro_hooks(mut settings: Value, command: &str) -> Value {
    if !settings.is_object() {
        settings = json!({});
    }
    let obj = settings.as_object_mut().unwrap();

    let hooks_entry = obj.entry("hooks").or_insert_with(|| json!({}));
    if !hooks_entry.is_object() {
        *hooks_entry = json!({});
    }
    let hooks = hooks_entry.as_object_mut().unwrap();

    for evt in EVENTS {
        let mut kept: Vec<Value> = Vec::new();
        if let Some(arr) = hooks.get(evt).and_then(|v| v.as_array()) {
            for grp in arr {
                if !is_faro_group(grp) {
                    kept.push(grp.clone());
                }
            }
        }
        kept.push(json!({
            "hooks": [ { "type": "command", "command": command } ]
        }));
        hooks.insert(evt.to_string(), Value::Array(kept));
    }
    settings
}

/// Write the reporter into `<claude_home>/hooks/` and register the 7 events in
/// `settings.json`. Aborts without writing if settings.json is malformed.
pub fn install_hooks(claude_home: &Path) -> InstallReport {
    let hooks_dir = claude_home.join("hooks");
    if let Err(e) = fs::create_dir_all(&hooks_dir) {
        return InstallReport { registered: false, backup_made: false, error: Some(format!("create hooks dir: {e}")) };
    }

    let dest = hooks_dir.join(REPORTER_NAME);
    if let Err(e) = fs::write(&dest, REPORTER_BODY.as_bytes()) {
        return InstallReport { registered: false, backup_made: false, error: Some(format!("write reporter: {e}")) };
    }

    let command = format!("bash \"{}\"", dest.to_string_lossy().replace('\\', "/"));

    let settings_path = claude_home.join("settings.json");
    let settings: Value = match fs::read_to_string(&settings_path) {
        Ok(raw) if !raw.trim().is_empty() => match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                return InstallReport { registered: false, backup_made: false, error: Some(format!("settings.json non valido: {e}")) }
            }
        },
        _ => json!({}),
    };

    let merged = merge_faro_hooks(settings, &command);

    let mut backup_made = false;
    if settings_path.exists() {
        let bak = claude_home.join("settings.json.faro-bak");
        if fs::copy(&settings_path, &bak).is_ok() {
            backup_made = true;
        }
    }

    let out = serde_json::to_string_pretty(&merged).unwrap();
    if let Err(e) = fs::write(&settings_path, out.as_bytes()) {
        return InstallReport { registered: false, backup_made, error: Some(format!("write settings: {e}")) };
    }

    InstallReport { registered: true, backup_made, error: None }
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cd src-tauri && cargo test hooks_install`
Expected: PASS — all 10 tests green.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/hooks_install.rs src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat(install): native cross-platform Claude Code hook registration"
```

---

### Task 2: Wire hook install + first-run gating commands into the app

The app calls `install_hooks` on launch only if the user has consented before; exposes commands the frontend uses for first-run consent. Verified by build + a dev smoke test against a throwaway `FARO_CLAUDE_HOME`.

**Files:**
- Modify: `src-tauri/src/lib.rs` (commands, setup wiring, invoke handler)

**Interfaces:**
- Consumes: `hooks_install::{install_hooks, claude_home, InstallReport}` (Task 1).
- Produces (Tauri commands, callable from the frontend via `invoke`):
  - `faro_setup_state() -> bool` — true if the user has already consented.
  - `faro_register_hooks() -> Result<bool, String>` — registers hooks, writes the consent marker, enables autostart; returns `Ok(true)` or `Err(message)`.

- [ ] **Step 1: Add imports and helpers in lib.rs**

In `src-tauri/src/lib.rs`, add near the other `use` lines (after `use tauri::{Emitter, Manager, PhysicalPosition};`):

```rust
use std::path::PathBuf;
```

Add these helpers above `pub fn run()`:

```rust
/// Path of the one-shot consent marker inside the app config dir.
fn consent_marker(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("setup-consented"))
}

/// Register hooks and, on success, persist consent and enable login autostart.
/// Used by both the command and the silent re-assert on launch.
fn do_register(app: &tauri::AppHandle) -> crate::hooks_install::InstallReport {
    let Some(home) = crate::hooks_install::claude_home() else {
        return crate::hooks_install::InstallReport {
            registered: false, backup_made: false,
            error: Some("home directory non trovata".into()),
        };
    };
    let report = crate::hooks_install::install_hooks(&home);
    if report.registered {
        if let Some(marker) = consent_marker(app) {
            if let Some(parent) = marker.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&marker, "1");
        }
    }
    report
}
```

- [ ] **Step 2: Add the two commands**

Add to `src-tauri/src/lib.rs` (near the other `#[tauri::command]` fns):

```rust
#[tauri::command]
fn faro_setup_state(app: tauri::AppHandle) -> bool {
    consent_marker(&app).map(|p| p.exists()).unwrap_or(false)
}

#[tauri::command]
fn faro_register_hooks(app: tauri::AppHandle) -> Result<bool, String> {
    let report = do_register(&app);
    if report.registered {
        Ok(true)
    } else {
        Err(report.error.unwrap_or_else(|| "registrazione fallita".into()))
    }
}
```

- [ ] **Step 3: Register the commands in the invoke handler**

In `src-tauri/src/lib.rs`, extend `tauri::generate_handler!`:

```rust
        .invoke_handler(tauri::generate_handler![
            greet,
            cursor_in_window,
            set_cursor_passthrough,
            resize_to_content,
            faro_setup_state,
            faro_register_hooks
        ])
```

- [ ] **Step 4: Silent re-assert on launch when already consented**

In the `.setup(|app| { ... })` closure in `src-tauri/src/lib.rs`, immediately after `app.manage(AnchorTop(Mutex::new(None)));`, add:

```rust
            // If the user consented in a past launch, silently re-assert the hook
            // registration (idempotent) so a moved home path or updated reporter heals.
            {
                let handle = app.handle().clone();
                if consent_marker(&handle).map(|p| p.exists()).unwrap_or(false) {
                    let _ = do_register(&handle);
                }
            }
```

- [ ] **Step 5: Build**

Run: `cd src-tauri && cargo build`
Expected: builds with no errors (warnings about the still-unused autostart enable are fine — autostart is added in Task 4).

- [ ] **Step 6: Dev smoke test against a throwaway Claude home**

Run (PowerShell), from the repo root:

```powershell
$env:FARO_CLAUDE_HOME = "$env:TEMP\faro-smoke"
Remove-Item -Recurse -Force $env:FARO_CLAUDE_HOME -ErrorAction SilentlyContinue
npm run tauri dev
```

In a second terminal, simulate consent by calling the command path is not trivial from outside; instead verify the *silent re-assert* branch: stop the app, create the marker, set the same env, relaunch:

```powershell
$cfg = "$env:APPDATA\com.stefano.faro-app"
New-Item -ItemType Directory -Force $cfg | Out-Null
Set-Content "$cfg\setup-consented" "1"
npm run tauri dev
```

Expected: `Get-Content $env:TEMP\faro-smoke\settings.json` shows the 7 events registered with a `bash "..."` command. Then clean up: `Remove-Item "$cfg\setup-consented"`.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(install): launch-time hook registration gated on first-run consent"
```

---

### Task 3: First-run consent UI (React)

A consent card shown only when `faro_setup_state()` is false. On "Attiva" it calls `faro_register_hooks()`; on success the card disappears and normal monitoring UI shows. Verified by build + visual run.

**Files:**
- Create: `src/components/FirstRunConsent.tsx`
- Modify: `src/App.tsx` (gate rendering on consent state)

**Interfaces:**
- Consumes: Tauri commands `faro_setup_state`, `faro_register_hooks` (Task 2) via `@tauri-apps/api/core` `invoke`.

- [ ] **Step 1: Create the consent component**

Create `src/components/FirstRunConsent.tsx`:

```tsx
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export function FirstRunConsent({ onDone }: { onDone: () => void }) {
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  async function activate() {
    setBusy(true);
    setErr(null);
    try {
      await invoke("faro_register_hooks");
      onDone();
    } catch (e) {
      setErr(String(e));
      setBusy(false);
    }
  }

  return (
    <div className="consent">
      <div className="consent-title">Faro</div>
      <div className="consent-body">
        Registro gli hook di Claude Code per monitorare le sessioni.
        Nessun comando da lanciare.
      </div>
      {err && <div className="consent-err">{err}</div>}
      <div className="consent-actions">
        <button className="consent-primary" onClick={activate} disabled={busy}>
          {busy ? "Attivo…" : "Attiva"}
        </button>
        <button className="consent-later" onClick={onDone} disabled={busy}>
          Più tardi
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Gate the panel content on consent state (NOT an early return)**

`App.tsx` measures the `.drawer` element via `useWindowFit(rootRef)`, which captures `rootRef.current` once at mount and assumes it stays mounted. An early `return <FirstRunConsent/>` would leave `rootRef` unattached → the window never resizes and the card is clipped and unclickable (cursor hit-testing uses the measured content rect). So the consent MUST render as the panel content inside the existing `DrawerPanel`, with the drawer forced open.

`App.tsx` already imports `useEffect`, `useState` (line 1) and `invoke` (line 3) — do NOT re-import them. Add only the component import after the existing component imports (near line 14):

```tsx
import { FirstRunConsent } from "./components/FirstRunConsent";
```

Inside the `App` component, alongside the other `useState`/`useEffect` calls (e.g. after the `now` state on line 23), add the consent state and loader:

```tsx
  const [consented, setConsented] = useState<boolean | null>(null);
  useEffect(() => {
    invoke<boolean>("faro_setup_state")
      .then(setConsented)
      .catch(() => setConsented(true)); // fail open: never block the overlay
  }, []);
  const showConsent = consented === false;
```

Change the `DrawerPanel` `open` prop (line 81) to force the drawer open while consent is showing:

```tsx
        open={open || showConsent}
```

Wrap the existing `panel={ ... }` content so consent takes precedence. The `panel` prop becomes:

```tsx
        panel={
          showConsent ? (
            <FirstRunConsent onDone={() => setConsented(true)} />
          ) : selected ? (
            <SessionDetail
              session={selected} now={now} muted={isMuted(settings, selected.id)}
              onClose={() => setSelectedId(null)}
              onMute={() => update({
                ...settings,
                mutedSessionIds: isMuted(settings, selected.id)
                  ? settings.mutedSessionIds.filter((x) => x !== selected.id)
                  : [...settings.mutedSessionIds, selected.id],
              })}
              onPinTop={() => setPinnedTop((p) =>
                p.includes(selected.id) ? p.filter((x) => x !== selected.id) : [...p, selected.id])}
              onArchive={() => {
                setSessions((list) => list.filter((s) => s.id !== selected.id));
                setSelectedId(null);
              }}
            />
          ) : (
            <>
              <div className="phdr"><span>Faro</span><span>{agg.total} sessioni</span></div>
              {ordered.length === 0
                ? <div className="empty">nessuna sessione</div>
                : ordered.map((s) => (
                    <SessionCard key={s.id} session={s} now={now} onClick={() => setSelectedId(s.id)} compact={ordered.length >= 7} />
                  ))}
            </>
          )
        }
```

This keeps `.drawer` mounted at all times (so `useWindowFit` and cursor hit-testing keep working) and sizes the window to the consent card. When `onDone` flips `consented` to true, `showConsent` goes false and the overlay reverts to its normal collapsed pill.

- [ ] **Step 3: Style the consent card**

Append to `src/App.css`:

```css
.consent {
  font: 13px/1.4 system-ui, sans-serif;
  color: #e8e8ea;
  background: rgba(20, 20, 24, 0.92);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 10px;
  padding: 12px 14px;
  width: 260px;
  box-shadow: 0 6px 24px rgba(0, 0, 0, 0.4);
}
.consent-title { font-weight: 700; margin-bottom: 6px; }
.consent-body { opacity: 0.85; margin-bottom: 10px; }
.consent-err { color: #ff8585; margin-bottom: 8px; font-size: 12px; }
.consent-actions { display: flex; gap: 8px; }
.consent-primary {
  background: #4c8bf5; color: white; border: 0;
  border-radius: 6px; padding: 6px 12px; cursor: pointer;
}
.consent-later {
  background: transparent; color: #b8b8bd;
  border: 1px solid rgba(255,255,255,0.15);
  border-radius: 6px; padding: 6px 12px; cursor: pointer;
}
```

- [ ] **Step 4: Type-check the frontend**

Run: `npm run build`
Expected: `tsc` passes with no errors; Vite build completes.

- [ ] **Step 5: Visual smoke test**

Run (PowerShell), forcing a fresh first-run:

```powershell
$cfg = "$env:APPDATA\com.stefano.faro-app"
Remove-Item "$cfg\setup-consented" -ErrorAction SilentlyContinue
$env:FARO_CLAUDE_HOME = "$env:TEMP\faro-consent"
Remove-Item -Recurse -Force $env:FARO_CLAUDE_HOME -ErrorAction SilentlyContinue
npm run tauri dev
```

Expected: the consent card appears. Click **Attiva** → card disappears and `Get-Content $env:TEMP\faro-consent\settings.json` shows the 7 events. Relaunch → consent card does NOT reappear.

- [ ] **Step 6: Commit**

```bash
git add src/components/FirstRunConsent.tsx src/App.tsx src/App.css
git commit -m "feat(ui): first-run consent card gating hook registration"
```

---

### Task 4: System tray + login autostart

The chromeless overlay gains a tray icon (its only clean exit + control surface) and login autostart, default ON at first registration. Verified by build + manual tray interaction.

**Files:**
- Create: `src-tauri/src/tray.rs`
- Modify: `src-tauri/Cargo.toml` (tauri `tray-icon` feature, autostart plugin)
- Modify: `src-tauri/src/lib.rs` (init plugin, build tray, enable autostart on register)
- Modify: `src-tauri/capabilities/default.json` (autostart permissions)

**Interfaces:**
- Consumes: `do_register` (Task 2), `tauri-plugin-autostart`.
- Produces: `pub fn build_tray(app: &tauri::App) -> tauri::Result<()>`.

- [ ] **Step 1: Add the tray feature and autostart plugin**

In `src-tauri/Cargo.toml`, update the `tauri` line and add the plugin:

```toml
tauri = { version = "2", features = ["macos-private-api", "tray-icon"] }
tauri-plugin-opener = "2"
tauri-plugin-autostart = "2"
```

- [ ] **Step 2: Grant autostart capability**

Replace `src-tauri/capabilities/default.json` permissions array so it reads:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Capability for the main window",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "opener:default",
    "autostart:allow-enable",
    "autostart:allow-disable",
    "autostart:allow-is-enabled"
  ]
}
```

- [ ] **Step 3: Enable autostart by default on successful registration**

In `src-tauri/src/lib.rs`, add the import:

```rust
use tauri_plugin_autostart::ManagerExt;
```

In `do_register`, inside the `if report.registered {` block (after writing the marker), add:

```rust
        let _ = app.autolaunch().enable();
```

- [ ] **Step 4: Write the tray module**

Create `src-tauri/src/tray.rs`:

```rust
//! System-tray control surface for the chromeless overlay.

use tauri::menu::{CheckMenuItemBuilder, Menu, MenuItem, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri_plugin_autostart::ManagerExt;

/// Build the tray icon and menu. Called once from setup().
pub fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let handle = app.handle();

    let autostart_on = handle.autolaunch().is_enabled().unwrap_or(false);

    let status = MenuItemBuilder::with_id("status", bash_status())
        .enabled(false)
        .build(app)?;
    let autostart = CheckMenuItemBuilder::with_id("autostart", "Avvio automatico al login")
        .checked(autostart_on)
        .build(app)?;
    let reinstall: MenuItem<_> = MenuItemBuilder::with_id("reinstall", "Ripristina hook").build(app)?;
    let quit: MenuItem<_> = MenuItemBuilder::with_id("quit", "Esci").build(app)?;

    let menu = Menu::with_items(app, &[&status, &autostart, &reinstall, &quit])?;

    TrayIconBuilder::with_id("faro-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Faro")
        .menu(&menu)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "quit" => app.exit(0),
            "reinstall" => {
                let _ = crate::do_register(app);
            }
            "autostart" => {
                let mgr = app.autolaunch();
                if mgr.is_enabled().unwrap_or(false) {
                    let _ = mgr.disable();
                } else {
                    let _ = mgr.enable();
                }
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn bash_status() -> &'static str {
    let ok = std::process::Command::new("bash").arg("--version").output().is_ok();
    if ok { "● hook attivi" } else { "⚠ Git Bash non trovato" }
}

#[cfg(not(target_os = "windows"))]
fn bash_status() -> &'static str {
    "● hook attivi"
}
```

- [ ] **Step 5: Register module, init plugin, build tray**

In `src-tauri/src/lib.rs`:

Add to the module list at the top:

```rust
pub mod tray;
```

Make `do_register` reachable from `tray.rs` — it is already a module-level `fn` in `lib.rs`, so `crate::do_register` resolves. Confirm it is NOT declared inside another function.

Add the autostart plugin to the builder chain (right after `.plugin(tauri_plugin_opener::init())`):

```rust
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
```

In the `.setup(|app| { ... })` closure, after the silent re-assert block from Task 2, add:

```rust
            crate::tray::build_tray(app)?;
```

- [ ] **Step 6: Build**

Run: `cd src-tauri && cargo build`
Expected: builds with no errors.

- [ ] **Step 7: Manual tray smoke test**

Run: `npm run tauri dev`
Expected (Windows 11, icon may be in the hidden ∧ overflow):
- Right-click tray → menu shows status line, "Avvio automatico al login" (checked), "Ripristina hook", "Esci".
- "Ripristina hook" rewrites `settings.json` (check with a `FARO_CLAUDE_HOME` as before).
- Toggling "Avvio automatico al login" off then on flips the check and the OS login entry (verify in Task Manager → Startup apps after re-open).
- "Esci" quits the app cleanly.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/tray.rs src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/capabilities/default.json
git commit -m "feat(tray): tray control surface + login autostart (default on)"
```

---

### Task 5: Auto-update (updater plugin + minisign keypair + startup check)

The app checks GitHub Releases for a newer signed build, installs it, and relaunches; the tray gains "Controlla aggiornamenti". Verified by build (full end-to-end update is exercised in Task 6 once a Release exists).

**Files:**
- Modify: `src-tauri/Cargo.toml` (updater plugin)
- Modify: `src-tauri/tauri.conf.json` (plugins.updater, createUpdaterArtifacts)
- Modify: `src-tauri/capabilities/default.json` (updater permission)
- Modify: `src-tauri/src/lib.rs` (init plugin, startup check)
- Modify: `src-tauri/src/tray.rs` (add "Controlla aggiornamenti" item)

**Interfaces:**
- Produces: `pub async fn check_and_update(app: tauri::AppHandle)` in `lib.rs`, callable from the tray.

- [ ] **Step 1: Generate the updater keypair (one-time, manual)**

Run, from the repo root:

```bash
npm run tauri signer generate -- -w ./faro-updater.key
```

This prints a **public key** and writes a password-protected **private key** to `./faro-updater.key`. Record both and the password you set. The private key file MUST NOT be committed — add it to `.gitignore`:

```bash
echo "faro-updater.key" >> .gitignore
echo "faro-updater.key.pub" >> .gitignore
git add .gitignore && git commit -m "chore: ignore updater signing key"
```

Store as GitHub repo secrets (Settings → Secrets and variables → Actions): `TAURI_SIGNING_PRIVATE_KEY` = contents of `faro-updater.key`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` = the password.

- [ ] **Step 2: Add the updater plugin dependency**

In `src-tauri/Cargo.toml`, add under `[dependencies]`:

```toml
tauri-plugin-updater = "2"
```

- [ ] **Step 3: Configure the updater and updater artifacts**

In `src-tauri/tauri.conf.json`, set `bundle.createUpdaterArtifacts` and add a `plugins` block. The `bundle` object becomes:

```json
  "bundle": {
    "active": true,
    "targets": "all",
    "createUpdaterArtifacts": true,
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  },
  "plugins": {
    "updater": {
      "endpoints": ["https://github.com/stefff94/faro/releases/latest/download/latest.json"],
      "pubkey": "PASTE_PUBLIC_KEY_FROM_STEP_1"
    }
  }
```

Replace `PASTE_PUBLIC_KEY_FROM_STEP_1` with the exact public key string printed in Step 1.

- [ ] **Step 4: Grant the updater capability**

In `src-tauri/capabilities/default.json`, add `"updater:default"` to the permissions array (after the autostart entries).

- [ ] **Step 5: Init the plugin and add the update routine**

In `src-tauri/src/lib.rs`:

Add the plugin to the builder chain (after the autostart plugin):

```rust
        .plugin(tauri_plugin_updater::Builder::new().build())
```

Add the import near the top:

```rust
use tauri_plugin_updater::UpdaterExt;
```

Add the update routine above `pub fn run()`:

```rust
/// Check the configured endpoint; if a newer signed build exists, install and relaunch.
pub async fn check_and_update(app: tauri::AppHandle) {
    let updater = match app.updater() {
        Ok(u) => u,
        Err(_) => return,
    };
    if let Ok(Some(update)) = updater.check().await {
        if update.download_and_install(|_, _| {}, || {}).await.is_ok() {
            app.restart();
        }
    }
}
```

In the `.setup(...)` closure, after `crate::tray::build_tray(app)?;`, add a non-blocking startup check:

```rust
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    crate::check_and_update(handle).await;
                });
            }
```

- [ ] **Step 6: Add the tray menu item**

In `src-tauri/src/tray.rs`, add the item and wire it:

After building `reinstall`, add:

```rust
    let update: MenuItem<_> = MenuItemBuilder::with_id("update", "Controlla aggiornamenti").build(app)?;
```

Change the `Menu::with_items` call to include it:

```rust
    let menu = Menu::with_items(app, &[&status, &autostart, &reinstall, &update, &quit])?;
```

Add a match arm in `on_menu_event`:

```rust
            "update" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    crate::check_and_update(app).await;
                });
            }
```

- [ ] **Step 7: Build**

Run: `cd src-tauri && cargo build`
Expected: builds with no errors. (The updater will report "no updates" at runtime until a Release exists — that is expected and harmless.)

- [ ] **Step 8: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json src-tauri/capabilities/default.json src-tauri/src/lib.rs src-tauri/src/tray.rs
git commit -m "feat(update): auto-update from GitHub Releases + tray check"
```

---

### Task 6: CI release pipeline (GitHub Actions)

A tag-triggered workflow builds Windows + universal macOS, publishes installers + `latest.json` to a GitHub Release, signing updater artifacts with the secret key. Verified on a throwaway tag.

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Write the workflow**

Create `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags:
      - "v*"

jobs:
  release:
    permissions:
      contents: write
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: macos-latest
            args: "--target universal-apple-darwin"
          - platform: windows-latest
            args: ""
    runs-on: ${{ matrix.platform }}
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.platform == 'macos-latest' && 'aarch64-apple-darwin,x86_64-apple-darwin' || '' }}

      - name: Install Node
        uses: actions/setup-node@v4
        with:
          node-version: 20

      - name: Install frontend deps
        run: npm ci

      - name: Build and release
        uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
        with:
          tagName: ${{ github.ref_name }}
          releaseName: "Faro ${{ github.ref_name }}"
          releaseBody: "See the assets below. First launch on an unsigned build: Windows → More info → Run anyway; macOS → right-click the app → Open."
          releaseDraft: false
          prerelease: false
          includeUpdaterJson: true
          args: ${{ matrix.args }}
```

- [ ] **Step 2: Verify the workflow file is valid**

Run: `cd "c:\Users\stefano.vannucchi\Projects\SV\faro" && git add .github/workflows/release.yml && git status`
Expected: the file is staged. (YAML is validated by GitHub on push; there is no local unit test.)

- [ ] **Step 3: Commit**

```bash
git commit -m "ci: tag-triggered Windows + universal macOS release with updater artifacts"
```

- [ ] **Step 4: End-to-end release smoke test (throwaway tag)**

Ensure Step 1 of Task 5 secrets are set in GitHub, then:

```bash
git tag v0.0.0-test
git push origin v0.0.0-test
```

Expected: the Actions run completes; a Release `v0.0.0-test` appears with a Windows `.msi`/setup, a macOS `.dmg`, `.sig` files, and `latest.json`. Inspect `latest.json` — it lists both platforms and signatures. Then delete the test Release and tag:

```bash
git push --delete origin v0.0.0-test
git tag -d v0.0.0-test
```

(Delete the GitHub Release from the web UI.)

---

### Task 7: Docs — README, bypass instructions, retire the PS1

Document the colleague install flow and the unsigned-app bypass; mark the PowerShell installer superseded.

**Files:**
- Modify: `README.md`
- Modify: `hooks/install-windows.ps1` (header comment only)

- [ ] **Step 1: Rewrite the install/distribution section of README**

In `README.md`, replace the `## Hook registration` section (and the macOS/Windows subsections under it) with an install-for-colleagues section:

```markdown
## Install (for colleagues)

Download the latest installer from the [Releases page](https://github.com/stefff94/faro/releases/latest):

- **Windows:** run the `.msi`. SmartScreen may say "unknown publisher" → **More info → Run anyway** (once).
- **macOS:** open the `.dmg`, drag Faro to Applications, then **right-click the app → Open** the first time (it is not yet notarized). Alternatively: `xattr -dr com.apple.quarantine /Applications/Faro.app`.

On first launch Faro shows a one-time card — click **Attiva** and it registers its Claude Code hooks itself. Nothing else to configure. It then lives in the system tray (Windows: behind the ∧ overflow; macOS: menu bar), starts at login, and auto-updates.

**Windows prerequisite:** Git for Windows (Git Bash) must be installed — Claude Code needs it anyway, and Faro's reporter runs through it. If the tray shows "⚠ Git Bash non trovato", install [Git for Windows](https://git-scm.com/download/win).

## Updating

Faro checks GitHub Releases on launch and updates itself. You can also trigger a check from the tray ("Controlla aggiornamenti").

## Building from source (maintainers)

Requirements — macOS: macOS 12+, Rust, Node 20+. Windows: Rust MSVC toolchain, Visual Studio Build Tools (Desktop C++), Node 20+, Git for Windows.

```bash
npm install
npm run tauri dev      # run locally
```

Cutting a release: bump the version, then `git tag vX.Y.Z && git push --tags`. CI builds both platforms and publishes the Release.
```

- [ ] **Step 2: Update the Known limitations list in README**

In `README.md`, under `## Known limitations`, replace the line about manual hook installation (if present) and ensure these entries exist:

```markdown
- **No OS code signing yet** — first launch shows a SmartScreen (Windows) / Gatekeeper (macOS) warning; bypass once as described under Install. Apple notarization + a Windows certificate are future work.
- **Windows requires Git Bash** — the reporter runs via `bash`; without Git for Windows the hooks are not operative (the tray flags this).
- **Port 8765 is hardcoded** — conflict resolution is future work.
```

- [ ] **Step 3: Mark the PowerShell installer superseded**

At the very top of `hooks/install-windows.ps1`, add a header comment block above the existing `[CmdletBinding()]`:

```powershell
# SUPERSEDED — manual fallback only.
# Faro now registers its Claude Code hooks itself on first launch (see src-tauri/src/hooks_install.rs).
# This script is retained only for registering hooks WITHOUT launching the app.
```

- [ ] **Step 4: Verify README renders and links are intact**

Run: `cd "c:\Users\stefano.vannucchi\Projects\SV\faro" && git diff --stat`
Expected: `README.md` and `hooks/install-windows.ps1` modified. Re-read the Install section to confirm no leftover references to running `install-windows.ps1` in the happy path.

- [ ] **Step 5: Commit**

```bash
git add README.md hooks/install-windows.ps1
git commit -m "docs: colleague install flow, unsigned-app bypass, retire PS1 to fallback"
```

---

## Notes for the implementer

- **macOS unsigned auto-update** is the one known gray area: the updater replaces the `.app` in place and works in practice, but if a future macOS build refuses to relaunch after an update, the fix is Apple signing + notarization (the updater keypair and pipeline are unchanged). Out of scope here; do not block on it.
- **`do_register` must stay a module-level `fn` in `lib.rs`** (not nested in `run()`), because `tray.rs` calls `crate::do_register`.
- The `status` tray line is computed once at build time. A live-updating status is out of scope.
