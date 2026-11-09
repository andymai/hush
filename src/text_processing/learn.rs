//! `hush learn`: the selected text, or the words given, become a vocabulary
//! term with exactly that spelling and casing.

use super::vocabulary::DomainVocabulary;
use anyhow::{anyhow, Context, Result};
use std::collections::HashSet;
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

/// Remove the term with exactly this spelling; returns whether it was there.
pub fn forget_in(vocab: &mut DomainVocabulary, term: &str) -> bool {
    let before = vocab.technical_terms.len();
    vocab.technical_terms.retain(|_, value| value != term);
    vocab.technical_terms.len() != before
}

/// Replace `old` with `new`, spelled exactly as given; returns the new term.
pub fn rename_in(vocab: &mut DomainVocabulary, old: &str, new: &str) -> Result<String> {
    let new = normalise(new)?;
    forget_in(vocab, old);
    learn_into(vocab, &new);
    Ok(new)
}

/// What an import of a word list did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ListImport {
    pub added: usize,
    /// Terms the vocabulary or the list already had, in any case
    pub skipped: usize,
    /// Lines that are not a term [`normalise`] accepts
    pub invalid: usize,
}

/// The learned terms as plain text, one on each line.
pub fn to_list(vocab: &DomainVocabulary) -> String {
    let mut terms: Vec<&String> = vocab.technical_terms.values().collect();
    terms.sort_by_key(|term| (term.to_lowercase(), term.as_str()));
    terms.iter().map(|term| format!("{}\n", term)).collect()
}

/// Learn each line of `text`. A term that is already there, in any case,
/// stays as it is.
pub fn import_list(vocab: &mut DomainVocabulary, text: &str) -> ListImport {
    let mut known: HashSet<String> = vocab
        .technical_terms
        .iter()
        .flat_map(|(key, term)| [key.to_lowercase(), term.to_lowercase()])
        .collect();
    let mut result = ListImport::default();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(term) = normalise(line) else {
            result.invalid += 1;
            continue;
        };
        if known.insert(term.to_lowercase()) {
            learn_into(vocab, &term);
            result.added += 1;
        } else {
            result.skipped += 1;
        }
    }
    result
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
    fn edits_round_trip_through_the_file_and_keep_other_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vocabulary.json");
        let mut vocab = DomainVocabulary::default();
        vocab.add_abbreviation("k8s".into(), "Kubernetes".into());
        learn_into(&mut vocab, "Postgres");
        learn_into(&mut vocab, "Hush");
        assert_eq!(
            rename_in(&mut vocab, "Postgres", " PostgreSQL ").unwrap(),
            "PostgreSQL"
        );
        assert!(rename_in(&mut vocab, "Hush", "  ").is_err());
        assert!(forget_in(&mut vocab, "Hush"));
        assert!(!forget_in(&mut vocab, "Hush"), "already gone");
        vocab.save_to_file(&path).unwrap();

        let loaded = DomainVocabulary::load_from_file(&path).unwrap();
        assert_eq!(loaded, vocab);
        let terms: Vec<&String> = loaded.technical_terms.values().collect();
        assert_eq!(terms, ["PostgreSQL"]);
        assert_eq!(loaded.expand("k8s").map(String::as_str), Some("Kubernetes"));
    }

    #[test]
    fn a_word_list_import_skips_duplicates_in_any_case() {
        let mut vocab = DomainVocabulary::default();
        learn_into(&mut vocab, "Kubernetes");
        let list = "kubernetes\n\nPostgreSQL\r\n  Hush   daemon \nPOSTGRESQL\nbad\u{7}term\n";
        let result = import_list(&mut vocab, list);
        assert_eq!(
            result,
            ListImport {
                added: 2,
                skipped: 2,
                invalid: 1
            }
        );
        assert_eq!(vocab.technical_terms["kubernetes"], "Kubernetes");
        assert_eq!(vocab.technical_terms["postgresql"], "PostgreSQL");
        assert_eq!(vocab.technical_terms["hush daemon"], "Hush daemon");
    }

    #[test]
    fn a_word_list_round_trips() {
        let mut vocab = DomainVocabulary::default();
        for term in ["zsh", "Kubernetes", "C#"] {
            learn_into(&mut vocab, term);
        }
        let list = to_list(&vocab);
        assert_eq!(list, "C#\nKubernetes\nzsh\n");
        let mut copy = DomainVocabulary::default();
        assert_eq!(import_list(&mut copy, &list).added, 3);
        assert_eq!(copy, vocab);
        assert_eq!(to_list(&DomainVocabulary::default()), "");
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
