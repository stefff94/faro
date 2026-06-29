# Faro Windows Parity (Overlay) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Portare l'app overlay Faro a parità di capability su Windows (desktop corrente), riempiendo i seam `#[cfg(target_os = "windows")]` mancanti e verificando le superfici già cross-platform.

**Architecture:** Tutto il codice OS-specifico vive in `src-tauri/src/lib.rs` dietro `#[cfg]`. Il porting è additivo: una sola funzione FFI nativa nuova (`cursor_pos_physical` via Win32 `GetCursorPos`) + il collasso di `cursor_in_window` in un corpo condiviso `#[cfg(any(macos, windows))]`. Il resto (click-through, config finestra, posizionamento, resize-to-content, frontend) è già cross-platform e va solo verificato a runtime.

**Tech Stack:** Tauri 2.x (Rust core, target `x86_64-pc-windows-msvc`), React/TS + Vite (frontend), Win32 `user32.dll` FFI raw, `cargo test` (Rust), vitest (TS).

## Global Constraints

- Target di build Windows: **`x86_64-pc-windows-msvc`** (path ufficiale Tauri).
- **Zero nuove dipendenze** (FFI raw, come già per CoreGraphics su macOS).
- **Zero regressioni macOS:** i rami `#[cfg]` isolano il nuovo codice; il corpo del ramo macOS di `cursor_in_window` resta identico nel comportamento.
- ⚠️ **Gotcha scale:** `GetCursorPos` restituisce **pixel fisici** (processo per-monitor DPI-aware) → la impl Windows **non** moltiplica per `scale` (a differenza di CoreGraphics, che dà punti logici). Il parametro `scale` resta nella firma per simmetria ma su Windows non è applicato alla posizione.
- Convenzione hit-test rettangolo: left/top inclusivi, right/bottom esclusivi (invariata, vive in `window_geom`).
- **v1 non include** la presenza su tutti i desktop virtuali (deferred), né reporter/install (M4).

### Environment notes (questo host Windows)

- `cargo`/`rustc` sono in `C:\Users\stefano.vannucchi\.cargo\bin` (**non in PATH**). In bash: prefissare `export PATH="/c/Users/stefano.vannucchi/.cargo/bin:$PATH"`.
- I comandi `cargo`/`rustc` vanno eseguiti con **sandbox disabilitato** (il sandbox della Bash tool ha un cap di memoria ~16MB che fa fallire rustc/cargo).
- La toolchain attuale è host **GNU** (da Fase 1.2) e **non** compila il full-crate (manca `as`). Task 1 aggiunge la toolchain **MSVC**, che invece compila.

---

## File Structure

- **Modify** `src-tauri/src/lib.rs` — aggiunge `#[cfg(target_os = "windows")] fn cursor_pos_physical`; refactor del comando `cursor_in_window` a corpo condiviso `#[cfg(any(macos, windows))]`. Nessun altro cambiamento.
- **Modify** `README.md` — breve sezione "Building on Windows (dev)" che documenta il prerequisito MSVC (Task 1).
- **Create** `docs/superpowers/progress-windows-parity.md` — ledger di verifica runtime + gate (Task 3).

Nessun file Rust nuovo: la logica pura condivisa è già in `window_geom.rs` (Fase 1).

---

## Task 1: Toolchain MSVC + build baseline su Windows

**Files:**
- Modify: `README.md`

**Interfaces:**
- Consumes: nessuno (prerequisito di ambiente).
- Produces: una toolchain `stable-x86_64-pc-windows-msvc` funzionante e la prova che il crate **attuale** compila + i test `window_geom` passano su MSVC. Nessuna interfaccia di codice.

> Questo task è il **gate di fondazione**: prima di scrivere il seam Windows, dimostra che la toolchain MSVC compila il crate Tauri (cosa che la toolchain GNU non fa). L'install dei Visual Studio Build Tools può richiedere un passo **umano** (installer GUI / download multi-GB): se l'esecutore non può installarli da solo, deve fermarsi con stato **BLOCKED** e chiedere all'umano di installarli, poi riprendere.

- [ ] **Step 1: Assicurare i Visual Studio Build Tools (MSVC)**

Verifica se il linker MSVC è presente:

```bash
cmd.exe //c "where link.exe" 2>/dev/null | grep -i "Microsoft Visual Studio" || echo "MSVC link.exe NOT found"
```

