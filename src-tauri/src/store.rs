use std::collections::HashMap;

use crate::classify::{classify, Transition};
use crate::model::{HookEvent, SessionState, Status};

pub fn label_from_cwd(cwd: Option<&str>) -> String {
    cwd.and_then(|c| c.trim_end_matches('/').rsplit('/').next())
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
                    transcript_path: event.transcript_path.clone(),
                });
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
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }

    /// Transition Working sessions to Stale if they exceed the TTL.
    /// Only Working sessions are affected; other statuses are left unchanged.
    pub fn mark_stale(&mut self, now_ms: i64, ttl_ms: i64) {
        for session in self.sessions.values_mut() {
            if session.status == Status::Working && now_ms - session.last_update > ttl_ms {
                session.status = Status::Stale;
            }
        }
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
    }

    #[test]
    fn mark_stale_ignores_non_working() {
        let mut s = SessionStore::new();
        s.apply("claude-code", &ev("UserPromptSubmit", "a", Some("/x/p"), None), 1000);
        s.apply("claude-code", &ev("Stop", "a", Some("/x/p"), None), 2000); // Status::Done
        s.apply("claude-code", &ev("Notification", "b", Some("/x/p"), Some("permission_prompt")), 3000); // Status::Blocked

        s.mark_stale(10000, 5000); // ttl_ms=5000, now_ms=10000
        let snap = s.snapshot();
        assert_eq!(snap[0].status, Status::Done);
        assert_eq!(snap[1].status, Status::Blocked);
    }

    #[test]
    fn mark_stale_with_expired_ttl() {
        let mut s = SessionStore::new();
        s.apply("claude-code", &ev("UserPromptSubmit", "a", Some("/x/p"), None), 1000);
        assert_eq!(s.snapshot()[0].status, Status::Working);

        s.mark_stale(7000, 5000); // now_ms=7000, ttl_ms=5000, delta=6000 > 5000
        let snap = s.snapshot();
        assert_eq!(snap[0].status, Status::Stale);
    }

    #[test]
    fn mark_stale_with_fresh_session() {
        let mut s = SessionStore::new();
        s.apply("claude-code", &ev("UserPromptSubmit", "a", Some("/x/p"), None), 1000);
        assert_eq!(s.snapshot()[0].status, Status::Working);

        s.mark_stale(4000, 5000); // now_ms=4000, ttl_ms=5000, delta=3000 < 5000
        let snap = s.snapshot();
        assert_eq!(snap[0].status, Status::Working);
    }
}
