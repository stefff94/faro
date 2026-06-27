# Faro MVP (macOS) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A frameless, transparent, always-on-top macOS desktop widget that shows one horizontal traffic light (🔴🟡🟢) per live Claude Code session, updating in real time from the session's hooks.

**Architecture:** Three layers, exactly as `HANDOFF.md` §3. (1) A *reporter* — a tiny shell script registered as Claude Code hooks that `curl`s each hook payload to the local broker. (2) A *broker* — an in-process Rust/axum HTTP listener on `127.0.0.1:8765` that maps hook events to a per-session status (§4), keeps an in-memory store, and pushes snapshots to the UI via a Tauri event. (3) A *widget* — a React/TS Tauri window that listens for snapshots and renders one row per session. The broker keeps source-specific logic behind a `Source` seam so that "source = engine, not surface" holds (a Claude Code session in a terminal, VS Code, or Cursor all look identical).

**Tech Stack:** Tauri 2.x, Rust (axum 0.7, tokio, serde, serde_json), React 18 + TypeScript + Vite, vitest for frontend unit tests.

**Scope of THIS plan:** Milestones **M0 + M1 + M2** from `HANDOFF.md` §9, macOS only. Out of scope (own follow-up plans): **M3** (free-breadth validation across IDEs — verification work, no new code), **M4** (idempotent installer, settings persistence, `.dmg` packaging, port-conflict strategy), and Windows parity (§11.5).

## Global Constraints

Copied verbatim from `HANDOFF.md`; every task inherits these.

- **Tauri 2.x.** Set `app.macOSPrivateApi: true` (required for macOS transparency).
- **macOS first.** Keep all platform-specific window/transparency/positioning code behind a thin seam; the Windows port must be additive (§11.b).
- **Broker address:** fixed `http://127.0.0.1:8765/event`. Port discovery is deferred to M4.
- **Reporter must never break a session:** 1s curl timeout, backgrounded, **always `exit 0`**.
- **Hook registration must MERGE, never overwrite** the user's existing `~/.claude/settings.json`. Use absolute `$HOME/...` paths (`~` is not reliably expanded in hook contexts).
- **Source seam:** session identity and status mapping go through the `Source` abstraction; never special-case a surface (terminal vs IDE) in the store.
- **Wire contract `SessionState` is camelCase JSON** exactly as §5.1: `id, source, sessionId, label, cwd, status, lastEventName, lastUpdate, transcriptPath`.
- **States:** `idle | working | blocked | done | stale | error`.
- **Stale only applies to `working`** sessions. Default TTL **90s** (configurable). Keep stale rows until `SessionEnd` or a longer purge TTL (30 min).
- **Validation gates carried from §11.b:** (6) the exact JSON key for the `Notification` discriminator is unconfirmed — parse defensively and pin it from a live event before trusting the mapping; (7) 🔴 "blocked" is under-detected by design (only tool-permission gates) — document it; (8) register `StopFailure` so `error` can actually fire.

---

## File Structure

```
faro/
├── HANDOFF.md
├── README.md                       # Task 19
├── package.json                    # scaffold (Task 1)
├── vite.config.ts                  # scaffold (Task 1)
├── vitest.config.ts                # Task 16
├── index.html                      # scaffold
├── src/                            # React + TS frontend
│   ├── main.tsx                    # scaffold
│   ├── App.tsx                     # Task 17 (listen + render)
│   ├── App.css                     # Task 2 / Task 17
│   ├── types.ts                    # Task 16 (SessionState, Status)
│   ├── snapshot.ts                 # Task 16 (pure helpers, tested)
│   ├── snapshot.test.ts            # Task 16
│   └── components/
│       ├── SessionRow.tsx          # Task 17
│       └── TrafficLight.tsx        # Task 2 / Task 17
└── src-tauri/
    ├── tauri.conf.json             # Task 2 (window config)
    ├── Cargo.toml                   # Task 3 (deps)
    └── src/
        ├── main.rs                  # scaffold; Task 18 (wire broker into Tauri)
        ├── model.rs                 # Task 4 (Status, SessionState, HookEvent)
        ├── classify.rs             # Task 5 (event → Transition, §4)
        ├── store.rs                # Task 6/7/8 (SessionStore: apply/snapshot/stale/purge/remove)
        ├── source.rs               # Task 9 (Source trait + ClaudeCodeSource)
        └── http.rs                 # Task 10/11 (axum router: POST /event, GET /sessions)
hooks/
└── agent-monitor-report.sh         # Task 12
```

**Responsibilities (one job each):**
- `model.rs` — data types + serde wire contract only. No logic.
- `classify.rs` — the pure §4 state machine: `HookEvent → Transition`. No I/O, no store.
- `store.rs` — owns the `HashMap` of sessions; applies transitions, computes snapshots, ages out stale/purged rows. No HTTP, no Tauri.
- `source.rs` — the engine seam: how a `claude-code` event becomes a session id/label.
- `http.rs` — axum wiring only: deserialize body, call store, return status. No business rules.
- `main.rs` — Tauri lifecycle: shares the store, spawns the listener + stale ticker, emits `sessions-updated`.
- Frontend mirrors this: `types.ts`/`snapshot.ts` are pure & tested; components are dumb renderers; `App.tsx` does the Tauri `listen` plumbing.

---

# Milestone M0 — Scaffold + the UI risk

## Task 1: Scaffold the Tauri 2 + React + TS project

**Files:**
- Create: whole `faro/` Tauri scaffold (`package.json`, `vite.config.ts`, `index.html`, `src/main.tsx`, `src-tauri/**`).

**Interfaces:**
- Consumes: nothing (first task).
- Produces: a booting Tauri app; `npm run tauri dev` opens a default window.

- [ ] **Step 1: Scaffold with the official template**

Run from the repo root (`/Users/stefano/Progetti/claude/faro`). The directory already contains `HANDOFF.md` and `docs/`, so scaffold in place with the current dir as target:

```bash
npm create tauri-app@latest faro-app -- --template react-ts --manager npm
```

Then move its contents up into the repo root (keep `HANDOFF.md`/`docs/`), or scaffold into a temp dir and copy `package.json`, `vite.config.ts`, `index.html`, `src/`, `src-tauri/` into the repo root. Verify the tree matches the File Structure section above.

- [ ] **Step 2: Install dependencies**

```bash
cd /Users/stefano/Progetti/claude/faro && npm install
```

- [ ] **Step 3: Boot it once to confirm the toolchain works**

Run: `npm run tauri dev`
Expected: a default Tauri window opens showing the template page. Close it (Ctrl-C).

