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
pub fn last_user_prompt(path: &str) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let lines: Vec<&str> = content.lines().collect();

    for line in lines.iter().rev() {
        if line.trim().is_empty() {
            continue;
        }

        let val: serde_json::Value = serde_json::from_str(line).ok()?;

        let is_user = val
            .get("type")
            .and_then(|t| t.as_str())
            .map(|s| s == "user")
            .unwrap_or(false)
            || val
                .get("message")
                .and_then(|m| m.get("role"))
                .and_then(|r| r.as_str())
                .map(|s| s == "user")
                .unwrap_or(false);

        if !is_user {
            continue;
        }

        let text = if let Some(s) = val.get("content").and_then(|c| c.as_str()) {
            Some(s.to_string())
        } else if let Some(arr) = val.get("content").and_then(|c| c.as_array()) {
            arr.iter()
                .find_map(|item| item.get("text").and_then(|t| t.as_str()))
                .map(|s| s.to_string())
        } else {
            None
        };

        if let Some(t) = text {
            return Some(normalize(&t));
        }
    }

    None
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
}
