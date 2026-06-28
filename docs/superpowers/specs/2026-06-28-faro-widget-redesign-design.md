# Faro — Redesign del widget (PRD / Design)

**Data:** 2026-06-28
**Stato:** approvato per Fase 1 · Fase 2 da disegnare e pianificare separatamente
**Ambito:** macOS (v0). Windows resta lavoro additivo futuro.

---

## 1. Contesto e problema

Faro è un widget menubar/overlay per macOS che mostra lo stato di ogni sessione
Claude Code attiva sulla macchina, aggiornato in tempo reale via hook → broker HTTP
(`127.0.0.1:8765`) → evento Tauri `sessions-updated` → UI React.

Lo stato attuale dell'UI è un prototipo: finestra trasparente 220×120 ancorata in
alto a destra (`position_top_right` in `src-tauri/src/lib.rs`), che elenca righe
`semaforo a 3 pallini + label`. Tre problemi:

1. **Estetica** da rifare: il semaforo a 3 pallini è povero; nessun materiale/identità.
2. **Contenuto informativo** insufficiente: il solo nome progetto non distingue più
   sessioni Claude Code sullo **stesso** repo.
3. **Nessun modello di presenza/attenzione**: non si collassa, non ha una linguetta
   riassuntiva sempre presente, non "fa notare" quando una sessione finisce o si blocca.

Faro è (e va trattato come) un **prodotto** usato da molti: deve adattarsi da 1 a molte
sessioni senza degradare.

## 2. Obiettivi

- Un widget **bello e nativo** (vetro macOS) con un'identità visiva coerente.
- Una **linguetta riassuntiva sempre presente**, sottile a riposo, che si espande
  in un pannello completo su interazione.
- Un **modello di attenzione** che resta calmo quando tutto fila e diventa
  inequivocabile quando serve l'input dell'utente — senza usare le notifiche di sistema.
- **Contenuto per sessione** che disambigua più sessioni sullo stesso progetto.
- Scalabilità **1 → N** sessioni.

## 3. Non-obiettivi (Fase 1)

- Notifiche native di macOS, badge persistenti di sistema (esclusi per scelta: il
  widget stesso è la superficie di notifica).
