# Faro Distribution & Packaging — Design

**Date:** 2026-06-30
**Status:** Approved (design), pending implementation plan

## Goal

Make Faro installable by non-technical colleagues on Windows and macOS with **no commands to run and no files to edit by hand**, and keep it up to date automatically.

## Architecture

Faro stays the same overlay app but gains four runtime capabilities and one CI pipeline, turning it from a "build-and-run-from-source dev tool" into a self-contained, self-installing, auto-updating tray application distributed as prebuilt installers via GitHub Releases:

1. **In-app hook self-registration** (native Rust) — replaces `install-windows.ps1` and all manual `settings.json` editing.
2. **System tray** — the control surface a chromeless overlay otherwise lacks (status, quit, autostart toggle, re-register hooks, check updates).
3. **Auto-start at login** — `tauri-plugin-autostart`, default ON.
4. **Auto-update** — `tauri-plugin-updater` pulling signed artifacts from GitHub Releases.
5. **CI build/release** — GitHub Actions matrix (Windows + macOS) so macOS builds are produced without owning a Mac.

The colleague experience becomes: download one installer → double-click → click "Attiva" once → done. Updates arrive on their own.

## Tech Stack

- Tauri 2, React 19 + TypeScript, Vite, Rust (axum broker on `127.0.0.1:8765`).
- New crates/plugins: `tauri-plugin-updater`, `tauri-plugin-autostart`, `serde_json` with the `preserve_order` feature, `dirs` (or equivalent) for home resolution.
- CI: `tauri-apps/tauri-action` on GitHub Actions.

## Global Constraints (decisions made during brainstorming)

- **Platforms:** Windows 10/11 **and** macOS (universal binary, Intel + Apple Silicon).
- **OS code signing:** NONE for now (SmartScreen / Gatekeeper bypass documented). The design must not preclude adding Apple notarization + a Windows cert later — deferring signing must require only adding certs/secrets, not re-architecting.
- **Updater signing:** REQUIRED and separate from OS signing. A free minisign (ed25519) keypair is generated via `tauri signer generate`; the public key is committed in `tauri.conf.json`, the private key + password live only as GitHub Actions secrets.
- **Distribution:** GitHub Releases on repo `stefff94/faro`. Release trigger is pushing a `v*` git tag.
- **Auto-update:** enabled, endpoint `https://github.com/stefff94/faro/releases/latest/download/latest.json`.
- **Hook command stability:** the registered hook command must point at a path **outside** the app bundle (`~/.claude/hooks/agent-monitor-report.sh`) so app updates never invalidate it.
- **No silent settings corruption:** malformed `settings.json` aborts the write with no changes; every write is preceded by a `.faro-bak` backup; output is UTF-8 without BOM; existing key order is preserved.
- **The 7 hook events** (unchanged from the PS1): `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `Notification`, `Stop`, `StopFailure`, `SessionEnd`.
- **Idempotency marker:** a Faro hook group is identified by the substring `agent-monitor-report` in its command; re-registration drops existing Faro groups and re-adds one fresh group per event, preserving all non-Faro groups.

---

## Component 1 — In-app hook self-registration (native Rust)

**Where:** new module `src-tauri/src/hooks_install.rs`, called from the `.setup()` closure in `src-tauri/src/lib.rs`.

**Behavior on launch:**
1. Resolve the Claude home directory: `%USERPROFILE%\.claude` on Windows, `$HOME/.claude` on macOS. A `FARO_CLAUDE_HOME` environment override is honored (used by tests). Create the directory (and `hooks/` subdir) if missing.
2. Write the bundled reporter `agent-monitor-report.sh` into `<claude-home>/hooks/`, overwriting to refresh content. The script ships **inside the app bundle** via `bundle.resources` and is read at runtime through `app.path().resource_dir()`.
3. Read `<claude-home>/settings.json` (treat missing/empty as `{}`). If it exists but is not valid JSON → **abort hook registration without writing**, and report the error to the tray/first-run UI.
4. Build the hook command string: `bash "<forward-slash absolute path to the reporter>"` (forward slashes + `bash` prefix so it does not depend on an exec bit a file copy cannot set, and tolerates spaces in the home path).
5. For each of the 7 events: drop any existing Faro group (command contains `agent-monitor-report`), keep every other group, append one fresh Faro group `{ "hooks": [ { "type": "command", "command": <cmd> } ] }`.
6. Back up the existing `settings.json` to `settings.json.faro-bak`, then write the merged result as UTF-8 (no BOM) with key order preserved (`serde_json` `preserve_order`).

This is the exact behavior of `install-windows.ps1` re-expressed in cross-platform Rust, so it works identically on macOS and Windows with no PowerShell dependency.

**Re-assertion:** runs every launch (idempotent), so a moved home path or an updated reporter script is self-healing.

## Component 2 — First-run consent UX

On first launch the app shows a small one-shot window: *"Faro registra gli hook in Claude Code per monitorare le sessioni — [Attiva] [Più tardi]."* A single click on **Attiva** runs Component 1. The window also surfaces the macOS first-open instruction (right-click → Open) when relevant. After the first run, registration is re-asserted silently on each launch. The app detects an already-registered state and reflects it rather than re-prompting.

## Component 3 — System tray

**Where:** new module `src-tauri/src/tray.rs`, wired in `.setup()`.

Tray menu items:
- **Status line** (non-interactive): `● broker in ascolto · hook attivi` / warning states.
- **Avvio automatico al login** — checkable, toggles `tauri-plugin-autostart`.
- **Ripristina hook** — re-runs Component 1 on demand.
- **Controlla aggiornamenti** — triggers Component 4 manually.
- **Esci** — clean quit (the only clean exit path today, given `skipTaskbar: true`).

Tray icon reuses the existing app icon asset. On Windows 11 the icon defaults to the hidden overflow (behind the ∧); users may drag it onto the taskbar.

## Component 4 — Auto-update

**Plugin:** `tauri-plugin-updater`.
**Config (`tauri.conf.json`):** `plugins.updater.endpoints = ["https://github.com/stefff94/faro/releases/latest/download/latest.json"]`, `plugins.updater.pubkey = <committed minisign public key>`.
**Flow:** on startup and on the tray "Controlla aggiornamenti" action, check the manifest; if a newer version exists, download, verify the minisign signature, install, and relaunch.
**Keypair:** generated once with `tauri signer generate`; public key committed; private key + password stored as GitHub Actions secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

## Component 5 — CI build/release pipeline

**Where:** `.github/workflows/release.yml`.
**Trigger:** push of a `v*` tag (e.g. `git tag v0.2.0 && git push --tags`).
**Matrix:**
- `windows-latest` → x64 `.msi` / NSIS setup + updater artifacts.
- `macos-latest` with target `universal-apple-darwin` → universal `.dmg` / `.app.tar.gz` + updater artifacts (covers Intel and Apple Silicon in one download).

Uses `tauri-apps/tauri-action` to build each target, create/update the GitHub Release for the tag, upload the installers, and generate/refresh `latest.json` with minisign signatures. Secrets: the two updater-key secrets + the automatic `GITHUB_TOKEN`.

This is what lets the maintainer, working on Windows, produce macOS builds without a local Mac.

---

## Data Flow (unchanged at runtime, added at install time)

```
Install time (new):
  app launch → hooks_install (Component 1) → ~/.claude/hooks/agent-monitor-report.sh
                                           → ~/.claude/settings.json (7 events registered)

