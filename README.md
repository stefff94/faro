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

## Install (for colleagues)

Download the latest installer from the [Releases page](https://github.com/stefff94/faro/releases/latest):

- **Windows:** run the `.msi`. SmartScreen may say "unknown publisher" → **More info → Run anyway** (once).
- **macOS:** open the `.dmg`, drag Faro to Applications, then **right-click the app → Open** the first time (it is not yet notarized). Alternatively: `xattr -dr com.apple.quarantine /Applications/Faro.app`.

On first launch Faro shows a one-time card — click **Attiva** and it registers its Claude Code hooks itself. Nothing else to configure. It then lives in the system tray (Windows: behind the ∧ overflow; macOS: menu bar), starts at login, and auto-updates.

**Windows prerequisite:** Git for Windows (Git Bash) must be installed — Claude Code needs it anyway, and Faro's reporter runs through it. If the tray shows "⚠ Git Bash non trovato", install [Git for Windows](https://git-scm.com/download/win).

## Updating

Faro checks GitHub Releases on launch and updates itself. You can also trigger a check from the tray ("Controlla aggiornamenti").

## Building from source (maintainers)

Requirements — macOS: macOS 12+, Rust, Node 20+. Windows: Rust MSVC toolchain, Visual Studio Build Tools (Desktop C++), Node 20+, Git for Windows.

```bash
npm install
npm run tauri dev      # run locally
```

Cutting a release: bump the version, then `git tag vX.Y.Z && git push --tags`. CI builds both platforms and publishes the Release.

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
- **No OS code signing yet** — first launch shows a SmartScreen (Windows) / Gatekeeper (macOS) warning; bypass once as described under Install. Apple notarization + a Windows certificate are future work.
- **Windows requires Git Bash** — the reporter runs via `bash`; without Git for Windows the hooks are not operative (the tray flags this).
- **Overlay anchor does not re-track display changes** — the overlay locks to its first placement; resolution, DPI, or monitor changes require a restart to re-anchor.
- **Port 8765 is hardcoded** — conflict resolution is future work.
- **VS Code extension never reaches `done`** — the Claude Code VS Code extension does not fire the `Stop` hook ([#40029](https://github.com/anthropics/claude-code/issues/40029), [#49851](https://github.com/anthropics/claude-code/issues/49851)). Sessions started from the extension chat panel go `working` → `stale` and never show `done`. Run Claude from the **integrated terminal** (`claude`) for accurate status.
- **Windows: all-virtual-desktops presence not ported** — the overlay is visible on the active desktop only; macOS "all spaces" behaviour is not replicated on Windows.
