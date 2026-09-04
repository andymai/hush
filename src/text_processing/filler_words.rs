use super::config::EditingMode;
use once_cell::sync::Lazy;
/// Rule-based filler word removal
use regex::Regex;
use std::borrow::Cow;

/// Common filler words and phrases to remove
static LIGHT_FILLERS: &[&str] = &["um", "uh", "hmm", "er", "ah"];

static MEDIUM_FILLERS: &[&str] = &[
    "um",
    "uh",
    "hmm",
    "er",
    "ah",
    "like",
    "you know",
    "i mean",
    "sort of",
    "kind of",
    "basically",
    "actually",
    "literally",
    "totally",
];

static AGGRESSIVE_FILLERS: &[&str] = &[
    "um",
    "uh",
    "hmm",
    "er",
    "ah",
    "like",
    "you know",
    "i mean",
    "sort of",
    "kind of",
    "basically",
    "actually",
    "literally",
    "totally",
    "just",
    "really",
    "very",
    "quite",
    "i think",
    "i guess",
    "maybe",
    "perhaps",
    "anyway",
    "so yeah",
    "well",
];

/// Precompiled regex patterns
/// Note: These use expect() since the patterns are hardcoded compile-time constants
static MULTIPLE_SPACES: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\s+").expect("MULTIPLE_SPACES regex is valid"));

static SPACE_BEFORE_PUNCTUATION: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\s+([,.!?;:])").expect("SPACE_BEFORE_PUNCTUATION regex is valid"));

// Multiple patterns for repeated punctuation (Rust regex doesn't support backreferences)
static REPEATED_PERIODS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\.{2,}").expect("REPEATED_PERIODS regex is valid"));

static REPEATED_COMMAS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r",{2,}").expect("REPEATED_COMMAS regex is valid"));

static REPEATED_EXCLAMATION: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"!{2,}").expect("REPEATED_EXCLAMATION regex is valid"));

static REPEATED_QUESTION: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\?{2,}").expect("REPEATED_QUESTION regex is valid"));

pub struct FillerWordRemover {
    light_patterns: Vec<Regex>,
    medium_patterns: Vec<Regex>,
    aggressive_patterns: Vec<Regex>,
}

impl Default for FillerWordRemover {
    fn default() -> Self {
        Self::new()
    }
}

impl FillerWordRemover {
    pub fn new() -> Self {
        Self {
            light_patterns: Self::compile_patterns(LIGHT_FILLERS),
            medium_patterns: Self::compile_patterns(MEDIUM_FILLERS),
            aggressive_patterns: Self::compile_patterns(AGGRESSIVE_FILLERS),
        }
    }

    /// Compile filler words into regex patterns
    fn compile_patterns(fillers: &[&str]) -> Vec<Regex> {
        fillers
            .iter()
            .map(|filler| {
                // Match the filler word with word boundaries
                // Case insensitive, with optional surrounding spaces
                let pattern = format!(r"(?i)\b{}\b", regex::escape(filler));
                // regex::escape() always produces valid regex, so expect() is safe here
                Regex::new(&pattern).expect("Escaped regex pattern is valid")
            })
            .collect()
    }

    /// Remove filler words based on editing mode
    pub fn remove(&self, text: &str, mode: EditingMode) -> String {
        self.remove_with(text, mode, true, true)
    }

