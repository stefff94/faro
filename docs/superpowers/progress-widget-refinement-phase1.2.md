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

## Deferred to macOS (could not run on Windows)

The plan targets **macOS**; this machine is Windows. The following are deferred:

- [ ] `cargo build` / `cargo test` of the full Tauri crate. The Windows GNU toolchain
      cannot generate `windows-sys` import libraries — rustup's bundled mingw lacks the
      `as` assembler, and installing a full MinGW-w64 was declined. Task 2's Rust was
      instead verified by careful compile-correctness inspection in review (types,
      imports, borrows, `cfg` branches, serde mapping, `State`/`manage` ordering all
      confirmed). The real compile + the `#[cfg(target_os = "macos")]` `cursor_in_window`
      branch will be validated on macOS.
- [ ] §5 #4 — clicks pass through to apps around the widget; only the visible
      pixel-footprint interacts with Faro. (Requires the macOS CoreGraphics cursor path.)
- [ ] §5 #1/#2 — visible on all Spaces; yields to (disappears in) fullscreen apps.
- [ ] §5 #3 — text/pill remain legible over busy light and dark backgrounds.
- [ ] No passthrough flicker during hover/resize; window resizes cleanly.

## Runtime notes

- Task 2 Step 6 (collection-behavior gate): `set_visible_on_all_workspaces(true)` is set
  WITHOUT requesting `fullScreenAuxiliary`, so the widget should yield to fullscreen apps.
  If at macOS runtime it instead shows in fullscreen, a `#[cfg(target_os = "macos")]`
  AppKit seam is needed to set fine-grained collection behavior (`canJoinAllSpaces`
  without `fullScreenAuxiliary`). Observed behavior: **to be recorded on macOS.**

## Minor findings carried to the final whole-branch review

- Task 1 tests: no case exercises `.round()` on fractional physical px (round vs truncate
  indistinguishable in current tests); `point_outside_is_false` covers only left/above;
  no `win_h == screen_h` boundary test for `right_edge_position`.
- Task 2: redundant `PhysicalPosition` re-import inside `resize_to_content` (plan-mandated,
  cosmetic); `ContentFitState` does not derive `Debug`.
- Pre-existing duplication: `position_right_edge` (lib.rs) overlaps `window_geom::right_edge_position`;
  the plan intentionally keeps both (initial placement vs. resize-time reposition).
