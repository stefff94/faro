# Faro — Agent Session Monitor · Handoff / PRD

> **Codename:** *Faro* (lighthouse — it signals status across distance). Placeholder, rename freely.
> **Status:** Ready for implementation. v0 beta.
> **Owner:** Stefano
> **This document is the single source of truth to (re)start the project in Claude Code.**

---

## 0. How to use this doc in Claude Code

1. Create an empty repo, drop this file in the root as `HANDOFF.md` (and optionally copy it to `CLAUDE.md`).
2. Start `claude` in the repo and prompt: *"Read HANDOFF.md. Confirm the locked decisions back to me, then implement Milestone M0. Stop after M0 so I can verify before M1."*
3. Build milestone-by-milestone (Section 9). **Do not jump ahead** — each milestone has an explicit verify gate.

Decisions in Section 2 are **locked**. Open questions are fenced in Section 11 — do not silently resolve them, surface them.

---

## 1. Problem & product

A developer running several agent sessions in parallel (Claude Code in terminals, IDEs, etc.) constantly context-switches just to check "is it done? is it stuck waiting for me?". 

**Faro** is an always-on-top desktop widget (Windows + macOS) that shows one **horizontal traffic light** per active agent session, so the developer can keep working and glance at status without alt-tabbing.

**Traffic-light semantics (verbatim spec — honor exactly):**
- 🟢 **green = finished** — the turn/task is complete.
- 🟡 **yellow = working** — the agent is actively executing.
- 🔴 **red = blocked** — the agent is waiting on a blocking response from the user to proceed.

The widget is **passive/read-only** in v0 (no actions, just observe).

---

## 2. Locked decisions (v0)

| Decision | Choice | Rationale |
|---|---|---|
| First & only agent in beta | **Claude Code** | Cleanest, most granular status signal (hooks). |
| Widget framework | **Tauri 2.x** (Rust core + React/TS frontend, Vite) | Tiny binary, low RAM, native transparent/always-on-top on Win+Mac. |
| Broker | **Embedded in the Tauri Rust core** | Single process, single binary. No separate daemon. |
| Status source (v0) | **`ClaudeCodeHookSource` only** | One hook block covers terminal + all IDEs + desktop Code tab (same engine, same `~/.claude/settings.json`). |
| Reporter transport | **Command hook → `curl` POST to broker** | Bulletproof, known syntax. (Native HTTP hooks are a later upgrade — see §11.) |
| Discovery fallback (filesystem-watch) | **Stub behind the `Source` trait, not shipped in v0** | Keep the vertical slice lean. |
| Window placement (v0) | **Floating pill, top-right** | 90% of the value, 10% of the risk. Notch is later. |
| Out of scope (v0) | Codex adapter, click-to-focus, Mac notch, Cowork, remote/SSH/web sources | See §10. |

---

## 3. Architecture (3 layers)

```
┌─────────────────────────────────────────────────────────────┐
│  REPORTER (per agent)                                        │
│  Claude Code hooks (~/.claude/settings.json)                 │
│  → agent-monitor-report.sh → curl POST                       │
└───────────────┬─────────────────────────────────────────────┘
                │  POST 127.0.0.1:8765/event   (hook JSON body)
                ▼
┌─────────────────────────────────────────────────────────────┐
│  BROKER  (Rust, embedded in Tauri core)                      │
│  • axum HTTP listener on 127.0.0.1 ONLY                      │
│  • Source trait → ClaudeCodeHookSource                       │
│  • in-memory session store + event→status mapping            │
│  • stale ticker (TTL)                                        │
└───────────────┬─────────────────────────────────────────────┘
                │  Tauri event: emit("sessions-updated", snapshot)
                ▼
┌─────────────────────────────────────────────────────────────┐
│  WIDGET  (React/TS in the Tauri webview)                     │
│  • transparent, frameless, always-on-top, top-right          │
│  • one SessionRow per session → TrafficLight                 │
└─────────────────────────────────────────────────────────────┘
```

The broker **must** expose an HTTP listener because the reporters are *external* Claude Code processes that POST in. Internally it pushes to the React frontend via Tauri's event system (no polling needed). An optional `GET /sessions` (snapshot) and `GET /stream` (SSE) are useful for debugging and for decoupled testing.