Se assente: installare **Visual Studio Build Tools** con il workload "Desktop development with C++" (fornisce `link.exe` + Windows SDK, che include `user32.lib`). Questo è tipicamente un passo umano (installer GUI). Se non eseguibile in autonomia → riportare **BLOCKED** con questa indicazione.

- [ ] **Step 2: Installare e impostare la toolchain Rust MSVC**

```bash
export PATH="/c/Users/stefano.vannucchi/.cargo/bin:$PATH"
rustup toolchain install stable-x86_64-pc-windows-msvc
```

Expected: la toolchain si installa (scarica rustc/cargo/std per il target MSVC).

- [ ] **Step 3: Compilare il crate attuale con MSVC (gate)**

Eseguire con **sandbox disabilitato**:

```bash
export PATH="/c/Users/stefano.vannucchi/.cargo/bin:$PATH"
cargo +stable-x86_64-pc-windows-msvc build --manifest-path src-tauri/Cargo.toml
```

Expected: **compila senza errori** (a differenza della toolchain GNU, che falliva su `dlltool`/`as` per `windows-sys`). Se fallisce per linker mancante → tornare allo Step 1 (Build Tools).

- [ ] **Step 4: Eseguire i test `window_geom` su MSVC**

```bash
export PATH="/c/Users/stefano.vannucchi/.cargo/bin:$PATH"
cargo +stable-x86_64-pc-windows-msvc test --manifest-path src-tauri/Cargo.toml window_geom
```

Expected: **7 passed** (`window_geom`).

- [ ] **Step 5: Documentare il prerequisito in README**

In `README.md`, dopo la riga sullo scope (`**Scope (v0):** macOS only...`), aggiungere:

```markdown

### Building on Windows (dev)

Windows builds use the MSVC target. Requirements:
- Visual Studio Build Tools with "Desktop development with C++" (provides `link.exe` + Windows SDK).
- `rustup toolchain install stable-x86_64-pc-windows-msvc`, then build with that toolchain
  (e.g. `cargo +stable-x86_64-pc-windows-msvc build` or set it as default).

Windows support is in progress (overlay parity); the all-virtual-desktops presence is not yet ported.
```

- [ ] **Step 6: Commit**

```bash
git add README.md
git commit -m "docs: document Windows (MSVC) dev build requirements"
```

---

## Task 2: Seam cursore Windows (`GetCursorPos`) + refactor `cursor_in_window`

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `window_geom::{ContentFit, content_rect_physical, point_in_rect}` (Fase 1) e lo stato `ContentFitState` (Fase 1.2), entrambi già presenti.
- Produces:
  - `#[cfg(target_os = "windows")] fn cursor_pos_physical(scale: f64) -> Option<(i32, i32)>` — posizione globale del cursore in pixel fisici (top-left origin) via Win32 `GetCursorPos`.
  - `cursor_in_window` con corpo condiviso `#[cfg(any(target_os = "macos", target_os = "windows"))]`: ora ritorna `true` quando il cursore è dentro il content-rect anche su Windows.

> Nota sul testing: la funzione FFI interroga l'OS (posizione del cursore) e **non è unit-testabile**, esattamente come l'esistente `cursor_pos_physical` macOS (CoreGraphics). La verifica di questo task è: (a) **compila** su MSVC con il nuovo seam, (b) i test `window_geom` (la logica pura condivisa) restano verdi. La validazione runtime della FFI avviene nel Task 3. **Non** inventare uno unit test fittizio per la FFI.

- [ ] **Step 1: Aggiungere `cursor_pos_physical` per Windows**

In `src-tauri/src/lib.rs`, **subito dopo** il blocco `#[cfg(target_os = "macos")] fn cursor_pos_physical(...) { ... }` esistente (il blocco CoreGraphics che termina con la sua `}`), aggiungere:

```rust
/// Returns the global cursor position in physical pixels (top-left origin) on Windows.
/// `GetCursorPos` already reports PHYSICAL screen pixels when the process is
/// per-monitor DPI aware (Tauri/WebView2 declares PerMonitorV2), so — unlike the
/// CoreGraphics path — we do NOT multiply by `scale`. `scale` is accepted only to
/// keep the signature symmetric with the macOS implementation.
#[cfg(target_os = "windows")]
fn cursor_pos_physical(_scale: f64) -> Option<(i32, i32)> {
    #[repr(C)]
    struct POINT {
        x: i32,
        y: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetCursorPos(point: *mut POINT) -> i32; // BOOL: nonzero on success
    }

    unsafe {
        let mut pt = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut pt) == 0 {
            return None;
        }
        Some((pt.x, pt.y))
    }
}
```

