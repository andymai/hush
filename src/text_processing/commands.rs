use once_cell::sync::Lazy;
/// Voice command detection and execution
///
/// This module provides support for voice commands like:
/// - "new paragraph" - insert double newline
/// - "new line" - insert single newline
/// - "delete that" / "undo" - remove last insertion
/// - "all caps" / "cap that" - capitalize text
///
/// Commands can be:
/// - Standalone: "new paragraph" → action only
/// - With text: "hello world new paragraph" → text + action
/// - Multiple: "first line new paragraph second line" → multiple actions
use regex::Regex;
use tracing::{debug, info};

/// A voice command parsed from transcribed text
#[derive(Debug, Clone, PartialEq)]
pub enum VoiceCommand {
    /// Insert a new paragraph (double newline)
    NewParagraph,

    /// Insert a single newline
    NewLine,

    /// Undo the last text insertion
    Undo,

    /// Delete the last sentence
    DeleteThat,

    /// Capitalize the preceding text
    CapitalizeThat,

    /// Convert preceding text to all caps
    AllCaps,

    /// Insert literal text (not a command)
    Text(String),
}

/// Result of parsing transcribed text for commands
#[derive(Debug, Clone)]
pub struct ParsedCommand {
    /// The segments of text and commands
    pub segments: Vec<VoiceCommand>,
}

impl ParsedCommand {
    /// Check if this contains any actual commands (not just text)
    pub fn has_commands(&self) -> bool {
        self.segments
            .iter()
            .any(|seg| !matches!(seg, VoiceCommand::Text(_)))
    }

    /// Check if this is only text with no commands
    pub fn is_text_only(&self) -> bool {
        !self.has_commands()
    }

    /// Get all text segments combined
    pub fn get_text(&self) -> String {
        self.segments
            .iter()
            .filter_map(|seg| match seg {
                VoiceCommand::Text(text) => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Patterns for detecting voice commands
static COMMAND_PATTERNS: Lazy<Vec<(Regex, VoiceCommand)>> = Lazy::new(|| {
    vec![
        // Paragraph and line breaks
        (
            Regex::new(r"\b(new paragraph|next paragraph)\b").unwrap(),
            VoiceCommand::NewParagraph,
        ),
        (
            Regex::new(r"\b(new line|next line)\b").unwrap(),
            VoiceCommand::NewLine,
        ),
        // Undo/delete
        (
            Regex::new(r"\b(undo|undo that)\b").unwrap(),
            VoiceCommand::Undo,
        ),
        (
            Regex::new(r"\b(delete that|scratch that)\b").unwrap(),
            VoiceCommand::DeleteThat,
        ),
        // Capitalization
        (
            Regex::new(r"\b(cap that|capitalize that)\b").unwrap(),
            VoiceCommand::CapitalizeThat,
        ),
        (
            Regex::new(r"\b(all caps|upper case)\b").unwrap(),
            VoiceCommand::AllCaps,
        ),
    ]
});

/// Voice command parser
pub struct CommandParser {
    enabled: bool,
}

impl CommandParser {
    /// Create a new command parser
    pub fn new() -> Self {
        Self { enabled: true }
    }

    /// Create a disabled parser (passes through all text)
    pub fn disabled() -> Self {
        Self { enabled: false }
    }

    /// Parse transcribed text for voice commands
    pub fn parse(&self, text: &str) -> ParsedCommand {
        if !self.enabled {
            return ParsedCommand {
                segments: vec![VoiceCommand::Text(text.to_string())],
            };
        }

        debug!("Parsing text for voice commands: '{}'", text);

        let mut segments = Vec::new();
        // Use lowercase for pattern matching but preserve original for extraction
        let lowercase = text.to_lowercase();
        let mut last_end = 0;

        // Find all command matches in lowercase version
        let mut matches: Vec<(usize, usize, VoiceCommand)> = Vec::new();

        for (pattern, command) in COMMAND_PATTERNS.iter() {
            for mat in pattern.find_iter(&lowercase) {
                matches.push((mat.start(), mat.end(), command.clone()));
            }
        }

        // Sort matches by position
        matches.sort_by_key(|(start, _, _)| *start);

        // Split text into segments using original case
        for (start, end, command) in matches {
            // Add text before this command (preserve original case)
            if start > last_end {
                let text_segment = text[last_end..start].trim();
                if !text_segment.is_empty() {
                    segments.push(VoiceCommand::Text(text_segment.to_string()));
                }
            }

            // Add the command
            segments.push(command);
            last_end = end;
        }

        // Add remaining text (preserve original case)
        if last_end < text.len() {
            let text_segment = text[last_end..].trim();
            if !text_segment.is_empty() {
                segments.push(VoiceCommand::Text(text_segment.to_string()));
            }
        }

        // If no commands found, treat entire input as text
        if segments.is_empty() {
            segments.push(VoiceCommand::Text(text.to_string()));
        }

        let parsed = ParsedCommand { segments };

        if parsed.has_commands() {
            info!(
                "Detected {} command(s) in text",
                parsed
                    .segments
                    .iter()
                    .filter(|s| !matches!(s, VoiceCommand::Text(_)))
                    .count()
            );
            debug!("Parsed segments: {:?}", parsed.segments);
        }

        parsed
    }

    /// Enable or disable command detection
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

impl Default for CommandParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_commands() {
        let parser = CommandParser::new();
        let result = parser.parse("hello world");

        assert!(result.is_text_only());
        assert_eq!(result.get_text(), "hello world");
    }

    #[test]
    fn test_new_paragraph() {
        let parser = CommandParser::new();
        let result = parser.parse("hello world new paragraph how are you");

        assert!(result.has_commands());
        assert_eq!(result.segments.len(), 3);
        assert!(matches!(result.segments[0], VoiceCommand::Text(_)));
        assert!(matches!(result.segments[1], VoiceCommand::NewParagraph));
        assert!(matches!(result.segments[2], VoiceCommand::Text(_)));
    }

    #[test]
    fn test_undo_command() {
        let parser = CommandParser::new();
        let result = parser.parse("undo that");

        assert!(result.has_commands());
        assert!(matches!(result.segments[0], VoiceCommand::Undo));
    }

    #[test]
    fn test_multiple_commands() {
        let parser = CommandParser::new();
        let result = parser.parse("first line new line second line new paragraph third line");

        assert!(result.has_commands());
        assert_eq!(result.segments.len(), 5); // text, newline, text, newpara, text
    }

    #[test]
    fn test_capitalize_command() {
        let parser = CommandParser::new();
        let result = parser.parse("hello world cap that");

        assert!(result.has_commands());
        assert!(matches!(result.segments[1], VoiceCommand::CapitalizeThat));
    }

    #[test]
    fn test_disabled_parser() {
        let parser = CommandParser::disabled();
        let result = parser.parse("hello new paragraph world");

        assert!(result.is_text_only());
        assert_eq!(result.get_text(), "hello new paragraph world");
    }

    #[test]
    fn test_preserves_case() {
        let parser = CommandParser::new();
        let result = parser.parse("Hello World new paragraph This Is A Test");

        assert!(result.has_commands());
        // Should preserve original case
        match &result.segments[0] {
            VoiceCommand::Text(text) => assert_eq!(text, "Hello World"),
            _ => unreachable!("Expected text segment"),
        }
        match &result.segments[2] {
            VoiceCommand::Text(text) => assert_eq!(text, "This Is A Test"),
            _ => unreachable!("Expected text segment"),
        }
    }
}
