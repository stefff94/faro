# Faro — Windows Reporter (Hook) — Design

**Data:** 2026-06-30
**Branch:** feat/windows-reporter (da `main`)
**Stato:** in design (brainstorming) — da approvare, poi pianificare
**Ambito:** portare il **reporter/hook su Windows** + un **installer automatico**, così le sessioni Claude Code reali su Windows alimentano il widget. NON copre auto-start del widget, packaging/.msi, né un installer automatico per macOS.

> Obiettivo: chiudere l'ultimo anello della "Windows parity" funzionale — oggi su Windows il widget resta al nub idle perché nessun reporter inoltra gli eventi al broker. Con questo pezzo le sessioni vere popolano il widget come su macOS.

---

## 1. Contesto

Dopo la Windows parity dell'overlay (mergiata in `main` @ `d4a88a8`), il **broker** (axum, `127.0.0.1:8765`, `POST /event` + `GET /sessions`) è cross-platform e **confermato funzionante su Windows** (provato a runtime: 3 sessioni iniettate a mano hanno popolato il widget; la branch detection di `git.rs` — fixata in `d4a88a8` con `Path::join` — ha risolto `"branch":"main"`).

Ciò che alimenta il broker è il **reporter**: oggi `hooks/agent-monitor-report.sh` (bash). Contratto: legge il payload dell'hook (JSON su **stdin**) → `POST` al broker via `curl` → **exit 0 sempre** (1s di timeout, in background) così non può mai bloccare o rompere una sessione Claude Code. Registrazione (solo macOS, manuale): copia in `~/.claude/hooks/` + merge di **7 eventi** in `~/.claude/settings.json`.

Su Windows non esiste reporter né registrazione → il widget non riceve dati. Questo design colma il gap. **Nessuna modifica a Rust/frontend**: broker, `classify`, schema eventi restano invariati.

### Ambiente Windows verificato (host corrente)
- `curl.exe` nativo presente: `C:\WINDOWS\system32\curl.exe` (curl 8.18). Builtin da Windows 10 1803+.
- **Solo Windows PowerShell 5.1** (`powershell.exe`); `pwsh` (PS7) assente.
- Il `bash.exe` su PATH è quello di **WSL** (`System32\bash.exe`), non Git Bash → riuso diretto dello `.sh` non affidabile.
- `~/.claude/settings.json` esiste (plugin, `model`, ecc.) ma **senza** blocco `hooks`; nessuna cartella `~/.claude/hooks`.

---

## 2. Obiettivi

