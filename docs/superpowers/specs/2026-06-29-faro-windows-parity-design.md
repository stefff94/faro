# Faro — Windows Parity (Overlay) — Design

**Data:** 2026-06-29
**Branch:** feat/windows-parity (da `main`, dopo la migrazione baseline)
**Stato:** in design (brainstorming) — da approvare, poi pianificare
**Ambito:** porting dell'**app overlay** su Windows a parità di capability. NON copre reporter/install/packaging (M4) né le feature di Fase 2.

> Obiettivo strategico: portare avanti macOS e Windows **insieme**, a parità di capability sul *core* (monitoraggio, overlay, attenzione, hit-test, resize). Alcune feature future divergono per natura (notch, click-to-focus): è atteso.

---

## 1. Contesto

Faro è un overlay desktop (Tauri 2 + React/TS) che monitora sessioni agentiche e segnala quali richiedono attenzione. Oggi è macOS v0, con la Fase 1.2 (comportamento finestra: resize-to-content, hit-test sul content-rect, presenza, leggibilità) **validata a runtime su macOS** (fix in `636e515`).

Tutto il codice OS-specifico vive in `src-tauri/src/lib.rs` dietro `#[cfg]`. Il `Source` trait isola già la logica di stato dall'OS. Il porting Windows è quindi **additivo**: riempire i rami `#[cfg(target_os = "windows")]` mancanti e verificare le superfici già cross-platform.

---

## 2. Obiettivi (v1 Windows)

1. L'overlay si comporta su Windows come su macOS, **sul desktop virtuale corrente**:
   - click-through corretto (i click attorno al widget passano alle app dietro);
   - hover funzionante (entra → apre, esci → chiude);
   - posizionamento ancorato in alto-a-destra + resize-to-content che abbraccia il contenuto;
   - finestra trasparente, frameless, always-on-top, fuori dalla taskbar;
   - testo/pillola leggibili su sfondi affollati chiari e scuri.
2. **Codice nuovo minimo**, isolato dietro seam `#[cfg(target_os = "windows")]`, **zero nuove dipendenze**.
3. **Nessuna regressione** del comportamento macOS (i `#[cfg]` isolano i rami).
4. Target di build: **`x86_64-pc-windows-msvc`** (path ufficiale Tauri su Windows).

---

## 3. Non-obiettivi (v1)

- **Presenza su tutti i desktop virtuali** (analogo "tutti gli Spaces"). Richiede API non ufficiali/fragili (`IVirtualDesktopManager`, variabili tra build Win10/11) → **follow-up dedicato**. La v1 accetta presenza sul solo desktop corrente.
- **Reporter / hook-install / packaging su Windows** → concern **M4** separato, per entrambe le piattaforme. Il contratto HTTP (`127.0.0.1:8765`) è già cross-platform; per testare si fa POST diretta.
- **Multi-monitor avanzato** (spanning a DPI miste, spostamento tra monitor a DPI diverse) → best-effort, **non** un gate v1.
- Feature di **Fase 2** (notch, bring-terminal-to-front, task summary intelligente).

---

## 4. Architettura — i seam

### 4.A Seam cursore (l'unico vero codice nativo nuovo) — Approccio A

Oggi `cursor_pos_physical` è `#[cfg(target_os = "macos")]` (FFI CoreGraphics) e `cursor_in_window` ha rami `#[cfg]`. Approccio scelto: **`#[cfg]` per-funzione, minimale** (coerente col pattern esistente; niente trait/astrazioni — YAGNI; niente crate esterne — no new deps).

- Aggiungere `#[cfg(target_os = "windows")] fn cursor_pos_physical(scale: f64) -> Option<(i32, i32)>` via Win32 **`GetCursorPos`** (FFI raw su `user32.dll`, specchio del blocco CoreGraphics).
- ⚠️ **Gotcha di correttezza:** CoreGraphics restituisce *punti logici* → la impl macOS moltiplica per `scale`. `GetCursorPos` restituisce **già pixel fisici** (con processo per-monitor DPI-aware) → la impl Windows **non** deve moltiplicare per `scale`. Il parametro `scale` resta nella firma per simmetria ma su Windows non viene applicato alla posizione.
- Refactor di `cursor_in_window`: i rami macOS e Windows sono **identici** salvo la chiamata a `cursor_pos_physical` → collassare in **un corpo condiviso** `#[cfg(any(target_os = "macos", target_os = "windows"))]` (scale_factor → `cursor_pos_physical` → `outer_position` → leggi `fit_state` → `content_rect_physical` → `point_in_rect`). Il ramo `#[cfg(not(any(...)))]` resta `let _ = (&window, &fit_state); return false;`.

### 4.B Presenza su tutti i desktop — deferred (v1)

