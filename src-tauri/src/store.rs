use std::collections::HashMap;

use crate::classify::{classify, Transition};
use crate::git;
use crate::model::{HookEvent, SessionState, Status};
use crate::transcript;

pub fn label_from_cwd(cwd: Option<&str>) -> String {
    cwd.and_then(|c| c.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next())
        .filter(|s| !s.is_empty())
        .unwrap_or("session")
        .to_string()
}

#[derive(Default)]
pub struct SessionStore {
    sessions: HashMap<String, SessionState>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply one hook event. Returns true if the visible snapshot changed.
    pub fn apply(&mut self, source: &str, event: &HookEvent, now_ms: i64) -> bool {
        let id = format!("{}:{}", source, event.session_id);
        match classify(event) {
            Transition::Ignore => false,
            Transition::Remove => self.sessions.remove(&id).is_some(),
            Transition::Set(status) => {
                let entry = self.sessions.entry(id.clone()).or_insert_with(|| SessionState {
                    id: id.clone(),
                    source: source.to_string(),
                    session_id: event.session_id.clone(),
                    label: label_from_cwd(event.cwd.as_deref()),
                    cwd: event.cwd.clone().unwrap_or_default(),
                    status,
                    last_event_name: event.hook_event_name.clone(),
                    last_update: now_ms,
                    status_since: now_ms,
                    transcript_path: event.transcript_path.clone(),
                    branch: None,
                    task_summary: None,
                });
                if entry.status != status {
                    entry.status_since = now_ms;
                }
                entry.status = status;
                entry.last_event_name = event.hook_event_name.clone();
                entry.last_update = now_ms;
                if let Some(cwd) = event.cwd.as_deref() {
                    entry.cwd = cwd.to_string();
                    entry.label = label_from_cwd(Some(cwd));
                }
                if event.transcript_path.is_some() {
                    entry.transcript_path = event.transcript_path.clone();
                }
                true
            }
        }
    }

    pub fn snapshot(&self) -> Vec<SessionState> {
        let mut v: Vec<SessionState> = self.sessions.values().cloned().collect();
        for s in &mut v {
            s.branch = git::branch_for(&s.cwd);
            s.task_summary = s.transcript_path.as_deref().and_then(transcript::last_user_prompt);
        }
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }

    /// Remove sessions untouched for longer than purge_ms, regardless of status.
    pub fn purge(&mut self, purge_ms: i64, now_ms: i64) -> bool {
        let before = self.sessions.len();
        self.sessions.retain(|_, s| now_ms - s.last_update <= purge_ms);
        self.sessions.len() != before
    }