1. **Reporter Windows nativo** che rispetta il contratto: stdin JSON → POST al broker → **exit 0 sempre** (non blocca/rompe la sessione; critico per `PreToolUse` che scatta ad ogni tool-use).
2. **Installer automatico** (Windows PowerShell 5.1) che (a) copia il reporter in `%USERPROFILE%\.claude\hooks\` e (b) fa il **merge non distruttivo** dei 7 eventi in `settings.json`, **idempotente**, con **backup**.
3. **Documentazione README** per Windows (installer + smoke test + fallback manuale).
4. **Zero modifiche** a Rust/frontend; broker/classify/schema invariati; **zero nuove dipendenze** runtime (curl è builtin).
5. **Parità funzionale:** una sessione Claude Code reale su Windows popola il widget come su macOS.

---

## 3. Non-obiettivi

- **Auto-start del widget** al login.
- **Packaging/installer .msi** del widget (oggi gira via `npm run tauri dev`).
- **Installer automatico per macOS** (macOS resta col flusso manuale del README attuale).
- Modifiche a broker / `classify` / schema eventi / frontend.
- Uninstaller (idempotenza del re-run è sufficiente per v1; eventuale `-Uninstall` è follow-up).

---

## 4. Architettura — componenti

Tutto additivo: **2 file nuovi** in `hooks/` + una sezione README. Lo `.sh` macOS resta invariato.

### 4.A Reporter — `hooks/agent-monitor-report.cmd`

Specchio funzionale dello `.sh`, nativo Windows via `curl.exe`:

```bat
@echo off
setlocal
if "%FARO_BROKER_URL%"=="" (set "U=http://127.0.0.1:8765/event") else (set "U=%FARO_BROKER_URL%")
curl.exe -s -m 1 -X POST "%U%" -H "Content-Type: application/json" --data-binary @- >nul 2>&1
exit /b 0
```

- **stdin → curl:** `--data-binary @-` legge il payload da stdin (curl eredita lo stdin del `.cmd`, che è il JSON dell'evento passato da Claude Code). `--data-binary` (non `-d`) invia il corpo grezzo senza manipolare i newline.
- **Non blocca:** su loopback la POST è ~10-30ms quando il broker è su (risposta `200` immediata); quando è giù, `connect` a `127.0.0.1:8765` dà RST istantaneo → curl esce subito. Il cap `-m 1` copre solo il caso raro "broker connesso ma appeso".
- **`exit /b 0` sempre:** l'esito di curl non si propaga mai → l'hook non può far fallire/bloccare l'azione di Claude Code.
- **Override:** stesso `FARO_BROKER_URL` dello `.sh`.

**Scelta (.cmd + curl) vs alternative considerate:**
- *PowerShell `.ps1`*: scartato per la **latenza di avvio di PS 5.1 (~200-400ms) ad ogni evento** (pesa su `PreToolUse`).
- *Riuso dello `.sh` via Git Bash*: scartato per **dipendenza dura** da Git for Windows + fragilità del path (`bash.exe` su PATH è WSL).
- *curl diretto in `settings.json` senza wrapper*: scartato perché **non garantisce exit 0** (broker giù → curl esce non-zero → un hook `PreToolUse` con exit non-zero può bloccare il tool-use). Il wrapper `.cmd` con `exit /b 0` è la ragione d'essere del file.

**Dipendenza:** `curl.exe` (builtin Windows 10 1803+, 2018). Documentata come prerequisito.

### 4.B Installer — `hooks/install-windows.ps1`

Windows PowerShell 5.1. Idempotente, non distruttivo, con backup. Accetta un parametro opzionale **`-ClaudeHome`** (default `%USERPROFILE%\.claude`) così i gate §6.1/§6.2 girano contro una dir temporanea senza toccare il setup reale. Passi:

1. **Copia reporter:** crea `<ClaudeHome>\hooks\` se assente; copia `agent-monitor-report.cmd` lì. Path installato: `$DEST = <ClaudeHome>\hooks\agent-monitor-report.cmd`.
2. **Carica settings:** legge `<ClaudeHome>\settings.json`. Se assente → oggetto vuoto `{}`. Se presente ma **JSON malformato** → **aborta** con messaggio chiaro, **senza scrivere** (non corrompe il file).
3. **Backup:** prima di scrivere, copia `settings.json` → `settings.json.faro-bak` (sovrascrivibile a ogni run).
4. **Merge `hooks` (non distruttivo + idempotente):** per ciascuno dei **7 eventi** (`SessionStart, UserPromptSubmit, PreToolUse, Notification, Stop, StopFailure, SessionEnd`):
   - assicura che `settings.hooks.<Event>` sia un array;
   - **rimuove** eventuali matcher-group il cui comando referenzia il reporter Faro (match per nome file `agent-monitor-report.cmd` nel campo `command`) — così il re-run non duplica e un cambio-path si aggiorna;
   - **aggiunge** il matcher-group Faro fresco: `{"hooks":[{"type":"command","command":"$DEST"}]}`.
   - I matcher-group **non-Faro** in quegli eventi sono preservati. Tutte le altre chiavi top-level di `settings.json` (plugin, `model`, ecc.) sono preservate.
5. **Scrittura:** serializza con profondità sufficiente (`ConvertTo-Json -Depth 10` — PS 5.1 tronca a depth 2 di default: gotcha noto, obbligatorio) e riscrive `settings.json`.
6. **Output:** stampa cosa è stato fatto (reporter copiato, N eventi registrati, path del backup) e ricorda di **riavviare/ricaricare Claude Code** perché rilegga `settings.json`.

**Forma del blocco hooks prodotto** (identica a quella documentata per macOS, con `command` = path del `.cmd`):

```json
{
  "hooks": {
    "SessionStart":     [{"hooks": [{"type": "command", "command": "C:\\Users\\<you>\\.claude\\hooks\\agent-monitor-report.cmd"}]}],
    "UserPromptSubmit": [{"hooks": [{"type": "command", "command": "C:\\Users\\<you>\\.claude\\hooks\\agent-monitor-report.cmd"}]}],
    "PreToolUse":       [{"hooks": [{"type": "command", "command": "..."}]}],
    "Notification":     [{"hooks": [{"type": "command", "command": "..."}]}],
    "Stop":             [{"hooks": [{"type": "command", "command": "..."}]}],
    "StopFailure":      [{"hooks": [{"type": "command", "command": "..."}]}],
    "SessionEnd":       [{"hooks": [{"type": "command", "command": "..."}]}]
  }
}
```

### 4.C README — sezione Windows

Sotto la registrazione hook macOS esistente, aggiungere "Registering the hook on Windows":
- one-liner installer: `powershell -ExecutionPolicy Bypass -File hooks\install-windows.ps1`;
- smoke test Windows (vedi §6);
- fallback manuale (copia del `.cmd` + blocco `hooks` con il path del `.cmd`), per chi non vuole l'installer.

---

## 5. Data flow

Identico a macOS, cambia solo il file reporter:

```
sessione Claude Code (Windows)
  → evento hook (SessionStart/UserPromptSubmit/PreToolUse/Notification/Stop/StopFailure/SessionEnd)
  → Claude esegue il comando registrato = agent-monitor-report.cmd, JSON dell'evento su stdin
  → curl POST 127.0.0.1:8765/event
  → broker post_event → store.apply → classify (status: working/blocked/done/…)
  → snapshot → frontend → il widget si aggiorna in tempo reale