### The `Source` abstraction (key extensibility point)
A `Source` has two responsibilities: **discover** existing sessions, and **feed status events** into the central store. v0 ships exactly one concrete source.

```rust
// Illustrative — implement idiomatically.
pub trait Source: Send + Sync {
    fn id(&self) -> &'static str;                 // e.g. "claude-code"
    fn discover(&self) -> Vec<SessionState>;      // hook source: empty (learns on first event)
    // Status events arrive out-of-band (HTTP handler) and are routed
    // into the broker store keyed by (source_id, session_id).
}
```
Filesystem-watch, Codex, and remote sources are future `Source` impls behind this same trait. **Do not special-case Claude Code in the store** — go through the trait/contract so adding sources later requires no re-architecture.

---

## 4. Status state machine (the heart)

States: `idle | working | blocked | done | stale | error`.

| Claude Code hook event | `notification_type` | → status | light |
|---|---|---|---|
| `SessionStart` | — | `idle` (register session) | dim/grey |
| `UserPromptSubmit` | — | `working` | 🟡 |
| `PreToolUse` | — | `working` (heartbeat — resets stale timer) | 🟡 |
| `Notification` | `permission_prompt` | `blocked` | 🔴 |
| `Notification` | `idle_prompt` | `done` *(attention)* — see §11 | 🟢 |
| `Stop` | — | `done` | 🟢 |
| `StopFailure` | — | `error` | 🔴 (distinct icon/blink) |
| `SessionEnd` | — | remove session | — |
| *(no event while `working` > TTL)* | — | `stale` | dim/pulsing |

Rules:
- **Stale only applies to `working`.** A `blocked` or `done` session legitimately sits silent waiting for the user — never stale-out those.
- The cycle loops correctly: `done` → user sends new prompt → `UserPromptSubmit` → `working`.
- Default TTL: **90s** (configurable). Render `stale` dim but keep the row until `SessionEnd` or a longer purge TTL (e.g. 30 min).

---

## 5. Data contracts

### 5.1 Broker → frontend: `SessionState`
```jsonc
{
  "id": "claude-code:abc123",          // `${source}:${sessionId}` — store key
  "source": "claude-code",
  "sessionId": "abc123",
  "label": "my-project",               // derived from basename(cwd); user-renamable later
  "cwd": "/Users/stefano/dev/my-project",
  "status": "working",                 // idle|working|blocked|done|stale|error
  "lastEventName": "PreToolUse",
  "lastUpdate": 1719500000000,         // epoch ms
  "transcriptPath": "/Users/.../abc123.jsonl"  // optional
}
```
The frontend receives a **full snapshot array** on every `sessions-updated` event (simple, robust for a handful of sessions).

### 5.2 Reporter → broker: `POST /event`
Body = the **raw Claude Code hook JSON** on stdin, forwarded as-is. The broker reads `hook_event_name`, `session_id`, `cwd`, `notification_type` (when present), `transcript_path` and maps per §4. Common fields available on every hook: `session_id`, `transcript_path`, `cwd`, `hook_event_name`.

---

## 6. The reporter (drop-in, ready to use)

### 6.1 Script — `hooks/agent-monitor-report.sh`
Install to `$HOME/.claude/hooks/agent-monitor-report.sh`, then `chmod +x`.
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

### 6.2 Hook registration — append to `~/.claude/settings.json`
> ⚠️ **MERGE, do not overwrite.** The user already has hooks configured. The installer must *append* these entries to the existing arrays, preserving everything else. Use an absolute path (`$HOME/...`); `~` is not reliably expanded in all hook contexts.

```jsonc
{
  "hooks": {
    "SessionStart":     [{ "hooks": [{ "type": "command", "command": "$HOME/.claude/hooks/agent-monitor-report.sh" }] }],
    "UserPromptSubmit": [{ "hooks": [{ "type": "command", "command": "$HOME/.claude/hooks/agent-monitor-report.sh" }] }],
    "PreToolUse":       [{ "hooks": [{ "type": "command", "command": "$HOME/.claude/hooks/agent-monitor-report.sh" }] }],
    "Notification":     [{ "hooks": [{ "type": "command", "command": "$HOME/.claude/hooks/agent-monitor-report.sh" }] }],
    "Stop":             [{ "hooks": [{ "type": "command", "command": "$HOME/.claude/hooks/agent-monitor-report.sh" }] }],
    "SessionEnd":       [{ "hooks": [{ "type": "command", "command": "$HOME/.claude/hooks/agent-monitor-report.sh" }] }]
  }
}
```
`PreToolUse` fires frequently; it is intentionally used as the **working-state heartbeat**. Drop it if it proves noisy — state correctness does not depend on it (only stale detection is slightly coarser without it).