- [ ] **Step 4: Commit**

```bash
git init 2>/dev/null; git add -A
git commit -m "chore: scaffold Tauri 2 + React + TS app"
```

---

## Task 2: Configure the widget window + render a hardcoded traffic light

This is the M0 risk: prove a transparent, frameless, always-on-top, top-right, non-focus-stealing window works on macOS. No real data yet.

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/src/main.rs` (compute + set top-right position at runtime)
- Create: `src/components/TrafficLight.tsx`
- Modify: `src/App.tsx`, `src/App.css`

**Interfaces:**
- Consumes: scaffold from Task 1.
- Produces: `TrafficLight` component with props `{ status: "idle"|"working"|"blocked"|"done"|"stale"|"error" }` rendering three dots where only the active color is lit.

- [ ] **Step 1: Set the window config**

In `src-tauri/tauri.conf.json`, set `app.macOSPrivateApi: true` and the single window under `app.windows` (verbatim from §7):

```jsonc
{
  "label": "main",
  "transparent": true,
  "decorations": false,
  "alwaysOnTop": true,
  "skipTaskbar": true,
  "resizable": false,
  "shadow": false,
  "width": 220,
  "height": 120,
  "focus": false
}
```

- [ ] **Step 2: Position top-right at runtime in `main.rs`**

In the Tauri `setup` hook, compute the top-right corner from the active monitor's work area (don't hardcode x/y — multi-monitor safe per §7):

```rust
use tauri::{Manager, PhysicalPosition};

fn position_top_right(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    if let Some(monitor) = window.current_monitor()? {
        let screen = monitor.size();
        let win = window.outer_size()?;
        let margin = 16;
        let x = (screen.width as i32) - (win.width as i32) - margin;
        let y = margin;
        window.set_position(PhysicalPosition::new(x, y))?;
    }
    Ok(())
}
```

Call it from `setup`:

```rust
.setup(|app| {
    let window = app.get_webview_window("main").unwrap();
    position_top_right(&window)?;
    Ok(())
})
```

- [ ] **Step 3: Make the webview background transparent**

In `src/App.css`, ensure no opaque background:

```css
html, body, #root { margin: 0; background: transparent; }
* { box-sizing: border-box; }
```

- [ ] **Step 4: Write the TrafficLight component**

Create `src/components/TrafficLight.tsx`:

```tsx
type Status = "idle" | "working" | "blocked" | "done" | "stale" | "error";

const COLOR: Record<Status, string> = {
  idle: "#6b7280",     // grey
  working: "#f5c518",  // yellow
  blocked: "#ef4444",  // red
  done: "#22c55e",     // green
  stale: "#6b7280",    // dim grey
  error: "#ef4444",    // red (distinct via blink class)
};

const LIT: Record<Status, "red" | "yellow" | "green" | "none"> = {
  idle: "none", working: "yellow", blocked: "red",
  done: "green", stale: "none", error: "red",
};

export function TrafficLight({ status }: { status: Status }) {
  const lit = LIT[status];
  const dot = (which: "red" | "yellow" | "green", color: string) => (
    <span
      className={"dot" + (status === "error" && which === "red" ? " blink" : "")}
      style={{
        background: lit === which ? color : "#2a2a2a",
        opacity: lit === which ? 1 : 0.35,
      }}
    />
  );
  return (
    <span className="traffic-light">
      {dot("red", COLOR.blocked)}
      {dot("yellow", COLOR.working)}
      {dot("green", COLOR.done)}
    </span>
  );
}
```

Add to `src/App.css`:

```css
.traffic-light { display: inline-flex; gap: 6px; }
.dot { width: 12px; height: 12px; border-radius: 50%; display: inline-block; }
.blink { animation: blink 1s steps(2, start) infinite; }
@keyframes blink { 50% { opacity: 0.2; } }
```

- [ ] **Step 5: Render a hardcoded light in App.tsx**

Replace `src/App.tsx` body with:

```tsx
import "./App.css";
import { TrafficLight } from "./components/TrafficLight";

