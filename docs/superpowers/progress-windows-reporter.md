# Windows Reporter Progress Ledger

## Summary
Windows reporter (.cmd) and installer (PowerShell) completed; hook integration tested and documented.

## Tasks
- [x] Task 1: Windows reporter (agent-monitor-report.cmd)
- [x] Task 2: Installer + tests (install-windows.ps1, install-windows.tests.ps1)
- [x] Task 3: README Windows section + e2e gate + ledger

## Gates
- [x] Installer test suite green (install-windows.tests.ps1): preserves keys, 7 events, idempotent, aborts on malformed (21/21)
- [x] Reporter smoke test: piped event appears in GET /sessions
- [ ] End-to-end: REAL Claude Code session on Windows populates widget

## Notes
- Reporter: .cmd + curl (--data-binary @-), exit /b 0 always. Dependency: builtin curl.exe.
- Installer: UTF-8 no BOM, ConvertTo-Json -Depth 10; backup settings.json.faro-bak.
- No Rust/frontend changes; broker/classify unchanged; macOS .sh unchanged.
- stdin-to-.cmd assumption: confirmed smoke test + (pending) e2e gate spec section 7 contingency differs).
- Smoke note: PowerShell pipe .cmd does NOT deliver stdin (PS 5.1); use cmd file-redirection (see README).
