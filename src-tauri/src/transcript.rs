use serde::Deserialize;

#[derive(Deserialize)]
struct Entry {
    #[serde(rename = "type")]
    kind: Option<String>,
    message: Option<Message>,
    #[serde(default)]
    content: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct Message {
    role: Option<String>,
    #[serde(default)]
    content: Option<serde_json::Value>,
}

fn text_from_content(content: &serde_json::Value) -> Option<String> {
    match content {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(items) => items.iter().find_map(|it| {
            it.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
        }),
        _ => None,
    }
}

/// Normalize a prompt to a single trimmed line, truncated to 60 chars with an ellipsis.
pub fn normalize(text: &str) -> String {
    let one_line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() > 60 {
        let cut: String = one_line.chars().take(59).collect();
        format!("{cut}…")
    } else {
        one_line
    }
}

/// Extract the most recent user prompt from a JSONL transcript file.
/// Returns the normalized prompt, or None on any error.
/// Skips malformed JSON lines and continues searching.
pub fn last_user_prompt(path: &str) -> Option<String> {
    let body = std::fs::read_to_string(path).ok()?;
    let prompt = body
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<Entry>(line).ok())
        .find(|e| {
            e.kind.as_deref() == Some("user")
                || e.message.as_ref().and_then(|m| m.role.as_deref()) == Some("user")
        })
        .and_then(|e| {
            e.message
                .and_then(|m| m.content)
                .or(e.content)
        })
        .and_then(|c| text_from_content(&c))?;
    let n = normalize(&prompt);
    if n.is_empty() { None } else { Some(n) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn normalize_collapses_whitespace() {
        assert_eq!(normalize("  fix\n the   auth bug "), "fix the auth bug");
    }

    #[test]
    fn normalize_truncates_long_text() {
        let long = "a".repeat(100);
        let out = normalize(&long);
        assert_eq!(out.chars().count(), 60); // 59 + ellipsis
        assert!(out.ends_with('…'));
    }

    #[test]
    fn last_user_prompt_finds_type_user() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.jsonl");
        let mut file = fs::File::create(&path).unwrap();
        writeln!(file, r#"{{"type":"assistant","content":"response"}}"#).unwrap();
        writeln!(file, r#"{{"type":"user","content":"fix the auth"}}"#).unwrap();

        let result = last_user_prompt(path.to_str().unwrap());
        assert_eq!(result, Some("fix the auth".to_string()));
    }

    #[test]
    fn last_user_prompt_finds_message_role_user() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.jsonl");
        let mut file = fs::File::create(&path).unwrap();
        writeln!(file, r#"{{"message":{{"role":"assistant"}},"content":"response"}}"#).unwrap();
        writeln!(file, r#"{{"message":{{"role":"user"}},"content":"find bug"}}"#).unwrap();

        let result = last_user_prompt(path.to_str().unwrap());
        assert_eq!(result, Some("find bug".to_string()));
    }

    #[test]
    fn last_user_prompt_handles_array_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.jsonl");
        let mut file = fs::File::create(&path).unwrap();
        writeln!(file, r#"{{"type":"user","content":[{{"type":"text","text":"fix bug"}}]}}"#)
            .unwrap();

        let result = last_user_prompt(path.to_str().unwrap());
        assert_eq!(result, Some("fix bug".to_string()));
    }

    #[test]
    fn last_user_prompt_returns_none_on_missing_file() {
        let result = last_user_prompt("/nonexistent/file.jsonl");
        assert_eq!(result, None);
    }

    #[test]
    fn last_user_prompt_returns_most_recent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.jsonl");
        let mut file = fs::File::create(&path).unwrap();
        writeln!(file, r#"{{"type":"user","content":"first"}}"#).unwrap();
        writeln!(file, r#"{{"type":"user","content":"second"}}"#).unwrap();
        writeln!(file, r#"{{"type":"assistant","content":"response"}}"#).unwrap();

        let result = last_user_prompt(path.to_str().unwrap());
        assert_eq!(result, Some("second".to_string()));
    }

    #[test]
    fn skips_malformed_lines() {
        let path = std::env::temp_dir().join(format!("faro-tx-malformed-{}.jsonl", std::process::id()));
        let jsonl = "{not json at all}\n\
                     {\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"the real prompt\"}}\n\
                     {\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":\"ok\"}}\n\
                     {broken again}\n";
        std::fs::write(&path, jsonl).unwrap();
        assert_eq!(last_user_prompt(path.to_str().unwrap()), Some("the real prompt".into()));
        std::fs::remove_file(&path).ok();
    }
}
