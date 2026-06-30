# Faro — SessionCard Redesign

**Date:** 2026-06-30  
**Status:** Approved  
**Scope:** `SessionCard` layout + `CollapsedPill` overflow fix

---

## Problem

The current `SessionCard` layout is `[chip] | [body: label+branch inline] | [duration]`.  
The chip (`~65 px`) and duration (`~35 px`) flanking the body leave only `~144 px` for the session label, which is a Claude session name (often 25-40 chars). Names overflow without any truncation because `.proj` has no `overflow`/`text-overflow` CSS.

The `CollapsedPill` in `needs-input-evident` phase shows the same label in `.proj`, with the same missing truncation, causing the pill to grow horizontally.

---

## Design Decision

**Layout G** — two-row card structure. Approved by user 2026-06-30.

### SessionCard — Standard Mode (< 7 active sessions)

```
┌───────────────────────────────────────────────────────────────┐  ← card border (status-color)
│ session-label-truncated-with-ellipsis              12m        │  ← row 1: label · time
│ ● working  feat/redesign-widget-overlay            ···        │  ← row 2: chip · branch
│ fixing the CSS overflow in DrawerPanel                        │  ← row 3: taskSummary (optional)
└───────────────────────────────────────────────────────────────┘
```

**Row 1 — Label + Time**
- `label`: `font-weight: 600`, `font-size: 13px`, `color: #f1f5f9`, truncated with ellipsis, `flex: 1`, `min-width: 0`. Native `title` attribute set to full label value for tooltip.
- `time`: `font-size: 11px`, `color: #475569`, `flex: none`, `font-variant-numeric: tabular-nums`, right-aligned.

**Row 2 — Chip + Branch**
- `chip`: unchanged — same text labels, same colors, same border-radius, same font-size (10px 700). No changes to `StatusChip.tsx`.
- `branch`: `font-family: ui-monospace, monospace`, `font-size: 10.5px`, `color: #64748b`, truncated with ellipsis, `flex: 1`, `min-width: 0`.

**Row 3 — Task Summary** (conditional, only when `taskSummary` is set)
- `font-size: 11.5px`, `color: #94a3b8`, single-line ellipsis. Unchanged from current.
- `margin-top: 4px`.

**Card padding:** `10px 12px` (unchanged from current).  
**Card spacing:** `margin-bottom: 6px` (unchanged).  
**Card background gradient and border-color:** unchanged per status.

### SessionCard — Compact Mode (≥ 7 active sessions)

Same two-row structure (row 1 + row 2) but:
- Padding: `7px 10px`
- Label `font-size: 12px`
- Chip padding: `2px 6px`, font-size `9.5px`
- Branch `font-size: 10px`
- Row 3 (taskSummary) **hidden**
- Row gap: `3px`

Compact mode activates automatically when `sessions.length >= 7`. No user toggle.

### CollapsedPill — overflow fix only

The `bhi` (needs-input-evident) state shows `topSession.label` in `.proj`. Add truncation:

```css
.pill.bhi .proj {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 100%;
}
```

No structural changes to CollapsedPill. All other pill states are unaffected.

---

## Files Changed

| File | Change |
|------|--------|
| `src/App.css` | Rewrite `.card` block + add `.card.compact` variant + add `.pill.bhi .proj` truncation |
| `src/components/SessionCard.tsx` | New JSX structure (two-row layout), compact prop from parent |
| `src/App.tsx` | Pass `compact={ordered.length >= 7}` to each `SessionCard` |
| `src/components/StatusChip.tsx` | **No change** |
| `src/components/CollapsedPill.tsx` | **No change** (CSS-only fix in App.css) |

---

## CSS Diff (key changes)

**Remove from `.card` block:**
```css
/* current — body gets squeezed between chip and meta */
.card { display: flex; gap: 11px; align-items: center; ... }
.proj { font-weight: 600; font-size: 13px; color: #f3f4f6; }  /* no overflow! */
.branch { font-size: 11px; color: #6b7280; font-family: ui-monospace, monospace; }
```

**Add:**
```css
/* card inner layout — block, no more flex row */
.card { padding: 10px 12px; border-radius: 13px; margin-bottom: 6px;
  border: 1px solid transparent; cursor: pointer;
  transition: filter 120ms ease; }
.card:hover { filter: brightness(1.06); }

/* row 1 */
.card-r1 { display: flex; align-items: baseline; gap: 8px; margin-bottom: 5px; }
.proj { font-weight: 600; font-size: 13px; color: #f1f5f9;
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  flex: 1; min-width: 0; }
.meta { font-size: 11px; color: #475569; flex: none; font-variant-numeric: tabular-nums; }

/* row 2 */
.card-r2 { display: flex; align-items: center; gap: 6px; }
.branch { font-size: 10.5px; color: #64748b; font-family: ui-monospace, monospace;
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  flex: 1; min-width: 0; }

/* row 3 */
.task { font-size: 11.5px; color: #94a3b8; margin-top: 4px;
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

/* compact variant */
.card.compact { padding: 7px 10px; }
.card.compact .card-r1 { margin-bottom: 3px; gap: 6px; }
.card.compact .proj { font-size: 12px; }
.card.compact .meta { font-size: 10.5px; }
.card.compact .chip { font-size: 9.5px; padding: 2px 6px; }
.card.compact .branch { font-size: 10px; }
.card.compact .task { display: none; }

/* pill fix */
.pill.bhi .proj { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 100%; }
```

---

## JSX Diff (SessionCard)

```tsx
// Before
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

// After
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
```

`SessionCard` gains a `compact?: boolean` prop. `App.tsx` passes `compact={ordered.length >= 7}`.

---

## What Does NOT Change

- `StatusChip.tsx` — zero modifications
- `CollapsedPill.tsx` — zero modifications (fix is CSS-only)
- Card background gradients and border colors per status
- Glass morphism panel style (backdrop-filter, border, box-shadow)
- Collapsed pill structure and animations
- `SessionDetail` component
- All Rust/Tauri backend code

---

## Out of Scope

- Hover-to-expand label (tooltip nativo via `title` è sufficiente)
- Click-to-copy branch name
- Reordering via drag
- Any change to the reporting/hooks infrastructure