---

## 7. Widget UX & window config

- **Layout:** vertical stack of rows; each row = one session. Row = `[ ● ● ● ]` horizontal traffic light + label. Only the active color is lit; others dim.
- **Empty state:** a minimal idle pill (or auto-hide) when no sessions.
- **Window (Tauri 2.x `tauri.conf.json` → `app.windows`):**
  ```jsonc
  {
    "transparent": true,
    "decorations": false,
    "alwaysOnTop": true,
    "skipTaskbar": true,
    "resizable": false,
    "shadow": false,
    "width": 220, "height": 120
  }
  ```
  Also set `app.macOSPrivateApi: true` (required for macOS transparency).
- **Positioning:** compute top-right at runtime from the active monitor's work area (don't hardcode x/y — multi-monitor & resolution safe). Allow drag to reposition; persist position in app settings.
- **Click-through:** optional later (`set_ignore_cursor_events`). Not v0.

---

## 8. Repo structure (proposed)

```
faro/
├── HANDOFF.md                 # this file
├── README.md
├── package.json
├── vite.config.ts
├── src/                       # React + TS frontend
│   ├── main.tsx
│   ├── App.tsx
│   ├── types.ts               # SessionState, Status
│   └── components/
│       ├── SessionRow.tsx
│       └── TrafficLight.tsx
├── src-tauri/                 # Rust core + embedded broker
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/
│       ├── main.rs
│       ├── window.rs          # runtime top-right positioning
│       └── broker/
│           ├── mod.rs
│           ├── server.rs      # axum: POST /event, GET /sessions, GET /stream
│           ├── state.rs       # store + event→status mapping + stale ticker
│           └── source.rs      # Source trait + ClaudeCodeHookSource
└── hooks/
    └── agent-monitor-report.sh
```

Suggested crates: `tauri` (2.x), `axum`, `tokio`, `serde`/`serde_json`. Frontend: `@tauri-apps/api`.

---

## 9. Milestones (build in order; verify gate after each)

**M0 — Scaffold + the UI risk.** Tauri+React boots. Transparent, frameless, always-on-top window pinned top-right, rendering a **hardcoded** dummy traffic light. 
*Verify:* it floats over other apps on macOS, transparent, doesn't grab focus.

**M1 — Vertical slice (the milestone that matters).** Rust broker runs an axum listener on `127.0.0.1:8765`, accepts `POST /event`, maps **one** real Claude Code session's hook events to status (§4), emits `sessions-updated`, and **one real light changes color live** from a terminal Claude Code session. 
*Verify:* run `claude` in a terminal → light goes 🟡 on prompt, 🔴 on a permission prompt, 🟢 on finish.

**M2 — Multi-session + lifecycle.** Multiple concurrent sessions as rows; `SessionStart`/`SessionEnd` add/remove; stale TTL works; labels from `cwd` basename. 
*Verify:* two terminals, two rows, independent colors, dead session goes stale/removed.

**M3 — Free breadth check.** Confirm VS Code / Cursor / desktop **Code** tab sessions appear with **zero extra code** (same engine + settings.json). This is the proof the "source = engine, not surface" model holds. 
*Verify:* start Claude Code in VS Code → a row appears, no new code.

**M4 — Hardening / install.** Idempotent install script that **safely appends** hooks to `settings.json` + installs the reporter script; basic settings (port, position persistence); packaging (`.dmg` / `.msi`). 
*Verify:* clean install on a second machine; re-running install doesn't duplicate hooks.

**Definition of done (v0):** with several Claude Code sessions across terminal + at least one IDE, Faro shows a live, correct 🔴🟡🟢 row per session; sessions appear/disappear correctly; stale handled; survives broker restart.

---

