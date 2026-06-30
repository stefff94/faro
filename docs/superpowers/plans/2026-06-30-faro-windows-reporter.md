# Faro Windows Reporter (Hook) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> ⚠️ **SUPERSEDED (2026-06-30) — historical record only.** The `.cmd` reporter design below is obsolete. The e2e gate showed Claude Code on Windows runs hooks via its own **Git Bash**, so a backslash `.cmd` path is mangled into `command not found`. The shipped design reuses the macOS `agent-monitor-report.sh`, registered as `bash "<forward-slash path>"`, and removes the `.cmd`. See the **Addendum** in `docs/superpowers/specs/2026-06-30-faro-windows-reporter-design.md` and the notes in `docs/superpowers/progress-windows-reporter.md`.

**Goal:** Add a native Windows reporter (`.cmd` + `curl`) and an idempotent PowerShell installer that registers the 7 Claude Code hooks, so real Windows sessions feed the Faro broker and populate the widget.

**Architecture:** Purely additive, Windows-only. Two new files in `hooks/`: a `.cmd` reporter (mirror of the existing `.sh`) and a PowerShell installer that copies the reporter into `%USERPROFILE%\.claude\hooks\` and merges the hook events into `settings.json` non-destructively. No Rust/frontend changes; the broker (`127.0.0.1:8765`, already cross-platform and runtime-confirmed on Windows) and `classify` are untouched. macOS `.sh` and its README flow stay as-is.

**Tech Stack:** Windows batch (`.cmd`), built-in `curl.exe`, Windows PowerShell 5.1 (installer + its test). No new runtime dependencies.

## Global Constraints

- **Reporter contract (verbatim from spec):** read the hook payload from **stdin** → POST to the broker → **`exit /b 0` ALWAYS** (must never block or fail a session; critical for `PreToolUse`, which fires before every tool use).
- **Broker URL:** default `http://127.0.0.1:8765/event`, overridable via env var `FARO_BROKER_URL` (same as the `.sh`).
- **Dependency:** `curl.exe` (built into Windows 10 1803+). **Zero new runtime dependencies.**
- **Installer:** non-destructive (preserve all existing `settings.json` keys and non-Faro hooks), **idempotent** (re-run adds no duplicates), writes a **backup** (`settings.json.faro-bak`) before writing, **aborts without writing on malformed JSON**, serializes with **`ConvertTo-Json -Depth 10`** (PS 5.1 truncates at depth 2 by default), and writes **UTF-8 without BOM**.
- **The 7 events (exact names, exact order):** `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `Notification`, `Stop`, `StopFailure`, `SessionEnd`.
- **Hook JSON shape (per event, identical to the macOS README, `command` = the `.cmd` path):** `[{"hooks": [{"type": "command", "command": "<...>\agent-monitor-report.cmd"}]}]`.
- **Installer takes `-ClaudeHome` (default `%USERPROFILE%\.claude`)** so tests run against a temp dir.
- **Build/runtime on this Windows host:** the Faro widget runs via `npm run tauri dev` on the MSVC toolchain. See the project memory "Faro Windows build" (use `RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-msvc`; cargo at `C:\Users\stefano.vannucchi\.cargo\bin`; run cargo/PowerShell natively, not under the bash sandbox).
- **Zero Rust/frontend changes; broker/`classify`/event schema unchanged; macOS `.sh` and flow unchanged.**

## File Structure

- **Create** `hooks/agent-monitor-report.cmd` — the Windows reporter (stdin → curl POST → exit 0).
- **Create** `hooks/install-windows.ps1` — the installer (copy reporter + non-destructive idempotent merge of the 7 hooks into `settings.json`).
- **Create** `hooks/install-windows.tests.ps1` — plain assert-based test for the installer's merge logic (no test framework; runnable via `powershell -File`).
- **Modify** `README.md` — add a "Registering the hook on Windows" subsection (installer + smoke test + manual fallback).
- **Create** `docs/superpowers/progress-windows-reporter.md` — verification ledger (Task 3).

---

## Task 1: Windows reporter (`agent-monitor-report.cmd`)

**Files:**
- Create: `hooks/agent-monitor-report.cmd`

**Interfaces:**
- Consumes: nothing (reads the hook JSON from stdin; the broker is at `FARO_BROKER_URL` or the default).
- Produces: a reporter the installer (Task 2) copies and registers. No code interface.

> Not unit-testable (it is an OS/curl wrapper) — exactly like the existing `.sh`. Verified by the smoke test below (requires the Faro widget/broker running) and the end-to-end gate in Task 3. **Do not invent a fake unit test.**

- [ ] **Step 1: Create the reporter**

Create `hooks/agent-monitor-report.cmd` with EXACTLY this content (note: `--data-binary @-` makes curl read the POST body from stdin, which curl inherits from the `.cmd`, which Claude Code fills with the event JSON):

```bat
@echo off
setlocal
if "%FARO_BROKER_URL%"=="" (set "U=http://127.0.0.1:8765/event") else (set "U=%FARO_BROKER_URL%")
curl.exe -s -m 1 -X POST "%U%" -H "Content-Type: application/json" --data-binary @- >nul 2>&1
exit /b 0
```

- [ ] **Step 2: Smoke test (runtime gate — requires the Faro widget running)**

Start the widget if it is not already up (see Global Constraints / project memory):
```
$env:RUSTUP_TOOLCHAIN = "stable-x86_64-pc-windows-msvc"
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
npm run tauri dev    # leave running; broker binds 127.0.0.1:8765
```
Then, in another PowerShell, pipe a fake event into the reporter and read the snapshot:
```
'{"hook_event_name":"UserPromptSubmit","session_id":"smoke","cwd":"C:/x/proj"}' | & ".\hooks\agent-monitor-report.cmd"
Invoke-RestMethod -Uri http://127.0.0.1:8765/sessions -TimeoutSec 5 | ConvertTo-Json -Depth 6
```
Expected: the `/sessions` snapshot contains a session with `sessionId: "smoke"` and `status: "working"`. (This is the same chain validated manually during the parity work.)

- [ ] **Step 3: Commit**

```
git add hooks/agent-monitor-report.cmd
git commit -m "feat(reporter): native Windows hook reporter (.cmd + curl)"
```

---

## Task 2: Installer + its test (`install-windows.ps1`, `install-windows.tests.ps1`)

**Files:**
- Create: `hooks/install-windows.ps1`
- Create: `hooks/install-windows.tests.ps1`

**Interfaces:**
- Consumes: `hooks/agent-monitor-report.cmd` (Task 1) — the installer copies the `.cmd` that sits next to it (`$PSScriptRoot`).
- Produces: a registered Faro hook setup. The installer signature is `install-windows.ps1 [-ClaudeHome <path>]` (default `%USERPROFILE%\.claude`).

> TDD: write the test first, watch it fail (installer missing), implement the installer, watch it pass. The test exercises the merge logic against a temp `-ClaudeHome` so it never touches the real `settings.json`.

- [ ] **Step 1: Write the failing test**

Create `hooks/install-windows.tests.ps1` with EXACTLY this content:

```powershell
# Plain assert-based test for install-windows.ps1 (no framework). Run: powershell -File hooks\install-windows.tests.ps1
$ErrorActionPreference = "Stop"
$installer = Join-Path $PSScriptRoot "install-windows.ps1"
$script:fail = 0
function Check($cond, $msg) {
  if ($cond) { Write-Host "  ok:   $msg" } else { Write-Host "  FAIL: $msg"; $script:fail++ }
}
function New-TempHome {
  $t = Join-Path ([System.IO.Path]::GetTempPath()) ("faro-test-" + [System.Guid]::NewGuid().ToString("N"))
  New-Item -ItemType Directory -Force -Path $t | Out-Null
  return $t
}
function Run-Installer($claudeHome) {
  & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $installer -ClaudeHome $claudeHome | Out-Null
  return $LASTEXITCODE
}
$events = @("SessionStart","UserPromptSubmit","PreToolUse","Notification","Stop","StopFailure","SessionEnd")

