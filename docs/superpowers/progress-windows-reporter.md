# Faro Windows Reporter (Hook) — Progress Ledger

Spec: docs/superpowers/specs/2026-06-30-faro-windows-reporter-design.md
Plan: docs/superpowers/plans/2026-06-30-faro-windows-reporter.md
Branch: feat/windows-reporter

## Tasks
- [x] Task 1: Windows reporter (agent-monitor-report.cmd)
- [x] Task 2: Installer + tests (install-windows.ps1, install-windows.tests.ps1)
- [x] Task 3: README Windows section + e2e gate + this ledger

## Gates
- [x] Installer test suite green (install-windows.tests.ps1): preserves keys, 7 events, idempotent, aborts on malformed (21/21)
- [x] Reporter smoke test: piped event appears in GET /sessions
- [ ] End-to-end: a REAL Claude Code session on Windows populates the widget

## Notes
- Reporter: .cmd + curl (--data-binary @-), exit /b 0 always. Dependency: builtin curl.exe.
- Installer: UTF-8 no BOM, ConvertTo-Json -Depth 10; backup settings.json.faro-bak.
- No Rust/frontend changes; broker/classify unchanged; macOS .sh unchanged.
- stdin-to-.cmd assumption: confirmed by the smoke test + (pending) e2e gate (see spec section 7 contingency if it differs).
- Smoke note: PowerShell pipe to a .cmd does NOT deliver stdin (PS 5.1); use cmd file-redirection (see README).
