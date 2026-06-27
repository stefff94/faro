/// A trait for abstracting the source of messages/requests.
/// This allows the HTTP layer (and other components) to be tested independently.
pub trait Source {
    /// Returns a static identifier for this source.
    fn name(&self) -> &'static str;
}

/// The ClaudeCode source implementation.
pub struct ClaudeCodeSource;

impl Source for ClaudeCodeSource {
    fn name(&self) -> &'static str {
        "claude-code"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_code_source_name() {
        let source = ClaudeCodeSource;
        assert_eq!(source.name(), "claude-code");
    }
}
