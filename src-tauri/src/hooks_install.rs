
//! Native, cross-platform registration of Faro's Claude Code hooks.
//! Mirrors the behaviour of the retired hooks/install-windows.ps1 in Rust.

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

/// Substring identifying a Faro hook group (any path/form).
pub const FARO_MARK: &str = "agent-monitor-report";

/// The Claude Code hook events Faro registers.
pub const EVENTS: [&str; 7] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "Notification",
    "Stop",
    "StopFailure",
    "SessionEnd",
];

/// The reporter script, embedded at compile time so it ships with the binary
/// and is identical in `tauri dev` and packaged builds.
pub const REPORTER_BODY: &str = include_str!("../../hooks/agent-monitor-report.sh");

const REPORTER_NAME: &str = "agent-monitor-report.sh";

#[derive(Debug)]
pub struct InstallReport {
    pub registered: bool,
    pub backup_made: bool,
    pub error: Option<String>,
}

/// Resolve the Claude home dir: `FARO_CLAUDE_HOME` override, else `~/.claude`.
pub fn claude_home() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("FARO_CLAUDE_HOME") {
        return Some(PathBuf::from(p));
    }
    dirs::home_dir().map(|h| h.join(".claude"))
}

fn is_faro_group(grp: &Value) -> bool {
    grp.get("hooks")
        .and_then(|h| h.as_array())
        .map(|arr| {
            arr.iter().any(|hh| {
                hh.get("command")
                    .and_then(|c| c.as_str())
                    .map(|s| s.contains(FARO_MARK))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// Merge one fresh Faro group per event into `settings`, dropping any existing
/// Faro group and preserving every other group and top-level key.
pub fn merge_faro_hooks(mut settings: Value, command: &str) -> Value {
    if !settings.is_object() {
        settings = json!({});
    }
    let obj = settings.as_object_mut().unwrap();

    let hooks_entry = obj.entry("hooks").or_insert_with(|| json!({}));
    if !hooks_entry.is_object() {
        *hooks_entry = json!({});
    }
    let hooks = hooks_entry.as_object_mut().unwrap();

    for evt in EVENTS {
        let mut kept: Vec<Value> = Vec::new();
        if let Some(arr) = hooks.get(evt).and_then(|v| v.as_array()) {
            for grp in arr {
                if !is_faro_group(grp) {
                    kept.push(grp.clone());
                }
            }
        }
        kept.push(json!({
            "hooks": [ { "type": "command", "command": command } ]
        }));
        hooks.insert(evt.to_string(), Value::Array(kept));
    }
    settings
}

/// Write the reporter into `<claude_home>/hooks/` and register the 7 events in
/// `settings.json`. Aborts without writing if settings.json is malformed.
pub fn install_hooks(claude_home: &Path) -> InstallReport {
    let hooks_dir = claude_home.join("hooks");
    if let Err(e) = fs::create_dir_all(&hooks_dir) {
        return InstallReport { registered: false, backup_made: false, error: Some(format!("create hooks dir: {e}")) };
    }

    let dest = hooks_dir.join(REPORTER_NAME);
    if let Err(e) = fs::write(&dest, REPORTER_BODY.as_bytes()) {
        return InstallReport { registered: false, backup_made: false, error: Some(format!("write reporter: {e}")) };
    }

    let command = format!("bash \"{}\"", dest.to_string_lossy().replace('\\', "/"));

    let settings_path = claude_home.join("settings.json");
    let settings: Value = match fs::read_to_string(&settings_path) {
        Ok(raw) if !raw.trim().is_empty() => match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                return InstallReport { registered: false, backup_made: false, error: Some(format!("settings.json non valido: {e}")) }
            }
        },
        _ => json!({}),
    };

    let merged = merge_faro_hooks(settings, &command);

    let mut backup_made = false;
    if settings_path.exists() {
        let bak = claude_home.join("settings.json.faro-bak");
        if fs::copy(&settings_path, &bak).is_ok() {
            backup_made = true;
        }
    }

    let out = serde_json::to_string_pretty(&merged).unwrap();
    if let Err(e) = fs::write(&settings_path, out.as_bytes()) {
        return InstallReport { registered: false, backup_made, error: Some(format!("write settings: {e}")) };
    }

    InstallReport { registered: true, backup_made, error: None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const CMD: &str = "bash \"/home/u/.claude/hooks/agent-monitor-report.sh\"";

    #[test]
    fn registers_all_seven_events() {
        let out = merge_faro_hooks(json!({}), CMD);
        let hooks = out["hooks"].as_object().unwrap();
        for evt in EVENTS {
            let arr = hooks[evt].as_array().unwrap();
            assert_eq!(arr.len(), 1, "event {evt} should have one group");
            assert_eq!(arr[0]["hooks"][0]["command"], CMD);
            assert_eq!(arr[0]["hooks"][0]["type"], "command");
        }
    }

    #[test]
    fn idempotent_no_duplicate_faro_groups() {
        let once = merge_faro_hooks(json!({}), CMD);
        let twice = merge_faro_hooks(once.clone(), CMD);
        assert_eq!(once, twice);
        for evt in EVENTS {
            assert_eq!(twice["hooks"][evt].as_array().unwrap().len(), 1);
        }
    }

    #[test]
    fn preserves_non_faro_groups() {
        let existing = json!({
            "hooks": { "PreToolUse": [
                { "hooks": [ { "type": "command", "command": "/usr/bin/other-tool" } ] }
            ] }
        });
        let out = merge_faro_hooks(existing, CMD);
        let arr = out["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["hooks"][0]["command"], "/usr/bin/other-tool");
        assert!(arr[1]["hooks"][0]["command"].as_str().unwrap().contains(FARO_MARK));
    }

    #[test]
    fn replaces_stale_faro_group_path() {
        let existing = json!({
            "hooks": { "Stop": [
                { "hooks": [ { "type": "command", "command": "bash \"/old/agent-monitor-report.sh\"" } ] }
            ] }
        });
        let out = merge_faro_hooks(existing, CMD);
        let arr = out["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["hooks"][0]["command"], CMD);
    }

    #[test]
    fn preserves_other_top_level_keys() {
        let out = merge_faro_hooks(json!({ "model": "opus", "hooks": {} }), CMD);
        assert_eq!(out["model"], "opus");
    }

    #[test]
    fn coerces_non_object_settings() {
        let out = merge_faro_hooks(json!(null), CMD);
        assert!(out["hooks"].is_object());
    }

    #[test]
    fn install_creates_script_and_settings() {
        let tmp = tempfile::tempdir().unwrap();
        let rep = install_hooks(tmp.path());
        assert!(rep.registered, "error: {:?}", rep.error);
        assert!(tmp.path().join("hooks/agent-monitor-report.sh").exists());
        let raw = std::fs::read_to_string(tmp.path().join("settings.json")).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(settings["hooks"]["Stop"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn install_aborts_on_malformed_settings() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("settings.json"), "{ not json").unwrap();
        let rep = install_hooks(tmp.path());
        assert!(!rep.registered);
        assert!(rep.error.is_some());
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("settings.json")).unwrap(),
            "{ not json"
        );
    }

    #[test]
    fn install_makes_backup_when_settings_exists() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("settings.json"), "{\"model\":\"opus\"}").unwrap();
        let rep = install_hooks(tmp.path());
        assert!(rep.registered);
        assert!(rep.backup_made);
        assert!(tmp.path().join("settings.json.faro-bak").exists());
    }

    #[test]
    fn install_command_uses_forward_slashes_and_bash() {
        let tmp = tempfile::tempdir().unwrap();
        install_hooks(tmp.path());
        let raw = std::fs::read_to_string(tmp.path().join("settings.json")).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let cmd = settings["hooks"]["Stop"][0]["hooks"][0]["command"].as_str().unwrap();
        assert!(cmd.starts_with("bash \""));
        assert!(!cmd.contains('\\'));
        assert!(cmd.contains("/hooks/agent-monitor-report.sh"));
    }
}