Write-Host "Test 1: preserves existing keys, registers 7 events, copies reporter, makes backup"
$h1 = New-TempHome
$seed = @{
  model = "sonnet"
  enabledPlugins = @{ foo = $true }
  hooks = @{ PreToolUse = @( @{ hooks = @( @{ type = "command"; command = "C:/other/hook.cmd" } ) } ) }
}
$seed | ConvertTo-Json -Depth 10 | Set-Content -Path (Join-Path $h1 "settings.json") -Encoding UTF8
$code = Run-Installer $h1
Check ($code -eq 0) "installer exits 0"
$s = Get-Content (Join-Path $h1 "settings.json") -Raw | ConvertFrom-Json
Check ($s.model -eq "sonnet") "existing key 'model' preserved"
Check ($s.enabledPlugins.foo -eq $true) "existing key 'enabledPlugins' preserved"
foreach ($e in $events) {
  $arr = @($s.hooks.$e)
  $hasFaro = (@($arr | Where-Object { @($_.hooks.command) -like "*agent-monitor-report.cmd*" }).Count -ge 1)
  Check $hasFaro "event $e registers the Faro reporter"
}
$pre = @($s.hooks.PreToolUse)
Check ((@($pre | Where-Object { @($_.hooks.command) -like "*other/hook.cmd*" }).Count) -eq 1) "PreToolUse keeps the pre-existing non-Faro hook"
Check (Test-Path (Join-Path $h1 "hooks\agent-monitor-report.cmd")) "reporter copied into ClaudeHome\hooks"
Check (Test-Path (Join-Path $h1 "settings.json.faro-bak")) "backup created"

