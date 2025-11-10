/// Intent detection for adaptive text processing
use regex::Regex;
use once_cell::sync::Lazy;
use tracing::debug;

/// Detected user intent from transcription
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserIntent {
    /// Writing code
    Code {
        /// Programming language if detected
        language: Option<String>,
        /// Whether this is a comment vs actual code
        is_comment: bool,
    },
    /// Composing a message/email
    Communication {
        /// Communication style
        style: CommunicationStyle,
    },
    /// Taking notes or writing documentation
    Documentation,
    /// Executing a command
    Command {
        /// Shell or command type
        shell: Option<String>,
    },
    /// Creating a list or steps
    List,
    /// Asking a question
    Question,
    /// TODO/task item
    Todo,
    /// General dictation (default)
    Dictation,
}

/// Communication style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommunicationStyle {
    Formal,      // Email, professional communication
    Casual,      // Chat, informal messaging
}

/// Regex patterns for intent detection
static CODE_PATTERNS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(function|class|def|const|let|var|import|return|if|for|while)\b").unwrap()
});

static COMMENT_PATTERNS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^(comment|note|todo|fixme|hack)[:.]?\s").unwrap()
});

static COMMAND_PATTERNS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^(run|execute|start|stop|install|uninstall|cd|ls|mkdir|git)\b").unwrap()
});

static LIST_PATTERNS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(first|second|third|next|then|finally|steps?|items?)\b.*\b(first|second|and|also)\b").unwrap()
});

static QUESTION_PATTERNS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^(what|how|why|when|where|who|which|can|could|would|should|is|are|do|does)\b").unwrap()
});

static TODO_PATTERNS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(todo|task|reminder|need to|have to|must|should)\b").unwrap()
});

static EMAIL_PATTERNS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(dear|hi|hello|regards|sincerely|best|thanks|subject|email|message)\b").unwrap()
});

static CHAT_PATTERNS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(hey|yo|sup|lol|btw|omg|brb|imo|fyi)\b").unwrap()
});

/// Intent detector
pub struct IntentDetector {
    enabled: bool,
}

impl IntentDetector {
    /// Create a new intent detector
    pub fn new() -> Self {
        Self { enabled: true }
    }

    /// Create a disabled detector
    pub fn disabled() -> Self {
        Self { enabled: false }
    }

    /// Detect intent from transcribed text
    pub fn detect(&self, text: &str) -> UserIntent {
        if !self.enabled {
            return UserIntent::Dictation;
        }

        debug!("Detecting intent from: '{}'", text);

        let text_lower = text.to_lowercase();

        // Check for TODO items
        if TODO_PATTERNS.is_match(&text_lower) {
            debug!("Detected intent: Todo");
            return UserIntent::Todo;
        }

        // Check for questions
        if QUESTION_PATTERNS.is_match(&text_lower) {
            debug!("Detected intent: Question");
            return UserIntent::Question;
        }

        // Check for commands
        if COMMAND_PATTERNS.is_match(&text_lower) {
            debug!("Detected intent: Command");
            return UserIntent::Command { shell: None };
        }

        // Check for code
        if CODE_PATTERNS.is_match(&text_lower) {
            let is_comment = COMMENT_PATTERNS.is_match(&text_lower);
            debug!("Detected intent: Code (comment: {})", is_comment);
            return UserIntent::Code {
                language: self.detect_language(&text_lower),
                is_comment,
            };
        }

        // Check for lists
        if LIST_PATTERNS.is_match(&text_lower) {
            debug!("Detected intent: List");
            return UserIntent::List;
        }

        // Check for communication style
        if EMAIL_PATTERNS.is_match(&text_lower) {
            debug!("Detected intent: Communication (Formal)");
            return UserIntent::Communication {
                style: CommunicationStyle::Formal,
            };
        }

        if CHAT_PATTERNS.is_match(&text_lower) {
            debug!("Detected intent: Communication (Casual)");
            return UserIntent::Communication {
                style: CommunicationStyle::Casual,
            };
        }

        // Default to dictation
        debug!("Detected intent: Dictation (default)");
        UserIntent::Dictation
    }

    /// Try to detect programming language from text
    fn detect_language(&self, text: &str) -> Option<String> {
        let text_lower = text.to_lowercase();

        // Simple language detection based on keywords
        if text_lower.contains("function") || text_lower.contains("const ")
            || text_lower.contains("let ") || text_lower.contains("var ") {
            return Some("JavaScript".to_string());
        }
        if text_lower.contains("def ") || text_lower.contains("import ")
            && text_lower.contains("from") {
            return Some("Python".to_string());
        }
        if text_lower.contains("func ") || text_lower.contains("package ") {
            return Some("Go".to_string());
        }
        if text_lower.contains("fn ") || text_lower.contains("let mut")
            || text_lower.contains("impl ") {
            return Some("Rust".to_string());
        }

        None
    }

