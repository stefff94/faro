# Faro Windows Reporter (Hook) — Progress Ledger

Spec: docs/superpowers/specs/2026-06-30-faro-windows-reporter-design.md
Plan: docs/superpowers/plans/2026-06-30-faro-windows-reporter.md
Branch: feat/windows-reporter

## Tasks
- [x] Task 1: Windows reporter — reuses the macOS POSIX reporter `agent-monitor-report.sh` (Claude Code runs hooks via Git Bash on Windows; a backslash `.cmd` path is mangled by bash). The standalone `.cmd` was removed.
- [x] Task 2: Installer + tests (install-windows.ps1, install-windows.tests.ps1) — registers `bash "<forward-slash path>"`; migrates old `.cmd` registrations (27/27 assertions)
- [x] Task 3: README Windows section + e2e gate + this ledger

## Gates
- [x] Installer test suite green (install-windows.tests.ps1): preserves keys, 7 events, idempotent, aborts on malformed (21/21)
- [x] Reporter smoke test: piped event appears in GET /sessions
- [x] End-to-end: REAL Claude Code sessions on Windows populate the broker/widget (verified 2026-06-30: Askme-trunk, SV/faro, AskMe Desk sessions live in GET /sessions with real UUIDs + branch)

## Notes
- Executor finding (2026-06-30): Claude Code on Windows runs hook `command` strings via Git Bash
  (`/usr/bin/bash`), NOT cmd.exe. A backslash path like `C:\Users\...\report.cmd` is mangled by
  bash (the `\U`, `\s`, ... are eaten) into `command not found`. The original `.cmd` approach was
  therefore dead on arrival; the fix reuses the macOS POSIX reporter and registers `bash "<fwd path>"`.
- Reporter: the macOS `agent-monitor-report.sh` (curl, exit 0 always); Git Bash provides curl.
  No separate Windows reporter; the `.cmd` was removed.
- Command form: `bash "C:/Users/<you>/.claude/hooks/agent-monitor-report.sh"` — forward slashes +
  `bash` prefix (no dependency on the exec bit, which a PowerShell copy cannot set).
- Installer: UTF-8 no BOM, ConvertTo-Json -Depth 10; backup settings.json.faro-bak; Faro-group
  detection by substring `agent-monitor-report`, so re-running migrates old `.cmd` registrations.
- No Rust/frontend changes; broker/classify unchanged; macOS .sh unchanged.