- [ ] **Step 2: Refactor di `cursor_in_window` a corpo condiviso macOS+Windows**

In `src-tauri/src/lib.rs`, sostituire l'INTERO comando `cursor_in_window` esistente:

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

con la versione che condivide il corpo tra macOS e Windows:

```rust
#[tauri::command]
fn cursor_in_window(window: tauri::WebviewWindow, fit_state: tauri::State<ContentFitState>) -> bool {
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (&window, &fit_state);
        return false;
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
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

> Il corpo è identico a quello macOS precedente: su Windows risolve la nuova `cursor_pos_physical` (Step 1), su macOS quella CoreGraphics. `content_rect_physical` applica `scale` al content-rect (logico→fisico) su entrambe le piattaforme; `cx,cy` sono fisici su entrambe (macOS: logico×scale; Windows: già fisico). Confronto fisico-fisico, coerente.

- [ ] **Step 3: Compilare su MSVC (verifica del seam)**

```bash
export PATH="/c/Users/stefano.vannucchi/.cargo/bin:$PATH"
cargo +stable-x86_64-pc-windows-msvc build --manifest-path src-tauri/Cargo.toml
```

Expected: **compila senza errori** (la FFI `user32`/`GetCursorPos` linka col Windows SDK; il refactor `#[cfg]` è ben formato).

- [ ] **Step 4: Test `window_geom` ancora verdi (regressione logica condivisa)**

```bash
export PATH="/c/Users/stefano.vannucchi/.cargo/bin:$PATH"
cargo +stable-x86_64-pc-windows-msvc test --manifest-path src-tauri/Cargo.toml window_geom
```

Expected: **7 passed** (la logica pura usata dal corpo condiviso è intatta).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(window): Windows cursor hit-test via GetCursorPos (shared cursor_in_window body)"
```

---

## Task 3: Verifica runtime dei gate + check DPI + ledger

**Files:**
- Create: `docs/superpowers/progress-windows-parity.md`

**Interfaces:**
- Consumes: Task 1 (toolchain) + Task 2 (seam cursore).
- Produces: nessuna interfaccia di codice (verifica runtime + documentazione). Eventuale fix di contingenza DPI se emerge un disallineamento.

> Questo task valida sul vero OS i comportamenti non verificabili per ispezione (analogo della §5 macOS). Eseguire l'app e annotare l'esito di ogni gate. Se il gate **3 (hover)** o **2 (click-through)** falliscono perché il cursore è disallineato, la causa probabile è il **DPI awareness** (Step 2): risolvere lì.

- [ ] **Step 1: Avviare l'app su Windows**

```bash
export PATH="/c/Users/stefano.vannucchi/.cargo/bin:$PATH"
npm run tauri dev -- --target x86_64-pc-windows-msvc
```

(oppure impostare `stable-x86_64-pc-windows-msvc` come default rustup e lanciare `npm run tauri dev`.) Expected: la finestra overlay compare in alto a destra, trasparente, frameless, always-on-top, fuori dalla taskbar.

> I gate principali (click-through, hover, posizionamento, resize, aspetto) si verificano **anche con il solo nub idle**, senza dati: non serve popolare sessioni. Se si vuole comunque popolare il widget per la leggibilità su molte card, il reporter è fuori scope (M4) — leggere l'endpoint del broker e il suo schema da `src-tauri/src/http.rs` e fare una POST manuale a `127.0.0.1:8765` con quella forma. Opzionale.

- [ ] **Step 2: Check DPI awareness (correttezza #1)**

Muovere il cursore **sul bordo visibile** del widget e osservare la transizione passthrough/hover. Se il widget reagisce quando il cursore è **effettivamente sopra** il pixel-footprint visibile → DPI ok. Se reagisce con un **offset** (es. a 1.25×/1.5× di scaling), `GetCursorPos` e `scale_factor()` non sono allineati: assicurare che il processo sia **Per-Monitor-DPI-Aware v2**.

Diagnosi/fix: verificare il manifest dell'eseguibile (Tauri/WebView2 dichiara PerMonitorV2 di default). Se manca, aggiungere/forzare l'awareness — opzioni in ordine di preferenza:
1. confermare che il manifest dell'app embeddi `<dpiAwareness>PerMonitorV2</dpiAwareness>` (default Tauri);
2. in alternativa, chiamata esplicita a `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` all'avvio, dietro `#[cfg(target_os = "windows")]` nel setup.

