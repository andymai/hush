/// Voice command executor
///
/// This module handles executing parsed voice commands, including:
/// - Text transformations (capitalization, etc.)
/// - Actions that require state (undo/delete)
/// - Building the final output text
use super::commands::{ParsedCommand, VoiceCommand};
use tracing::{debug, info, warn};

/// Result of command execution
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// The final text to insert (if any)
    pub text: Option<String>,

    /// Whether an undo operation was requested
    pub undo_requested: bool,

    /// Whether text should be processed (filler removal, etc.)
    pub should_process: bool,
}

impl ExecutionResult {
    /// Create a result with text to insert
    pub fn insert(text: String) -> Self {
        Self {
            text: Some(text),
            undo_requested: false,
            should_process: true,
        }
    }

    /// Create a result requesting undo
    pub fn undo() -> Self {
        Self {
            text: None,
            undo_requested: true,
            should_process: false,
        }
    }

    /// Create a result with no action
    pub fn no_action() -> Self {
        Self {
            text: None,
            undo_requested: false,
            should_process: false,
        }
    }
}

/// Voice command executor
pub struct CommandExecutor {
    /// Whether command execution is enabled
    enabled: bool,
}

impl CommandExecutor {
    /// Create a new command executor
    pub fn new() -> Self {
        Self { enabled: true }
    }

    /// Create a disabled executor (no-op)
    pub fn disabled() -> Self {
        Self { enabled: false }
    }

    /// Execute parsed commands and build the output
    pub fn execute(&self, parsed: &ParsedCommand) -> ExecutionResult {
        if !self.enabled {
            // If disabled, just return the raw text
            let text = parsed.get_text();
            return if text.is_empty() {
                ExecutionResult::no_action()
            } else {
                ExecutionResult::insert(text)
            };
        }

        debug!("Executing command with {} segments", parsed.segments.len());

        // Check for standalone undo/delete commands
        if parsed.segments.len() == 1 {
            match &parsed.segments[0] {
                VoiceCommand::Undo | VoiceCommand::DeleteThat => {
                    info!("Undo command detected");
                    return ExecutionResult::undo();
                },
                _ => {},
            }
        }

        // Build output text by processing segments
        let mut output = String::new();
        let mut pending_text = String::new();

        for segment in &parsed.segments {
            match segment {
                VoiceCommand::Text(text) => {
                    // Accumulate text
                    if !pending_text.is_empty() {
                        pending_text.push(' ');
                    }
                    pending_text.push_str(text);
                },

                VoiceCommand::NewParagraph => {
                    // Flush pending text
                    if !pending_text.is_empty() {
                        output.push_str(&pending_text);
                        pending_text.clear();
                    }
                    // Add double newline
                    output.push_str("\n\n");
                    debug!("Added paragraph break");
                },

                VoiceCommand::NewLine => {
                    // Flush pending text
                    if !pending_text.is_empty() {
                        output.push_str(&pending_text);
                        pending_text.clear();
                    }
                    // Add single newline
                    output.push('\n');
                    debug!("Added line break");
                },

                VoiceCommand::CapitalizeThat => {
                    // Capitalize the pending text
                    if !pending_text.is_empty() {
                        pending_text = capitalize_first(&pending_text);
                        debug!("Capitalized text: '{}'", pending_text);
                    } else {
                        warn!("'cap that' command with no preceding text");
                    }
                },

                VoiceCommand::AllCaps => {
                    // Convert pending text to uppercase
                    if !pending_text.is_empty() {
                        pending_text = pending_text.to_uppercase();
                        debug!("Converted to all caps: '{}'", pending_text);
                    } else {
                        warn!("'all caps' command with no preceding text");
                    }
                },

                VoiceCommand::Undo | VoiceCommand::DeleteThat => {
                    // If undo appears mid-stream, treat as deletion of pending text
                    warn!("Undo/delete command in middle of text, clearing pending text");
                    pending_text.clear();
                },
            }
        }

        // Flush any remaining pending text
        if !pending_text.is_empty() {
            output.push_str(&pending_text);
        }

        if output.is_empty() {
            debug!("Command execution resulted in no output");
            ExecutionResult::no_action()
        } else {
            debug!("Command execution complete, output: '{}'", output);
            ExecutionResult::insert(output)
        }
    }

    /// Enable or disable command execution
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

impl Default for CommandExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Capitalize the first letter of a string
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let mut result = first.to_uppercase().collect::<String>();
            result.push_str(chars.as_str());
            result
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text_processing::commands::CommandParser;

    #[test]
    fn test_simple_text() {
        let parser = CommandParser::new();
        let executor = CommandExecutor::new();

        let parsed = parser.parse("hello world");
        let result = executor.execute(&parsed);

        assert!(result.text.is_some());
        assert_eq!(result.text.unwrap(), "hello world");
        assert!(!result.undo_requested);
    }

    #[test]
    fn test_new_paragraph() {
        let parser = CommandParser::new();
        let executor = CommandExecutor::new();

        let parsed = parser.parse("first paragraph new paragraph second paragraph");
        let result = executor.execute(&parsed);

        assert!(result.text.is_some());
        let text = result.text.unwrap();
        assert!(text.contains("\n\n"));
        assert!(text.contains("first paragraph"));
        assert!(text.contains("second paragraph"));
    }

    #[test]
    fn test_new_line() {
        let parser = CommandParser::new();
        let executor = CommandExecutor::new();

        let parsed = parser.parse("first line new line second line");
        let result = executor.execute(&parsed);

        assert!(result.text.is_some());
        let text = result.text.unwrap();
        assert_eq!(text.matches('\n').count(), 1);
    }

    #[test]
    fn test_undo_standalone() {
        let parser = CommandParser::new();
        let executor = CommandExecutor::new();

        let parsed = parser.parse("undo that");
        let result = executor.execute(&parsed);

        assert!(result.undo_requested);
        assert!(result.text.is_none());
    }

    #[test]
    fn test_capitalize_that() {
        let parser = CommandParser::new();
        let executor = CommandExecutor::new();

        let parsed = parser.parse("hello world cap that");
        let result = executor.execute(&parsed);

        assert!(result.text.is_some());
        let text = result.text.unwrap();
        assert!(
            text.starts_with('H'),
            "Text should be capitalized: {}",
            text
        );
    }

    #[test]
    fn test_all_caps() {
        let parser = CommandParser::new();
        let executor = CommandExecutor::new();

        let parsed = parser.parse("hello world all caps");
        let result = executor.execute(&parsed);

        assert!(result.text.is_some());
        assert_eq!(result.text.unwrap(), "HELLO WORLD");
    }

    #[test]
    fn test_multiple_commands() {
        let parser = CommandParser::new();
        let executor = CommandExecutor::new();

        let parsed = parser.parse("line one new line line two new paragraph line three");
        let result = executor.execute(&parsed);

        assert!(result.text.is_some());
        let text = result.text.unwrap();
        assert!(text.contains("\n"));
        assert!(text.contains("\n\n"));
    }

    #[test]
    fn test_disabled_executor() {
        let parser = CommandParser::new();
        let executor = CommandExecutor::disabled();

        let parsed = parser.parse("hello new paragraph world");
        let result = executor.execute(&parsed);

        assert!(result.text.is_some());
        // Should return combined text without processing commands
        assert_eq!(result.text.unwrap(), "hello new paragraph world");
    }
}
