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

### Registering the hook on Windows

On Windows, Claude Code runs hook commands through **Git Bash** (which it requires), so Faro
reuses the same POSIX reporter as macOS — `agent-monitor-report.sh` — registered as
`bash "<path>"`. A native `.cmd` with a backslash path does **not** work: Git Bash mangles the
backslashes into `command not found`. Requires Git Bash and its bundled `curl`.

**Automated (recommended)** — from Windows PowerShell:

```powershell
powershell -ExecutionPolicy Bypass -File hooks\install-windows.ps1
```

This copies `agent-monitor-report.sh` into `%USERPROFILE%\.claude\hooks\` and merges the seven
hook events into `%USERPROFILE%\.claude\settings.json`, each registered as
`bash "C:/Users/YOU/.claude/hooks/agent-monitor-report.sh"` (non-destructive and idempotent —
re-running also upgrades an older `.cmd` registration — with a `settings.json.faro-bak` backup).
Start a **new** Claude Code session afterward; hooks are read at session start, not live.

**Smoke test** (with the Faro widget running), from **Git Bash** — identical to macOS:

```bash
echo '{"hook_event_name":"UserPromptSubmit","session_id":"smoke","cwd":"/c/x"}' \
  | bash ~/.claude/hooks/agent-monitor-report.sh
curl -s http://127.0.0.1:8765/sessions
```

Expected: a JSON array containing `"sessionId":"smoke"` with `"status":"working"`.

**Manual fallback:** copy `hooks\agent-monitor-report.sh` into `%USERPROFILE%\.claude\hooks\`,
then add a `hooks` block to `settings.json` with the same seven events, each set to
`[{"hooks": [{"type": "command", "command": "bash \"C:/Users/YOU/.claude/hooks/agent-monitor-report.sh\""}]}]`.

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
- **Windows: overlay + reporter/hook work** — overlay parity (current virtual desktop) and the hook reporter (run via Git Bash) are ported; packaging/auto-start (M4) and presence across all virtual desktops are deferred.
- **Overlay anchor does not re-track display changes** — the overlay's top edge locks to its first placement; display resolution/DPI/monitor changes are not re-anchored until restart.
- **Port 8765 is hardcoded** — conflict resolution is future work.
