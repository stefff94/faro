# Faro Widget Refinement (Fase 1.2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminare i difetti di comportamento-finestra dell'overlay Faro: hit-area invisibile gigante, presenza su un solo Space, sparizione, scarsa leggibilità.

**Architecture:** La finestra OS viene ridimensionata dinamicamente per abbracciare il widget dipinto (difesa primaria contro l'hit-area invisibile); l'hit-test del cursore avviene contro il *content-rect* visibile riportato dal frontend (cintura di sicurezza). La logica geometrica pura vive in un modulo Rust testabile (`window_geom.rs`); il frontend misura il contenuto con un `ResizeObserver` e invoca un comando Rust di resize+riposizionamento. Presenza su tutti gli Spaces via `set_visible_on_all_workspaces(true)`. Leggibilità via CSS.

**Tech Stack:** Tauri 2.x (Rust core), React/TS + Vite (frontend), vitest (test TS, env node), `cargo test` (test Rust), macOS CoreGraphics per la posizione globale del cursore (già presente).

## Global Constraints

- Target piattaforma: **macOS** (la logica OS-specifica resta dietro `#[cfg(target_os = "macos")]`, come `cursor_pos_physical`).
- Il broker/finestra non deve mai introdurre operazioni bloccanti sul path eventi.
- Presenza voluta: **tutti gli Spaces, MA non in fullscreen** → `canJoinAllSpaces`, niente `fullScreenAuxiliary`.
- Tutte le dimensioni/posizioni passate da TS a Rust sono **logiche** (CSS px); la conversione a fisico avviene lato Rust via scale factor.
- La costante `FIT_MARGIN.top` (TS) e `.drawer { top: ... }` (CSS) **devono coincidere** (valore: `30`).
- Convenzione hit-test rettangolo: left/top inclusivi, right/bottom esclusivi (coerente con `cursor_in_window` attuale).
- Niente nuove dipendenze.

---

## File Structure

- **Create** `src-tauri/src/window_geom.rs` — funzioni geometriche pure (posizione bordo-destro, content-rect fisico, point-in-rect) + unit test. Nessuna dipendenza Tauri/OS.
- **Modify** `src-tauri/src/lib.rs` — registra il modulo; aggiunge stato condiviso del content-fit; comando `resize_to_content`; riscrive `cursor_in_window` per usare il content-rect; chiama `set_visible_on_all_workspaces(true)` in setup.
- **Modify** `src-tauri/tauri.conf.json` — dimensione iniziale finestra = nub piccolo.
- **Create** `src/hooks/useWindowFit.ts` — `computeFit` (puro) + hook `useWindowFit` con `ResizeObserver` che invoca `resize_to_content`.
- **Create** `src/hooks/useWindowFit.test.ts` — test vitest di `computeFit`.
- **Modify** `src/components/DrawerPanel.tsx` — accetta `rootRef` e lo attacca al `.drawer`.
- **Modify** `src/App.tsx` — crea il ref, usa `useWindowFit`, lo passa a `DrawerPanel`.
- **Modify** `src/App.css` — `.drawer` absolute top-right, contrasto/halo/ombra rinforzati, opacità nub alzata.
- **Modify** `.superpowers/sdd/` o ledger di progresso (Task finale).

---

## Task 1: Modulo geometria pura (Rust)

**Files:**
- Create: `src-tauri/src/window_geom.rs`
- Modify: `src-tauri/src/lib.rs:1-7` (dichiarazione moduli)
- Test: in-file `#[cfg(test)]` dentro `window_geom.rs`

**Interfaces:**
- Produces:
  - `pub struct RectPx { pub left: i32, pub top: i32, pub right: i32, pub bottom: i32 }`
  - `pub struct ContentFit { pub x: f64, pub y: f64, pub w: f64, pub h: f64 }` (offset+size logici del contenuto nella finestra)
  - `pub fn content_rect_physical(outer_x: i32, outer_y: i32, fit: ContentFit, scale: f64) -> RectPx`
  - `pub fn point_in_rect(px: i32, py: i32, r: RectPx) -> bool`
  - `pub fn right_edge_position(screen_w: i32, screen_h: i32, win_w: i32, win_h: i32) -> (i32, i32)`

- [ ] **Step 1: Scrivi il modulo con i test (test prima della logica nello stesso file)**

Create `src-tauri/src/window_geom.rs`:

```rust
//! Pure window-geometry helpers (no Tauri/OS deps) so they can be unit-tested.

/// A rectangle in physical pixels, top-left origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectPx {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Logical content offset+size within the window (CSS px), reported by the frontend.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContentFit {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Physical rectangle of the painted content given the window's outer position,
/// the logical content fit, and the display scale factor.
pub fn content_rect_physical(outer_x: i32, outer_y: i32, fit: ContentFit, scale: f64) -> RectPx {
    let left = outer_x + (fit.x * scale).round() as i32;
    let top = outer_y + (fit.y * scale).round() as i32;
    RectPx {
        left,
        top,
        right: left + (fit.w * scale).round() as i32,
        bottom: top + (fit.h * scale).round() as i32,
    }
}

/// True if the physical point lies inside the rectangle (left/top inclusive,
/// right/bottom exclusive — matches the existing cursor_in_window convention).
pub fn point_in_rect(px: i32, py: i32, r: RectPx) -> bool {
    px >= r.left && px < r.right && py >= r.top && py < r.bottom
}

/// Right-edge anchored window position (physical px): flush to the right of the
/// screen, vertically in the upper quarter. All params are physical px.
pub fn right_edge_position(screen_w: i32, screen_h: i32, win_w: i32, win_h: i32) -> (i32, i32) {
    let x = (screen_w - win_w).max(0);
    let y = ((screen_h - win_h) / 4).max(0);
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_rect_at_scale_1() {
        let fit = ContentFit { x: 40.0, y: 30.0, w: 100.0, h: 50.0 };
        let r = content_rect_physical(1000, 200, fit, 1.0);
        assert_eq!(r, RectPx { left: 1040, top: 230, right: 1140, bottom: 280 });
    }

    #[test]
    fn content_rect_at_scale_2() {
        let fit = ContentFit { x: 40.0, y: 30.0, w: 100.0, h: 50.0 };
        let r = content_rect_physical(1000, 200, fit, 2.0);
        assert_eq!(r, RectPx { left: 1080, top: 260, right: 1280, bottom: 360 });
    }

    #[test]
    fn point_inside_is_true() {
        let r = RectPx { left: 10, top: 10, right: 20, bottom: 20 };
        assert!(point_in_rect(10, 10, r)); // left/top inclusive
        assert!(point_in_rect(19, 19, r));
    }

    #[test]
    fn point_on_right_or_bottom_edge_is_false() {
        let r = RectPx { left: 10, top: 10, right: 20, bottom: 20 };
        assert!(!point_in_rect(20, 15, r)); // right exclusive
        assert!(!point_in_rect(15, 20, r)); // bottom exclusive
    }

    #[test]
    fn point_outside_is_false() {
        let r = RectPx { left: 10, top: 10, right: 20, bottom: 20 };
        assert!(!point_in_rect(5, 15, r));
        assert!(!point_in_rect(15, 5, r));
    }

    #[test]
    fn right_edge_flush_and_upper_quarter() {
        let (x, y) = right_edge_position(2000, 1200, 200, 100);
        assert_eq!(x, 1800); // 2000 - 200
        assert_eq!(y, 275); // (1200 - 100) / 4
    }

    #[test]
    fn right_edge_clamps_oversized_window() {
        let (x, y) = right_edge_position(800, 600, 1000, 800);
        assert_eq!(x, 0);
        assert_eq!(y, 0);
    }
}
```

- [ ] **Step 2: Registra il modulo in lib.rs**

In `src-tauri/src/lib.rs`, dopo la riga `pub mod transcript;` (riga 7), aggiungi:

```rust
pub mod window_geom;
```

- [ ] **Step 3: Esegui i test e verifica che passino**

Run: `cargo test --manifest-path src-tauri/Cargo.toml window_geom`
Expected: PASS (7 test del modulo `window_geom`).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/window_geom.rs src-tauri/src/lib.rs
git commit -m "feat(window): pure window-geometry helpers"
```

---

## Task 2: Resize command, content-rect hit-test, presenza su tutti gli Spaces (Rust)

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/tauri.conf.json:16-17`

**Interfaces:**
- Consumes: `window_geom::{ContentFit, content_rect_physical, point_in_rect, right_edge_position}` (Task 1).
- Produces:
  - Comando Tauri `resize_to_content` con payload JS `{ fit: { winW, winH, contentX, contentY, contentW, contentH } }` (tutti `f64` logici).
  - `cursor_in_window` ora restituisce `true` solo quando il cursore è dentro il content-rect riportato.

- [ ] **Step 1: Riduci la dimensione iniziale della finestra al nub**

In `src-tauri/tauri.conf.json`, sostituisci le righe:

```json
        "width": 360,
        "height": 700,
```

con:

```json
        "width": 160,
        "height": 90,
```

- [ ] **Step 2: Aggiungi lo stato condiviso del content-fit e il comando resize_to_content**

In `src-tauri/src/lib.rs`, dopo la funzione `now_ms()` (dopo la riga 33), aggiungi:

```rust
/// Last content-fit reported by the frontend, used for cursor hit-testing.
struct ContentFitState(Mutex<crate::window_geom::ContentFit>);

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct FitArgs {
    win_w: f64,
    win_h: f64,
    content_x: f64,
    content_y: f64,
    content_w: f64,
    content_h: f64,
}

/// Resize the window to hug the painted widget and re-anchor it to the right edge,
/// then remember the content rect so cursor hit-testing targets only painted pixels.
#[tauri::command]
fn resize_to_content(
    window: tauri::WebviewWindow,
    fit_state: tauri::State<ContentFitState>,
    fit: FitArgs,
) -> tauri::Result<()> {
    use tauri::{LogicalSize, PhysicalPosition};

    window.set_size(LogicalSize::new(fit.win_w, fit.win_h))?;
    *fit_state.0.lock().unwrap() = crate::window_geom::ContentFit {
        x: fit.content_x,
        y: fit.content_y,
        w: fit.content_w,
        h: fit.content_h,
    };

    if let Some(monitor) = window.current_monitor()? {
        let screen = monitor.size();
        let win = window.outer_size()?;
        let (x, y) = crate::window_geom::right_edge_position(
            screen.width as i32,
            screen.height as i32,
            win.width as i32,
            win.height as i32,
        );
        window.set_position(PhysicalPosition::new(x, y))?;
    }
    Ok(())
}
```

- [ ] **Step 3: Riscrivi cursor_in_window per usare il content-rect**

In `src-tauri/src/lib.rs`, sostituisci l'INTERO comando `cursor_in_window` (righe 72-94 attuali) con:

```rust
#[tauri::command]
fn cursor_in_window(window: tauri::WebviewWindow, fit_state: tauri::State<ContentFitState>) -> bool {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (&window, &fit_state);
        return false;
    }

    #[cfg(target_os = "macos")]
    {
        let scale = window.scale_factor().unwrap_or(1.0);
        let Some((cx, cy)) = cursor_pos_physical(scale) else {
            return false;
        };
        let Ok(pos) = window.outer_position() else {
            return false;
        };
        let fit = *fit_state.0.lock().unwrap();
        let rect = crate::window_geom::content_rect_physical(pos.x, pos.y, fit, scale);
        crate::window_geom::point_in_rect(cx, cy, rect)
    }
}
```

- [ ] **Step 4: Registra il comando, lo stato e la presenza su tutti gli Spaces**

In `src-tauri/src/lib.rs`, sostituisci la riga `invoke_handler`:

```rust
        .invoke_handler(tauri::generate_handler![greet, cursor_in_window, set_cursor_passthrough])
```

con:

```rust
        .invoke_handler(tauri::generate_handler![
            greet,
            cursor_in_window,
            set_cursor_passthrough,
            resize_to_content
        ])
```

Poi, dentro la chiusura `.setup(|app| {`, subito dopo `let window = app.get_webview_window("main").unwrap();` (riga 107), aggiungi:

```rust
            app.manage(ContentFitState(Mutex::new(
                crate::window_geom::ContentFit { x: 0.0, y: 0.0, w: 0.0, h: 0.0 },
            )));
            // Show on every Space/desktop. We intentionally do NOT request
            // fullScreenAuxiliary: the widget yields to fullscreen apps.
            let _ = window.set_visible_on_all_workspaces(true);
```

> Nota: `app.manage` richiede `tauri::Manager`, già importato a riga 12. Lo stato di default `{0,0,0,0}` produce un content-rect vuoto, quindi `cursor_in_window` resta `false` finché il frontend non riporta il primo fit (al mount).

- [ ] **Step 5: Compila e verifica**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: compila senza errori. (I test esistenti restano verdi: `cargo test --manifest-path src-tauri/Cargo.toml`.)

- [ ] **Step 6: ⚠️ Verifica a runtime del collection-behavior (gate del task)**

Avvia l'app (`npm run tauri dev` da `/Users/stefano/Progetti/claude/faro`), poi:
- Cambia Space/desktop → il widget deve restare visibile su tutti gli Spaces.
- Entra in un'app in fullscreen → il widget deve sparire (comportamento voluto), e riapparire all'uscita.

Se `set_visible_on_all_workspaces(true)` rende il widget visibile ANCHE in fullscreen (non voluto), nota il comportamento e annota in un commento `// TODO runtime`: servirà un seam AppKit `#[cfg(target_os = "macos")]` per impostare i collection-behavior fini (`canJoinAllSpaces` senza `fullScreenAuxiliary`). Non bloccare il task su questo: registra l'esito osservato.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/tauri.conf.json
git commit -m "feat(window): content-fit resize, content-rect hit-test, all-spaces presence"
```

---

## Task 3: Hook di window-fit nel frontend + wiring

**Files:**
- Create: `src/hooks/useWindowFit.ts`
- Create: `src/hooks/useWindowFit.test.ts`
- Modify: `src/components/DrawerPanel.tsx`
- Modify: `src/App.tsx:71-110` (il blocco `return`) e gli import/ref

**Interfaces:**
- Consumes: comando Tauri `resize_to_content` con payload `{ fit: { winW, winH, contentX, contentY, contentW, contentH } }` (Task 2).
- Produces:
  - `export const FIT_MARGIN = { left: 40, top: 30, bottom: 40 }`
  - `export function computeFit(contentW: number, contentH: number): { winW, winH, contentX, contentY, contentW, contentH }`
  - `export function useWindowFit(ref: React.RefObject<HTMLElement | null>): void`
  - `DrawerPanel` accetta una prop opzionale `rootRef?: React.RefObject<HTMLDivElement | null>`.

- [ ] **Step 1: Scrivi il test di computeFit (puro)**

Create `src/hooks/useWindowFit.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { computeFit, FIT_MARGIN } from "./useWindowFit";

describe("computeFit", () => {
  it("adds left margin to width and top+bottom margins to height", () => {
    const f = computeFit(100, 50);
    expect(f.winW).toBe(100 + FIT_MARGIN.left);
    expect(f.winH).toBe(50 + FIT_MARGIN.top + FIT_MARGIN.bottom);
  });

  it("places content offset at the reserved left/top margins", () => {
    const f = computeFit(100, 50);
    expect(f.contentX).toBe(FIT_MARGIN.left);
    expect(f.contentY).toBe(FIT_MARGIN.top);
    expect(f.contentW).toBe(100);
    expect(f.contentH).toBe(50);
  });
});
```

- [ ] **Step 2: Esegui il test e verifica che fallisca**

Run: `npx vitest run src/hooks/useWindowFit.test.ts`
Expected: FAIL (`Failed to resolve import "./useWindowFit"`).

- [ ] **Step 3: Implementa l'hook**

Create `src/hooks/useWindowFit.ts`:

```ts
import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

// Transparent margins (CSS px) reserved around the painted widget for shadow/glow.
// `top` MUST match `.faro-root .drawer { top: ... }` in App.css.
export const FIT_MARGIN = { left: 40, top: 30, bottom: 40 } as const;

export function computeFit(contentW: number, contentH: number) {
  return {
    winW: contentW + FIT_MARGIN.left,
    winH: contentH + FIT_MARGIN.top + FIT_MARGIN.bottom,
    contentX: FIT_MARGIN.left,
    contentY: FIT_MARGIN.top,
    contentW,
    contentH,
  };
}

// Observe the painted widget and keep the OS window sized to hug it. Coalesces
// bursts of resize callbacks into one report per animation frame.
export function useWindowFit(ref: React.RefObject<HTMLElement | null>) {
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    let raf = 0;
    const report = () => {
      const r = el.getBoundingClientRect();
      if (r.width === 0 || r.height === 0) return;
      const fit = computeFit(Math.ceil(r.width), Math.ceil(r.height));
      invoke("resize_to_content", { fit }).catch(() => {
        /* ignore — not in Tauri context (e.g. browser dev) */
      });
    };
    const ro = new ResizeObserver(() => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(report);
    });
    ro.observe(el);
    report();
    return () => {
      ro.disconnect();
      cancelAnimationFrame(raf);
    };
  }, [ref]);
}
```

- [ ] **Step 4: Esegui il test e verifica che passi**

Run: `npx vitest run src/hooks/useWindowFit.test.ts`
Expected: PASS (2 test).

- [ ] **Step 5: Fai accettare il ref a DrawerPanel**

In `src/components/DrawerPanel.tsx`, sostituisci l'intero file con:

```tsx
import type { ReactNode, RefObject } from "react";

