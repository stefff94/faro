# Faro — Fase 1.2: Refinement Overlay (Design)

**Stato:** Approvato, pronto per pianificazione.
**Owner:** Stefano
**Data:** 2026-06-28
**Branch:** design/widget-redesign (continua dalla Fase 1, completata)

> Refinement di ciò che è stato costruito nella Fase 1. **Non** è la Fase 2 (notch /
> click-to-focus / task-summary LLM): quelli restano nel backbone di handoff. Qui si
> correggono i difetti di comportamento-finestra dell'overlay attuale.

---

## 1. Problema

Il widget della Fase 1 funziona a livello di dati/UI ma il comportamento della finestra
è inutilizzabile in pratica. Quattro difetti, dal più grave:

1. **Hit-area invisibile gigante (critico).** Anche se il widget dipinto è minuscolo,
   la finestra OS cattura il mouse su una striscia molto più grande, rendendo
   non-cliccabili le app sottostanti. **Questo non deve assolutamente succedere.**
2. **Presente solo in un Space/desktop.** Il widget appare nel solo desktop in cui è
   stato creato, non su tutti.
3. **Sparisce.** In certe condizioni (cambio Space, fullscreen) non è più visibile.
4. **Scarsa leggibilità** quando ci sono app affollate sotto di sé.

## 2. Cause radice (analisi del codice attuale)

| # | Sintomo | Causa radice nel codice |
|---|---------|--------------------------|
| 4 | Hit-area invisibile | Finestra OS fissa **360×700** (`tauri.conf.json`), interamente trasparente; il widget dipinto ne occupa una frazione. `cursor_in_window` (`lib.rs`) fa hit-test sull'**intero** `outer_size` 360×700: appena il cursore entra in quella scatola invisibile, `set_cursor_passthrough(false)` rende l'intera finestra catturante. Polling a 80ms (`App.tsx`) aggiunge lag/flicker. |
| 1 | Solo un desktop | Nessuna chiamata a `set_visible_on_all_workspaces` / collection-behavior `canJoinAllSpaces`. macOS lega la finestra allo Space di creazione. |
| 2 | Sparisce | Stessa radice di #1 (cambio Space) + `alwaysOnTop` a livello floating normale che sta sotto le app fullscreen. Nub idle a `opacity:.55` quasi invisibile. |
| 3 | Poco leggibile | CSS glass semitrasparente su sfondi affollati; nub idle opacity .55, bordi sottili, `shadow:false` in config. |

Le radici si raggruppano in tre cantieri: **(A)** geometria finestra + hit-testing,
**(B)** comportamento macOS (Spaces + window level), **(C)** leggibilità.

## 3. Decisioni (locked)