`set_visible_on_all_workspaces(true)` resta nel setup (innocuo cross-platform). Su Windows non effettua pinning cross-virtual-desktop → **v1 accetta solo-desktop-corrente**. Nessun codice Windows nuovo qui per la v1; la capability è un non-obiettivo (§3).

### 4.C Superfici cross-platform — verificare, non riscrivere

Tutte API Tauri già in uso, da **validare** su Windows (seam aggiuntivi solo se emerge una divergenza, come fu il piano di contingenza per il fullscreen su macOS):

- **Click-through** — `set_ignore_cursor_events` (su Windows → `WS_EX_TRANSPARENT`/`WS_EX_LAYERED`).
- **Config finestra** — `transparent`, `decorations:false`, `alwaysOnTop`, `shadow:false`, `skipTaskbar` (tutti supportati su Windows; `macOSPrivateApi` ignorato).
- **Posizionamento** — `position_right_edge` / `window_geom::right_edge_position` (usa `current_monitor` + `outer_size`).
- **Resize-to-content** — `resize_to_content` (`LogicalSize`/`PhysicalPosition`). Il fix `636e515` (derivare la dimensione fisica da `fit.win_w * scale_factor()` invece di rileggere `outer_size()`) **rafforza anche Windows**.
- **Frontend** — `useWindowFit`, hover-via-poll, CSS: già verde su Windows (16/16 vitest, build clean). L'hover-via-poll **dipende** dal seam cursore (§4.A): appena atterra Seam 1, l'hover funziona.

---

## 5. DPI / multi-monitor

- **DPI awareness (correttezza #1).** `GetCursorPos` restituisce pixel fisici solo se il processo è **Per-Monitor-DPI-Aware v2**. Le app Tauri/WebView2 dichiarano PerMonitorV2 di default via manifest → **da verificare a runtime** che valga; se non vale, impostarlo. Deve risultare coerente con `window.scale_factor()` e con l'uso di `scale` in `content_rect_physical` (no doppio-scale, cfr. §4.A).
- **Multi-monitor (stance v1).** `current_monitor()` dà il monitor su cui sta la finestra; l'ancoraggio usa la sua dimensione. **v1 = corretto sul monitor dove vive il widget** (primario di default). Spanning a DPI miste e spostamenti cross-monitor = best-effort, non gate.

---

## 6. Gate di verifica (Windows — analogo della §5 macOS)

1. **Build:** `cargo build` + `cargo test` sul target MSVC — i 7 test `window_geom` passano e il full-crate compila col ramo `#[cfg(target_os = "windows")]`.
2. **Click-through:** i click attorno al widget arrivano alle app dietro; solo il pixel-footprint visibile interagisce con Faro.
3. **Hover:** cursore entra → pannello si espande; esce → si richiude (hover-via-poll pilotato dal seam cursore Windows).
4. **Posizionamento:** pill/pannello ancorati in alto-a-destra, nessun drift su espansione/collasso.
5. **Resize:** la finestra abbraccia il contenuto; nessun flicker/oscillazione (no-op guard).
6. **Aspetto:** trasparenza, frameless, always-on-top, skip-taskbar e leggibilità su sfondi chiari/scuri resi correttamente.
7. **Esplicitamente NON testato in v1:** presenza su tutti i desktop virtuali (deferred, §3).

---

## 7. Testing

- **Logica pura** (`window_geom`) → già coperta da unit test; identica su MSVC (riconfermare nel gate build).
- **FFI `cursor_pos_physical` Windows** → non unit-testabile (chiama l'OS); mantenuta minimale e validata dai gate runtime (stessa scelta già fatta per CoreGraphics su macOS).
- **Frontend** → suite vitest invariata (già verde su Windows).
- **Manuale** → i gate §6 su questa macchina Windows, una volta installato il toolchain MSVC.

---

## 8. Prerequisiti (assunzioni, non feature)

- **Visual Studio Build Tools (MSVC)** installati (Desktop development with C++ → linker `link.exe` + Windows SDK).
- Toolchain Rust MSVC: `rustup toolchain install stable-x86_64-pc-windows-msvc` e usarla per la build (default o `--toolchain`). La toolchain Rust è già presente sulla macchina (host GNU, da Fase 1.2); serve aggiungere/usare quella MSVC.
- Questi sono i prerequisiti di build per tutto il resto del lavoro; finché non sono presenti, `cargo build`/`tauri dev` su Windows non sono eseguibili (come emerso in Fase 1.2).

---

## 9. Criteri di successo (v1)

- Gate §6 punti 1–6 **verdi** su questa macchina Windows; parità funzionale con l'overlay macOS **sul desktop corrente**.
- **Diff contenuto:** ~una funzione FFI Win32 + un refactor `#[cfg]` di `cursor_in_window`; **nessuna nuova dipendenza**.
- **Zero regressioni macOS:** i rami `#[cfg]` isolano il nuovo codice; il comportamento del ramo macOS resta invariato (riconfermabile su macOS al merge).
