# Faro

Faro is a lightweight desktop overlay widget for macOS and Windows that shows a real-time status indicator for every active Claude Code session on your machine. It sits transparently in the top-right corner of your screen and updates as Claude Code moves through idle → working → blocked → done states.

---

## Requirements

**macOS**
- macOS 12+
- [Rust](https://rustup.rs/) (1.70+)
- Node.js 20+ (`nvm use 24`)
- `curl` (pre-installed)

**Windows**
- Windows 10/11
- [Rust](https://rustup.rs/) with the MSVC toolchain:
  `rustup toolchain install stable-x86_64-pc-windows-msvc`
- Visual Studio Build Tools — "Desktop development with C++" workload (`link.exe` + Windows SDK)
- Node.js 20+
- Git for Windows (provides Git Bash and `curl`, required for hook execution)

---

## Run

```bash
npm install
npm run tauri dev
```

The overlay appears top-right. The broker listens on `127.0.0.1:8765`.

---

## Hook registration

Faro receives events from Claude Code via `hooks/agent-monitor-report.sh`, which forwards every hook payload to the broker.

### macOS

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

### Windows

Claude Code runs hooks through **Git Bash**, so the same POSIX script is used on Windows, registered as `bash "<forward-slash-path>"`. A native `.cmd` with backslash paths does **not** work.

**Automated (recommended)** — from PowerShell:

```powershell
powershell -ExecutionPolicy Bypass -File hooks\install-windows.ps1
```

This copies `agent-monitor-report.sh` to `%USERPROFILE%\.claude\hooks\` and merges the seven hook events into `%USERPROFILE%\.claude\settings.json` (non-destructive, idempotent — creates a `settings.json.faro-bak` backup). Start a **new** Claude Code session afterward; hooks are read at session start.

**Smoke test** — from **Git Bash**:

```bash
echo '{"hook_event_name":"UserPromptSubmit","session_id":"smoke","cwd":"/c/x"}' \
  | bash ~/.claude/hooks/agent-monitor-report.sh
curl -s http://127.0.0.1:8765/sessions
```

Expected: JSON array containing `"sessionId":"smoke"` with `"status":"working"`.

**Manual fallback:** Copy `hooks\agent-monitor-report.sh` to `%USERPROFILE%\.claude\hooks\`, then add the seven events to `settings.json`, each registered as:
`[{"hooks": [{"type": "command", "command": "bash \"C:/Users/YOU/.claude/hooks/agent-monitor-report.sh\""}]}]`

---

## Session cards

Each active session is shown as a card with two rows:

- **Row 1:** project name (last segment of the working directory) and elapsed time in current status
- **Row 2:** status chip and git branch (if detected)
- **Row 3 (optional):** last user prompt summary

When 7 or more sessions are active, cards switch to compact mode — reduced padding, no summary row — so all sessions remain visible without scrolling.

Hovering over a card shows the full working directory path as a tooltip. Clicking a card opens the session detail panel with status, last event, full path, and quick actions (mute, pin to top, archive).

---

## Status chips

| Chip | Meaning |
|------|---------|
| `● working` | Claude is processing or using a tool |
| `◆ input` | Waiting for a tool-use permission prompt |
| `◌ stale` | Was working; no event received within the timeout |
| `✓ done` | Turn complete |
| `· idle` | No recent activity |
| `◆ error` | Session ended with failure |

The overlay collapses to a right-edge count pill. The pill escalates to an evident attention state when any session is waiting for input, then decays to a compact persistent reminder until the prompt is answered.

---

## Known limitations

- **Only detects tool-permission prompts** — not plain-text approval questions or plan-mode confirmations. The `Notification` discriminator key is unconfirmed; Faro tries both `notification_type` and `type` fields.
- **No auto-start** — must run `npm run tauri dev` manually each session. Packaging and installer are future work.
- **Overlay anchor does not re-track display changes** — the overlay locks to its first placement; resolution, DPI, or monitor changes require a restart to re-anchor.
- **Port 8765 is hardcoded** — conflict resolution is future work.
- **VS Code extension never reaches `done`** — the Claude Code VS Code extension does not fire the `Stop` hook ([#40029](https://github.com/anthropics/claude-code/issues/40029), [#49851](https://github.com/anthropics/claude-code/issues/49851)). Sessions started from the extension chat panel go `working` → `stale` and never show `done`. Run Claude from the **integrated terminal** (`claude`) for accurate status.
- **Windows: all-virtual-desktops presence not ported** — the overlay is visible on the active desktop only; macOS "all spaces" behaviour is not replicated on Windows.