- Skin/posizionamento nel **notch** → Fase 2.
- "Porta il terminale in primo piano" (mappatura sessione → finestra OS) → Fase 2.
- Sintesi "intelligente"/LLM del task → Fase 2 (Fase 1 usa l'ultimo prompt utente).
- Packaging, auto-start, porting Windows.

## 4. Decisioni di design (validate)

### 4.1 Forma e posizionamento

- **Forma canonica:** tendina ancorata al **bordo destro** dello schermo.
  - **Collassata:** pillola in vetro (vedi 4.3) agganciata al bordo destro
    (angoli arrotondati solo a sinistra: `border-radius: 12px 0 0 12px`).
  - **Peek (hover):** il pannello completo scivola fuori verso sinistra finché il
    mouse è sopra; rientra all'uscita.
  - **Pin (click):** il pannello resta aperto; click esterno o ri-click → richiude.
- **Adattivo per numero schermi:** funziona con/senza notch, su monitor esterni,
  a qualsiasi risoluzione. È il motivo per cui la tendina (non il notch) è la forma base.

**Razionale notch-vs-tendina.** Il notch è disponibile solo su MacBook recenti, mentre
l'utente lavora anche su monitor esterni; lo spazio orizzontale del notch mal si concilia
con "molte sessioni"; la menubar è già contesa. Quindi: tendina come forma universale,
notch come *enhancement* opzionale (Fase 2) che riusa lo stesso modello dati.

### 4.2 Look (estetica)

Combinazione **"A + C"**:

- **Materiale (da A): vetro smerigliato** con vibrancy macOS — sfondo
  `linear-gradient(160deg, rgba(28,30,38,.86), rgba(18,19,26,.9))` + `backdrop-filter: blur(20px)`,
  bordo `1px rgba(255,255,255,.08)`, ombra morbida. **Opacità coerente** in tutti gli stati.
- **Linguaggio di stato (da C): chip-pillola con icona + alone colorato** per card,
  al posto del semaforo a 3 pallini.

Palette stato:

| Stato | Colore | Chip |
|------|--------|------|
| working | giallo `#f5c518` | `● working` |
| needs-input (blocked) | rosso `#ef4444` | `◆ input` |
| done | verde `#22c55e` | `✓ done` |
| idle / stale | grigio `#6b7280` | dim / nub |
| error | rosso (blink) | `◆ error` |

Tipografia: `-apple-system / SF Pro` per l'UI; `ui-monospace / SF Mono` per branch e path.

### 4.3 Linguetta collassata — "pillola conteggi"

A riposo la linguetta **aggrega per stato** mostrando i conteggi (es. `◆1 ●2 ✓1`)
invece di un elemento per sessione. Motivo: scala a 20+ sessioni senza diventare una
fila infinita di puntini. Ordine verticale: needs-input in cima, poi working, poi done.

### 4.4 Modello di attenzione (macchina a stati della linguetta)

Stati aggregati e relativa resa:

- **`idle` / `all-done`** → si ritira a un **nub minimo** (verde tenue, opacità ridotta).
  Nessun suono. Esce dai pensieri.
- **`working`** (nessuno bloccato) → **conteggi calmi**, il giallo "respira"
  (animazione opacità lenta). Presenza discreta. Nessun suono.
- **`needs-input`** (≥1 sessione bloccata in attesa di input) → **escalation inversa**:
  1. **Parte EVIDENTE** (comportamento "notifica"): la pillola si **espande**, pulsa
     rosso, **sporge un peek** della sessione che reclama (`progetto` + riga task),
     e suona il cue "serve input".
  2. **Dopo ~8s (configurabile) DECADE a compatta** (bordo rosso sottile + ◆ in cima
     ai conteggi) ma **resta segnalato** finché l'utente non se ne occupa
     (promemoria persistente, non invasivo).
- **`done` appena avvenuto** → cue "completato" (suono distinto da needs-input);
  poi confluisce nel conteggio `✓`.

Suoni: due cue distinti ("completato" vs "serve input"). Timer di decadimento e
volumi/abilitazione suoni **configurabili** (impostazioni — UI minima, vedi 7).

### 4.5 Contenuto per sessione (pannello espanso)

Ogni card mostra:

- **Progetto · branch** — `cwd` basename + branch git. Disambigua sessioni sullo
  stesso repo (es. `faro feat/notch-hud` vs `faro main`).
- **Task summary** — cosa sta facendo la sessione (Fase 1: ultimo prompt utente troncato).
- **Chip di stato** (4.2).
- **Tempo-in-stato** — da quanto è in quello stato (es. `1m 12s`, `22s`, `ora`).

Ordinamento: needs-input → working → done → idle/stale, poi per `lastUpdate` desc.

### 4.6 Interazioni

- **Espandi dettaglio nel widget** (Fase 1): click su una card → mostra info estese
  (ultimo messaggio, branch, path completo, cronologia/durata stati).
- **Azioni rapide** (Fase 1): silenzia notifiche per quella sessione · fissa in cima ·
  archivia/nascondi sessione finita.
- **Porta il terminale in primo piano** (Fase 2): vedi 8.

## 5. Dati nuovi da raccogliere

Il modello attuale (`SessionState`: `id, source, sessionId, label, cwd, status,
lastEventName, lastUpdate, transcriptPath`) non basta. Aggiunte:

1. **`branch`** — letto dal `cwd` della sessione (`git rev-parse --abbrev-ref HEAD`,
   o lettura di `.git/HEAD`), con cache per evitare di rilanciare git a ogni evento.
   Gestire: repo assente (mostra solo progetto), detached HEAD, worktree.
2. **`taskSummary`** — derivato dal `transcriptPath`. Fase 1: ultimo prompt utente,
   normalizzato e troncato (~60 char). Gestire: transcript assente/illeggibile → vuoto.
3. **`statusSince`** — timestamp dell'ultimo **cambio** di stato (non `lastUpdate`).
   Da calcolare nello store quando lo stato transita; alimenta "tempo-in-stato".

Questi tre arricchimenti vivono lato broker/store (Rust) o nel layer di classificazione
esistente (`classify.rs` / `store.rs` / `model.rs`); il dettaglio implementativo è
delegato al piano. Vincolo: nessuna chiamata bloccante sul path dell'evento HTTP
(usare cache / task asincroni).

## 6. Architettura (impatto)

```
hook → broker HTTP (axum, :8765)
        └─ store (SessionStore) ──┐
            + branch (cache)       │ snapshot arricchito
            + taskSummary          │
            + statusSince          │
                                   ▼
        on_change → emit "sessions-updated" → React UI
                                              ├─ <CollapsedPill>  (macchina attenzione 4.4)
                                              ├─ <DrawerPanel>    (peek/pin 4.1)
                                              │    └─ <SessionCard> (contenuto 4.5)
                                              ├─ <SessionDetail>  (espanso 4.6)
                                              └─ audio cues
```

- **Posizionamento finestra (Rust):** la finestra diventa un overlay alto e stretto
  ancorato al bordo destro; larghezza che varia tra stato collassato ed espanso
  (ridimensionamento/animazione della webview, oppure finestra a larghezza max con
  contenuto allineato a destra e zone trasparenti cliccabili-attraverso). Il piano
  sceglierà l'approccio Tauri concreto (`set_size`/`ignore_cursor_events` per le
  aree trasparenti).
