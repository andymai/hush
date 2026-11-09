//! Whisper's initial prompt: names from the focused window's title plus the
//! user's learned terms, so those are spelled the way the user spells them.

use crate::text::WindowInfo;

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

fn render(title: &[&str], terms: &[String]) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if !title.is_empty() {
        parts.push(title.join(" "));
    }
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
    Some(prompt)
}

/// The window title, then the learned terms, cut until `fits` accepts the
/// prompt. The cut takes words from the end of the title first, and learned
/// terms from the end of the list only after the whole title is gone.
pub fn whisper_prompt(
    window: Option<&WindowInfo>,
    terms: &[String],
    fits: impl Fn(&str) -> bool,
) -> Option<String> {
    let title = window
        .map(|window| sanitize(&window.title, MAX_TITLE_CHARS))
        .unwrap_or_default();
    let mut title: Vec<&str> = title.split_whitespace().collect();
    let mut terms: Vec<String> = terms
        .iter()
        .map(|t| sanitize(t, 64))
        .filter(|t| !t.is_empty())
        .take(MAX_TERMS)
        .collect();
    loop {
        let prompt = render(&title, &terms)?;
        if fits(&prompt) {
            return Some(prompt);
        }
        if title.pop().is_some() {
            while title
                .last()
                .is_some_and(|word| !word.chars().any(char::is_alphanumeric))
            {
                title.pop();
            }
        } else {
            terms.pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn any(_: &str) -> bool {
        true
    }

    fn window(title: &str) -> WindowInfo {
        WindowInfo {
            title: title.to_string(),
            class: "Code".to_string(),
        }
    }

    #[test]
    fn title_and_terms_become_one_prompt() {
        let terms = vec![
            "Kubernetes".to_string(),
            " ".to_string(),
            "PostgreSQL".to_string(),
        ];
        assert_eq!(
            whisper_prompt(
                Some(&window("  hush/src/main.rs \u{0} - Code  ")),
                &terms,
                any
            )
            .unwrap(),
            "hush/src/main.rs - Code. Kubernetes, PostgreSQL."
        );
        assert_eq!(whisper_prompt(None, &[], any), None);
        assert_eq!(
            whisper_prompt(None, &["Hush".to_string()], any).unwrap(),
            "Hush."
        );
    }

    #[test]
    fn prompts_are_bounded_and_free_of_control_characters() {
        let prompt = whisper_prompt(Some(&window(&"x".repeat(500))), &[], any).unwrap();
        assert!(prompt.chars().count() <= MAX_TITLE_CHARS + 1);
        assert!(!prompt.contains('\u{0}'));
        let many: Vec<String> = (0..100).map(|i| format!("term{}", i)).collect();
        let prompt = whisper_prompt(None, &many, any).unwrap();
        assert_eq!(prompt.matches("term").count(), MAX_TERMS);
    }

    #[test]
    fn the_cap_cuts_the_title_before_any_learned_term() {
        let terms = vec!["Kubernetes".to_string(), "PostgreSQL".to_string()];
        let title = window("hush/src/main.rs - Visual Studio Code");
        let short = |limit: usize| move |prompt: &str| prompt.chars().count() <= limit;

        assert_eq!(
            whisper_prompt(Some(&title), &terms, short(50)).unwrap(),
            "hush/src/main.rs - Visual. Kubernetes, PostgreSQL."
        );
        assert_eq!(
            whisper_prompt(Some(&title), &terms, short(45)).unwrap(),
            "hush/src/main.rs. Kubernetes, PostgreSQL."
        );
        assert_eq!(
            whisper_prompt(Some(&title), &terms, short(30)).unwrap(),
            "Kubernetes, PostgreSQL."
        );
        assert_eq!(
            whisper_prompt(Some(&title), &terms, short(15)).unwrap(),
            "Kubernetes."
        );
        assert_eq!(whisper_prompt(Some(&title), &terms, short(5)), None);
    }
}
