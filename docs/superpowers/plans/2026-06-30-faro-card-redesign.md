# Faro SessionCard Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign `SessionCard` to a two-row layout where the session name gets full-width space on row 1, fixing the label overflow bug; add compact mode for ≥7 sessions; fix `CollapsedPill` label overflow via a CSS-only rule.

**Architecture:** CSS layout change (remove `display:flex` from `.card`, add `.card-r1`/`.card-r2` row containers) + minimal JSX restructure in `SessionCard.tsx` + one prop addition in `App.tsx`. No new files, no state changes, no Rust changes.

**Tech Stack:** React 18, TypeScript, CSS (no framework), Vite 5, Tauri 2.

## Global Constraints

- Panel width 330px; inner card content ~276px
- StatusChip colors, labels, and font-size (10px bold) — **zero changes**
- Card gradient backgrounds and border-colors per status — **zero changes**
- Glass-morphism panel (backdrop-filter, box-shadow, border) — **zero changes**
- Compact mode threshold: `ordered.length >= 7` — not configurable, not a user setting
- `CollapsedPill.tsx` — **zero JSX changes**; pill fix is CSS-only in `App.css`
- `StatusChip.tsx` — **zero modifications**

---

## File Structure

| File | Change |
|------|--------|
| `src/App.css` | Rewrite `.card` block; add `.card-r1`, `.card-r2`, `.card.compact` rules; fix `.pill.bhi .proj`; remove dead `.body` rule; update `.proj`, `.branch`, `.task`, `.meta` |
| `src/components/SessionCard.tsx` | Add `compact?: boolean` prop; two-row JSX structure |
| `src/App.tsx` | Pass `compact={ordered.length >= 7}` to each `SessionCard` |

Not modified: `src/components/StatusChip.tsx`, `src/components/CollapsedPill.tsx`, all Rust/Tauri code.

---

### Task 1: CSS + JSX — two-row card layout

CSS and JSX must be committed atomically: removing `display:flex` from `.card` breaks the old JSX; the new class names (`card-r1`, `card-r2`) only appear after the JSX emits them.

**Files:**
- Modify: `src/App.css` (lines 43–61)
- Modify: `src/components/SessionCard.tsx`

**Interfaces:**
- Produces: `SessionCard` with optional `compact?: boolean` prop (consumed by Task 2)

- [ ] **Step 1: Rewrite card rules in `src/App.css`**

Replace lines 43–61 (the `.card` block through `.meta`) with the block below. Keep everything before line 43 and after line 61 unchanged.

```css
/* cards */
.card { padding: 10px 12px; border-radius: 13px; margin-bottom: 6px;
  border: 1px solid transparent; cursor: pointer; transition: filter 120ms ease; }
.card:hover { filter: brightness(1.06); }
.card.cW { background: linear-gradient(120deg, rgba(245,197,24,.10), rgba(245,197,24,.02)); border-color: rgba(245,197,24,.18); }
.card.cB { background: linear-gradient(120deg, rgba(239,68,68,.13), rgba(239,68,68,.03)); border-color: rgba(239,68,68,.28); }
.card.cD { background: linear-gradient(120deg, rgba(34,197,94,.10), rgba(34,197,94,.02)); border-color: rgba(34,197,94,.16); }
.card.cI { background: rgba(255,255,255,.03); border-color: rgba(255,255,255,.06); opacity: .7; }

.card-r1 { display: flex; align-items: baseline; gap: 8px; margin-bottom: 5px; }
.proj { font-weight: 600; font-size: 13px; color: #f1f5f9;
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  flex: 1; min-width: 0; }
.meta { font-size: 11px; color: #475569; flex: none; font-variant-numeric: tabular-nums; }

.card-r2 { display: flex; align-items: center; gap: 6px; }
.branch { font-size: 10.5px; color: #64748b; font-family: ui-monospace, monospace;
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  flex: 1; min-width: 0; }

.task { font-size: 11.5px; color: #94a3b8; margin-top: 4px;
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

.card.compact { padding: 7px 10px; }
.card.compact .card-r1 { margin-bottom: 3px; gap: 6px; }
.card.compact .proj { font-size: 12px; }
.card.compact .meta { font-size: 10.5px; }
.card.compact .chip { font-size: 9.5px; padding: 2px 6px; }
.card.compact .branch { font-size: 10px; }
.card.compact .task { display: none; }

.pill.bhi .proj { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 100%; }
```

The old `.body { flex: 1; min-width: 0; }` rule is intentionally removed — the `.body` wrapper div is gone from the JSX in the next step.

- [ ] **Step 2: Replace `src/components/SessionCard.tsx`**

```tsx
import type { SessionState } from "../types";
import { formatDuration } from "../snapshot";
import { StatusChip } from "./StatusChip";

const cardClass: Record<string, string> = {
  working: "card cW", blocked: "card cB", error: "card cB",
  done: "card cD", idle: "card cI", stale: "card cI",
};

export function SessionCard(
  { session, now, onClick, compact }: {
    session: SessionState; now: number; onClick?: () => void; compact?: boolean;
  },
) {
  return (
    <div
      className={`${cardClass[session.status]}${compact ? " compact" : ""}`}
      onClick={onClick}
      title={session.cwd}
    >
      <div className="card-r1">
        <div className="proj" title={session.label}>{session.label}</div>
        <div className="meta">{formatDuration(now - session.statusSince)}</div>
      </div>
      <div className="card-r2">
        <StatusChip status={session.status} />
        {session.branch && <div className="branch">{session.branch}</div>}
      </div>
      {session.taskSummary && <div className="task">{session.taskSummary}</div>}
    </div>
  );
}
```