## 10. Explicitly out of scope for v0 (backlog)
- Filesystem-watch discovery source (`~/.claude/projects/<hash>/<session-id>.jsonl`) — discovery-only fallback for sessions without the hook configured. Cannot reliably distinguish blocked vs working.
- **Codex** adapter (via `~/.codex/config.toml` `notify` → `approval-requested`=🔴, `agent-turn-complete`=🟢; "working" is inferred — coarser than Claude Code).
- Click a light → focus/raise that terminal/IDE.
- Mac **notch** integration.
- **Cowork** source (sandboxed container, egress-controlled — needs a spike, §11).
- Remote / SSH / Web sources (engine runs off-machine).
- Session rename / pinning / ordering.

---

## 11. Open questions / spikes (surface, don't silently resolve)
1. **Native HTTP hooks.** Claude Code supports HTTP-type hooks (payload arrives as the POST body), which would remove the `curl` script entirely. Confirm exact `settings.json` syntax for `{ "type": "http", ... }` via `/hooks` or current docs, then offer it as a cleaner alternative to §6.
2. **`idle_prompt` mapping.** It fires when prompt input is idle ~60s (usually after a turn ends). v0 maps it to `done`. Decide whether it should instead be a distinct "needs attention" state. Make it a tunable.
3. **Cowork.** Same engine, but runs in an isolated container (macOS: Apple Virtualization Framework) with controlled network egress. Open: (a) are `~/.claude/settings.json` hooks loaded inside the sandbox? (b) can a hook reach the host's `127.0.0.1:8765`? Spike before committing.
4. **Port conflicts / multi-instance.** Fixed `8765` vs discovery. Decide for M4.
5. **Windows parity** for transparency/always-on-top/positioning — validate during M0/M3 on Windows, not just macOS.

---

### 11.b Validation pass (2026-06-27) — checked against the official Claude Code hooks docs

**Confirmed (the design holds):**
- All hook events used in §4 are real: `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `Notification`, `Stop`, `StopFailure`, `SessionEnd`.
- The `Notification` matcher really discriminates on notification type, with values incl. `permission_prompt` and `idle_prompt` (plus `auth_success`, `elicitation_*`). The 🔴-vs-🟢 distinction the whole product rests on is therefore supported.
- `StopFailure` exists and even carries an error-type discriminator (`rate_limit`, `overloaded`, `authentication_failed`, `billing_error`, …), so a failed turn can be told apart from a clean finish.

**To validate at runtime during M1 (do not block on these, but resolve under M1's verify gate):**
6. **Exact JSON field name for the Notification discriminator.** §4 assumes a field literally named `notification_type`. The docs confirm the *values* but the exact payload key was not verifiable from the docs (truncated). Action: log the first real `Notification` payload, read the actual key, and pin it in the §4 mapping + §5 contract. Don't ship the mapping until this is confirmed from a live event.
7. **🔴 "blocked" is under-detected by design.** Red fires only on `Notification`/`permission_prompt` (tool-permission gates). An agent that asks a question *in plain text*, or a plan-mode approval, may not emit a `Notification` — the row stays 🟡 while it actually waits on the user. Accepted as a v0 limitation; document it in the README and revisit post-v0 (possible spike: transcript tailing via `transcriptPath`).

**Concrete fix needed (inconsistency in this doc):**
8. **`StopFailure` is mapped in §4 (→ `error`/🔴) but NOT registered in §6.2.** As written, the `error` state can never trigger. Add a `"StopFailure"` entry to the hook registration in §6.2 (and the installer in M4), or drop `error` from §4. Recommendation: register it — distinguishing a crashed/rate-limited turn from a clean finish is cheap and valuable.

**Platform scope (per owner, 2026-06-27):** v0 targets **macOS first**; Windows is a deliberate later step. The `Source` trait already isolates status logic from the OS; keep all platform-specific window/transparency/positioning code behind a thin seam so the Windows port is additive. Item 5 (Windows parity) is deferred out of the first MVP — validate macOS only for now.

---

## 12. Guardrails for the implementing agent
- Honor the traffic-light semantics in §1 **exactly** (green=finished, yellow=working, red=blocked-on-user).
- Broker binds **loopback only** (`127.0.0.1`). Never bind `0.0.0.0`.
- The reporter must **never** block or fail a Claude Code session (1s timeout, backgrounded, exit 0).
- Route everything through the `Source` trait — no Claude-Code-specific shortcuts in the store.
- The hook installer must **append/merge**, never overwrite, `settings.json`.
- Stop at each milestone's verify gate.