| Tema | Scelta | Note |
|------|--------|------|
| Fix hit-area (#4) | **Finestra che traccia il contenuto + hit-test sulla regione visibile** (entrambi) | Dimensione-al-contenuto come difesa primaria; region hit-test come cintura di sicurezza. |
| Presenza (#1, #2) | **Tutti gli Spaces, ma NON in fullscreen** | `canJoinAllSpaces`, niente `fullScreenAuxiliary`. In fullscreen il widget cede il passo. |
| Leggibilità (#3) | **Contrasto rinforzato sempre** (solo CSS) | Mantiene estetica glass, aggiunge outline/halo + ombra, alza opacità nub idle. |

## 4. Design

### A. Geometria finestra + hit-testing (risolve #4)

**Difesa primaria — finestra che abbraccia il contenuto:**

- Il frontend misura la dimensione reale del widget dipinto (elemento radice `.drawer`)
  con un `ResizeObserver`.
- A ogni cambio dimensione, il frontend invoca un comando Rust
  `resize_to_content(width, height)` (valori **logici**) che, in un'unica operazione:
  1. ridimensiona la finestra OS esattamente al contenuto (`LogicalSize`);
  2. riposiziona la finestra per mantenere l'ancoraggio al **bordo destro**
     (`LogicalPosition`), riusando la logica esistente di `position_right_edge`.
  Operazione singola lato Rust per evitare flicker resize→reposition.
- A riposo la finestra è un **nub piccolo**; si espande al drawer su hover/pin e si
  ricontrae all'uscita. Niente più rettangolo invisibile 360×700.
- `tauri.conf.json`: la `width/height` iniziale diventa la dimensione del nub (piccola);
  la finestra cresce/decresce a runtime.

**Cintura di sicurezza — hit-test sulla regione visibile:**

- Il frontend riporta a Rust il **content-rect visibile** (bounding box dell'elemento
  dipinto, in pixel, escludendo padding/ombra trasparenti).
- `cursor_in_window` viene modificato per testare contro questo content-rect invece dei
  nudi `outer_size`.
- Il passthrough resta default `true`; si disabilita **solo** quando il cursore è sopra
  il content-rect reale. Così anche un eventuale margine trasparente residuo non cattura
  click.

**Robustezza scale-factor:** tutte le dimensioni/posizioni passate sono logiche; la
conversione a fisico avviene lato Tauri. Il content-rect riportato per l'hit-test deve
essere coerente con lo spazio coordinate usato da `cursor_pos_physical`
(fisico, origine top-left) — convertito via scale factor.

### B. Presenza macOS (risolve #1, #2 fuori-fullscreen)

- In `setup` (o comando dedicato), chiamare `set_visible_on_all_workspaces(true)` →
  il widget appare su **tutti** gli Spaces/desktop.
- Mantenere `alwaysOnTop` a livello floating normale (sopra le app comuni).
- **Non** aggiungere `fullScreenAuxiliary`: in fullscreen il widget resta nascosto
  (comportamento voluto).
- ⚠️ **Verifica a runtime:** confermare su macOS i flag esatti di collection-behavior
  applicati da Tauri per `set_visible_on_all_workspaces`, assicurandosi che non forzino
  anche la visibilità in fullscreen. Se Tauri non offre il controllo fine, valutare una
  chiamata diretta alle API AppKit dietro un seam `#[cfg(target_os = "macos")]`
  (coerente con `cursor_pos_physical`).

### C. Leggibilità (risolve #3) — solo CSS

- Outline/halo + ombra netta sotto la pillola, per staccare da qualsiasi sfondo.
- Opacità del nub idle alzata (da `.55` a un valore leggibile) mantenendo discrezione.
- Background glass più denso/scuro per garantire contrasto del testo su sfondi chiari.
- Nessun cambiamento strutturale ai componenti: solo `App.css`.

## 5. Verifica (gate manuali — è un overlay)

1. **Test #4:** con il widget visibile, le app sottostanti sono cliccabili **ovunque
   tranne** sopra il pixel-footprint del widget. Muovere il cursore attorno al widget
   non blocca i click dietro.
2. **Test #1/#2:** il widget appare su tutti gli Spaces; entrando in fullscreen sparisce,
   uscendo riappare; cambiando desktop resta visibile.
3. **Test #3:** leggibile su sfondo chiaro e scuro affollato.
4. **No flicker:** nessun lampeggio dello stato passthrough durante hover/espansione/
   contrazione.

**Testabile come unità (pura):** la funzione di calcolo resize + riposizionamento
bordo-destro (input: dimensione contenuto, dimensione schermo → output: size+position),
isolata e testata senza finestra reale.

## 6. Fuori scope

- Notch, click-to-focus, task-summary LLM → **Fase 2** (handoff esistente).
- Porting Windows.
- Redesign estetico oltre il contrasto.
- Persistenza posizione drag-custom (resta l'ancoraggio bordo destro automatico).

## 7. File toccati (previsione)

- `src-tauri/tauri.conf.json` — dimensione iniziale = nub.
- `src-tauri/src/lib.rs` — comando `resize_to_content`, modifica `cursor_in_window` a
  content-rect, chiamata `set_visible_on_all_workspaces`, eventuale seam AppKit.
- `src/App.tsx` (+ eventuale nuovo hook) — `ResizeObserver`, report content-rect,
  invocazione resize.
- `src/App.css` — contrasto/halo/ombra/opacità.
</content>
</invoke>
