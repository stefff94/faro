# Faro Windows Parity (Overlay) — Progress Ledger

Spec: docs/superpowers/specs/2026-06-29-faro-windows-parity-design.md
Plan: docs/superpowers/plans/2026-06-29-faro-windows-parity.md
Branch: feat/windows-parity
Target: x86_64-pc-windows-msvc

## Tasks
- [x] Task 1: MSVC toolchain + baseline build (README documented). Commit 84c97c2.
- [x] Task 2: Windows cursor seam (GetCursorPos) + shared cursor_in_window. Commit 1f3b10a.
- [x] Task 3: runtime gates + DPI check + this ledger. Anchor fix commit 21263ad.

## Gate di verifica runtime (§6 spec) — tutti verdi su Windows
- [x] 1 — cargo build + window_geom test verdi su MSVC (controller-verified: full crate build exit 0; `window_geom` 7/7)
- [x] 2 — click-through attorno al widget; solo il pixel-footprint visibile interagisce (user-confirmed)
- [x] 3 — hover apre/chiude correttamente (user-confirmed; esercita il seam `GetCursorPos`)
- [x] 4 — posizionamento ancorato in alto-destra, nessun drift su espansione/collasso (user-confirmed DOPO il fix 21263ad: drift verticale a scaling frazionario risolto bloccando la top-y al primo placement)
- [x] 5 — resize abbraccia il contenuto, no flicker/oscillazione (user-confirmed)
- [x] 6 — trasparenza/frameless/always-on-top/skip-taskbar + leggibilità su sfondi chiari/scuri (user-confirmed)

## Note runtime
- DPI awareness: OK. PerMonitorV2 di default (Tauri/WebView2): l'hover scatta sul footprint visibile, nessun offset segnalato → `GetCursorPos` (fisico) e `scale_factor()` allineati. Nessun fix manifest necessario.
- Bug trovato dai gate e risolto: il pannello si rialzava in espansione a scaling frazionario (`y=(screen_h-win_h)/4` ri-centra in px fisici → scala col DPI). Fix 21263ad: `AnchorTop` blocca la top-y al primo placement (idle invariato, cresce verso il basso). Codice condiviso -> riconfermare su macOS al merge.
- All-desktops: NON portato in v1 (deferred §3 spec). Cambio desktop virtuale -> il widget può sparire (atteso, non un fallimento).
- Regressione macOS: i rami `#[cfg]` isolano il nuovo codice; il corpo del ramo macOS di `cursor_in_window` è byte-identico (riconfermare su macOS al merge).
- Toolchain: VS2017 Build Tools (VC 14.16.27023, Hostx64\x64\link.exe) + Windows SDK 10.0.17763; rustup `stable-x86_64-pc-windows-msvc`.
