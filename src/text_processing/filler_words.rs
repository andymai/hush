use super::config::EditingMode;
use once_cell::sync::Lazy;
/// Rule-based filler word removal
use regex::Regex;

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
        let mut result = text.to_string();

        // Select patterns based on mode
        let patterns = match mode {
            EditingMode::Light => &self.light_patterns,
            EditingMode::Medium => &self.medium_patterns,
            EditingMode::Aggressive => &self.aggressive_patterns,
        };

        // Remove filler words
        for pattern in patterns {
            result = pattern.replace_all(&result, " ").to_string();
        }

        // Normalize whitespace
        result = MULTIPLE_SPACES.replace_all(&result, " ").to_string();

        // Fix punctuation spacing
        result = SPACE_BEFORE_PUNCTUATION
            .replace_all(&result, "$1")
            .to_string();

        // Remove repeated punctuation (each type separately)
        result = REPEATED_PERIODS.replace_all(&result, ".").to_string();
        result = REPEATED_COMMAS.replace_all(&result, ",").to_string();
        result = REPEATED_EXCLAMATION.replace_all(&result, "!").to_string();
        result = REPEATED_QUESTION.replace_all(&result, "?").to_string();

        // Trim before capitalizing so first char is actually a letter
        result = result.trim().to_string();

        // Capitalize first letter
        result = Self::capitalize_first(&result);

        // Ensure ends with punctuation
        Self::ensure_ending_punctuation(&result)
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

        // Safe to expect: we just checked that trimmed is not empty
        let last_char = trimmed.chars().last().expect("trimmed is not empty");
        if matches!(last_char, '.' | '!' | '?') {
            trimmed.to_string()
        } else {
            format!("{}.", trimmed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