    /// Stale only applies to `working` sessions past the TTL (HANDOFF.md §4 rule).
    pub fn mark_stale(&mut self, ttl_ms: i64, now_ms: i64) -> bool {
        let mut changed = false;
        for s in self.sessions.values_mut() {
            if s.status == Status::Working && now_ms - s.last_update > ttl_ms {
                s.status = Status::Stale;
                s.status_since = now_ms;
                changed = true;
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{HookEvent, Status};

    fn ev(name: &str, sid: &str, cwd: Option<&str>, kind: Option<&str>) -> HookEvent {
        HookEvent {
            hook_event_name: name.into(),
            session_id: sid.into(),
            cwd: cwd.map(|c| c.into()),
            transcript_path: None,
            notification_type: kind.map(|k| k.into()),
            type_field: None,
        }
    }

    #[test]
    fn apply_creates_session_with_derived_label_and_id() {
        let mut s = SessionStore::new();
        let changed = s.apply("claude-code", &ev("UserPromptSubmit", "abc", Some("/Users/x/my-project"), None), 1000);
        assert!(changed);
        let snap = s.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].id, "claude-code:abc");
        assert_eq!(snap[0].label, "my-project");
        assert_eq!(snap[0].status, Status::Working);
        assert_eq!(snap[0].last_update, 1000);
    }

    #[test]
    fn apply_updates_existing_session_in_place() {
        let mut s = SessionStore::new();
        s.apply("claude-code", &ev("UserPromptSubmit", "abc", Some("/x/p"), None), 1000);
        s.apply("claude-code", &ev("Stop", "abc", Some("/x/p"), None), 2000);
        let snap = s.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].status, Status::Done);
        assert_eq!(snap[0].last_update, 2000);
    }

    #[test]
    fn session_end_removes() {
        let mut s = SessionStore::new();
        s.apply("claude-code", &ev("SessionStart", "abc", Some("/x/p"), None), 1000);
        let changed = s.apply("claude-code", &ev("SessionEnd", "abc", Some("/x/p"), None), 1100);
        assert!(changed);
        assert_eq!(s.snapshot().len(), 0);
    }

    #[test]
    fn ignored_event_does_not_change_store() {
        let mut s = SessionStore::new();
        let changed = s.apply("claude-code", &ev("PostToolUse", "abc", Some("/x/p"), None), 1000);
        assert!(!changed);
        assert_eq!(s.snapshot().len(), 0);
    }

    #[test]
    fn two_sessions_are_independent_rows() {
        let mut s = SessionStore::new();
        s.apply("claude-code", &ev("UserPromptSubmit", "a", Some("/x/one"), None), 1000);
        s.apply("claude-code", &ev("Notification", "b", Some("/x/two"), Some("permission_prompt")), 1000);
        let snap = s.snapshot();
        assert_eq!(snap.len(), 2);
        assert_eq!(snap[0].status, Status::Working); // sorted by id: "...:a" before "...:b"
        assert_eq!(snap[1].status, Status::Blocked);
    }

    #[test]
    fn label_falls_back_when_no_cwd() {
        assert_eq!(label_from_cwd(None), "session");
        assert_eq!(label_from_cwd(Some("/Users/x/proj")), "proj");
        assert_eq!(label_from_cwd(Some("C:\\Users\\x\\proj")), "proj");
    }

    #[test]
    fn working_session_goes_stale_after_ttl() {
        let mut s = SessionStore::new();
        s.apply("claude-code", &ev("PreToolUse", "a", Some("/x/p"), None), 1_000);
        assert_eq!(s.snapshot()[0].status, Status::Working);
        let changed = s.mark_stale(90_000, 1_000 + 90_001);
        assert!(changed);
        assert_eq!(s.snapshot()[0].status, Status::Stale);
    }

    #[test]
    fn blocked_and_done_never_go_stale() {
        let mut s = SessionStore::new();
        s.apply("claude-code", &ev("Stop", "a", Some("/x/p"), None), 1_000);
        s.apply("claude-code", &ev("Notification", "b", Some("/x/p"), Some("permission_prompt")), 2_000);
        let changed = s.mark_stale(90_000, 1_000 + 1_000_000);
        assert!(!changed);
        let snap = s.snapshot();
        assert_eq!(snap[0].status, Status::Done);
        assert_eq!(snap[1].status, Status::Blocked);
    }

    #[test]
    fn working_within_ttl_stays_working() {
        let mut s = SessionStore::new();
        s.apply("claude-code", &ev("PreToolUse", "a", Some("/x/p"), None), 1_000);
        assert_eq!(s.snapshot()[0].status, Status::Working);
        let changed = s.mark_stale(90_000, 1_000 + 50_000);
        assert!(!changed);
        assert_eq!(s.snapshot()[0].status, Status::Working);
    }

    #[test]
    fn purge_removes_long_dead_sessions() {
        let mut s = SessionStore::new();
        s.apply("claude-code", &ev("Stop", "a", Some("/x/p"), None), 1_000);
        let changed = s.purge(1_800_000, 1_000 + 1_800_001);
        assert!(changed);
        assert_eq!(s.snapshot().len(), 0);
    }

    #[test]
    fn purge_keeps_recent_sessions() {
        let mut s = SessionStore::new();
        s.apply("claude-code", &ev("Stop", "a", Some("/x/p"), None), 1_000);
        assert!(!s.purge(1_800_000, 1_000 + 60_000));
        assert_eq!(s.snapshot().len(), 1);
    }

    #[test]
    fn status_since_updates_only_when_status_changes() {
        let mut s = SessionStore::new();
        s.apply("claude-code", &ev("UserPromptSubmit", "a", Some("/x/p"), None), 1_000); // Working
        assert_eq!(s.snapshot()[0].status_since, 1_000);
        // same status (still Working) at a later time → status_since unchanged
        s.apply("claude-code", &ev("PreToolUse", "a", Some("/x/p"), None), 2_000);
        assert_eq!(s.snapshot()[0].status_since, 1_000);
        assert_eq!(s.snapshot()[0].last_update, 2_000);
        // status change (Working → Done) → status_since moves
        s.apply("claude-code", &ev("Stop", "a", Some("/x/p"), None), 3_000);
        assert_eq!(s.snapshot()[0].status_since, 3_000);
    }

    #[test]
    fn snapshot_includes_branch_and_summary_fields_defaulting_none() {
        git::invalidate();
        let mut s = SessionStore::new();
        s.apply("claude-code", &ev("UserPromptSubmit", "a", Some("/nonexistent/cwd"), None), 1_000);
        let snap = s.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].branch, None);
        assert_eq!(snap[0].task_summary, None);
    }
}
