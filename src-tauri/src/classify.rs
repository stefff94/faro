use crate::model::{HookEvent, Status};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transition {
    Set(Status),
    Remove,
    Ignore,
}

/// Maps a Claude Code hook event to a status transition. Table is HANDOFF.md §4.
pub fn classify(event: &HookEvent) -> Transition {
    match event.hook_event_name.as_str() {
        "SessionStart" => Transition::Set(Status::Idle),
        "UserPromptSubmit" => Transition::Set(Status::Working),
        "PreToolUse" => Transition::Set(Status::Working),
        "Stop" => Transition::Set(Status::Done),
        "StopFailure" => Transition::Set(Status::Error), // §11.b(8)
        "SessionEnd" => Transition::Remove,
        "Notification" => match event.notification_kind() {
            Some("permission_prompt") => Transition::Set(Status::Blocked),
            Some("idle_prompt") => Transition::Set(Status::Done), // §11.b(7): tunable later
            _ => Transition::Ignore,
        },
        _ => Transition::Ignore,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{HookEvent, Status};

    fn ev(name: &str, kind: Option<&str>) -> HookEvent {
        HookEvent {
            hook_event_name: name.into(),
            session_id: "s".into(),
            cwd: Some("/tmp/proj".into()),
            transcript_path: None,
            notification_type: kind.map(|k| k.into()),
            type_field: None,
        }
    }

    #[test] fn session_start_is_idle()    { assert_eq!(classify(&ev("SessionStart", None)), Transition::Set(Status::Idle)); }
    #[test] fn prompt_is_working()        { assert_eq!(classify(&ev("UserPromptSubmit", None)), Transition::Set(Status::Working)); }
    #[test] fn pretooluse_is_working()    { assert_eq!(classify(&ev("PreToolUse", None)), Transition::Set(Status::Working)); }
    #[test] fn permission_is_blocked()    { assert_eq!(classify(&ev("Notification", Some("permission_prompt"))), Transition::Set(Status::Blocked)); }
    #[test] fn idle_prompt_is_done()      { assert_eq!(classify(&ev("Notification", Some("idle_prompt"))), Transition::Set(Status::Done)); }
    #[test] fn stop_is_done()             { assert_eq!(classify(&ev("Stop", None)), Transition::Set(Status::Done)); }
    #[test] fn stop_failure_is_error()    { assert_eq!(classify(&ev("StopFailure", None)), Transition::Set(Status::Error)); }
    #[test] fn session_end_removes()      { assert_eq!(classify(&ev("SessionEnd", None)), Transition::Remove); }
    #[test] fn unknown_notification_ignored() { assert_eq!(classify(&ev("Notification", Some("auth_success"))), Transition::Ignore); }
    #[test] fn unknown_event_ignored()    { assert_eq!(classify(&ev("PostToolUse", None)), Transition::Ignore); }
}