Run time (existing):
  Claude Code hook → bash agent-monitor-report.sh → POST 127.0.0.1:8765/event
                   → SessionStore → sessions-updated → React overlay
```

## Error Handling

- **Malformed `settings.json`:** abort registration, no write, surface error in tray/first-run UI; existing file untouched.
- **Missing Claude home:** create `~/.claude/hooks/` and a fresh `settings.json`.
- **`bash` not resolvable on Windows:** the reporter cannot run, so the app checks at startup whether `bash` is on PATH and, if not, shows `⚠ Git Bash non trovato — hook non operativi` in the tray instead of failing silently. Git for Windows is documented as the single prerequisite (Claude Code needs it anyway).
- **Updater failure:** non-fatal; retried on next launch; manual retry available from the tray.

## Unsigned-app first install (documented, not coded)

- **Windows:** SmartScreen "editore sconosciuto" → *Ulteriori info → Esegui comunque* (once).
- **macOS:** quarantined `.dmg` → **right-click the app → Open** the first time, or `xattr -dr com.apple.quarantine /Applications/Faro.app`.

Documented in `README.md` and in each Release's notes. The first-run window links to these.

## Testing

- **Rust unit tests** for the hook-merge logic (mirroring the existing Pester tests of the PS1), using a temp `FARO_CLAUDE_HOME`:
  - idempotency (re-run does not duplicate Faro groups);
  - non-Faro groups preserved;
  - malformed JSON aborts with no write;
  - `.faro-bak` backup created;
  - existing key order preserved;
  - all 7 events registered with the correct `bash "..."` command form.
- **CI** validated on a throwaway tag (e.g. `v0.0.0-test`) before the first real release.

## Retired / Kept

- `hooks/agent-monitor-report.sh` → **kept** (the reporter, now also bundled as an app resource).
- `hooks/install-windows.ps1` + `hooks/install-windows.tests.ps1` → **retired from the normal flow**, kept in-repo marked *superseded — manual fallback* (zero risk; still usable to register hooks without launching the app).
- Hardcoded broker port `8765` → **out of scope** here (already a documented limitation).

## New / Modified Files

- Create: `src-tauri/src/hooks_install.rs`, `src-tauri/src/tray.rs`, `.github/workflows/release.yml`.
- Modify: `src-tauri/src/lib.rs`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `package.json`, `README.md`.

## Risks

- **macOS unsigned auto-update:** the updater replaces the `.app` in place without re-applying quarantine, so it works in practice, but an unsigned bundle is Tauri's known gray area. Mitigation/escape hatch: add Apple signing + notarization later — the updater keypair and pipeline do not change. Tracked, not blocking.
- **Windows `bash` dependency:** addressed by the startup check + documented prerequisite, but a colleague without Git for Windows gets a non-operative monitor (clearly flagged, not silent).

## Out of Scope

- OS code signing / notarization (deferred; design stays compatible).
- Configurable broker port.
- Linux packaging.
