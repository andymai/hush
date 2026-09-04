//! Whisper's initial prompt: names from the focused window's title plus the
//! user's learned terms, so those are spelled the way the user spells them.

use crate::text::WindowInfo;

pub const MAX_PROMPT_CHARS: usize = 400;
const MAX_TITLE_CHARS: usize = 120;
const MAX_TERMS: usize = 40;

/// Drop control characters (whisper.cpp rejects interior NULs) and cap the
/// length on a character boundary.
fn sanitize(text: &str, max_chars: usize) -> String {
    let cleaned: String = text
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    cleaned.chars().take(max_chars).collect()
}

pub fn whisper_prompt(window: Option<&WindowInfo>, terms: &[String]) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(window) = window {
        let title = sanitize(&window.title, MAX_TITLE_CHARS);
        if !title.is_empty() {
            parts.push(title);
        }
    }
    let terms: Vec<String> = terms
        .iter()
        .map(|t| sanitize(t, 64))
        .filter(|t| !t.is_empty())
        .take(MAX_TERMS)
        .collect();
    if !terms.is_empty() {
        parts.push(terms.join(", "));
    }
    if parts.is_empty() {
        return None;
    }
    let mut prompt = parts.join(". ");
    if !prompt.ends_with('.') {
        prompt.push('.');
    }
    Some(prompt.chars().take(MAX_PROMPT_CHARS).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_and_terms_become_one_prompt() {
        let window = WindowInfo {
            title: "  hush/src/main.rs \u{0} - Code  ".to_string(),
            class: "Code".to_string(),
        };
        let terms = vec![
            "Kubernetes".to_string(),
            " ".to_string(),
            "PostgreSQL".to_string(),
        ];
        assert_eq!(
            whisper_prompt(Some(&window), &terms).unwrap(),
            "hush/src/main.rs - Code. Kubernetes, PostgreSQL."
        );
        assert_eq!(whisper_prompt(None, &[]), None);
        assert_eq!(
            whisper_prompt(None, &["Hush".to_string()]).unwrap(),
            "Hush."
        );
    }

    #[test]
    fn prompts_are_bounded_and_free_of_control_characters() {
        let window = WindowInfo {
            title: "x".repeat(500),
            class: String::new(),
        };
        let prompt = whisper_prompt(Some(&window), &[]).unwrap();
        assert!(prompt.chars().count() <= MAX_TITLE_CHARS + 1);
        assert!(!prompt.contains('\u{0}'));
        let many: Vec<String> = (0..100).map(|i| format!("term{}", i)).collect();
        let prompt = whisper_prompt(None, &many).unwrap();
        assert!(prompt.chars().count() <= MAX_PROMPT_CHARS);
    }
}