- [ ] **Step 3: TypeScript compile check**

```powershell
npx tsc --noEmit
```

Expected: zero errors. If errors appear, fix before committing.

- [ ] **Step 4: Commit**

```bash
git add src/App.css src/components/SessionCard.tsx
git commit -m "feat(ui): two-row SessionCard — label gets full width, ellipsis on overflow"
```

---

### Task 2: Wire compact mode in `src/App.tsx`

**Files:**
- Modify: `src/App.tsx:110`

**Interfaces:**
- Consumes: `compact?: boolean` prop on `SessionCard` (produced by Task 1)

- [ ] **Step 1: Add `compact` prop to the SessionCard render call**

In [src/App.tsx](src/App.tsx) at line 110, change:

```tsx
<SessionCard key={s.id} session={s} now={now} onClick={() => setSelectedId(s.id)} />
```

To:

```tsx
<SessionCard key={s.id} session={s} now={now} onClick={() => setSelectedId(s.id)} compact={ordered.length >= 7} />
```

`ordered` is already in scope (defined at line 61).

- [ ] **Step 2: TypeScript compile check**

```powershell
npx tsc --noEmit
```

Expected: zero errors.

- [ ] **Step 3: Commit**

```bash
git add src/App.tsx
git commit -m "feat(ui): activate compact SessionCard when ≥7 sessions"
```

---

### Task 3: Visual verification

No permanent code changes. Add mock sessions for browser-based verification, then remove them.

**Files:** `src/App.tsx` (temporary, reverted at end of task)

- [ ] **Step 1: Add mock sessions to `src/App.tsx` for browser testing**

In [src/App.tsx](src/App.tsx) at line 17, temporarily change the `useState` initial value:

```tsx
const [sessions, setSessions] = useState<SessionState[]>([
  { id: "1", source: "mock", sessionId: "s1",
    label: "my-very-long-session-name-for-testing-overflow", status: "blocked",
    cwd: "/dev/projects/faro", branch: "feat/redesign-widget-overlay-phase-2",
    taskSummary: "fixing the CSS overflow in DrawerPanel",
    lastEventName: "tool_use", lastUpdate: Date.now(), statusSince: Date.now() - 720000 },
  { id: "2", source: "mock", sessionId: "s2",
    label: "faro-backend-integration", status: "working",
    cwd: "/dev/projects/faro", branch: "main",
    taskSummary: "adding Rust command resize_to_content",
    lastEventName: "tool_use", lastUpdate: Date.now(), statusSince: Date.now() - 240000 },
  { id: "3", source: "mock", sessionId: "s3",
    label: "askme-lascaux", status: "done",
    cwd: "/dev/projects/askme", branch: "fix/hikari-pool-exhaustion",
    lastEventName: "stop", lastUpdate: Date.now(), statusSince: Date.now() - 3780000 },
  { id: "4", source: "mock", sessionId: "s4",
    label: "docs-update", status: "idle",
    cwd: "/dev/projects/docs", branch: "main",
    lastEventName: "idle", lastUpdate: Date.now(), statusSince: Date.now() - 7200000 },
]);
```

- [ ] **Step 2: Start the Vite dev server**

```powershell
npm run dev
```

Open `http://localhost:5173` in a browser (hover the panel to expand it).

- [ ] **Step 3: Verify standard mode (4 sessions)**

Checks in the browser:
- Row 1: session label truncates with `…` when it overflows — hover shows full name in the browser native `title` tooltip
- Row 1: duration appears right-aligned
- Row 2: StatusChip appears left of branch text; branch truncates if too long
- Row 3: `taskSummary` row appears for sessions that have it; sessions without it show no row 3
- Card gradient colors: yellow-tint=working, red-tint=blocked, green-tint=done, dim=idle

- [ ] **Step 4: Extend mock to 7 sessions for compact mode test**

Append these three entries to the mock array in [src/App.tsx](src/App.tsx):

```tsx
{ id: "5", source: "mock", sessionId: "s5",
  label: "ticket-service-refactor", status: "blocked",
  cwd: "/dev/projects/askme", branch: "feat/ticket-v2",
  taskSummary: "updating DB schema",
  lastEventName: "tool_use", lastUpdate: Date.now(), statusSince: Date.now() - 1980000 },
{ id: "6", source: "mock", sessionId: "s6",
  label: "camel-kafka-pipeline", status: "working",
  cwd: "/dev/projects/integrations", branch: "feat/kafka-kraft-migrate",
  lastEventName: "tool_use", lastUpdate: Date.now(), statusSince: Date.now() - 480000 },
{ id: "7", source: "mock", sessionId: "s7",
  label: "aiss-research-notes", status: "idle",
  cwd: "/dev/projects/aiss", branch: "main",
  taskSummary: "literature review notes",
  lastEventName: "idle", lastUpdate: Date.now(), statusSince: Date.now() - 18000000 },
```

- [ ] **Step 5: Verify compact mode (7 sessions)**

Save and check in browser (hot-reload updates automatically):
- Cards use reduced padding — denser appearance
- Task summary rows are hidden across all 7 cards
- All 7 cards visible without scroll in the panel
- Chip size is visually smaller (9.5px, 2px 6px padding)

- [ ] **Step 6: Revert mock sessions**

Restore [src/App.tsx](src/App.tsx) line 17 to:

```tsx
const [sessions, setSessions] = useState<SessionState[]>([]);
```

- [ ] **Step 7: TypeScript compile check after revert**

```powershell
npx tsc --noEmit
```

Expected: zero errors.

- [ ] **Step 8: Commit revert**

```bash
git add src/App.tsx
git commit -m "chore: remove mock sessions used for visual verification"
```