export default function App() {
  return (
    <div className="faro-root">
      <div className="session-row">
        <TrafficLight status="working" />
        <span className="label">dummy-session</span>
      </div>
    </div>
  );
}
```

Add to `App.css`:

```css
.faro-root { padding: 8px; font-family: -apple-system, system-ui, sans-serif; color: #e5e7eb; }
.session-row { display: flex; align-items: center; gap: 8px; padding: 4px 6px; }
.label { font-size: 12px; }
```

- [ ] **Step 6: Manual verify (M0 gate)**

Run: `npm run tauri dev`
Expected (§9 M0 verify): the widget floats over other apps on macOS; it is transparent (no window chrome/background); it sits top-right; clicking other apps it does **not** steal focus; a yellow-lit traffic light + "dummy-session" is visible.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(window): transparent always-on-top top-right widget with hardcoded light"
```

---

# Milestone M1 — Vertical slice (the milestone that matters)

The broker maps **one** real Claude Code session's hook events to status and one real light changes color live.

## Task 3: Add Rust broker dependencies

**Files:**
- Modify: `src-tauri/Cargo.toml`

**Interfaces:**
- Produces: `axum`, `tokio`, `serde`, `serde_json` available to later tasks.

- [ ] **Step 1: Add dependencies**

In `src-tauri/Cargo.toml` under `[dependencies]` (keep the existing `tauri`/`serde` lines, adjust versions to match the scaffold):

```toml
axum = "0.7"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

- [ ] **Step 2: Verify it builds**

Run: `cd src-tauri && cargo build`
Expected: compiles (a default scaffold `main.rs` still present is fine).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore(broker): add axum/tokio/serde deps"
```

---

## Task 4: Model types — `Status`, `SessionState`, `HookEvent`

**Files:**
- Create: `src-tauri/src/model.rs`
- Modify: `src-tauri/src/main.rs` (add `mod model;`)
- Test: inline `#[cfg(test)]` in `model.rs`

**Interfaces:**
- Produces:
  - `enum Status { Idle, Working, Blocked, Done, Stale, Error }` — serde `lowercase`.
  - `struct SessionState { id, source, session_id, label, cwd, status, last_event_name, last_update: i64, transcript_path: Option<String> }` — serde `camelCase` (wire contract §5.1).
  - `struct HookEvent { hook_event_name, session_id, cwd: Option<String>, transcript_path: Option<String>, notification_type: Option<String>, type_field: Option<String>, raw: serde_json::Value }`.
  - `HookEvent::notification_kind(&self) -> Option<&str>` returns the discriminator trying known keys.

- [ ] **Step 1: Write the failing test**

In `src-tauri/src/model.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_state_serializes_camelcase() {
        let s = SessionState {
            id: "claude-code:abc123".into(),
            source: "claude-code".into(),
            session_id: "abc123".into(),
            label: "my-project".into(),
            cwd: "/Users/x/my-project".into(),
            status: Status::Working,
            last_event_name: "PreToolUse".into(),
            last_update: 1719500000000,
            transcript_path: None,
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"sessionId\":\"abc123\""));
        assert!(json.contains("\"status\":\"working\""));
        assert!(json.contains("\"lastEventName\":\"PreToolUse\""));
    }

    #[test]
    fn hook_event_reads_notification_kind_from_either_key() {
        let a: HookEvent = serde_json::from_str(
            r#"{"hook_event_name":"Notification","session_id":"s","notification_type":"permission_prompt"}"#,
        ).unwrap();
        assert_eq!(a.notification_kind(), Some("permission_prompt"));

        let b: HookEvent = serde_json::from_str(
            r#"{"hook_event_name":"Notification","session_id":"s","type":"idle_prompt"}"#,
        ).unwrap();
        assert_eq!(b.notification_kind(), Some("idle_prompt"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test model::`
Expected: FAIL — `Status`/`SessionState`/`HookEvent` not found.

- [ ] **Step 3: Write the implementation**

At the top of `src-tauri/src/model.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Idle,
    Working,
    Blocked,
    Done,
    Stale,
    Error,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub id: String,
    pub source: String,
    pub session_id: String,
    pub label: String,
    pub cwd: String,
    pub status: Status,
    pub last_event_name: String,
    pub last_update: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<String>,
}

/// Raw hook payload forwarded by the reporter (Claude Code uses snake_case keys).
#[derive(Clone, Debug, Deserialize)]
pub struct HookEvent {
    pub hook_event_name: String,
    pub session_id: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub transcript_path: Option<String>,
    // Validation note §11.b(6): exact discriminator key unconfirmed. Try both.
    #[serde(default)]
    pub notification_type: Option<String>,
    #[serde(default, rename = "type")]
    pub type_field: Option<String>,
}

impl HookEvent {
    /// The Notification discriminator (`permission_prompt` / `idle_prompt` / ...),
    /// trying the candidate keys in priority order.
    pub fn notification_kind(&self) -> Option<&str> {
        self.notification_type
            .as_deref()
            .or(self.type_field.as_deref())
    }
}
```

Add `mod model;` to `src-tauri/src/main.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test model::`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/model.rs src-tauri/src/main.rs
git commit -m "feat(broker): model types + camelCase wire contract"
```

---

## Task 5: The §4 state machine — `classify(HookEvent) -> Transition`

This is the pure heart of the product. No store, no I/O.

**Files:**
- Create: `src-tauri/src/classify.rs`
- Modify: `src-tauri/src/main.rs` (`mod classify;`)
- Test: inline in `classify.rs`

**Interfaces:**
- Consumes: `model::{HookEvent, Status}`.
- Produces:
  - `enum Transition { Set(Status), Remove, Ignore }`
  - `fn classify(event: &HookEvent) -> Transition`

- [ ] **Step 1: Write the failing tests** (one per §4 row)

In `src-tauri/src/classify.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{HookEvent, Status};

    fn ev(name: &str, kind: Option<&str>) -> HookEvent {
        HookEvent {
            hook_event_name: name.into(),
            session_id: "s".into(),
            cwd: Some("/tmp/proj".into()),
            transcript_path: None,
            notification_type: kind.map(|k| k.into()),
            type_field: None,
        }
    }

    #[test] fn session_start_is_idle()    { assert_eq!(classify(&ev("SessionStart", None)), Transition::Set(Status::Idle)); }
    #[test] fn prompt_is_working()        { assert_eq!(classify(&ev("UserPromptSubmit", None)), Transition::Set(Status::Working)); }
    #[test] fn pretooluse_is_working()    { assert_eq!(classify(&ev("PreToolUse", None)), Transition::Set(Status::Working)); }
    #[test] fn permission_is_blocked()    { assert_eq!(classify(&ev("Notification", Some("permission_prompt"))), Transition::Set(Status::Blocked)); }
    #[test] fn idle_prompt_is_done()      { assert_eq!(classify(&ev("Notification", Some("idle_prompt"))), Transition::Set(Status::Done)); }
    #[test] fn stop_is_done()             { assert_eq!(classify(&ev("Stop", None)), Transition::Set(Status::Done)); }
    #[test] fn stop_failure_is_error()    { assert_eq!(classify(&ev("StopFailure", None)), Transition::Set(Status::Error)); }
    #[test] fn session_end_removes()      { assert_eq!(classify(&ev("SessionEnd", None)), Transition::Remove); }
    #[test] fn unknown_notification_ignored() { assert_eq!(classify(&ev("Notification", Some("auth_success"))), Transition::Ignore); }
    #[test] fn unknown_event_ignored()    { assert_eq!(classify(&ev("PostToolUse", None)), Transition::Ignore); }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test classify::`
Expected: FAIL — `classify`/`Transition` not found.

- [ ] **Step 3: Write the implementation**

At the top of `src-tauri/src/classify.rs`:

```rust
use crate::model::{HookEvent, Status};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transition {
    Set(Status),
    Remove,
    Ignore,
}

/// Maps a Claude Code hook event to a status transition. Table is HANDOFF.md §4.
pub fn classify(event: &HookEvent) -> Transition {
    match event.hook_event_name.as_str() {
        "SessionStart" => Transition::Set(Status::Idle),
        "UserPromptSubmit" => Transition::Set(Status::Working),
        "PreToolUse" => Transition::Set(Status::Working),
        "Stop" => Transition::Set(Status::Done),
        "StopFailure" => Transition::Set(Status::Error), // §11.b(8)
        "SessionEnd" => Transition::Remove,
        "Notification" => match event.notification_kind() {
            Some("permission_prompt") => Transition::Set(Status::Blocked),
            Some("idle_prompt") => Transition::Set(Status::Done), // §11.b(7): tunable later
            _ => Transition::Ignore,
        },
        _ => Transition::Ignore,
    }
}
```

Add `mod classify;` to `src-tauri/src/main.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test classify::`
Expected: PASS (10 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/classify.rs src-tauri/src/main.rs
git commit -m "feat(broker): §4 event→status state machine (classify)"
```

---

## Task 6: `SessionStore` — apply events & snapshot

**Files:**
- Create: `src-tauri/src/store.rs`
- Modify: `src-tauri/src/main.rs` (`mod store;`)
- Test: inline in `store.rs`

**Interfaces:**
- Consumes: `model::{SessionState, Status, HookEvent}`, `classify::{classify, Transition}`.
- Produces:
  - `struct SessionStore { sessions: HashMap<String, SessionState> }`
  - `SessionStore::new() -> Self`
  - `SessionStore::apply(&mut self, source: &str, event: &HookEvent, now_ms: i64) -> bool` (returns `true` if the snapshot changed)
  - `SessionStore::snapshot(&self) -> Vec<SessionState>` (sorted by `id` for stable rendering)
  - free fn `label_from_cwd(cwd: Option<&str>) -> String` (basename, else `"session"`)

- [ ] **Step 1: Write the failing tests**

In `src-tauri/src/store.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{HookEvent, Status};

    fn ev(name: &str, sid: &str, cwd: Option<&str>, kind: Option<&str>) -> HookEvent {
        HookEvent {
            hook_event_name: name.into(),
            session_id: sid.into(),
            cwd: cwd.map(|c| c.into()),
            transcript_path: None,
            notification_type: kind.map(|k| k.into()),
            type_field: None,
        }
    }

    #[test]
    fn apply_creates_session_with_derived_label_and_id() {
        let mut s = SessionStore::new();
        let changed = s.apply("claude-code", &ev("UserPromptSubmit", "abc", Some("/Users/x/my-project"), None), 1000);
        assert!(changed);
        let snap = s.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].id, "claude-code:abc");
        assert_eq!(snap[0].label, "my-project");
        assert_eq!(snap[0].status, Status::Working);
        assert_eq!(snap[0].last_update, 1000);
    }

    #[test]
    fn apply_updates_existing_session_in_place() {
        let mut s = SessionStore::new();
        s.apply("claude-code", &ev("UserPromptSubmit", "abc", Some("/x/p"), None), 1000);
        s.apply("claude-code", &ev("Stop", "abc", Some("/x/p"), None), 2000);
        let snap = s.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].status, Status::Done);
        assert_eq!(snap[0].last_update, 2000);
    }

    #[test]
    fn session_end_removes() {
        let mut s = SessionStore::new();
        s.apply("claude-code", &ev("SessionStart", "abc", Some("/x/p"), None), 1000);
        let changed = s.apply("claude-code", &ev("SessionEnd", "abc", Some("/x/p"), None), 1100);
        assert!(changed);
        assert_eq!(s.snapshot().len(), 0);
    }

    #[test]
    fn ignored_event_does_not_change_store() {
        let mut s = SessionStore::new();
        let changed = s.apply("claude-code", &ev("PostToolUse", "abc", Some("/x/p"), None), 1000);
        assert!(!changed);
        assert_eq!(s.snapshot().len(), 0);
    }

    #[test]
    fn two_sessions_are_independent_rows() {
        let mut s = SessionStore::new();
        s.apply("claude-code", &ev("UserPromptSubmit", "a", Some("/x/one"), None), 1000);
        s.apply("claude-code", &ev("Notification", "b", Some("/x/two"), Some("permission_prompt")), 1000);
        let snap = s.snapshot();
        assert_eq!(snap.len(), 2);
        assert_eq!(snap[0].status, Status::Working); // sorted by id: "...:a" before "...:b"
        assert_eq!(snap[1].status, Status::Blocked);
    }

    #[test]
    fn label_falls_back_when_no_cwd() {
        assert_eq!(label_from_cwd(None), "session");
        assert_eq!(label_from_cwd(Some("/Users/x/proj")), "proj");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test store::`
Expected: FAIL — `SessionStore` not found.

- [ ] **Step 3: Write the implementation**

At the top of `src-tauri/src/store.rs`:

```rust
use std::collections::HashMap;

use crate::classify::{classify, Transition};
use crate::model::{HookEvent, SessionState};

pub fn label_from_cwd(cwd: Option<&str>) -> String {
    cwd.and_then(|c| c.trim_end_matches('/').rsplit('/').next())
        .filter(|s| !s.is_empty())
        .unwrap_or("session")
        .to_string()
}

#[derive(Default)]
pub struct SessionStore {
    sessions: HashMap<String, SessionState>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply one hook event. Returns true if the visible snapshot changed.
    pub fn apply(&mut self, source: &str, event: &HookEvent, now_ms: i64) -> bool {
        let id = format!("{}:{}", source, event.session_id);
        match classify(event) {
            Transition::Ignore => false,
            Transition::Remove => self.sessions.remove(&id).is_some(),
            Transition::Set(status) => {
                let entry = self.sessions.entry(id.clone()).or_insert_with(|| SessionState {
                    id: id.clone(),
                    source: source.to_string(),
                    session_id: event.session_id.clone(),
                    label: label_from_cwd(event.cwd.as_deref()),
                    cwd: event.cwd.clone().unwrap_or_default(),
                    status,
                    last_event_name: event.hook_event_name.clone(),
                    last_update: now_ms,
                    transcript_path: event.transcript_path.clone(),
                });
                entry.status = status;
                entry.last_event_name = event.hook_event_name.clone();
                entry.last_update = now_ms;
                if let Some(cwd) = event.cwd.as_deref() {
                    entry.cwd = cwd.to_string();
                    entry.label = label_from_cwd(Some(cwd));
                }
                if event.transcript_path.is_some() {
                    entry.transcript_path = event.transcript_path.clone();
                }
                true
            }
        }
    }

    pub fn snapshot(&self) -> Vec<SessionState> {
        let mut v: Vec<SessionState> = self.sessions.values().cloned().collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }
}
```

Add `mod store;` to `src-tauri/src/main.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test store::`
Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/store.rs src-tauri/src/main.rs
git commit -m "feat(broker): SessionStore apply + snapshot"
```

---

## Task 7: Stale aging — `mark_stale` (working-only, TTL)

**Files:**
- Modify: `src-tauri/src/store.rs`
- Test: inline in `store.rs`

**Interfaces:**
- Consumes: existing `SessionStore`.
- Produces: `SessionStore::mark_stale(&mut self, ttl_ms: i64, now_ms: i64) -> bool` — only `Working` sessions older than `ttl_ms` become `Stale`; returns whether anything changed.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` mod in `src-tauri/src/store.rs`:

```rust
    #[test]
    fn working_session_goes_stale_after_ttl() {
        let mut s = SessionStore::new();
        s.apply("claude-code", &ev("PreToolUse", "a", Some("/x/p"), None), 1_000);
        let changed = s.mark_stale(90_000, 1_000 + 90_001);
        assert!(changed);
        assert_eq!(s.snapshot()[0].status, Status::Stale);
    }

    #[test]
    fn blocked_and_done_never_go_stale() {
        let mut s = SessionStore::new();
        s.apply("claude-code", &ev("Notification", "a", Some("/x/p"), Some("permission_prompt")), 1_000);
        s.apply("claude-code", &ev("Stop", "b", Some("/x/p"), None), 1_000);
        let changed = s.mark_stale(90_000, 1_000 + 1_000_000);
        assert!(!changed);
        assert_eq!(s.snapshot()[0].status, Status::Blocked);
        assert_eq!(s.snapshot()[1].status, Status::Done);
    }

    #[test]
    fn working_within_ttl_stays_working() {
        let mut s = SessionStore::new();
        s.apply("claude-code", &ev("PreToolUse", "a", Some("/x/p"), None), 1_000);
        assert!(!s.mark_stale(90_000, 1_000 + 50_000));
        assert_eq!(s.snapshot()[0].status, Status::Working);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test store::`
Expected: FAIL — `mark_stale` not found.

- [ ] **Step 3: Write the implementation**

Add to `impl SessionStore` in `src-tauri/src/store.rs`:

```rust
    /// Stale only applies to `working` sessions past the TTL (HANDOFF.md §4 rule).
    pub fn mark_stale(&mut self, ttl_ms: i64, now_ms: i64) -> bool {
        use crate::model::Status;
        let mut changed = false;
        for s in self.sessions.values_mut() {
            if s.status == Status::Working && now_ms - s.last_update > ttl_ms {
                s.status = Status::Stale;
                changed = true;
            }
        }
        changed
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test store::`
Expected: PASS (9 tests total in store).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/store.rs
git commit -m "feat(broker): stale aging for working sessions"
```

---

## Task 8: The `Source` seam

Keep "source = engine" explicit even though M1 has one source. This is the boundary that makes M3 free.

**Files:**
- Create: `src-tauri/src/source.rs`
- Modify: `src-tauri/src/main.rs` (`mod source;`)
- Test: inline in `source.rs`

**Interfaces:**
- Produces:
  - `trait Source { fn name(&self) -> &'static str; }`
  - `struct ClaudeCodeSource;` with `name() == "claude-code"`.

- [ ] **Step 1: Write the failing test**

In `src-tauri/src/source.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_code_source_names_itself() {
        let s = ClaudeCodeSource;
        assert_eq!(s.name(), "claude-code");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test source::`
Expected: FAIL — `ClaudeCodeSource` not found.

- [ ] **Step 3: Write the implementation**

At the top of `src-tauri/src/source.rs`:

```rust
/// A status source = an engine (not a surface). All Claude Code sessions —
/// terminal, VS Code, Cursor — share one source.
pub trait Source {
    fn name(&self) -> &'static str;
}

pub struct ClaudeCodeSource;

impl Source for ClaudeCodeSource {
    fn name(&self) -> &'static str {
        "claude-code"
    }
}
```

Add `mod source;` to `src-tauri/src/main.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test source::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/source.rs src-tauri/src/main.rs
git commit -m "feat(broker): Source seam (engine, not surface)"
```

---

## Task 9: HTTP layer — axum router `POST /event`, `GET /sessions`

**Files:**
- Create: `src-tauri/src/http.rs`
- Modify: `src-tauri/src/main.rs` (`mod http;`)
- Test: inline in `http.rs` using `tower::ServiceExt::oneshot`

**Interfaces:**
- Consumes: `store::SessionStore`, `model::HookEvent`, `source::ClaudeCodeSource`.
- Produces:
  - `type SharedStore = std::sync::Arc<std::sync::Mutex<SessionStore>>`
  - `fn router(store: SharedStore, on_change: OnChange) -> axum::Router` where `OnChange = Arc<dyn Fn(Vec<SessionState>) + Send + Sync>`
  - `POST /event` accepts the raw hook JSON, applies it, calls `on_change` with the new snapshot when changed, returns `200`. Malformed body → `200` anyway (never break the reporter), but logs the raw body (validation note §11.b(6)).
  - `GET /sessions` returns the JSON snapshot array.

Add to `Cargo.toml` `[dev-dependencies]`: `tower = { version = "0.5", features = ["util"] }` and `http-body-util = "0.1"`.

- [ ] **Step 1: Write the failing tests**

In `src-tauri/src/http.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;

    fn test_router() -> (axum::Router, SharedStore) {
        let store: SharedStore = Arc::new(Mutex::new(crate::store::SessionStore::new()));
        let noop: OnChange = Arc::new(|_snap| {});
        (router(store.clone(), noop), store)
    }

    #[tokio::test]
    async fn post_event_applies_to_store() {
        let (app, store) = test_router();
        let body = r#"{"hook_event_name":"UserPromptSubmit","session_id":"abc","cwd":"/x/proj"}"#;
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/event")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let snap = store.lock().unwrap().snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].id, "claude-code:abc");
    }

    #[tokio::test]
    async fn malformed_body_still_returns_200() {
        let (app, _store) = test_router();
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/event")
                    .header("content-type", "application/json")
                    .body(Body::from("not json"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn get_sessions_returns_snapshot() {
        let (app, store) = test_router();
        store.lock().unwrap().apply(
            "claude-code",
            &crate::model::HookEvent {
                hook_event_name: "Stop".into(),
                session_id: "z".into(),
                cwd: Some("/x/proj".into()),
                transcript_path: None,
                notification_type: None,
                type_field: None,
            },
            1000,
        );
        let res = app
            .oneshot(Request::builder().uri("/sessions").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(text.contains("\"status\":\"done\""));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test http::`
Expected: FAIL — `router`/`SharedStore` not found.

- [ ] **Step 3: Write the implementation**

At the top of `src-tauri/src/http.rs`:

```rust
use std::sync::{Arc, Mutex};

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};

use crate::model::{HookEvent, SessionState};
use crate::source::{ClaudeCodeSource, Source};
use crate::store::SessionStore;

pub type SharedStore = Arc<Mutex<SessionStore>>;
pub type OnChange = Arc<dyn Fn(Vec<SessionState>) + Send + Sync>;

#[derive(Clone)]
struct AppState {
    store: SharedStore,
    on_change: OnChange,
}

pub fn router(store: SharedStore, on_change: OnChange) -> Router {
    let state = AppState { store, on_change };
    Router::new()
        .route("/event", post(post_event))
        .route("/sessions", get(get_sessions))
        .with_state(state)
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

async fn post_event(State(state): State<AppState>, body: String) -> impl IntoResponse {
    match serde_json::from_str::<HookEvent>(&body) {
        Ok(event) => {
            // §11.b(6): log raw Notification payloads until the discriminator key is pinned.
            if event.hook_event_name == "Notification" {
                eprintln!("[faro] Notification payload: {body}");
            }
            let source = ClaudeCodeSource;
            let changed = {
                let mut store = state.store.lock().unwrap();
                store.apply(source.name(), &event, now_ms())
            };
            if changed {
                let snap = state.store.lock().unwrap().snapshot();
                (state.on_change)(snap);
            }
        }
        Err(e) => eprintln!("[faro] bad event body ({e}): {body}"),
    }
    // Always 200 — the reporter must never see an error.
    StatusCode::OK
}

async fn get_sessions(State(state): State<AppState>) -> impl IntoResponse {
    let snap = state.store.lock().unwrap().snapshot();
    Json(snap)
}
```

Add `mod http;` to `src-tauri/src/main.rs`. Add the dev-deps noted above to `Cargo.toml`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test http::`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/http.rs src-tauri/src/main.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat(broker): axum router POST /event + GET /sessions"
```

---

## Task 10: Wire the broker into the Tauri runtime + stale ticker

Spawns the axum listener and a stale ticker inside the Tauri app, and emits `sessions-updated` to the frontend whenever the snapshot changes.

**Files:**
- Modify: `src-tauri/src/main.rs`

**Interfaces:**
- Consumes: `http::{router, SharedStore, OnChange}`, `store::SessionStore`.
- Produces: Tauri app that emits event `"sessions-updated"` with payload `Vec<SessionState>` (camelCase JSON).

- [ ] **Step 1: Implement the wiring**

Replace `src-tauri/src/main.rs`'s `fn main` / run block with (keep the `mod` lines and `position_top_right` from Task 2):

```rust
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{Emitter, Manager};

use crate::http::{router, OnChange, SharedStore};
use crate::store::SessionStore;

const PORT: u16 = 8765;
const STALE_TTL_MS: i64 = 90_000;

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn main() {
    let store: SharedStore = Arc::new(Mutex::new(SessionStore::new()));

    tauri::Builder::default()
        .setup(move |app| {
            let window = app.get_webview_window("main").unwrap();
            position_top_right(&window)?;

            let handle = app.handle().clone();
            let emit_handle = handle.clone();
            let on_change: OnChange = Arc::new(move |snap| {
                let _ = emit_handle.emit("sessions-updated", snap);
            });

            // HTTP broker
            let app_router = router(store.clone(), on_change.clone());
            tauri::async_runtime::spawn(async move {
                let listener = tokio::net::TcpListener::bind(("127.0.0.1", PORT))
                    .await
                    .expect("faro: failed to bind broker port");
                axum::serve(listener, app_router).await.expect("faro: broker crashed");
            });

            // Stale ticker
            let stale_store = store.clone();
            let stale_emit = handle.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    let changed = stale_store.lock().unwrap().mark_stale(STALE_TTL_MS, now_ms());
                    if changed {
                        let snap = stale_store.lock().unwrap().snapshot();
                        let _ = stale_emit.emit("sessions-updated", snap);
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running faro");
}
```

- [ ] **Step 2: Verify it builds and boots**

Run: `cd src-tauri && cargo build` then from repo root `npm run tauri dev`
Expected: app boots; no panic. In another terminal:

```bash
curl -s -X POST http://127.0.0.1:8765/event \
  -H 'Content-Type: application/json' \
  -d '{"hook_event_name":"UserPromptSubmit","session_id":"manual","cwd":"/tmp/demo"}'
curl -s http://127.0.0.1:8765/sessions
```
Expected: `/sessions` returns a JSON array containing `"sessionId":"manual"`, `"status":"working"`.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/main.rs
git commit -m "feat(broker): run axum + stale ticker in Tauri, emit sessions-updated"
```

---

## Task 11: Frontend — types + pure snapshot helpers (vitest)

**Files:**
- Create: `vitest.config.ts`
- Modify: `package.json` (add `vitest` dev dep + `test` script)
- Create: `src/types.ts`, `src/snapshot.ts`, `src/snapshot.test.ts`

**Interfaces:**
- Produces:
  - `types.ts`: `type Status = ...; interface SessionState { id; source; sessionId; label; cwd; status: Status; lastEventName; lastUpdate; transcriptPath? }`
  - `snapshot.ts`: `sortSessions(list: SessionState[]): SessionState[]` (stable, by `id`).

- [ ] **Step 1: Add vitest**

```bash
cd /Users/stefano/Progetti/claude/faro && npm install -D vitest
```

Add to `package.json` `scripts`: `"test": "vitest run"`. Create `vitest.config.ts`:

```ts
import { defineConfig } from "vitest/config";
export default defineConfig({ test: { environment: "node" } });
```

- [ ] **Step 2: Write the failing test**

Create `src/snapshot.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { sortSessions } from "./snapshot";
import type { SessionState } from "./types";

const mk = (id: string): SessionState => ({
  id, source: "claude-code", sessionId: id.split(":")[1] ?? id,
  label: "p", cwd: "/x/p", status: "working",
  lastEventName: "PreToolUse", lastUpdate: 0,
});

describe("sortSessions", () => {
  it("orders by id for stable rows", () => {
    const out = sortSessions([mk("claude-code:b"), mk("claude-code:a")]);
    expect(out.map((s) => s.id)).toEqual(["claude-code:a", "claude-code:b"]);
  });
});
```

- [ ] **Step 3: Run test to verify it fails**

Run: `npm test`
Expected: FAIL — cannot find `./snapshot`.

- [ ] **Step 4: Write the implementation**

Create `src/types.ts`:

```ts
export type Status = "idle" | "working" | "blocked" | "done" | "stale" | "error";

export interface SessionState {
  id: string;
  source: string;
  sessionId: string;
  label: string;
  cwd: string;
  status: Status;
  lastEventName: string;
  lastUpdate: number;
  transcriptPath?: string;
}
```

Create `src/snapshot.ts`:

```ts
import type { SessionState } from "./types";

export function sortSessions(list: SessionState[]): SessionState[] {
  return [...list].sort((a, b) => a.id.localeCompare(b.id));
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `npm test`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add package.json package-lock.json vitest.config.ts src/types.ts src/snapshot.ts src/snapshot.test.ts
git commit -m "feat(ui): session types + tested snapshot sort helper"
```

---

## Task 12: Frontend — listen to `sessions-updated` and render real rows

**Files:**
- Modify: `src/App.tsx`
- Create: `src/components/SessionRow.tsx`
- Modify: `src/components/TrafficLight.tsx` (import `Status` from `types.ts` — remove its local duplicate)
- Modify: `src/App.css`

**Interfaces:**
- Consumes: `types.SessionState`, `snapshot.sortSessions`, Tauri `@tauri-apps/api/event` `listen`.
- Produces: live UI; empty state when no sessions.

- [ ] **Step 1: Dedupe the Status type**

In `src/components/TrafficLight.tsx`, delete the local `type Status = ...` line and instead `import type { Status } from "../types";`.

- [ ] **Step 2: Create SessionRow**

Create `src/components/SessionRow.tsx`:

```tsx
import type { SessionState } from "../types";
import { TrafficLight } from "./TrafficLight";

export function SessionRow({ session }: { session: SessionState }) {
  return (
    <div className="session-row" title={session.cwd}>
      <TrafficLight status={session.status} />
      <span className="label">{session.label}</span>
    </div>
  );
}
```

- [ ] **Step 3: Wire App to the broker event**

Replace `src/App.tsx`:

```tsx
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import "./App.css";
import type { SessionState } from "./types";
import { sortSessions } from "./snapshot";
import { SessionRow } from "./components/SessionRow";

export default function App() {
  const [sessions, setSessions] = useState<SessionState[]>([]);

  useEffect(() => {
    const unlisten = listen<SessionState[]>("sessions-updated", (e) => {
      setSessions(sortSessions(e.payload));
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  return (
    <div className="faro-root">
      {sessions.length === 0 ? (
        <div className="empty-pill">idle</div>
      ) : (
        sessions.map((s) => <SessionRow key={s.id} session={s} />)
      )}
    </div>
  );
}
```

Add to `src/App.css`:

```css
.empty-pill { font-size: 11px; opacity: 0.4; padding: 4px 6px; }
```

- [ ] **Step 4: Verify frontend tests + type check still pass**

Run: `npm test && npx tsc --noEmit`
Expected: PASS / no type errors.

- [ ] **Step 5: Commit**

```bash
git add src/App.tsx src/App.css src/components/SessionRow.tsx src/components/TrafficLight.tsx
git commit -m "feat(ui): render live session rows from sessions-updated"
```

---

## Task 13: The reporter script + dev hook registration

**Files:**
- Create: `hooks/agent-monitor-report.sh`

**Interfaces:**
- Produces: a hook script that forwards stdin JSON to the broker; manual registration in `~/.claude/settings.json`.

- [ ] **Step 1: Write the reporter (verbatim from §6.1)**

Create `hooks/agent-monitor-report.sh`:

```bash
#!/usr/bin/env bash
# Forwards a Claude Code hook payload (stdin JSON) to the local Faro broker.
# Defensive: 1s timeout, backgrounded, ALWAYS exit 0 so it can never block
# or break a Claude Code session.
BROKER_URL="${FARO_BROKER_URL:-http://127.0.0.1:8765/event}"
payload="$(cat)"
curl -s -m 1 -X POST "$BROKER_URL" \
  -H 'Content-Type: application/json' \
  -d "$payload" >/dev/null 2>&1 &
exit 0
```

- [ ] **Step 2: Install it for dev**

```bash
mkdir -p "$HOME/.claude/hooks"
cp hooks/agent-monitor-report.sh "$HOME/.claude/hooks/agent-monitor-report.sh"
chmod +x "$HOME/.claude/hooks/agent-monitor-report.sh"
```

- [ ] **Step 3: Register hooks (MERGE by hand for dev — automated installer is M4)**

⚠️ **MERGE, do not overwrite** your existing `~/.claude/settings.json`. Append these entries to the existing `hooks` arrays, using absolute `$HOME` paths. Note `StopFailure` is included (§11.b(8)):

```jsonc
{
  "hooks": {
    "SessionStart":     [{ "hooks": [{ "type": "command", "command": "$HOME/.claude/hooks/agent-monitor-report.sh" }] }],
    "UserPromptSubmit": [{ "hooks": [{ "type": "command", "command": "$HOME/.claude/hooks/agent-monitor-report.sh" }] }],
    "PreToolUse":       [{ "hooks": [{ "type": "command", "command": "$HOME/.claude/hooks/agent-monitor-report.sh" }] }],
    "Notification":     [{ "hooks": [{ "type": "command", "command": "$HOME/.claude/hooks/agent-monitor-report.sh" }] }],
    "Stop":             [{ "hooks": [{ "type": "command", "command": "$HOME/.claude/hooks/agent-monitor-report.sh" }] }],
    "StopFailure":      [{ "hooks": [{ "type": "command", "command": "$HOME/.claude/hooks/agent-monitor-report.sh" }] }],
    "SessionEnd":       [{ "hooks": [{ "type": "command", "command": "$HOME/.claude/hooks/agent-monitor-report.sh" }] }]
  }
}
```

If `$HOME` is not expanded in your hook context, substitute the literal absolute path.

- [ ] **Step 4: Smoke test the reporter against a running broker**

With `npm run tauri dev` running:

```bash
echo '{"hook_event_name":"UserPromptSubmit","session_id":"smoke","cwd":"/tmp/smoke"}' | "$HOME/.claude/hooks/agent-monitor-report.sh"
sleep 1
curl -s http://127.0.0.1:8765/sessions
```
Expected: snapshot contains `"sessionId":"smoke"`, `"status":"working"`.

- [ ] **Step 5: Commit**

```bash
git add hooks/agent-monitor-report.sh
git commit -m "feat(reporter): hook forwarder script + dev registration notes"
```

---

## Task 14: M1 end-to-end verify gate + pin the Notification discriminator

This closes M1 and resolves validation note §11.b(6).

- [ ] **Step 1: Run the live slice**

Ensure `npm run tauri dev` is running and hooks are registered (Task 13). In a separate terminal, run `claude` in any project and submit a prompt that triggers a tool that needs permission.

- [ ] **Step 2: Observe the M1 verify behaviors (§9 M1)**

Expected:
- On prompt submit → the row's light goes **🟡 working**.
- On a permission prompt → **🔴 blocked**.
- On finish → **🟢 done**.

- [ ] **Step 3: Pin the Notification discriminator key**

Look at the broker's stderr (the `[faro] Notification payload:` lines logged in Task 9 / the `npm run tauri dev` console). Confirm the actual JSON key carrying `permission_prompt`/`idle_prompt`.

- If it is `notification_type` → no change needed; the mapping already works.
- If it is a different key (e.g. `type` or nested) → update `model.rs::HookEvent` / `notification_kind()` so it reads the real key, re-run `cargo test classify:: store:: http::`, and re-verify Step 2.

Record the confirmed key in `HANDOFF.md` §4 (replace the "unconfirmed" caveat in §11.b(6) with the answer).

- [ ] **Step 4: Commit (if any change was needed)**

```bash
git add -A
git commit -m "fix(broker): pin Notification discriminator from live event; close M1"
```

---

# Milestone M2 — Multi-session + lifecycle

Most of M2 already falls out of the store being keyed by `(source, session_id)`. This milestone adds the long-purge TTL and proves multi-session + lifecycle behavior end-to-end.

## Task 15: Purge TTL for long-dead rows

Stale rows persist (dim) but should eventually disappear if the session never ends cleanly. §4: purge after e.g. 30 min.

**Files:**
- Modify: `src-tauri/src/store.rs` (add `purge`)
- Modify: `src-tauri/src/main.rs` (call `purge` in the stale ticker)
- Test: inline in `store.rs`

**Interfaces:**
- Produces: `SessionStore::purge(&mut self, purge_ms: i64, now_ms: i64) -> bool` — removes any session whose `last_update` is older than `purge_ms`, regardless of status.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` mod in `src-tauri/src/store.rs`:

```rust
    #[test]
    fn purge_removes_long_dead_sessions() {
        let mut s = SessionStore::new();
        s.apply("claude-code", &ev("Stop", "a", Some("/x/p"), None), 1_000);
        let changed = s.purge(1_800_000, 1_000 + 1_800_001);
        assert!(changed);
        assert_eq!(s.snapshot().len(), 0);
    }

    #[test]
    fn purge_keeps_recent_sessions() {
        let mut s = SessionStore::new();
        s.apply("claude-code", &ev("Stop", "a", Some("/x/p"), None), 1_000);
        assert!(!s.purge(1_800_000, 1_000 + 60_000));
        assert_eq!(s.snapshot().len(), 1);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test store::`
Expected: FAIL — `purge` not found.

- [ ] **Step 3: Write the implementation**

Add to `impl SessionStore`:

```rust
    /// Remove sessions untouched for longer than purge_ms, regardless of status.
    pub fn purge(&mut self, purge_ms: i64, now_ms: i64) -> bool {
        let before = self.sessions.len();
        self.sessions.retain(|_, s| now_ms - s.last_update <= purge_ms);
        self.sessions.len() != before
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test store::`
Expected: PASS.

- [ ] **Step 5: Call purge from the ticker**

In `src-tauri/src/main.rs`, add a constant `const PURGE_TTL_MS: i64 = 30 * 60_000;` and in the stale ticker loop, after `mark_stale`, also run purge and treat either change as a reason to emit:

```rust
                    let mut store = stale_store.lock().unwrap();
                    let c1 = store.mark_stale(STALE_TTL_MS, now_ms());
                    let c2 = store.purge(PURGE_TTL_MS, now_ms());
                    let changed = c1 || c2;
                    let snap = if changed { Some(store.snapshot()) } else { None };
                    drop(store);
                    if let Some(snap) = snap {
                        let _ = stale_emit.emit("sessions-updated", snap);
                    }
```

(Replace the previous `mark_stale`-only body.)

- [ ] **Step 6: Verify build**

Run: `cd src-tauri && cargo build`
Expected: compiles.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/store.rs src-tauri/src/main.rs
git commit -m "feat(broker): purge long-dead sessions from ticker"
```

---

## Task 16: M2 multi-session verify gate

- [ ] **Step 1: Run two concurrent sessions**

With `npm run tauri dev` running and hooks registered, open **two** terminals and run `claude` in two different project directories.

- [ ] **Step 2: Observe (§9 M2 verify)**

Expected:
- Two rows appear, each labelled with its project's `basename(cwd)`.
- Their colors change **independently** as each session works/blocks/finishes.
- Quitting one session (`SessionEnd`) removes its row, leaving the other.
- A session left mid-work goes dim (**stale**) after ~90s but stays visible.

- [ ] **Step 3: Commit any fixes**

```bash
git add -A
git commit -m "test(m2): verify multi-session rows, independent colors, lifecycle"
```

---

## Task 17: README (run + dev instructions)

**Files:**
- Create: `README.md`

- [ ] **Step 1: Write the README**

Create `README.md` documenting: what Faro is (one-paragraph), the macOS-only v0 scope, how to run (`npm install`, `npm run tauri dev`), how to install the reporter + register hooks (point to Task 13), the **known limitation** that 🔴 only detects tool-permission prompts (not plain-text questions / plan-mode approvals) per §11.b(7), and that installer/packaging/Windows are future work (M4 / §11.5).

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: README with run instructions and v0 scope/limitations"
```

---

## Self-Review (done while writing — notes for the implementer)

- **Spec coverage:** §3 layers → reporter (T13), broker (T3–T10), widget (T2, T11–T12). §4 state machine → T5 + tests; stale rule → T7; purge → T15. §5.1 contract → T4 (camelCase test). §6 reporter → T13 (verbatim) incl. `StopFailure` fix (§11.b(8)). §7 window/positioning → T2. §8 repo layout → File Structure + T1. §9 M0/M1/M2 → milestone headers + verify-gate tasks (T2, T14, T16). §11.b(6) → T9 logging + T14 pinning; §11.b(7) → documented in T17.
- **Deferred deliberately (own plans):** M3 breadth check, M4 installer/persistence/packaging/port-conflict, Windows parity.
- **Type consistency:** `Status` defined once in Rust (`model.rs`) and once in TS (`types.ts`) and imported everywhere (TrafficLight de-duped in T12). `apply(source, event, now_ms) -> bool`, `mark_stale(ttl, now) -> bool`, `purge(purge, now) -> bool`, `snapshot() -> Vec<SessionState>`, `classify(&HookEvent) -> Transition`, `router(SharedStore, OnChange) -> Router` are used identically across tasks. Event name `"sessions-updated"` matches between `main.rs` emit and `App.tsx` listen.
- **Open risk for the implementer:** the exact Tauri 2.x scaffold may name things slightly differently (`lib.rs` vs `main.rs`, builder helpers). Adapt module wiring to the generated scaffold; the logic modules are scaffold-independent.