export function DrawerPanel({
  open, pill, panel, onEnter, onLeave, onToggle, rootRef,
}: {
  open: boolean; pill: ReactNode; panel: ReactNode;
  onEnter: () => void; onLeave: () => void; onToggle: () => void;
  rootRef?: RefObject<HTMLDivElement | null>;
}) {
  return (
    <div className="drawer" ref={rootRef} onMouseEnter={onEnter} onMouseLeave={onLeave}>
      {open ? (
        <div className="panel" onClick={(e) => e.stopPropagation()}>{panel}</div>
      ) : (
        <div onClick={onToggle}>{pill}</div>
      )}
    </div>
  );
}
```

- [ ] **Step 6: Collega l'hook in App.tsx**

In `src/App.tsx`, aggiorna gli import in cima (riga 1) per includere `useRef`:

```tsx
import { useEffect, useMemo, useRef, useState } from "react";
```

Aggiungi l'import dell'hook dopo la riga `import { useAudioCues } from "./hooks/useAudioCues";` (riga 9):

```tsx
import { useWindowFit } from "./hooks/useWindowFit";
```

Dentro `App()`, dopo la riga `const [now, setNow] = useState(Date.now());` (riga 22), aggiungi:

```tsx
  const rootRef = useRef<HTMLDivElement>(null);
  useWindowFit(rootRef);