    /// Enable or disable intent detection
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

impl Default for IntentDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl UserIntent {
    /// Get a description of this intent
    pub fn description(&self) -> &str {
        match self {
            UserIntent::Code { is_comment: true, .. } => "Code comment",
            UserIntent::Code { is_comment: false, .. } => "Code",
            UserIntent::Communication { style: CommunicationStyle::Formal } => "Formal communication",
            UserIntent::Communication { style: CommunicationStyle::Casual } => "Casual chat",
            UserIntent::Documentation => "Documentation",
            UserIntent::Command { .. } => "Command",
            UserIntent::List => "List",
            UserIntent::Question => "Question",
            UserIntent::Todo => "TODO item",
            UserIntent::Dictation => "Dictation",
        }
    }

    /// Check if this intent should preserve technical formatting
    pub fn preserve_technical_formatting(&self) -> bool {
        matches!(
            self,
            UserIntent::Code { .. } | UserIntent::Command { .. }
        )
    }

    /// Check if this intent should be more formal
    pub fn should_be_formal(&self) -> bool {
        matches!(
            self,
            UserIntent::Communication {
                style: CommunicationStyle::Formal
            } | UserIntent::Documentation
        )
    }

    /// Get suggested formatting hints
    pub fn formatting_hints(&self) -> Vec<String> {
        match self {
            UserIntent::Code { .. } => vec![
                "Preserve code syntax".to_string(),
                "Use snake_case/camelCase appropriately".to_string(),
            ],
            UserIntent::List => vec![
                "Format as bullet points".to_string(),
                "One item per line".to_string(),
            ],
            UserIntent::Todo => vec![
                "Format as TODO comment or task".to_string(),
            ],
            UserIntent::Question => vec![
                "End with question mark".to_string(),
            ],
            UserIntent::Communication { style: CommunicationStyle::Formal } => vec![
                "Use professional language".to_string(),
                "Add greeting/closing if appropriate".to_string(),
            ],
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_code() {
        let detector = IntentDetector::new();

        let intent = detector.detect("function foo returns bar");
        assert!(matches!(intent, UserIntent::Code { .. }));

        let intent = detector.detect("const myVar equals 42");
        assert!(matches!(intent, UserIntent::Code { .. }));
    }

    #[test]
    fn test_detect_comment() {
        let detector = IntentDetector::new();

        let intent = detector.detect("todo: fix this bug");
        assert!(matches!(intent, UserIntent::Todo));

        let intent = detector.detect("comment: this needs work");
        assert!(matches!(intent, UserIntent::Code { is_comment: true, .. }));
    }

    #[test]
    fn test_detect_command() {
        let detector = IntentDetector::new();

        let intent = detector.detect("run npm install");
        assert!(matches!(intent, UserIntent::Command { .. }));

        let intent = detector.detect("git commit message");
        assert!(matches!(intent, UserIntent::Command { .. }));
    }

    #[test]
    fn test_detect_question() {
        let detector = IntentDetector::new();

        let intent = detector.detect("what is the meaning of life");
        assert_eq!(intent, UserIntent::Question);

        let intent = detector.detect("how do I fix this error");
        assert_eq!(intent, UserIntent::Question);
    }

    #[test]
    fn test_detect_list() {
        let detector = IntentDetector::new();

        let intent = detector.detect("first item is foo second item is bar");
        assert_eq!(intent, UserIntent::List);

        let intent = detector.detect("the steps are first do this then do that");
        assert_eq!(intent, UserIntent::List);
    }

    #[test]
    fn test_detect_email() {
        let detector = IntentDetector::new();

        let intent = detector.detect("dear john I hope this email finds you well");
        assert!(matches!(
            intent,
            UserIntent::Communication {
                style: CommunicationStyle::Formal
            }
        ));
    }

    #[test]
    fn test_detect_chat() {
        let detector = IntentDetector::new();

        let intent = detector.detect("hey btw did you see that lol");
        assert!(matches!(
            intent,
            UserIntent::Communication {
                style: CommunicationStyle::Casual
            }
        ));
    }

    #[test]
    fn test_default_dictation() {
        let detector = IntentDetector::new();

        let intent = detector.detect("this is just regular text");
        assert_eq!(intent, UserIntent::Dictation);
    }

    #[test]
    fn test_disabled_detector() {
        let detector = IntentDetector::disabled();

        let intent = detector.detect("function foo returns bar");
        assert_eq!(intent, UserIntent::Dictation);
    }

    #[test]
    fn test_language_detection() {
        let detector = IntentDetector::new();

        let intent = detector.detect("function foo returns bar");
        match intent {
            UserIntent::Code { language: Some(lang), .. } => {
                assert_eq!(lang, "JavaScript");
            }
            _ => panic!("Expected Code intent with language"),
        }

        let intent = detector.detect("def foo return bar");
        match intent {
            UserIntent::Code { language: Some(lang), .. } => {
                assert_eq!(lang, "Python");
            }
            _ => panic!("Expected Code intent with language"),
        }
    }
}
