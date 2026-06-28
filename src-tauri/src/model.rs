use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Idle,
    Working,
    Blocked,
    Done,
    Stale,
    Error,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub id: String,
    pub source: String,
    pub session_id: String,
    pub label: String,
    pub cwd: String,
    pub status: Status,
    pub last_event_name: String,
    pub last_update: i64,
    pub status_since: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<String>,
}

/// Raw hook payload forwarded by the reporter (Claude Code uses snake_case keys).
#[derive(Clone, Debug, Deserialize)]
pub struct HookEvent {
    pub hook_event_name: String,
    pub session_id: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub transcript_path: Option<String>,
    // Validation note §11.b(6): exact discriminator key unconfirmed. Try both.
    #[serde(default)]
    pub notification_type: Option<String>,
    #[serde(default, rename = "type")]
    pub type_field: Option<String>,
}

impl HookEvent {
    /// The Notification discriminator (`permission_prompt` / `idle_prompt` / ...),
    /// trying the candidate keys in priority order.
    pub fn notification_kind(&self) -> Option<&str> {
        self.notification_type
            .as_deref()
            .or(self.type_field.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_state_serializes_camelcase() {
        let s = SessionState {
            id: "claude-code:abc123".into(),
            source: "claude-code".into(),
            session_id: "abc123".into(),
            label: "my-project".into(),
            cwd: "/Users/x/my-project".into(),
            status: Status::Working,
            last_event_name: "PreToolUse".into(),
            last_update: 1719500000000,
            status_since: 1000,
            transcript_path: None,
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"sessionId\":\"abc123\""));
        assert!(json.contains("\"status\":\"working\""));
        assert!(json.contains("\"lastEventName\":\"PreToolUse\""));
        assert!(json.contains("\"statusSince\":1000"));
    }

    #[test]
    fn hook_event_reads_notification_kind_from_either_key() {
        let a: HookEvent = serde_json::from_str(
            r#"{"hook_event_name":"Notification","session_id":"s","notification_type":"permission_prompt"}"#,
        ).unwrap();
        assert_eq!(a.notification_kind(), Some("permission_prompt"));

        let b: HookEvent = serde_json::from_str(
            r#"{"hook_event_name":"Notification","session_id":"s","type":"idle_prompt"}"#,
        ).unwrap();
        assert_eq!(b.notification_kind(), Some("idle_prompt"));
    }
}