    /// Remove filler words, then capitalise the first letter and add a
    /// trailing period only when the application profile wants them.
    pub fn remove_with(
        &self,
        text: &str,
        mode: EditingMode,
        capitalize: bool,
        ending_punctuation: bool,
    ) -> String {
        let mut result: Cow<str> = Cow::Borrowed(text);

        // Select patterns based on mode
        let patterns = match mode {
            EditingMode::Light => &self.light_patterns,
            EditingMode::Medium => &self.medium_patterns,
            EditingMode::Aggressive => &self.aggressive_patterns,
        };

        // Remove filler words - only allocate if there's a match
        for pattern in patterns {
            if pattern.is_match(&result) {
                result = Cow::Owned(pattern.replace_all(&result, " ").into_owned());
            }
        }

        // Normalize whitespace - only allocate if there's a match
        if MULTIPLE_SPACES.is_match(&result) {
            result = Cow::Owned(MULTIPLE_SPACES.replace_all(&result, " ").into_owned());
        }

        // Fix punctuation spacing - only allocate if there's a match
        if SPACE_BEFORE_PUNCTUATION.is_match(&result) {
            result = Cow::Owned(
                SPACE_BEFORE_PUNCTUATION
                    .replace_all(&result, "$1")
                    .into_owned(),
            );
        }

        // Remove repeated punctuation (each type separately) - only allocate if there's a match
        if REPEATED_PERIODS.is_match(&result) {
            result = Cow::Owned(REPEATED_PERIODS.replace_all(&result, ".").into_owned());
        }
        if REPEATED_COMMAS.is_match(&result) {
            result = Cow::Owned(REPEATED_COMMAS.replace_all(&result, ",").into_owned());
        }
        if REPEATED_EXCLAMATION.is_match(&result) {
            result = Cow::Owned(REPEATED_EXCLAMATION.replace_all(&result, "!").into_owned());
        }
        if REPEATED_QUESTION.is_match(&result) {
            result = Cow::Owned(REPEATED_QUESTION.replace_all(&result, "?").into_owned());
        }

        // Trim before capitalizing so first char is actually a letter
        let trimmed = result.trim();
        let result = if trimmed.len() != result.len() {
            trimmed.to_string()
        } else {
            result.into_owned()
        };

        let result = if capitalize {
            Self::capitalize_first(&result)
        } else {
            result
        };
        if ending_punctuation {
            Self::ensure_ending_punctuation(&result)
        } else {
            result.trim().to_string()
        }
    }

    fn capitalize_first(text: &str) -> String {
        let mut chars = text.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }

    fn ensure_ending_punctuation(text: &str) -> String {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return trimmed.to_string();
        }

        match trimmed.chars().last() {
            Some('.' | '!' | '?') | None => trimmed.to_string(),
            Some(_) => format!("{}.", trimmed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_patterns_compile() {
        let _ = &*MULTIPLE_SPACES;
        let _ = &*SPACE_BEFORE_PUNCTUATION;
        let _ = &*REPEATED_PERIODS;
        let _ = &*REPEATED_COMMAS;
        let _ = &*REPEATED_EXCLAMATION;
        let _ = &*REPEATED_QUESTION;
    }

    #[test]
    fn test_light_mode() {
        let remover = FillerWordRemover::new();
        let result = remover.remove("um so I think we should uh do this", EditingMode::Light);

        // Should remove um and uh but keep "I think"
        assert!(!result.contains("um"));
        assert!(!result.contains("uh"));
        assert!(result.contains("think"));
    }

    #[test]
    fn test_medium_mode() {
        let remover = FillerWordRemover::new();
        let result = remover.remove(
            "um so like I think you know we should uh do this",
            EditingMode::Medium,
        );

        // Should remove more fillers
        assert!(!result.contains("um"));
        assert!(!result.contains("uh"));
        assert!(!result.contains("like"));
        assert!(!result.contains("you know"));
    }

    #[test]
    fn test_capitalization() {
        let remover = FillerWordRemover::new();
        let result = remover.remove("um hello world", EditingMode::Light);

        assert!(result.starts_with("H"));
    }

    #[test]
    fn test_ending_punctuation() {
        let remover = FillerWordRemover::new();
        let result = remover.remove("um hello world", EditingMode::Light);

        assert!(result.ends_with('.'));
    }

    #[test]
    fn test_whitespace_normalization() {
        let remover = FillerWordRemover::new();
        let result = remover.remove("um   hello    world   um", EditingMode::Light);

        assert!(!result.contains("  ")); // No double spaces
    }

    #[test]
    fn profiles_can_skip_capitalization_and_the_period() {
        let remover = FillerWordRemover::new();
        assert_eq!(
            remover.remove_with("um cargo build", EditingMode::Medium, false, false),
            "cargo build"
        );
        assert_eq!(
            remover.remove_with("hello there", EditingMode::Light, true, false),
            "Hello there"
        );
        assert_eq!(
            remover.remove_with("hello there", EditingMode::Light, true, true),
            "Hello there."
        );
    }
}