Write-Host "Test 2: idempotent re-run (no duplicate Faro groups)"
Run-Installer $h1 | Out-Null
$s2 = Get-Content (Join-Path $h1 "settings.json") -Raw | ConvertFrom-Json
$preFaro = (@(@($s2.hooks.PreToolUse) | Where-Object { @($_.hooks.command) -like "*agent-monitor-report.cmd*" }).Count)
Check ($preFaro -eq 1) "PreToolUse has exactly one Faro group after re-run"
Check ((@($s2.hooks.SessionStart).Count) -eq 1) "SessionStart has exactly one group after re-run"

Write-Host "Test 3: aborts on malformed settings.json without overwriting it"
$h3 = New-TempHome
$bad = "{ this is not json"
Set-Content -Path (Join-Path $h3 "settings.json") -Value $bad -Encoding UTF8
$code3 = Run-Installer $h3
Check ($code3 -ne 0) "installer exits non-zero on malformed settings"
Check (((Get-Content (Join-Path $h3 "settings.json") -Raw).Trim()) -eq $bad) "malformed settings.json left unchanged"

Remove-Item -Recurse -Force $h1, $h3 -ErrorAction SilentlyContinue
if ($script:fail -gt 0) { Write-Host "`n$($script:fail) assertion(s) FAILED"; exit 1 }
Write-Host "`nAll installer tests passed"; exit 0
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File hooks\install-windows.tests.ps1`
Expected: FAIL (the installer file does not exist yet → `Run-Installer` returns a non-zero/`$LASTEXITCODE` from a missing-file error, assertions fail). This confirms the test actually exercises the installer.

- [ ] **Step 3: Implement the installer**

Create `hooks/install-windows.ps1` with EXACTLY this content:

```powershell
[CmdletBinding()]
param(
  [string]$ClaudeHome = (Join-Path $env:USERPROFILE ".claude")
)

$REPORTER = "agent-monitor-report.cmd"
$EVENTS = @("SessionStart","UserPromptSubmit","PreToolUse","Notification","Stop","StopFailure","SessionEnd")

# Deep-convert ConvertFrom-Json output (PSCustomObject) into ordered hashtables/arrays
# so we can merge and re-serialize without losing or reordering existing keys.
function ConvertTo-HashtableDeep($obj) {
  if ($null -eq $obj) { return $null }
  if ($obj -is [System.Management.Automation.PSCustomObject]) {
    $h = [ordered]@{}
    foreach ($p in $obj.PSObject.Properties) { $h[$p.Name] = ConvertTo-HashtableDeep $p.Value }
    return $h
  }
  if ($obj -is [System.Collections.IDictionary]) {
    $h = [ordered]@{}
    foreach ($k in $obj.Keys) { $h[$k] = ConvertTo-HashtableDeep $obj[$k] }
    return $h
  }
  if ($obj -is [System.Collections.IEnumerable] -and $obj -isnot [string]) {
    return @(foreach ($i in $obj) { ConvertTo-HashtableDeep $i })
  }
  return $obj
}

$hooksDir     = Join-Path $ClaudeHome "hooks"
$dest         = Join-Path $hooksDir $REPORTER
$settingsPath = Join-Path $ClaudeHome "settings.json"
$srcReporter  = Join-Path $PSScriptRoot $REPORTER