```

Infine, passa il ref a `DrawerPanel`: sostituisci la riga di apertura del componente (riga 73):

```tsx
      <DrawerPanel
```

con:

```tsx
      <DrawerPanel
        rootRef={rootRef}
```

- [ ] **Step 7: Verifica build frontend e suite TS completa**

Run: `npm run build`
Expected: build TypeScript + Vite senza errori.

Run: `npm test`
Expected: PASS (tutta la suite vitest, inclusi i nuovi 2 test).

- [ ] **Step 8: Commit**

```bash
git add src/hooks/useWindowFit.ts src/hooks/useWindowFit.test.ts src/components/DrawerPanel.tsx src/App.tsx
git commit -m "feat(frontend): window-fit hook keeps OS window sized to content"
```

---

## Task 4: Leggibilità — contrasto rinforzato (CSS)

**Files:**
- Modify: `src/App.css:4-8`, `src/App.css:11-16`, `src/App.css:24`

**Interfaces:**
- Consumes: `FIT_MARGIN.top = 30` (Task 3) — il valore `top: 30px` su `.drawer` deve combaciare.
- Produces: nessuna interfaccia di codice (solo presentazione).

- [ ] **Step 1: Rendi `.faro-root` un contenitore di posizionamento e ancora `.drawer` in alto a destra**

In `src/App.css`, sostituisci le righe 4-8:

```css
.faro-root {
  font-family: -apple-system, system-ui, sans-serif; color: #e5e7eb;
  height: 100%; display: flex; justify-content: flex-end; align-items: flex-start;
}
.drawer { padding: 8px 0; }
```

con:

```css
.faro-root {
  font-family: -apple-system, system-ui, sans-serif; color: #e5e7eb;
  height: 100%; width: 100%; position: relative;
}
/* top MUST match FIT_MARGIN.top in src/hooks/useWindowFit.ts */
.drawer { position: absolute; top: 30px; right: 0; }
```

- [ ] **Step 2: Rinforza il contrasto della superficie glass**

In `src/App.css`, sostituisci le righe 11-16:

```css
.pill, .panel {
  backdrop-filter: blur(20px);
  background: linear-gradient(160deg, rgba(28,30,38,.86), rgba(18,19,26,.9));
  border: 1px solid rgba(255,255,255,.08); border-right: none;
  border-radius: 12px 0 0 12px; box-shadow: -6px 8px 30px rgba(0,0,0,.45);
}
```

con:

```css
.pill, .panel {
  backdrop-filter: blur(20px);
  background: linear-gradient(160deg, rgba(24,26,34,.95), rgba(14,15,21,.97));
  border: 1px solid rgba(255,255,255,.14); border-right: none;
  border-radius: 12px 0 0 12px;
  /* dark separation ring + soft light top edge + deep shadow → reads on any bg */
  box-shadow: 0 0 0 1px rgba(0,0,0,.55), inset 0 1px 0 rgba(255,255,255,.10), -8px 10px 34px rgba(0,0,0,.55);
}
```

- [ ] **Step 3: Alza l'opacità del nub idle**

In `src/App.css`, sostituisci la riga 24:

```css
.pill.nub { opacity: .55; padding: 9px; }
```

con:

```css
.pill.nub { opacity: .85; padding: 9px; }
```

- [ ] **Step 4: Verifica build**

Run: `npm run build`
Expected: build senza errori.

- [ ] **Step 5: Commit**

```bash
git add src/App.css
git commit -m "style(widget): reinforced contrast + content-anchored drawer"
```

---

## Task 5: Verifica d'integrazione + aggiornamento ledger

**Files:**
- Create: `.superpowers/sdd/progress-widget-refinement-phase1.2.md`

**Interfaces:**
- Consumes: tutti i task precedenti.
- Produces: nessuna (verifica + documentazione).

- [ ] **Step 1: Esegui l'intera suite di test**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS (inclusi i 7 test di `window_geom`).

Run: `npm test`
Expected: PASS (intera suite vitest).

- [ ] **Step 2: Verifica manuale dei gate dello spec (§5)**

Avvia: `npm run tauri dev` (da `/Users/stefano/Progetti/claude/faro`). Verifica nell'ordine e annota l'esito di ciascuno:

1. **Test #4 (il più importante):** con il widget visibile, clicca sulle app sottostanti **attorno** al widget (sopra, sotto, a sinistra dell'area trasparente). I click devono arrivare alle app dietro; solo i click **sopra il pixel-footprint visibile** del widget interagiscono con Faro.
2. **Test #1/#2:** cambia Space → il widget resta visibile; entra in fullscreen → sparisce; esci → riappare.
3. **Test #3:** porta sotto al widget una finestra con sfondo chiaro affollato e una scura → testo e pillola restano leggibili in entrambi i casi.
4. **No flicker:** muovi il cursore dentro/fuori e fai hover per espandere → nessun lampeggio dello stato passthrough; la finestra si ridimensiona in modo pulito.

- [ ] **Step 3: Crea il ledger di progresso**

Create `.superpowers/sdd/progress-widget-refinement-phase1.2.md`:

```markdown
# Faro Widget Refinement Phase 1.2 — SDD Progress Ledger