```

---

## 6. Gate di verifica

1. **Installer — merge non distruttivo + idempotente (testabile in isolamento):** dato un `settings.json` di esempio con chiavi preesistenti (es. `enabledPlugins`, `model`) ed eventualmente un hook non-Faro, dopo `install-windows.ps1`:
   - tutte le chiavi preesistenti restano invariate;
   - i 7 eventi contengono il matcher-group Faro col path del `.cmd`;
   - eventuali matcher-group non-Faro restano;
   - **re-run → stesso risultato** (nessun duplicato);
   - viene creato `settings.json.faro-bak`.
   Eseguibile passando `-ClaudeHome <tempdir>` (vedi §4.B), così il test non tocca il `settings.json` reale.
2. **Installer — abort su JSON malformato:** dato un `settings.json` non valido, l'installer aborta senza scrivere e senza backup-sovrascrittura distruttiva.
3. **Reporter — smoke test:** con il broker su, pipe di un evento finto nel `.cmd` → `GET /sessions` mostra la sessione (è la catena già provata a mano).
   ```
   echo {"hook_event_name":"UserPromptSubmit","session_id":"smoke","cwd":"C:/x"} | %USERPROFILE%\.claude\hooks\agent-monitor-report.cmd
   curl -s http://127.0.0.1:8765/sessions
   ```
4. **End-to-end (la prova "sessioni vere"):** installer eseguito → widget acceso (`npm run tauri dev`) → **avvio una sessione Claude Code reale su Windows** → il widget si popola da solo (almeno una sessione `working`; un permission-prompt fa salire l'attenzione 🔴).

---

## 7. Assunzione chiave + contingenza

**Assunzione:** Claude Code su Windows esegue il comando hook passando il payload dell'evento su **stdin** (il contratto hook, lo stesso che lo `.sh` consuma con `cat`), e un path a `.cmd` come `command` è eseguibile (Windows instrada i `.cmd` a `cmd.exe`).

**Contingenza (isolata):** se lo smoke test §6.3 o il gate e2e §6.4 mostrano che lo stdin non arriva al `.cmd`, o che la forma del comando va aggiustata (es. serve `cmd /c "<path>"`, oppure Claude su Windows passa il JSON diversamente), si adatta **solo la forma del `command`** registrato (e/o il modo in cui il `.cmd` legge l'input). Broker, schema e merge non cambiano. È l'unico punto di incertezza ed è confinato al reporter+registrazione.

---

## 8. Testing

- **Installer (merge):** logica testabile — opera su un file JSON dato un path di settings parametrizzabile. Gate §6.1/§6.2. È il pezzo con più logica e va coperto.
- **Reporter `.cmd`:** non unit-testabile (è un wrapper su curl/OS); validato dallo smoke test §6.3 e dall'e2e §6.4 (stessa scelta dello `.sh`, anch'esso non unit-testato).
- **Broker/classify:** già coperti (Rust 50/50 su MSVC) e invariati — non ritestare qui.

---

## 9. Prerequisiti (assunzioni, non feature)

- **`curl.exe`** builtin (Windows 10 1803+). Presente sull'host.
- **Windows PowerShell 5.1** per l'installer (sempre presente su Windows).
- Il **widget Faro in esecuzione** (broker su `8765`) per smoke test ed e2e — oggi via `npm run tauri dev` (MSVC; vedi memoria di build Windows).

---

## 10. Criteri di successo (v1)

- Gate §6 (1–4) **verdi** su questo host Windows: installer non distruttivo+idempotente, reporter che inoltra, e una **sessione Claude Code reale popola il widget**.
- **Diff contenuto:** 1 reporter `.cmd` + 1 installer `.ps1` + sezione README. **Zero** modifiche Rust/frontend, **zero** nuove dipendenze runtime.
- **Zero regressioni macOS:** lo `.sh` e il flusso macOS restano invariati; il nuovo codice è solo additivo per Windows.