if (-not (Test-Path $srcReporter)) {
  Write-Host "Faro: reporter not found next to installer: $srcReporter"
  exit 1
}

# 1. Copy the reporter into <ClaudeHome>\hooks
New-Item -ItemType Directory -Force -Path $hooksDir | Out-Null
Copy-Item -Path $srcReporter -Destination $dest -Force

# 2. Load settings (or empty); abort on malformed JSON WITHOUT writing
$settings = [ordered]@{}
if (Test-Path $settingsPath) {
  $raw = Get-Content -Path $settingsPath -Raw
  if (-not [string]::IsNullOrWhiteSpace($raw)) {
    try { $parsed = $raw | ConvertFrom-Json -ErrorAction Stop }
    catch {
      Write-Host "Faro: settings.json is not valid JSON ($settingsPath). Aborting without changes."
      exit 1
    }
    $settings = ConvertTo-HashtableDeep $parsed
  }
}

# 3. Ensure a hooks object
if (-not $settings.Contains("hooks") -or $null -eq $settings["hooks"] -or -not ($settings["hooks"] -is [System.Collections.IDictionary])) {
  $settings["hooks"] = [ordered]@{}
}
$hooks = $settings["hooks"]

# 4. For each event: drop any existing Faro group (idempotency / path refresh), keep
#    every non-Faro group, then append one fresh Faro group.
foreach ($evt in $EVENTS) {
  $kept = @()
  if ($hooks.Contains($evt) -and $hooks[$evt]) {
    foreach ($grp in @($hooks[$evt])) {
      $isFaro = $false
      if (($grp -is [System.Collections.IDictionary]) -and $grp.Contains("hooks") -and $grp["hooks"]) {
        foreach ($hh in @($grp["hooks"])) {
          if (($hh -is [System.Collections.IDictionary]) -and $hh.Contains("command") -and ("$($hh["command"])" -like "*$REPORTER*")) {
            $isFaro = $true
          }
        }
      }
      if (-not $isFaro) { $kept += ,$grp }
    }
  }
  $faroGroup = [ordered]@{ hooks = @( ([ordered]@{ type = "command"; command = $dest }) ) }
  $kept += ,$faroGroup
  $hooks[$evt] = @($kept)
}
$settings["hooks"] = $hooks

# 5. Backup then write (UTF-8 no BOM, depth 10)
if (Test-Path $settingsPath) { Copy-Item -Path $settingsPath -Destination "$settingsPath.faro-bak" -Force }
$jsonOut = $settings | ConvertTo-Json -Depth 10
[System.IO.File]::WriteAllText($settingsPath, $jsonOut, (New-Object System.Text.UTF8Encoding($false)))

Write-Host "Faro: reporter installed to $dest"
Write-Host "Faro: registered $($EVENTS.Count) hook events in $settingsPath (backup: $settingsPath.faro-bak)"
Write-Host "Faro: restart/reload Claude Code so it re-reads settings.json."
exit 0
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File hooks\install-windows.tests.ps1`
Expected: every line prints `ok:` and the script ends with `All installer tests passed` (exit 0). If any event serializes as a JSON object instead of a one-element array, Test 1 still passes via `@()` normalization — additionally open one generated temp `settings.json` (printed path on failure) and confirm each event value is a JSON array `[ {...} ]`; the end-to-end gate (Task 3) is the ultimate confirmation that Claude Code accepts the file.

- [ ] **Step 5: Commit**

```
git add hooks/install-windows.ps1 hooks/install-windows.tests.ps1
git commit -m "feat(installer): Windows hook installer with non-destructive idempotent settings merge"
```

---

## Task 3: README Windows section + end-to-end gate + ledger

**Files:**
- Modify: `README.md`
- Create: `docs/superpowers/progress-windows-reporter.md`

**Interfaces:**
- Consumes: the reporter (Task 1) and installer (Task 2).
- Produces: user-facing docs + the verification ledger. No code interface.

- [ ] **Step 1: Add the Windows section to the README**

In `README.md`, immediately AFTER the macOS smoke-test block (the fenced block that ends with `curl -s http://127.0.0.1:8765/sessions`), insert:

```markdown

### Registering the hook on Windows

Windows uses a native `.cmd` reporter and an installer that edits `settings.json` for you.
Requires `curl.exe` (built into Windows 10 1803+) and Windows PowerShell.

**Automated (recommended):**

```powershell
powershell -ExecutionPolicy Bypass -File hooks\install-windows.ps1
```

This copies `agent-monitor-report.cmd` into `%USERPROFILE%\.claude\hooks\` and merges the seven
hook events into `%USERPROFILE%\.claude\settings.json` (non-destructive, idempotent, with a
`settings.json.faro-bak` backup). Restart/reload Claude Code afterward so it re-reads the file.

**Smoke test** (with the Faro widget running):

```powershell
'{"hook_event_name":"UserPromptSubmit","session_id":"smoke","cwd":"C:/x"}' | & "$env:USERPROFILE\.claude\hooks\agent-monitor-report.cmd"
Invoke-RestMethod http://127.0.0.1:8765/sessions
```

**Manual fallback:** copy `hooks\agent-monitor-report.cmd` into `%USERPROFILE%\.claude\hooks\`,
then add a `hooks` block to `settings.json` with the same seven events, each set to
`[{"hooks": [{"type": "command", "command": "C:\\Users\\YOU\\.claude\\hooks\\agent-monitor-report.cmd"}]}]`.
```

- [ ] **Step 2: Create the progress ledger**

Create `docs/superpowers/progress-windows-reporter.md`:

```markdown
# Faro Windows Reporter (Hook) — Progress Ledger

Spec: docs/superpowers/specs/2026-06-30-faro-windows-reporter-design.md
Plan: docs/superpowers/plans/2026-06-30-faro-windows-reporter.md
Branch: feat/windows-reporter

## Tasks
- [x] Task 1: Windows reporter (agent-monitor-report.cmd)
- [x] Task 2: Installer + tests (install-windows.ps1, install-windows.tests.ps1)
- [x] Task 3: README Windows section + e2e gate + this ledger

## Gates
- [ ] Installer test suite green (install-windows.tests.ps1): preserves keys, 7 events, idempotent, aborts on malformed
- [ ] Reporter smoke test: piped event appears in GET /sessions
- [ ] End-to-end: a REAL Claude Code session on Windows populates the widget

## Notes
- Reporter: .cmd + curl (--data-binary @-), exit /b 0 always. Dependency: builtin curl.exe.
- Installer: UTF-8 no BOM, ConvertTo-Json -Depth 10; backup settings.json.faro-bak.
- No Rust/frontend changes; broker/classify unchanged; macOS .sh unchanged.
- stdin-to-.cmd assumption: confirmed by the smoke test + e2e gate (see spec §7 contingency if it differs).
```

- [ ] **Step 3: End-to-end gate (runtime — the "real sessions" proof)**

1. Run the installer against the real home: `powershell -ExecutionPolicy Bypass -File hooks\install-windows.ps1`.
2. Confirm `%USERPROFILE%\.claude\settings.json` now has the 7 events (and the backup exists).
3. Restart/reload Claude Code so it re-reads settings.
4. Ensure the Faro widget is running (`npm run tauri dev`, MSVC).
5. Start a REAL Claude Code session on Windows and submit a prompt / let it use a tool.
Expected: the widget leaves the idle nub and shows the session as `working` (and escalates to the attention state on a permission prompt). Tick the gate boxes in the ledger as each is confirmed.

- [ ] **Step 4: Commit**

```
git add README.md docs/superpowers/progress-windows-reporter.md
git commit -m "docs: Windows hook registration + reporter progress ledger"
```

---

## Notes for the executor

- **Runtime gates are controller-owned on this Windows host:** the reporter smoke test (Task 1 Step 2) and the end-to-end gate (Task 3 Step 3) need the Faro widget/broker running and (for e2e) a real Claude Code session — run them natively (PowerShell tool), not under the bash sandbox. The installer test suite (Task 2) is fully automated and needs no broker.
- **No `cargo`/Rust work in this plan** — the broker is already built and unchanged. If the widget is not running for the smoke/e2e gates, start it per the project memory ("Faro Windows build").
- **stdin assumption / contingency (spec §7):** if the smoke test shows the piped event does not reach the broker (e.g., Claude Code on Windows runs the hook command differently and stdin does not arrive), adjust ONLY the registered `command` form (e.g. wrap as `cmd /c "<path>"`) and/or how the `.cmd` reads input. Broker, schema, and the merge logic stay the same.
