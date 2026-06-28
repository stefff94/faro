# Faro Widget Redesign — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild Faro's UI into a right-edge glass drawer with a collapsed "count pill", a reverse-escalation attention model, and richer per-session content (project · branch · task · time-in-state).

**Architecture:** The Rust broker (`src-tauri`) enriches each `SessionState` with three new fields (`statusSince`, `branch`, `taskSummary`) without blocking the HTTP event path (git/transcript reads are cached/best-effort). The React frontend replaces the traffic-light list with pure, unit-tested helpers (sort, duration, aggregate counts, attention phase, audio cue detection) feeding thin presentational components (`StatusChip`, `SessionCard`, `CollapsedPill`, `DrawerPanel`, `SessionDetail`). The Tauri window becomes a tall right-edge overlay with click-through transparent regions.

**Tech Stack:** Rust (Tauri 2, axum, serde), TypeScript, React 19, Vitest (node env).

## Global Constraints

- Platform: macOS only (v0). No Windows code.
- Spec of record: `docs/superpowers/specs/2026-06-28-faro-widget-redesign-design.md`. Phase 2 items (notch skin, bring-terminal-to-front, intelligent task summary) are OUT OF SCOPE.
- No blocking work on the HTTP event path: git branch and transcript reads MUST be cached or best-effort/non-fatal (spec §5, §9).
- JSON wire format is camelCase. Rust structs use `#[serde(rename_all = "camelCase")]` (already established in `model.rs`). New Rust fields: `status_since` → `statusSince`, `branch` → `branch`, `task_summary` → `taskSummary`.
- Status palette (spec §4.2): working `#f5c518`, blocked/needs-input `#ef4444`, done `#22c55e`, idle/stale `#6b7280`.
- The broker must always return HTTP 200 to reporters (existing invariant in `http.rs`).
- Existing test commands: Rust `cd src-tauri && cargo test`; Frontend `npm test` (vitest, node environment — keep new tests as pure functions, no jsdom).
- TDD: pure logic gets a failing test first. Visual components and window/audio behavior get explicit manual verification steps via `npm run tauri dev`.
- Commit after every task. Work happens on a feature branch off `main`.

---

## File Structure

**Rust (broker) — modify:**
- `src-tauri/src/model.rs` — add `status_since`, `branch`, `task_summary` to `SessionState`.
- `src-tauri/src/store.rs` — set `status_since` on status transitions; wire branch + summary resolvers.
- `src-tauri/src/git.rs` *(new)* — cached git branch resolver from a cwd.
- `src-tauri/src/transcript.rs` *(new)* — extract last user prompt from a transcript file.

**Frontend — create/modify:**
- `src/types.ts` — extend `SessionState`, add `Aggregate`, `AttentionPhase`, `Cue` types.
- `src/snapshot.ts` — status-priority sort, duration formatter, aggregate counts.
- `src/attention.ts` *(new)* — pure `attentionPhase()` + `detectCues()`.
- `src/hooks/useAttention.ts` *(new)* — escalation timer hook around `attentionPhase`.
- `src/hooks/useAudioCues.ts` *(new)* — fires sounds from `detectCues`.
- `src/components/StatusChip.tsx` *(new, replaces TrafficLight)*.
- `src/components/SessionCard.tsx` *(replaces SessionRow)*.
- `src/components/SessionDetail.tsx` *(new)* — expanded detail + quick actions.
- `src/components/CollapsedPill.tsx` *(new)*.
- `src/components/DrawerPanel.tsx` *(new)* — peek/pin container.
- `src/App.tsx` — wire it all together.
- `src/App.css` — glass (A) + chip (C) styling, replaces current CSS.
- `src/settings.ts` *(new)* — local settings (decay timer, sound on/off, per-session mute).
- Delete `src/components/TrafficLight.tsx`, `src/components/SessionRow.tsx`.

**Window (Rust) — modify:**
- `src-tauri/src/lib.rs` — right-edge overlay positioning + size; `src-tauri/tauri.conf.json` window dims.

---

## Task 1: Add `statusSince` to the session model

**Files:**
- Modify: `src-tauri/src/model.rs` (the `SessionState` struct + its camelCase serialization test)
- Modify: `src-tauri/src/store.rs` (set the field on create + on status change)

**Interfaces:**
- Produces: `SessionState.status_since: i64` (JSON `statusSince`) — epoch ms of the most recent status transition. On create it equals `last_update`.

- [ ] **Step 1: Write the failing test** — add to `store.rs` `mod tests`:

```rust
#[test]
fn status_since_updates_only_when_status_changes() {
    let mut s = SessionStore::new();
    s.apply("claude-code", &ev("UserPromptSubmit", "a", Some("/x/p"), None), 1_000); // Working
    assert_eq!(s.snapshot()[0].status_since, 1_000);
    // same status (still Working) at a later time → status_since unchanged
    s.apply("claude-code", &ev("PreToolUse", "a", Some("/x/p"), None), 2_000);
    assert_eq!(s.snapshot()[0].status_since, 1_000);
    assert_eq!(s.snapshot()[0].last_update, 2_000);
    // status change (Working → Done) → status_since moves
    s.apply("claude-code", &ev("Stop", "a", Some("/x/p"), None), 3_000);
    assert_eq!(s.snapshot()[0].status_since, 3_000);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test status_since_updates_only_when_status_changes`
Expected: FAIL — compile error: no field `status_since` on `SessionState`.

- [ ] **Step 3: Add the field to `SessionState` in `model.rs`**

In the `SessionState` struct add (place after `last_update: i64,`):

```rust
    pub status_since: i64,
```

- [ ] **Step 4: Populate it in `store.rs` `apply()`**

In the `Transition::Set(status)` arm: add `status_since: now_ms,` to the `SessionState { … }` initializer (next to `last_update: now_ms,`). Then, where the existing code updates an existing entry, guard the status-change:

```rust
                if entry.status != status {
                    entry.status_since = now_ms;
                }
                entry.status = status;
                entry.last_event_name = event.hook_event_name.clone();
                entry.last_update = now_ms;
```

(Replace the existing `entry.status = status;` line with the block above.)

- [ ] **Step 5: Also set `status_since` when marking stale in `store.rs`**

In `mark_stale`, when flipping to `Status::Stale`, set `s.status_since = now_ms;` next to `s.status = Status::Stale;`.

- [ ] **Step 6: Update the model serialization test in `model.rs`**

Find the `session_state_serializes_camelcase()` test and add `status_since: 1000,` to the `SessionState { … }` it builds, and assert the JSON contains `"statusSince":1000`. (Match the existing assertion style in that test.)

- [ ] **Step 7: Run tests to verify they pass**