Plan: docs/superpowers/plans/2026-06-28-faro-widget-refinement-phase1.2.md
Spec: docs/superpowers/specs/2026-06-28-faro-widget-refinement-phase1.2-design.md
Branch: design/widget-redesign
Started: 2026-06-28

## Tasks

- [x] Task 1: Pure window-geometry module
- [x] Task 2: Resize command, content-rect hit-test, all-spaces presence
- [x] Task 3: Frontend window-fit hook + wiring
- [x] Task 4: Reinforced contrast (CSS)
- [x] Task 5: Integration verification + ledger

## Verifica manuale (§5 spec)

- [ ] #4 — app sottostanti cliccabili ovunque tranne sul widget visibile
- [ ] #1/#2 — visibile su tutti gli Spaces; cede al fullscreen
- [ ] #3 — leggibile su sfondi chiari e scuri affollati
- [ ] no flicker passthrough durante hover/resize

## Note runtime

- (annota qui l'esito dello Step 6 del Task 2 sul collection-behavior fullscreen)
```

Spunta le caselle di verifica manuale man mano che le confermi a runtime.

- [ ] **Step 4: Commit**

```bash
git add .superpowers/sdd/progress-widget-refinement-phase1.2.md
git commit -m "docs: phase 1.2 progress ledger + verification gates"
```

---

## Self-Review Notes (compilato dall'autore del piano)

- **Copertura spec §4.A (resize + hit-test):** Task 1 (geometria pura) + Task 2 (comando/stato/hit-test) + Task 3 (misura frontend). ✓
- **Copertura spec §4.B (presenza Spaces, non-fullscreen):** Task 2 Step 4 + gate runtime Step 6. ✓
- **Copertura spec §4.C (contrasto CSS):** Task 4. ✓
- **Copertura spec §5 (gate verifica):** Task 5 Step 2 + ledger. ✓
- **Consistenza tipi:** `ContentFit{x,y,w,h}` e `FitArgs{winW,...}`/`computeFit` allineati tra Rust e TS; il payload `{ fit: {...} }` corrisponde al param `fit: FitArgs` con `rename_all="camelCase"`. ✓
- **Coupling dichiarato:** `FIT_MARGIN.top` (TS) ↔ `.drawer { top: 30px }` (CSS), con commenti incrociati in entrambi i file. ✓
```