Annotare nel ledger l'esito (ok di default, oppure fix applicato).

- [ ] **Step 3: Verificare i gate §6 dello spec**

Verificare e annotare ciascuno:

1. **Build** (già verde da Task 1/2): `cargo +...-msvc build` + `cargo +...-msvc test window_geom` (7/7).
2. **Click-through:** i click attorno al widget arrivano alle app dietro; solo il pixel-footprint visibile interagisce.
3. **Hover:** cursore entra → pannello si espande; esce → si richiude.
4. **Posizionamento:** pill/pannello ancorati in alto-a-destra, nessun drift su espansione/collasso.
5. **Resize:** la finestra abbraccia il contenuto; nessun flicker/oscillazione.
6. **Aspetto:** trasparenza/frameless/always-on-top/skip-taskbar + leggibilità su sfondi chiari e scuri.

NON in scope v1: presenza su tutti i desktop virtuali (cambiando desktop il widget può sparire — atteso).

- [ ] **Step 4: Creare il ledger**

Create `docs/superpowers/progress-windows-parity.md`:

```markdown
# Faro Windows Parity (Overlay) — Progress Ledger

Spec: docs/superpowers/specs/2026-06-29-faro-windows-parity-design.md
Plan: docs/superpowers/plans/2026-06-29-faro-windows-parity.md
Branch: feat/windows-parity
Target: x86_64-pc-windows-msvc

## Tasks
- [x] Task 1: MSVC toolchain + baseline build (README documented)
- [x] Task 2: Windows cursor seam (GetCursorPos) + shared cursor_in_window
- [x] Task 3: runtime gates + DPI check + this ledger

## Gate di verifica runtime (§6 spec)
- [ ] 1 — cargo build + window_geom test verdi su MSVC
- [ ] 2 — click-through attorno al widget; solo il visibile interagisce
- [ ] 3 — hover apre/chiude correttamente
- [ ] 4 — posizionamento ancorato, nessun drift
- [ ] 5 — resize abbraccia il contenuto, no flicker
- [ ] 6 — trasparenza/frameless/always-on-top/skip-taskbar + leggibilità

## Note runtime
- DPI awareness: (annota qui — ok di default o fix PerMonitorV2 applicato)
- All-desktops: NON portato in v1 (deferred). Cambio desktop → widget sparisce (atteso).
- Regressione macOS: i rami #[cfg] isolano il nuovo codice; riconfermare su macOS al merge.
```

Spuntare le caselle dei gate man mano che si confermano a runtime.

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/progress-windows-parity.md
git commit -m "docs: Windows parity progress ledger + runtime gates"
```

---

## Self-Review Notes (compilato dall'autore del piano)

- **Copertura spec §4.A (seam cursore):** Task 2 Step 1 (`GetCursorPos`) + Step 2 (corpo condiviso). ✓
- **Copertura spec §4.B (all-desktops deferred):** nessun codice; documentato come non-obiettivo nel ledger (Task 3). ✓
- **Copertura spec §4.C (superfici cross-platform da verificare):** Task 3 gate 2–6. ✓
- **Copertura spec §5 (DPI/multi-monitor):** Task 3 Step 2 (DPI check + fix di contingenza). ✓
- **Copertura spec §6 (gate):** Task 3 Step 3 + ledger. ✓
- **Copertura spec §8 (prerequisito MSVC):** Task 1. ✓
- **Gotcha scale (no doppio-scale su Windows):** rispettato in Task 2 Step 1 (`_scale` non applicato) + commento. ✓
- **Zero nuove dipendenze:** FFI raw `user32`, nessuna crate. ✓
- **Zero regressione macOS:** il ramo `#[cfg(target_os = "macos")]` mantiene corpo identico; i `#[cfg]` isolano Windows. ✓
- **Test hygiene:** nessuno unit test fittizio per la FFI OS (dichiarato esplicitamente in Task 2); verifica via compile + `window_geom` + runtime. ✓
