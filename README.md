# Faro

Faro is a lightweight macOS menubar widget that shows a traffic-light status indicator for every active Claude Code session on your machine. It sits transparently in the top-right corner of your screen and updates in real time as Claude Code moves through idle → working → blocked → done states.

**Scope (v0):** macOS only. Windows port is additive future work (M4). No packaging or auto-start yet — run manually for now.

### Building on Windows (dev)

Windows builds use the MSVC target. Requirements:
- Visual Studio Build Tools "Desktop development with C++" (provides `link.exe` + Windows SDK).
- `rustup toolchain install stable-x86_64-pc-windows-msvc`, then build with that toolchain
  (e.g. `cargo +stable-x86_64-pc-windows-msvc build`, or set it as the rustup default).

Windows support is in progress (overlay parity); all-virtual-desktops presence is not yet ported.

---

## Requirements

- macOS 12+
- [Rust](https://rustup.rs/) (1.70+)
- Node.js 20+ (use [nvm](https://github.com/nvm-sh/nvm): `nvm use 24`)
- `curl` (pre-installed on macOS)

---

## Run

```bash
npm install
npm run tauri dev
```

The widget appears top-right. The broker listens on `127.0.0.1:8765`.

---

## Hook registration

Faro receives events from Claude Code via a hook script that forwards every hook payload to the broker.

**1. Install the script:**

```bash
mkdir -p ~/.claude/hooks
cp hooks/agent-monitor-report.sh ~/.claude/hooks/agent-monitor-report.sh
chmod +x ~/.claude/hooks/agent-monitor-report.sh
```

**2. Merge into `~/.claude/settings.json`** (do not overwrite — merge the `hooks` block):

```json
{
  "hooks": {
    "SessionStart":     [{"hooks": [{"type": "command", "command": "/Users/YOU/.claude/hooks/agent-monitor-report.sh"}]}],
    "UserPromptSubmit": [{"hooks": [{"type": "command", "command": "/Users/YOU/.claude/hooks/agent-monitor-report.sh"}]}],
    "PreToolUse":       [{"hooks": [{"type": "command", "command": "/Users/YOU/.claude/hooks/agent-monitor-report.sh"}]}],
    "Notification":     [{"hooks": [{"type": "command", "command": "/Users/YOU/.claude/hooks/agent-monitor-report.sh"}]}],
    "Stop":             [{"hooks": [{"type": "command", "command": "/Users/YOU/.claude/hooks/agent-monitor-report.sh"}]}],
    "StopFailure":      [{"hooks": [{"type": "command", "command": "/Users/YOU/.claude/hooks/agent-monitor-report.sh"}]}],
    "SessionEnd":       [{"hooks": [{"type": "command", "command": "/Users/YOU/.claude/hooks/agent-monitor-report.sh"}]}]
  }
}
```

Replace `/Users/YOU` with your actual home path (`echo $HOME`).

**3. Smoke test** (with `npm run tauri dev` running):

```bash
echo '{"hook_event_name":"UserPromptSubmit","session_id":"smoke","cwd":"/tmp/smoke"}' \
  | ~/.claude/hooks/agent-monitor-report.sh
sleep 1
curl -s http://127.0.0.1:8765/sessions
```

Expected: JSON array containing `"sessionId":"smoke"` with `"status":"working"`.

### Registering hook on Windows

Windows uses native `.cmd` reporter installer edits `settings.json` you. Requires `curl.exe` (built into Windows 10 1803+) Windows PowerShell.

**Automated (recommended):**

```powershell
powershell -ExecutionPolicy Bypass -File hooks\install-windows.ps1
```

This copies `agent-monitor-report.cmd` into `%USERPROFILE%\.claude\hooks\` merges seven hook events into `%USERPROFILE%\.claude\settings.json` (non-destructive, idempotent, `settings.json.faro-bak` backup). Restart/reload Claude Code afterward so re-reads file.

**Smoke test** Faro widget running). PowerShell's `'json' | & file.cmd` does **not** deliver stdin `.cmd`, so feed payload via file:

```powershell
'{"hook_event_name":"UserPromptSubmit","session_id":"smoke","cwd":"C:/x"}' | Out-File -Encoding ascii -NoNewline "$env:TEMP\faro-smoke.json"
& "$env:USERPROFILE\.claude\hooks\agent-monitor-report.cmd" < "$env:TEMP\faro-smoke.json"
Start-Sleep -Seconds 1
curl.exe -s http://127.0.0.1:8765/sessions
```

Expected: same as macOS — JSON array with `"sessionId":"smoke"` and `"status":"working"`.

---

## Status chips

| Chip | Meaning |
|------|---------|
| `● working` | Claude is processing / using a tool |
| `◆ input` | Waiting for tool-use permission prompt |
| `✓ done` | Turn complete |
| `· idle` | No recent activity |
| `◆ error` | Session ended with failure |

Faro collapses to a right-edge count pill in the corner of your screen. The pill escalates to an evident attention state when any session reaches `needs-input`, then decays to a compact persistent reminder until the prompt is answered.

---

## Known limitations (v0)

- **🔴 only detects tool-permission prompts** — not plain-text approval questions or plan-mode confirmations (§11.b(7)). The `Notification` discriminator key is unconfirmed; Faro tries both `notification_type` and `type` fields.
- **No auto-start** — must run `npm run tauri dev` manually each session.
- **No installer or packaging** — planned for M4.
- **Windows: overlay only** — the window/overlay behavior has Windows parity (current virtual desktop); the reporter/hook and packaging are not yet ported (M4), and presence across all virtual desktops is deferred.
- **Overlay anchor does not re-track display changes** — the overlay's top edge locks to its first placement; display resolution/DPI/monitor changes are not re-anchored until restart.
- **Port 8765 is hardcoded** — conflict resolution is future work.