Run: `cd src-tauri && cargo test`
Expected: PASS (all existing + new tests).

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/model.rs src-tauri/src/store.rs
git commit -m "feat(broker): track statusSince timestamp per session"
```

---

## Task 2: Cached git branch resolver

**Files:**
- Create: `src-tauri/src/git.rs`
- Modify: `src-tauri/src/lib.rs` (add `pub mod git;`)

**Interfaces:**
- Produces: `git::branch_for(cwd: &str) -> Option<String>` — reads the current branch by parsing `<repo>/.git/HEAD` (walking up from `cwd` to find `.git`), cached per resolved repo root. Returns `None` when not a repo or on detached HEAD.

This resolver reads files only (no subprocess), so it is cheap; we still cache by repo root to avoid re-walking on every event.

- [ ] **Step 1: Write the failing test** — create `src-tauri/src/git.rs`:

```rust
use std::collections::HashMap;
use std::sync::Mutex;

/// Parse the branch name out of a `.git/HEAD` file body.
/// "ref: refs/heads/main\n" -> Some("main"); a raw sha (detached) -> None.
pub fn parse_head(head_contents: &str) -> Option<String> {
    let line = head_contents.trim();
    let rest = line.strip_prefix("ref:")?.trim();
    rest.strip_prefix("refs/heads/").map(|b| b.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_branch_ref() {
        assert_eq!(parse_head("ref: refs/heads/feat/notch-hud\n"), Some("feat/notch-hud".into()));
        assert_eq!(parse_head("ref: refs/heads/main"), Some("main".into()));
    }

    #[test]
    fn detached_head_is_none() {
        assert_eq!(parse_head("a1b2c3d4e5f6\n"), None);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

First register the module: in `src-tauri/src/lib.rs` add `pub mod git;` near the other `pub mod` lines.
Run: `cd src-tauri && cargo test parses_branch_ref`
Expected: PASS for `parse_head` tests (this step's deliverable is the pure parser).

- [ ] **Step 3: Add the cached file-based resolver to `git.rs`**

Append below `parse_head`:

```rust
use std::path::{Path, PathBuf};

/// Walk up from `cwd` to find a directory containing `.git`; return that `.git` path.
fn find_git_dir(cwd: &str) -> Option<PathBuf> {
    let mut dir: Option<&Path> = Some(Path::new(cwd));
    while let Some(d) = dir {
        let candidate = d.join(".git");
        if candidate.exists() {
            return Some(candidate);
        }
        dir = d.parent();
    }
    None
}

static CACHE: Mutex<Option<HashMap<String, Option<String>>>> = Mutex::new(None);

/// Branch for a working directory, cached by the resolved `.git` path.
pub fn branch_for(cwd: &str) -> Option<String> {
    let git_dir = find_git_dir(cwd)?;
    let key = git_dir.to_string_lossy().to_string();
    {
        let mut guard = CACHE.lock().unwrap();
        let map = guard.get_or_insert_with(HashMap::new);
        if let Some(hit) = map.get(&key) {
            return hit.clone();
        }
    }
    // `.git` may be a file (worktree: "gitdir: <path>") — resolve it.
    let head_path = if git_dir.is_file() {
        std::fs::read_to_string(&git_dir).ok()
            .and_then(|s| s.trim().strip_prefix("gitdir:").map(|p| PathBuf::from(p.trim()).join("HEAD")))
    } else {
        Some(git_dir.join("HEAD"))
    };
    let branch = head_path
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|c| parse_head(&c));
    CACHE.lock().unwrap().get_or_insert_with(HashMap::new).insert(key, branch.clone());
    branch
}

/// Clear the cache (call when a session's status changes, so branch can refresh).
pub fn invalidate() {
    if let Some(map) = CACHE.lock().unwrap().as_mut() {
        map.clear();
    }
}
```

- [ ] **Step 4: Write a test for `find_git_dir` + `branch_for` against a temp repo**

Add to `git.rs` `mod tests`:

```rust
    #[test]
    fn branch_for_reads_real_head() {
        let dir = std::env::temp_dir().join(format!("faro-git-{}", std::process::id()));
        let git = dir.join(".git");
        std::fs::create_dir_all(&git).unwrap();
        std::fs::write(git.join("HEAD"), "ref: refs/heads/test-branch\n").unwrap();
        super::invalidate();
        assert_eq!(branch_for(dir.to_str().unwrap()), Some("test-branch".into()));
        std::fs::remove_dir_all(&dir).ok();
    }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --lib git`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/git.rs src-tauri/src/lib.rs
git commit -m "feat(broker): cached git branch resolver from .git/HEAD"
```

---

## Task 3: Transcript last-prompt extractor

**Files:**
- Create: `src-tauri/src/transcript.rs`
- Modify: `src-tauri/src/lib.rs` (add `pub mod transcript;`)

**Interfaces:**
- Produces: `transcript::last_user_prompt(path: &str) -> Option<String>` — reads a Claude Code transcript (JSONL), returns the most recent user prompt text, normalized to a single line and truncated to 60 chars (with `…` if cut). Best-effort: returns `None` on missing/unreadable/empty.

Phase 1 uses the last user prompt as the task summary (spec §5; intelligent summary is Phase 2).

- [ ] **Step 1: Write the failing test** — create `src-tauri/src/transcript.rs`:

```rust
/// Normalize a prompt to a single trimmed line, truncated to 60 chars with an ellipsis.
pub fn normalize(text: &str) -> String {
    let one_line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() > 60 {
        let cut: String = one_line.chars().take(59).collect();
        format!("{cut}…")
    } else {
        one_line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_collapses_whitespace() {
        assert_eq!(normalize("  fix\n the   auth bug "), "fix the auth bug");
    }

    #[test]
    fn normalize_truncates_long_text() {
        let long = "a".repeat(100);
        let out = normalize(&long);
        assert_eq!(out.chars().count(), 60); // 59 + ellipsis
        assert!(out.ends_with('…'));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Register module: in `lib.rs` add `pub mod transcript;`.
Run: `cd src-tauri && cargo test normalize_collapses_whitespace`
Expected: PASS for the `normalize` unit (this step's deliverable).

- [ ] **Step 3: Add the JSONL reader**

Append to `transcript.rs`:

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct Entry {
    #[serde(rename = "type")]
    kind: Option<String>,
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    role: Option<String>,
    content: Option<serde_json::Value>,
}

fn text_from_content(content: &serde_json::Value) -> Option<String> {
    match content {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(items) => items.iter().find_map(|it| {
            it.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
        }),
        _ => None,
    }
}

/// Last user prompt from a transcript JSONL file, normalized. Best-effort.
pub fn last_user_prompt(path: &str) -> Option<String> {
    let body = std::fs::read_to_string(path).ok()?;
    let prompt = body
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<Entry>(line).ok())
        .find(|e| {
            e.kind.as_deref() == Some("user")
                || e.message.as_ref().and_then(|m| m.role.as_deref()) == Some("user")
        })
        .and_then(|e| e.message)
        .and_then(|m| m.content)
        .and_then(|c| text_from_content(&c))?;
    let n = normalize(&prompt);
    if n.is_empty() { None } else { Some(n) }
}
```

- [ ] **Step 4: Write a test against a temp transcript**

Add to `transcript.rs` `mod tests`:

```rust
    #[test]
    fn reads_last_user_prompt_from_jsonl() {
        let path = std::env::temp_dir().join(format!("faro-tx-{}.jsonl", std::process::id()));
        let jsonl = "{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"first\"}}\n\
                     {\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":\"hi\"}}\n\
                     {\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"wire the notch detection\"}}\n";
        std::fs::write(&path, jsonl).unwrap();
        assert_eq!(last_user_prompt(path.to_str().unwrap()), Some("wire the notch detection".into()));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn missing_file_is_none() {
        assert_eq!(last_user_prompt("/no/such/file.jsonl"), None);
    }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --lib transcript`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/transcript.rs src-tauri/src/lib.rs
git commit -m "feat(broker): extract last user prompt from transcript as task summary"
```

---

## Task 4: Wire branch + taskSummary into the snapshot

**Files:**
- Modify: `src-tauri/src/model.rs` (add `branch`, `task_summary` fields)
- Modify: `src-tauri/src/store.rs` (`snapshot()` enriches each session)

**Interfaces:**
- Consumes: `git::branch_for`, `transcript::last_user_prompt` (Tasks 2–3).
- Produces: `SessionState.branch: Option<String>` (JSON `branch`), `SessionState.task_summary: Option<String>` (JSON `taskSummary`), filled in `snapshot()`.

Enrichment happens in `snapshot()` (read path), not `apply()` (event path), so the HTTP handler stays fast (spec §5 constraint). `git` caches; transcript reads are small and best-effort.

- [ ] **Step 1: Write the failing test** — add to `store.rs` `mod tests`:

```rust
#[test]
fn snapshot_includes_branch_and_summary_fields_defaulting_none() {
    let mut s = SessionStore::new();
    s.apply("claude-code", &ev("UserPromptSubmit", "a", Some("/x/p"), None), 1_000);
    let snap = s.snapshot();
    // /x/p is not a real repo and event has no transcript → both None, no panic
    assert_eq!(snap[0].branch, None);
    assert_eq!(snap[0].task_summary, None);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test snapshot_includes_branch_and_summary_fields_defaulting_none`
Expected: FAIL — no field `branch` / `task_summary`.

- [ ] **Step 3: Add fields to `SessionState` in `model.rs`**

After `transcript_path: Option<String>,` add:

```rust
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_summary: Option<String>,
```

- [ ] **Step 4: Default them in `store.rs` `apply()`**

In the `SessionState { … }` initializer add `branch: None,` and `task_summary: None,`. (They are recomputed in `snapshot()`; stored value is just a default.)

- [ ] **Step 5: Enrich in `store.rs` `snapshot()`**

Replace the body of `snapshot()` with:

```rust
    pub fn snapshot(&self) -> Vec<SessionState> {
        let mut v: Vec<SessionState> = self
            .sessions
            .values()
            .cloned()
            .map(|mut s| {
                s.branch = crate::git::branch_for(&s.cwd);
                s.task_summary = s
                    .transcript_path
                    .as_deref()
                    .and_then(crate::transcript::last_user_prompt);
                s
            })
            .collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }
```

- [ ] **Step 6: Invalidate the git cache on each status change**

In `apply()`, inside the `if entry.status != status { … }` block from Task 1, add `crate::git::invalidate();` so branch refreshes when a session meaningfully changes.

- [ ] **Step 7: Fix other `SessionState { … }` literals**

The model test in `model.rs` builds a `SessionState` literal — add `branch: None,` and `task_summary: None,` there too. Run `cargo test` and fix any remaining literal that fails to compile the same way.

- [ ] **Step 8: Run tests to verify they pass**

Run: `cd src-tauri && cargo test`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/model.rs src-tauri/src/store.rs
git commit -m "feat(broker): enrich snapshot with git branch and task summary"
```

---

## Task 5: Frontend types + snapshot helpers (sort, duration, counts)

**Files:**
- Modify: `src/types.ts`
- Modify: `src/snapshot.ts`
- Modify: `src/snapshot.test.ts`

**Interfaces:**
- Produces:
  - `SessionState` gains `statusSince: number`, `branch?: string`, `taskSummary?: string`.
  - `type Aggregate = { input: number; working: number; done: number; idle: number; total: number }`
  - `sortSessions(list: SessionState[]): SessionState[]` — by status priority (blocked/error → working → done → idle/stale), then `lastUpdate` desc.
  - `formatDuration(ms: number): string` — `< 5s`→`"ora"`, `< 60s`→`"22s"`, else `"1m 12s"`.
  - `aggregate(list: SessionState[]): Aggregate`.

- [ ] **Step 1: Write the failing tests** — replace `src/snapshot.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { sortSessions, formatDuration, aggregate } from "./snapshot";
import type { SessionState, Status } from "./types";

const mk = (id: string, status: Status, lastUpdate = 0): SessionState => ({
  id, source: "claude-code", sessionId: id, label: "p", cwd: "/x/p",
  status, lastEventName: "x", lastUpdate, statusSince: lastUpdate,
});

describe("sortSessions", () => {
  it("orders blocked before working before done", () => {
    const out = sortSessions([mk("a", "done", 9), mk("b", "working", 5), mk("c", "blocked", 1)]);
    expect(out.map((s) => s.status)).toEqual(["blocked", "working", "done"]);
  });
  it("within a status, newer lastUpdate first", () => {
    const out = sortSessions([mk("a", "working", 1), mk("b", "working", 9)]);
    expect(out.map((s) => s.id)).toEqual(["b", "a"]);
  });
});

describe("formatDuration", () => {
  it("formats buckets", () => {
    expect(formatDuration(2_000)).toBe("ora");
    expect(formatDuration(22_000)).toBe("22s");
    expect(formatDuration(72_000)).toBe("1m 12s");
  });
});

describe("aggregate", () => {
  it("counts by bucket", () => {
    const a = aggregate([mk("a", "blocked"), mk("b", "working"), mk("c", "done"), mk("d", "idle")]);
    expect(a).toEqual({ input: 1, working: 1, done: 1, idle: 1, total: 4 });
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm test`
Expected: FAIL — `formatDuration`/`aggregate` not exported; `statusSince` type error.

- [ ] **Step 3: Extend `src/types.ts`**

Add `statusSince: number;`, `branch?: string;`, `taskSummary?: string;` to `SessionState`. Add:

```ts
export type Aggregate = {
  input: number; working: number; done: number; idle: number; total: number;
};
```

- [ ] **Step 4: Implement helpers in `src/snapshot.ts`**

```ts
import type { SessionState, Status, Aggregate } from "./types";

const PRIORITY: Record<Status, number> = {
  blocked: 0, error: 0, working: 1, done: 2, idle: 3, stale: 3,
};

export function sortSessions(list: SessionState[]): SessionState[] {
  return [...list].sort(
    (a, b) =>
      PRIORITY[a.status] - PRIORITY[b.status] ||
      b.lastUpdate - a.lastUpdate ||
      a.id.localeCompare(b.id),
  );
}

export function formatDuration(ms: number): string {
  const s = Math.floor(ms / 1000);
  if (s < 5) return "ora";
  if (s < 60) return `${s}s`;
  return `${Math.floor(s / 60)}m ${s % 60}s`;
}

export function aggregate(list: SessionState[]): Aggregate {
  const a: Aggregate = { input: 0, working: 0, done: 0, idle: 0, total: list.length };
  for (const s of list) {
    if (s.status === "blocked" || s.status === "error") a.input++;
    else if (s.status === "working") a.working++;
    else if (s.status === "done") a.done++;
    else a.idle++;
  }
  return a;
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `npm test`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/types.ts src/snapshot.ts src/snapshot.test.ts
git commit -m "feat(frontend): status-priority sort, duration formatter, aggregate counts"
```

---

## Task 6: Attention phase + audio cue detection (pure logic)

**Files:**
- Create: `src/attention.ts`
- Create: `src/attention.test.ts`
- Modify: `src/types.ts`

**Interfaces:**
- Produces:
  - `type AttentionPhase = "idle" | "working" | "needs-input-evident" | "needs-input-compact"`
  - `type Cue = "done" | "needs-input"`
  - `attentionPhase(agg: Aggregate, blockedSince: number | null, now: number, decayMs: number): AttentionPhase`
  - `detectCues(prev: SessionState[], next: SessionState[]): Cue[]` — emits `"needs-input"` when a session newly enters blocked/error, `"done"` when one newly enters done.

- [ ] **Step 1: Write the failing tests** — create `src/attention.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { attentionPhase, detectCues } from "./attention";
import type { SessionState, Status, Aggregate } from "./types";

const agg = (input: number, working: number, done = 0): Aggregate => ({
  input, working, done, idle: 0, total: input + working + done,
});
const mk = (id: string, status: Status): SessionState => ({
  id, source: "c", sessionId: id, label: "p", cwd: "/x", status,
  lastEventName: "x", lastUpdate: 0, statusSince: 0,
});

describe("attentionPhase", () => {
  it("idle when nothing active", () => {
    expect(attentionPhase(agg(0, 0), null, 0, 8000)).toBe("idle");
  });
  it("working when something runs and none blocked", () => {
    expect(attentionPhase(agg(0, 2), null, 0, 8000)).toBe("working");
  });
  it("needs-input is evident before decay, compact after", () => {
    expect(attentionPhase(agg(1, 0), 1000, 5000, 8000)).toBe("needs-input-evident");
    expect(attentionPhase(agg(1, 0), 1000, 10000, 8000)).toBe("needs-input-compact");
  });
});

describe("detectCues", () => {
  it("fires needs-input when a session newly blocks", () => {
    expect(detectCues([mk("a", "working")], [mk("a", "blocked")])).toEqual(["needs-input"]);
  });
  it("fires done when a session newly completes", () => {
    expect(detectCues([mk("a", "working")], [mk("a", "done")])).toEqual(["done"]);
  });
  it("no cue when status unchanged", () => {
    expect(detectCues([mk("a", "blocked")], [mk("a", "blocked")])).toEqual([]);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm test`
Expected: FAIL — module `./attention` not found.

- [ ] **Step 3: Add types to `src/types.ts`**

```ts
export type AttentionPhase =
  | "idle" | "working" | "needs-input-evident" | "needs-input-compact";
export type Cue = "done" | "needs-input";
```

- [ ] **Step 4: Implement `src/attention.ts`**

```ts
import type { Aggregate, SessionState, AttentionPhase, Cue } from "./types";

export function attentionPhase(
  agg: Aggregate, blockedSince: number | null, now: number, decayMs: number,
): AttentionPhase {
  if (agg.input > 0 && blockedSince !== null) {
    return now - blockedSince < decayMs ? "needs-input-evident" : "needs-input-compact";
  }
  if (agg.working > 0) return "working";
  return "idle";
}

const isBlocked = (s: SessionState) => s.status === "blocked" || s.status === "error";

export function detectCues(prev: SessionState[], next: SessionState[]): Cue[] {
  const before = new Map(prev.map((s) => [s.id, s.status]));
  const cues: Cue[] = [];
  for (const s of next) {
    const was = before.get(s.id);
    if (was === s.status) continue;
    if (isBlocked(s) && !(was === "blocked" || was === "error")) cues.push("needs-input");
    else if (s.status === "done" && was !== "done") cues.push("done");
  }
  return cues;
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `npm test`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/attention.ts src/attention.test.ts src/types.ts
git commit -m "feat(frontend): pure attention-phase and audio-cue detection"
```

---

## Task 7: Settings module (decay timer, sounds, per-session mute)

**Files:**
- Create: `src/settings.ts`
- Create: `src/settings.test.ts`

**Interfaces:**
- Produces:
  - `type Settings = { decayMs: number; soundEnabled: boolean; mutedSessionIds: string[] }`
  - `DEFAULT_SETTINGS: Settings` (`decayMs: 8000`, `soundEnabled: true`, `mutedSessionIds: []`)
  - `loadSettings(): Settings` / `saveSettings(s: Settings): void` (localStorage, key `faro.settings`, tolerant of malformed/missing).
  - `isMuted(s: Settings, id: string): boolean`

- [ ] **Step 1: Write the failing tests** — create `src/settings.test.ts`:

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { loadSettings, saveSettings, isMuted, DEFAULT_SETTINGS } from "./settings";

beforeEach(() => {
  (globalThis as any).localStorage = {
    _v: {} as Record<string, string>,
    getItem(k: string) { return this._v[k] ?? null; },
    setItem(k: string, v: string) { this._v[k] = v; },
  };
});

describe("settings", () => {
  it("returns defaults when nothing stored", () => {
    expect(loadSettings()).toEqual(DEFAULT_SETTINGS);
  });
  it("round-trips saved settings", () => {
    saveSettings({ decayMs: 5000, soundEnabled: false, mutedSessionIds: ["x"] });
    expect(loadSettings().decayMs).toBe(5000);
    expect(isMuted(loadSettings(), "x")).toBe(true);
  });
  it("tolerates malformed storage", () => {
    localStorage.setItem("faro.settings", "{not json");
    expect(loadSettings()).toEqual(DEFAULT_SETTINGS);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm test`
Expected: FAIL — module `./settings` not found.

- [ ] **Step 3: Implement `src/settings.ts`**

```ts
export type Settings = {
  decayMs: number;
  soundEnabled: boolean;
  mutedSessionIds: string[];
};

export const DEFAULT_SETTINGS: Settings = {
  decayMs: 8000, soundEnabled: true, mutedSessionIds: [],
};

const KEY = "faro.settings";

export function loadSettings(): Settings {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return DEFAULT_SETTINGS;
    return { ...DEFAULT_SETTINGS, ...JSON.parse(raw) };
  } catch {
    return DEFAULT_SETTINGS;
  }
}

export function saveSettings(s: Settings): void {
  try { localStorage.setItem(KEY, JSON.stringify(s)); } catch { /* ignore */ }
}

export function isMuted(s: Settings, id: string): boolean {
  return s.mutedSessionIds.includes(id);
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `npm test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/settings.ts src/settings.test.ts
git commit -m "feat(frontend): local settings (decay, sound, per-session mute)"
```

---

## Task 8: StatusChip + SessionCard components

**Files:**
- Create: `src/components/StatusChip.tsx`
- Create: `src/components/SessionCard.tsx`
- Delete: `src/components/TrafficLight.tsx`, `src/components/SessionRow.tsx`

**Interfaces:**
- Consumes: `SessionState`, `formatDuration` (Task 5).
- Produces:
  - `<StatusChip status={Status} />` — renders the chip label + color per spec §4.2.
  - `<SessionCard session={SessionState} now={number} onClick?={() => void} />` — project · branch, task summary, chip, time-in-state (`formatDuration(now - statusSince)`).

These are presentational. Verified by rendering in the running app (Task 12), not unit tests.

- [ ] **Step 1: Create `src/components/StatusChip.tsx`**

```tsx
import type { Status } from "../types";

const CHIP: Record<Status, { cls: string; label: string }> = {
  working: { cls: "chip chW", label: "● working" },
  blocked: { cls: "chip chB", label: "◆ input" },
  error:   { cls: "chip chB", label: "◆ error" },
  done:    { cls: "chip chD", label: "✓ done" },
  idle:    { cls: "chip chI", label: "· idle" },
  stale:   { cls: "chip chI", label: "· idle" },
};

export function StatusChip({ status }: { status: Status }) {
  const c = CHIP[status];
  return <span className={c.cls}>{c.label}</span>;
}
```

- [ ] **Step 2: Create `src/components/SessionCard.tsx`**

```tsx
import type { SessionState } from "../types";
import { formatDuration } from "../snapshot";
import { StatusChip } from "./StatusChip";

const cardClass: Record<string, string> = {
  working: "card cW", blocked: "card cB", error: "card cB",
  done: "card cD", idle: "card cI", stale: "card cI",
};

export function SessionCard(
  { session, now, onClick }: { session: SessionState; now: number; onClick?: () => void },
) {
  return (
    <div className={cardClass[session.status]} onClick={onClick} title={session.cwd}>
      <StatusChip status={session.status} />
      <div className="body">
        <div className="proj">
          {session.label}
          {session.branch && <span className="branch"> {session.branch}</span>}
        </div>
        {session.taskSummary && <div className="task">{session.taskSummary}</div>}
      </div>
      <div className="meta">{formatDuration(now - session.statusSince)}</div>
    </div>
  );
}
```

- [ ] **Step 3: Delete the old components**

```bash
git rm src/components/TrafficLight.tsx src/components/SessionRow.tsx
```

- [ ] **Step 4: Verify it compiles**

Run: `npm run build`
Expected: PASS (no references to deleted files yet remain after Task 11 wires App; if `App.tsx` still imports them, this step will fail — that's fine, App is rewired in Task 11. To keep this task green, temporarily ensure `App.tsx` does not import the deleted files: if it does, proceed to Task 11 before building.)

> Note for the executor: Tasks 8–11 form a UI cluster. If `npm run build` fails here only because `App.tsx` still imports old components, that's expected; the cluster is green after Task 11. Commit this task's files now.

- [ ] **Step 5: Commit**

```bash
git add src/components/StatusChip.tsx src/components/SessionCard.tsx
git commit -m "feat(frontend): StatusChip + SessionCard, remove traffic light"
```

---

## Task 9: CollapsedPill component

**Files:**
- Create: `src/components/CollapsedPill.tsx`

**Interfaces:**
- Consumes: `Aggregate` (Task 5), `AttentionPhase` (Task 6), `SessionState`.
- Produces: `<CollapsedPill agg={Aggregate} phase={AttentionPhase} topSession={SessionState | null} />` — renders the resting pill per spec §4.4: nub (idle), calm counts (working), red-bordered counts (needs-input-compact), expanded peek (needs-input-evident, shows `topSession`).

- [ ] **Step 1: Create `src/components/CollapsedPill.tsx`**

```tsx
import type { Aggregate, AttentionPhase, SessionState } from "../types";

export function CollapsedPill(
  { agg, phase, topSession }: { agg: Aggregate; phase: AttentionPhase; topSession: SessionState | null },
) {
  if (phase === "idle") {
    return <div className="pill nub"><span className="cnt g">✓ {agg.done || agg.total}</span></div>;
  }
  if (phase === "needs-input-evident" && topSession) {
    return (
      <div className="pill bhi">
        <span className="chip chB">◆ serve input</span>
        <div className="proj">{topSession.label}</div>
        {topSession.taskSummary && <div className="task">{topSession.taskSummary}</div>}
        <div className="rest">+{agg.working} lavora · +{agg.done} fatto ▸</div>
      </div>
    );
  }
  const blocked = phase === "needs-input-compact";
  return (
    <div className={"pill " + (blocked ? "blo" : "calm")}>
      {agg.input > 0 && <span className="cnt r">◆ {agg.input}</span>}
      {agg.working > 0 && <span className={"cnt y" + (blocked ? "" : " breathe")}>● {agg.working}</span>}
      {agg.done > 0 && <span className="cnt g">✓ {agg.done}</span>}
    </div>
  );
}
```

- [ ] **Step 2: Verify it compiles**

Run: `npx tsc --noEmit`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/components/CollapsedPill.tsx
git commit -m "feat(frontend): CollapsedPill with attention-phase states"
```

---

## Task 10: SessionDetail (expanded detail + quick actions)

**Files:**
- Create: `src/components/SessionDetail.tsx`

**Interfaces:**
- Consumes: `SessionState`, `formatDuration`.
- Produces: `<SessionDetail session pinned onMute onPinTop onArchive onClose />` — shows last message/event, branch, full cwd, time-in-state, and quick-action buttons (mute, pin to top, archive). Actions are callbacks; state lives in `App` (Task 11).

- [ ] **Step 1: Create `src/components/SessionDetail.tsx`**

```tsx
import type { SessionState } from "../types";
import { formatDuration } from "../snapshot";

export function SessionDetail({
  session, now, muted, onMute, onPinTop, onArchive, onClose,
}: {
  session: SessionState; now: number; muted: boolean;
  onMute: () => void; onPinTop: () => void; onArchive: () => void; onClose: () => void;
}) {
  return (
    <div className="detail">
      <div className="detail-head">
        <span className="proj">{session.label}{session.branch && <span className="branch"> {session.branch}</span>}</span>
        <button className="x" onClick={onClose}>✕</button>
      </div>
      {session.taskSummary && <div className="task">{session.taskSummary}</div>}
      <dl className="kv">
        <dt>stato</dt><dd>{session.status} · da {formatDuration(now - session.statusSince)}</dd>
        <dt>evento</dt><dd>{session.lastEventName}</dd>
        <dt>path</dt><dd className="mono">{session.cwd}</dd>
      </dl>
      <div className="actions">
        <button onClick={onMute}>{muted ? "🔔 riattiva" : "🔕 silenzia"}</button>
        <button onClick={onPinTop}>📌 in cima</button>
        {(session.status === "done" || session.status === "stale") && (
          <button onClick={onArchive}>🗙 archivia</button>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Verify it compiles**

Run: `npx tsc --noEmit`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/components/SessionDetail.tsx
git commit -m "feat(frontend): SessionDetail with quick actions"
```

---

## Task 11: Hooks + DrawerPanel + App wiring

**Files:**
- Create: `src/hooks/useAttention.ts`
- Create: `src/hooks/useAudioCues.ts`
- Create: `src/components/DrawerPanel.tsx`
- Modify: `src/App.tsx`

**Interfaces:**
- Consumes: everything above.
- Produces:
  - `useAttention(sessions, decayMs): AttentionPhase` — tracks `blockedSince` (set when `aggregate(sessions).input` goes 0→>0, cleared when it returns to 0) and ticks ~1s to advance evident→compact.
  - `useAudioCues(sessions, settings)` — runs `detectCues` between renders and plays sounds (skips muted sessions and when `soundEnabled` is false).
  - `<DrawerPanel collapsed expanded onMouseEnter onMouseLeave onToggle>` — renders collapsed pill or full panel; hover = peek, click = pin.

- [ ] **Step 1: Create `src/hooks/useAttention.ts`**

```ts
import { useEffect, useRef, useState } from "react";
import type { SessionState, AttentionPhase } from "../types";
import { aggregate } from "../snapshot";
import { attentionPhase } from "../attention";

export function useAttention(sessions: SessionState[], decayMs: number): AttentionPhase {
  const blockedSince = useRef<number | null>(null);
  const [, tick] = useState(0);
  const agg = aggregate(sessions);

  if (agg.input > 0 && blockedSince.current === null) blockedSince.current = Date.now();
  if (agg.input === 0) blockedSince.current = null;

  useEffect(() => {
    const t = setInterval(() => tick((n) => n + 1), 1000);
    return () => clearInterval(t);
  }, []);

  return attentionPhase(agg, blockedSince.current, Date.now(), decayMs);
}
```

- [ ] **Step 2: Create `src/hooks/useAudioCues.ts`**

```ts
import { useEffect, useRef } from "react";
import type { SessionState } from "../types";
import type { Settings } from "../settings";
import { isMuted } from "../settings";
import { detectCues } from "../attention";

const SOUND: Record<string, string> = {
  "needs-input": "/sounds/needs-input.wav",
  "done": "/sounds/done.wav",
};

export function useAudioCues(sessions: SessionState[], settings: Settings) {
  const prev = useRef<SessionState[]>([]);
  useEffect(() => {
    if (settings.soundEnabled) {
      const audible = sessions.filter((s) => !isMuted(settings, s.id));
      const prevAudible = prev.current.filter((s) => !isMuted(settings, s.id));
      for (const cue of detectCues(prevAudible, audible)) {
        new Audio(SOUND[cue]).play().catch(() => {});
      }
    }
    prev.current = sessions;
  }, [sessions, settings]);
}
```

- [ ] **Step 3: Add sound assets**

Create `public/sounds/needs-input.wav` and `public/sounds/done.wav` (short, distinct cues). For the first pass, generate two short tones:

```bash
mkdir -p public/sounds
# If `sox` is available; otherwise drop any two short .wav files with these names.
command -v sox >/dev/null && sox -n public/sounds/needs-input.wav synth 0.18 sine 880 sine 660 gain -6 || true
command -v sox >/dev/null && sox -n public/sounds/done.wav synth 0.22 sine 523 sine 784 gain -6 || true
```

If `sox` is unavailable, place any two short `.wav` files at those paths; the code only needs the files to exist.

- [ ] **Step 4: Create `src/components/DrawerPanel.tsx`**

```tsx
import type { ReactNode } from "react";

export function DrawerPanel({
  open, pill, panel, onEnter, onLeave, onToggle,
}: {
  open: boolean; pill: ReactNode; panel: ReactNode;
  onEnter: () => void; onLeave: () => void; onToggle: () => void;
}) {
  return (
    <div className="drawer" onMouseEnter={onEnter} onMouseLeave={onLeave}>
      {open ? (
        <div className="panel" onClick={(e) => e.stopPropagation()}>{panel}</div>
      ) : (
        <div onClick={onToggle}>{pill}</div>
      )}
    </div>
  );
}
```

- [ ] **Step 5: Rewrite `src/App.tsx`**

```tsx
import { useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import "./App.css";
import type { SessionState } from "./types";
import { sortSessions, aggregate } from "./snapshot";
import { loadSettings, saveSettings, isMuted, type Settings } from "./settings";
import { useAttention } from "./hooks/useAttention";
import { useAudioCues } from "./hooks/useAudioCues";
import { CollapsedPill } from "./components/CollapsedPill";
import { DrawerPanel } from "./components/DrawerPanel";
import { SessionCard } from "./components/SessionCard";
import { SessionDetail } from "./components/SessionDetail";

export default function App() {
  const [sessions, setSessions] = useState<SessionState[]>([]);
  const [settings, setSettings] = useState<Settings>(() => loadSettings());
  const [hovering, setHovering] = useState(false);
  const [pinned, setPinned] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [pinnedTop, setPinnedTop] = useState<string[]>([]);
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    const un = listen<SessionState[]>("sessions-updated", (e) => setSessions(e.payload));
    return () => { un.then((f) => f()); };
  }, []);
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, []);

  const ordered = useMemo(() => {
    const sorted = sortSessions(sessions);
    return [...sorted].sort(
      (a, b) => Number(pinnedTop.includes(b.id)) - Number(pinnedTop.includes(a.id)),
    );
  }, [sessions, pinnedTop]);

  const agg = aggregate(sessions);
  const phase = useAttention(sessions, settings.decayMs);
  useAudioCues(sessions, settings);

  const update = (s: Settings) => { setSettings(s); saveSettings(s); };
  const open = hovering || pinned;
  const selected = ordered.find((s) => s.id === selectedId) ?? null;
  const topSession = ordered.find((s) => s.status === "blocked" || s.status === "error") ?? null;

  return (
    <div className="faro-root">
      <DrawerPanel
        open={open}
        onEnter={() => setHovering(true)}
        onLeave={() => setHovering(false)}
        onToggle={() => setPinned((p) => !p)}
        pill={<CollapsedPill agg={agg} phase={phase} topSession={topSession} />}
        panel={
          selected ? (
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
                    <SessionCard key={s.id} session={s} now={now} onClick={() => setSelectedId(s.id)} />
                  ))}
            </>
          )
        }
      />
    </div>
  );
}
```

- [ ] **Step 6: Verify build + tests**

Run: `npm run build && npm test`
Expected: PASS (build clean, all unit tests green).

- [ ] **Step 7: Commit**

```bash
git add src/hooks src/components/DrawerPanel.tsx src/App.tsx public/sounds
git commit -m "feat(frontend): attention + audio hooks, drawer panel, app wiring"
```

---

## Task 12: Glass + chip styling (App.css)

**Files:**
- Modify: `src/App.css`

**Interfaces:**
- Consumes: the class names emitted by the components above (`pill`, `nub`, `calm`, `blo`, `bhi`, `card cW/cB/cD/cI`, `chip chW/chB/chD/chI`, `panel`, `phdr`, `proj`, `branch`, `task`, `meta`, `body`, `detail`, `kv`, `actions`, `drawer`, `cnt r/y/g`, `breathe`, `rest`).

This is the visual layer validated in the brainstorm (look "A + C"). Verified visually in the running app.

- [ ] **Step 1: Replace `src/App.css`** with the glass+chip styling:

```css
* { margin: 0; padding: 0; box-sizing: border-box; }
html, body, #root { background: transparent; width: 100%; height: 100%; overflow: hidden; }

.faro-root {
  font-family: -apple-system, system-ui, sans-serif; color: #e5e7eb;
  height: 100%; display: flex; justify-content: flex-end; align-items: flex-start;
}
.drawer { padding: 8px 0; }

/* glass surface shared by pill + panel */
.pill, .panel {
  backdrop-filter: blur(20px);
  background: linear-gradient(160deg, rgba(28,30,38,.86), rgba(18,19,26,.9));
  border: 1px solid rgba(255,255,255,.08); border-right: none;
  border-radius: 12px 0 0 12px; box-shadow: -6px 8px 30px rgba(0,0,0,.45);
}
.panel { width: 330px; padding: 9px; }
.phdr { display: flex; justify-content: space-between; padding: 4px 6px 10px;
  font-size: 11px; letter-spacing: .08em; text-transform: uppercase; color: rgba(255,255,255,.4); }
.empty { font-size: 12px; opacity: .4; padding: 10px; text-align: center; }

/* collapsed pill states */
.pill { display: flex; flex-direction: column; gap: 6px; align-items: flex-end; padding: 9px 11px; }
.pill.nub { opacity: .55; padding: 9px; }
.pill.blo { box-shadow: -6px 8px 30px rgba(0,0,0,.45), 0 0 0 1px rgba(239,68,68,.4); }
.pill.bhi { min-width: 190px; align-items: stretch; padding: 12px 14px;
  border-color: rgba(239,68,68,.5); box-shadow: -6px 8px 30px rgba(0,0,0,.5), 0 0 26px rgba(239,68,68,.35);
  animation: pulseRed 1.6s infinite; }
.cnt { display: flex; align-items: center; gap: 5px; font-size: 12px; font-weight: 700; }
.cnt.r { color: #fca5a5; } .cnt.y { color: #f5c518; } .cnt.g { color: #86efac; }
.breathe { animation: bd 2s infinite; }
@keyframes bd { 0%,100% { opacity: 1; } 50% { opacity: .55; } }
@keyframes pulseRed {
  0%,100% { box-shadow: -6px 8px 30px rgba(0,0,0,.5), 0 0 18px rgba(239,68,68,.25); }
  50%     { box-shadow: -6px 8px 30px rgba(0,0,0,.5), 0 0 30px rgba(239,68,68,.55); }
}
.pill .rest { font-size: 11px; color: rgba(255,255,255,.4); border-top: 1px solid rgba(255,255,255,.08); padding-top: 6px; }

/* cards */
.card { display: flex; gap: 11px; align-items: center; padding: 11px 12px;
  border-radius: 13px; margin-bottom: 7px; border: 1px solid transparent; cursor: pointer; }
.card.cW { background: linear-gradient(120deg, rgba(245,197,24,.10), rgba(245,197,24,.02)); border-color: rgba(245,197,24,.18); }
.card.cB { background: linear-gradient(120deg, rgba(239,68,68,.13), rgba(239,68,68,.03)); border-color: rgba(239,68,68,.28); }
.card.cD { background: linear-gradient(120deg, rgba(34,197,94,.10), rgba(34,197,94,.02)); border-color: rgba(34,197,94,.16); }
.card.cI { background: rgba(255,255,255,.03); border-color: rgba(255,255,255,.06); opacity: .7; }

.chip { font-size: 10px; font-weight: 700; padding: 4px 8px; border-radius: 20px; flex: none; }
.chip.chW { background: rgba(245,197,24,.18); color: #f5c518; }
.chip.chB { background: rgba(239,68,68,.2); color: #fca5a5; }
.chip.chD { background: rgba(34,197,94,.18); color: #86efac; }
.chip.chI { background: rgba(255,255,255,.08); color: #9ca3af; }

.body { flex: 1; min-width: 0; }
.proj { font-weight: 600; font-size: 13px; color: #f3f4f6; }
.branch { font-size: 11px; color: #6b7280; font-family: ui-monospace, monospace; }
.task { font-size: 12px; color: #9ca3af; margin-top: 2px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.meta { font-size: 11px; color: #6b7280; flex: none; }

/* detail */
.detail { padding: 4px; }
.detail-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; }
.detail .x { background: none; border: none; color: #6b7280; cursor: pointer; font-size: 13px; }
.kv { display: grid; grid-template-columns: auto 1fr; gap: 4px 10px; font-size: 12px; margin: 10px 0; }
.kv dt { color: #6b7280; } .kv dd { color: #d1d5db; }
.kv .mono { font-family: ui-monospace, monospace; font-size: 11px; word-break: break-all; }
.actions { display: flex; gap: 6px; flex-wrap: wrap; }
.actions button { font-size: 11px; padding: 5px 9px; border-radius: 8px;
  border: 1px solid rgba(255,255,255,.1); background: rgba(255,255,255,.05); color: #e5e7eb; cursor: pointer; }
.actions button:hover { background: rgba(255,255,255,.1); }
```

- [ ] **Step 2: Manual visual verification**

Run: `npm run tauri dev`
Then drive synthetic events through the broker:

```bash
post() { curl -s -XPOST 127.0.0.1:8765/event -d "$1" >/dev/null; }
post '{"hook_event_name":"UserPromptSubmit","session_id":"s1","cwd":"'"$PWD"'"}'
post '{"hook_event_name":"Notification","session_id":"s2","cwd":"/tmp/api-gateway","notification_type":"permission_prompt"}'
post '{"hook_event_name":"Stop","session_id":"s3","cwd":"'"$PWD"'"}'
curl -s 127.0.0.1:8765/sessions
```

Expected, by eye: a right-edge glass drawer; collapsed pill shows counts; the blocked session (`s2`) triggers the evident peek then decays to compact after ~8s; hovering the pill opens the panel showing three cards (project · branch · task · time); clicking a card opens its detail with quick actions. **Look at the window** — a blank/empty frame is a failure.

- [ ] **Step 3: Commit**

```bash
git add src/App.css
git commit -m "feat(frontend): glass + chip styling (look A+C)"
```

---

## Task 13: Right-edge overlay window

**Files:**
- Modify: `src-tauri/tauri.conf.json` (window dims)
- Modify: `src-tauri/src/lib.rs` (`position_top_right` → right-edge full-height overlay)

**Interfaces:**
- Consumes: the existing `setup` hook in `lib.rs`.
- Produces: a tall, narrow, transparent overlay anchored to the right edge; transparent regions are click-through so the drawer doesn't block the desktop.

- [ ] **Step 1: Update window config in `tauri.conf.json`**

Change the `main` window block to a tall right-edge strip:

```json
{
  "title": "faro-app",
  "width": 360,
  "height": 700,
  "transparent": true,
  "decorations": false,
  "alwaysOnTop": true,
  "shadow": false,
  "resizable": false,
  "skipTaskbar": true
}
```

- [ ] **Step 2: Replace `position_top_right` in `lib.rs`**

```rust
fn position_right_edge(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    if let Some(monitor) = window.current_monitor()? {
        let screen = monitor.size();
        let win = window.outer_size()?;
        let x = (screen.width as i32) - (win.width as i32);
        let y = ((screen.height as i32) - (win.height as i32)) / 4; // upper-ish
        window.set_position(PhysicalPosition::new(x.max(0), y.max(0)))?;
    }
    Ok(())
}
```

Update the call site in `setup` from `position_top_right(&window)?;` to `position_right_edge(&window)?;`.

- [ ] **Step 3: Make transparent regions click-through**

In `setup`, after positioning, add:

```rust
            // The drawer only occupies the right strip; let clicks pass through the
            // transparent area. Re-enabled implicitly where the webview paints opaque
            // content is not automatic — Phase 1 keeps it simple: ignore cursor events
            // globally only when collapsed is a Phase 2 refinement. For now, size the
            // window to the drawer so there is little dead space.
            let _ = window.set_ignore_cursor_events(false);
```

> Note: full per-region click-through (transparent = pass-through, drawer = interactive) is a known refinement. Phase 1 keeps the window narrow (360px) so dead space is minimal; revisit with `set_ignore_cursor_events` toggling on hover if it proves annoying.

- [ ] **Step 4: Manual verification**

Run: `npm run tauri dev`
Expected: the drawer sits flush against the right edge of the screen, full-ish height, transparent background; the same synthetic events from Task 12 render correctly.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/tauri.conf.json src-tauri/src/lib.rs
git commit -m "feat(window): right-edge overlay anchoring"
```

---

## Task 14: Final integration pass + docs

**Files:**
- Modify: `README.md` (status colours table → chip language; mention drawer/attention)
- Verify: full suite.

- [ ] **Step 1: Run the full test suite**

Run: `cd src-tauri && cargo test && cd .. && npm test && npm run build`
Expected: all PASS, build clean.

- [ ] **Step 2: Update the README "Status colours" section**

Replace the traffic-light table with the chip language (working `● working`, input `◆ input`, done `✓ done`, idle `· idle`, error `◆ error`) and add one line that Faro now collapses to a right-edge count pill that escalates on `needs-input` then decays to a persistent compact reminder.

- [ ] **Step 3: End-to-end smoke (manual)**

Run `npm run tauri dev`, fire the Task 12 synthetic events, and confirm the spec §9 success criteria by eye: 1 / 5 / 15 sessions stay readable; two sessions on the same repo with different branches are distinguishable; a new `needs-input` produces evident→compact→persistent; idle is quiet; hover opens, click pins, click-out closes.

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: update README for redesigned widget"
```

---

## Self-Review (completed by plan author)

- **Spec coverage:** §4.1 form/positioning → Tasks 11,13; §4.2 look → Tasks 8,12; §4.3 count pill → Task 9; §4.4 attention/escalation → Tasks 6,9,11; §4.5 per-session content → Tasks 1,4,5,8; §4.6 interactions (detail + quick actions) → Tasks 10,11; §5 data (branch, summary, statusSince) → Tasks 1–4; §7 settings → Task 7; audio cues → Tasks 6,11. Phase 2 items (notch, bring-terminal-to-front, intelligent summary) intentionally absent.
- **Placeholder scan:** no TBD/TODO; every code step shows code; manual-verification steps used only for visual/window/audio behavior that cannot be unit-tested in the node vitest env.
- **Type consistency:** Rust `status_since/branch/task_summary` ↔ JSON `statusSince/branch/taskSummary` ↔ TS `statusSince/branch/taskSummary`; `AttentionPhase` values identical across `attention.ts`, `CollapsedPill`, `App`; `Aggregate` shape consistent across `snapshot.ts`, `attention.ts`, components.
```
