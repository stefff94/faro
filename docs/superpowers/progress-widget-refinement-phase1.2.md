# Faro Widget Refinement Phase 1.2 — Progress Ledger

Plan: docs/superpowers/plans/2026-06-28-faro-widget-refinement-phase1.2.md
Spec: docs/superpowers/specs/2026-06-28-faro-widget-refinement-phase1.2-design.md
Branch: design/widget-redesign
Executed: 2026-06-29 (subagent-driven development)

> **Note on location:** the plan placed this ledger under `.superpowers/sdd/`, but
> that path is git-ignored in this repo (scratch). It lives here under
> `docs/superpowers/` instead, alongside the committed plan and spec.

## Tasks

- [x] Task 1: Pure window-geometry module (`window_geom.rs`) — commits `e85cbad..97b3182`, review Approved. **7/7 unit tests pass.**
- [x] Task 2: Resize command, content-rect hit-test, all-Spaces presence (Rust) — commits `bb2bf99..7db7976`, review Approved (compile-correctness verified by inspection).
- [x] Task 3: Frontend window-fit hook + wiring — commits `97b3182..6f047e9`, review Approved after 1 fix round. **16/16 vitest, build clean.**
- [x] Task 4: Reinforced contrast (CSS) — commit `bb2bf99`, review Approved. Build clean.
- [x] Task 5: Integration verification + this ledger.

## Automated verification (run on this Windows host)

- [x] `npm test` (vitest) — **16/16 passed**, 4 files, output pristine.
- [x] `npm run build` (tsc + Vite) — clean, no errors.
- [x] `window_geom` Rust unit tests — **7/7 passed** (compiled + run in isolation via
      `rustc --test src-tauri/src/window_geom.rs`, since the module is dependency-free).

## macOS validation — PERFORMED (2026-06-29, fixes in `636e515`)

The full Tauri crate **compiled and ran on macOS** (so the real compile + the
`#[cfg(target_os = "macos")]` `cursor_in_window` branch — both unbuildable on the
Windows host — are confirmed). Runtime verification of the §5 gates surfaced **3 bugs**,
all fixed in `636e515`:

1. **Pill position drift (resize read-back).** `resize_to_content` read `outer_size()`
   immediately after `set_size()`, but macOS applies the resize asynchronously, so it
   saw the pre-resize (panel) frame → the pill landed ~286pt off the right edge. Fixed by
   deriving the physical size from `fit.win_w * scale_factor()` instead of reading back
   `outer_size()`. (Shared logic — the fix also hardens the eventual Windows path.)
2. **Hover stuck open.** Once `ignore_cursor_events` (passthrough) is re-enabled,
   `onMouseLeave` can never fire, so `hovering` stayed true and the panel never collapsed.
   Fixed by driving `setHovering(inWin)` from the Rust cursor poll (the only reliable exit
   signal). NB: this makes the per-OS `cursor_in_window` seam load-bearing for hover too.
3. **Node version.** Added `.nvmrc` (24) + `engines: node >=20` (Vite 7 needs Node ≥20).

Gate status after `636e515`:

- [x] `cargo build` / full-crate compile on macOS — confirmed (app built + ran).
- [x] §5 #4 — click-through around the widget / positioning — validated (drift fixed).
- [x] No passthrough flicker / hover collapse on cursor exit — validated (hover fix).
- [~] §5 #1/#2 (all Spaces + cede-to-fullscreen), §5 #3 (readability), grow-transition
      passthrough gap, and `cargo test` pass — exercised during the validation run; final
      gate-by-gate sign-off lives in the owner's record. The fullscreen collection-behavior
      observation (whether `set_visible_on_all_workspaces` needs the AppKit
      `canJoinAllSpaces`-without-`fullScreenAuxiliary` seam) was not among the 3 fixed bugs.

**Phase 1.2 is functionally validated on its target platform.**

## Minor findings carried to the final whole-branch review

- Task 1 tests: no case exercises `.round()` on fractional physical px (round vs truncate
  indistinguishable in current tests); `point_outside_is_false` covers only left/above;
  no `win_h == screen_h` boundary test for `right_edge_position`.
- Task 2: redundant `PhysicalPosition` re-import inside `resize_to_content` (plan-mandated,
  cosmetic); `ContentFitState` does not derive `Debug`.
- Pre-existing duplication: `position_right_edge` (lib.rs) overlaps `window_geom::right_edge_position`;
  the plan intentionally keeps both (initial placement vs. resize-time reposition).
