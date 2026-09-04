//! `hush learn`: the selected text, or the words given, become a vocabulary
//! term with exactly that spelling and casing.

use super::vocabulary::DomainVocabulary;
use anyhow::{anyhow, Context, Result};
use std::path::Path;

pub const MAX_WORDS: usize = 5;
pub const MAX_CHARS: usize = 64;

/// Collapse whitespace and reject anything that is not a short term.
pub fn normalise(term: &str) -> Result<String> {
    let collapsed = term.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return Err(anyhow!("Nothing to learn: the selection is empty"));
    }
    if collapsed.chars().any(char::is_control) {
        return Err(anyhow!(
            "Nothing to learn: the selection contains control characters"
        ));
    }
    let words = collapsed.split(' ').count();
    if words > MAX_WORDS || collapsed.chars().count() > MAX_CHARS {
        return Err(anyhow!(
            "Nothing to learn: select a term of at most {} words and {} characters",
            MAX_WORDS,
            MAX_CHARS
        ));
    }
    Ok(collapsed)
}

/// Add the term to the vocabulary; returns whether anything changed.
pub fn learn_into(vocab: &mut DomainVocabulary, term: &str) -> bool {
    let key = term.to_lowercase();
    if vocab.technical_terms.get(&key).map(String::as_str) == Some(term) {
        return false;
    }
    vocab.technical_terms.insert(key, term.to_string());
    true
}

/// Learn into the vocabulary file at `path`, creating it when missing.
pub fn learn_at(path: &Path, term: &str) -> Result<String> {
    let term = normalise(term)?;
    let mut vocab = if path.exists() {
        DomainVocabulary::load_from_file(path)?
    } else {
        DomainVocabulary::default()
    };
    if learn_into(&mut vocab, &term) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        vocab.save_to_file(path)?;
    }
    Ok(term)
}

/// Learn the given words, or the current selection when `text` is `None`.
pub fn learn(text: Option<&str>) -> Result<String> {
    let term = match text {
        Some(text) => text.to_string(),
        None => selected_text()
            .ok_or_else(|| anyhow!("Select a word first, or name it: hush learn <words>"))?,
    };
    learn_at(&DomainVocabulary::default_path(), &term)
}

#[cfg(target_os = "linux")]
fn selected_text() -> Option<String> {
    crate::text::selection::read_primary()
}

#[cfg(not(target_os = "linux"))]
fn selected_text() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terms_are_collapsed_and_bounded() {
        assert_eq!(normalise("  Kube\tFlow  ").unwrap(), "Kube Flow");
        assert!(normalise("   ").is_err());
        assert!(normalise("one two three four five six").is_err());
        assert!(normalise(&"x".repeat(65)).is_err());
        assert!(normalise("bad\u{7}term").is_err());
    }

    #[test]
    fn learning_keeps_exact_spelling_under_a_lowercase_key() {
        let mut vocab = DomainVocabulary::default();
        assert!(learn_into(&mut vocab, "Kubernetes"));
        assert!(!learn_into(&mut vocab, "Kubernetes"), "unchanged");
        assert!(learn_into(&mut vocab, "KUBERNETES"), "new casing replaces");
        assert_eq!(
            vocab.technical_terms.get("kubernetes").map(String::as_str),
            Some("KUBERNETES")
        );
        assert_eq!(
            vocab.apply("deploy to kubernetes now"),
            "deploy to KUBERNETES now"
        );
    }

    #[test]
    fn learning_writes_and_extends_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("vocabulary.json");
        assert_eq!(learn_at(&path, "PostgreSQL").unwrap(), "PostgreSQL");
        assert_eq!(learn_at(&path, " Hush  daemon ").unwrap(), "Hush daemon");
        let vocab = DomainVocabulary::load_from_file(&path).unwrap();
        assert_eq!(vocab.technical_terms.len(), 2);
        assert_eq!(
            vocab.apply("the hush daemon talks to postgresql"),
            "the Hush daemon talks to PostgreSQL"
        );
        assert!(learn_at(&path, "").is_err());
    }
}