- **UI React:** sostituire `App.tsx` + `SessionRow`/`TrafficLight` con i componenti
  sopra; isolare la macchina-attenzione in un hook dedicato testabile.

## 7. Impostazioni (Fase 1, minime)

Persistite localmente: timer di decadimento `needs-input` (default ~8s), suoni
on/off + scelta cue, silenziamento per-sessione. UI: pannello minimale accessibile
dal pannello espanso (es. icona ingranaggio). Niente over-engineering.

## 8. Handoff — Fase 2 (DA DISEGNARE, POI PIANIFICARE)

> La Fase 2 **non** è specificata qui: va prima disegnata (brainstorming/PRD dedicato)
> e poi pianificata. Questo è l'elenco dei temi da affrontare.

1. **Skin notch.** Pillola attorno/sotto la tacca su Mac che la possiedono, abilitabile
   da impostazioni; riusa la macchina-attenzione e il modello dati della Fase 1.
   Da disegnare: rilevamento geometria notch e robustezza tra versioni macOS,
   transizioni espansione (stile Dynamic Island), fallback automatico alla tendina su
   schermi senza notch.
2. **Porta il terminale in primo piano.** Click su una sessione → focus della
   finestra/tab di terminale corrispondente. Da disegnare: come mappare
   `sessionId`/`cwd`/`pid` → finestra OS, supporto multiplexer (tmux) e diversi
   terminali (Terminal.app, iTerm, VS Code, Ghostty…), permessi Accessibility.
3. **Task summary "intelligente".** Sintesi del task migliore dell'ultimo-prompt
   (es. riassunto LLM o euristiche sul transcript). Da disegnare: costo/latency,
   privacy (resta locale), quando rinfrescare.
4. **(Opzionale) Windows.** Porting dell'overlay e del posizionamento.

## 9. Criteri di successo (Fase 1)

- Con 1, 5 e 15 sessioni simulate la linguetta resta leggibile e la pillola-conteggi
  aggrega correttamente.
- Due sessioni sullo stesso repo su branch diversi sono distinguibili a colpo d'occhio.
- Una sessione che entra in `needs-input` produce: espansione+peek+cue immediati, poi
  decadimento a compatta dopo il timer, restando segnalata fino alla presa in carico.
- A riposo (tutto idle/done) il widget è discreto (nub) e silenzioso.
- Hover apre il peek; click aggancia; click fuori chiude.
- Nessuna operazione bloccante sul path dell'evento HTTP (git/transcript via cache/async).
