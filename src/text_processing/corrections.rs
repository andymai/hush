//! The terms that a fix to a transcript can teach: each word that the fix
//! adds or changes, except common English words, and each word whose case
//! the fix changes. Adjacent changed words make one term, such as
//! "New Relic".
//!
//! The words of the two texts are aligned, so a word that the fix adds does
//! not make the words after it look changed.

use super::learn;
use super::vocabulary::DomainVocabulary;
use once_cell::sync::Lazy;
use std::collections::HashSet;

/// One word on each line; lines that start with `#` give the source and the
/// license.
const COMMON_WORDS: &str = include_str!("common_words.txt");

/// Every common word, in lower case.
static COMMON: Lazy<HashSet<String>> = Lazy::new(|| entries().map(str::to_lowercase).collect());

/// The common words that books write with a capital, mostly names such as
/// "Ralph", in lower case.
static NAMES: Lazy<HashSet<String>> = Lazy::new(|| {
    entries()
        .filter(|word| word.chars().any(char::is_uppercase))
        .map(str::to_lowercase)
        .collect()
});

/// Grammar words and replies. A capital on one of them marks the start of a
/// sentence or the pronoun "I", not a name.
const FUNCTION_WORDS: &[&str] = &[
    "a", "about", "after", "all", "also", "am", "an", "and", "are", "as", "at", "be", "because",
    "been", "before", "but", "by", "can", "could", "did", "do", "does", "for", "from", "had",
    "has", "have", "he", "her", "here", "hey", "hi", "him", "his", "how", "i", "if", "in", "into",
    "is", "it", "its", "just", "let", "me", "my", "no", "not", "now", "of", "oh", "ok", "okay",
    "on", "or", "our", "please", "she", "so", "than", "thanks", "that", "the", "their", "them",
    "then", "there", "these", "they", "this", "those", "to", "up", "us", "was", "we", "well",
    "were", "what", "when", "where", "which", "while", "who", "why", "will", "with", "would",
    "yeah", "yes", "you", "your",
];

/// The alignment table has one cell for each pair of changed words. Above
/// this size, the changed words count as new without an alignment.
const MAX_CELLS: usize = 1 << 22;

/// A word of a text.
#[derive(Debug)]
struct Token {
    /// The word without the punctuation around it
    word: String,
    /// `word` in lower case, for the alignment
    key: String,
    sentence_start: bool,
    /// No punctuation came off the start of the word
    joins_previous: bool,
    /// No punctuation or possessive came off the end of the word
    joins_next: bool,
}

/// How strongly a word of the fixed text marks a term to learn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Signal {
    None,
    /// Part of a term only next to a strong word, such as "New" at the start
    /// of "New Relic is down"
    Weak,
    Strong,
}

/// The terms to propose after a fix turns `before` into `after`, in the
/// order of `after`. A term that `vocabulary` already has with the same
/// spelling is left out.
pub fn proposals(before: &str, after: &str, vocabulary: &DomainVocabulary) -> Vec<String> {
    let old = tokens(before);
    let new = tokens(after);
    let aligned = align(&old, &new);
    let signals: Vec<Signal> = new
        .iter()
        .zip(&aligned)
        .map(|(token, at)| signal(token, at.map(|index| &old[index])))
        .collect();

    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (index, token) in new.iter().enumerate() {
        if signals[index] == Signal::None {
            continue;
        }
        let joins = index > 0
            && signals[index - 1] != Signal::None
            && new[index - 1].joins_next
            && token.joins_previous
            && !token.sentence_start;
        match groups.last_mut() {
            Some(group) if joins => group.push(index),
            _ => groups.push(vec![index]),
        }
    }

    let mut seen = HashSet::new();
    let mut terms = Vec::new();
    for group in groups {
        let strong: Vec<usize> = group
            .iter()
            .copied()
            .filter(|&index| signals[index] == Signal::Strong)
            .collect();
        if strong.is_empty() {
            continue;
        }
        let parts = if group.len() > learn::MAX_WORDS {
            strong.into_iter().map(|index| vec![index]).collect()
        } else {
            vec![group]
        };
        for part in parts {
            let words: Vec<&str> = part.iter().map(|&index| new[index].word.as_str()).collect();
            let Ok(term) = learn::normalise(&words.join(" ")) else {
                continue;
            };
            let key = term.to_lowercase();
            if vocabulary.technical_terms.get(&key) == Some(&term) || !seen.insert(key) {
                continue;
            }
            terms.push(term);
        }
    }
    terms
}

/// `before` is the word of the old text that this word aligns with.
fn signal(token: &Token, before: Option<&Token>) -> Signal {
    let word = token.word.as_str();
    if before.is_some_and(|old| old.word == word)
        || !word.chars().any(char::is_alphabetic)
        || (word.is_ascii() && word.len() == 1)
        || ordinal(&token.key)
        || function_word(&token.key)
    {
        return Signal::None;
    }
    if !common(word, &token.key) {
        return Signal::Strong;
    }
    if !word.chars().any(char::is_uppercase) {
        return Signal::None;
    }
    let case_changed = before.is_some();
    if case_changed && !(token.sentence_start && title_case(word)) {
        Signal::Strong
    } else {
        Signal::Weak
    }
}

/// A common English word, or an inflection or contraction of one. A word
/// that books write with a capital is common only in lower case.
fn common(word: &str, key: &str) -> bool {
    if NAMES.contains(key) && word.chars().any(char::is_uppercase) {
        return false;
    }
    common_key(key)
}

fn common_key(key: &str) -> bool {
    let key = key.replace('’', "'");
    if key.contains(['-', '/']) {
        return key
            .split(['-', '/'])
            .filter(|part| !part.is_empty())
            .all(common_key);
    }
    if let Some((base, ending)) = key.split_once('\'') {
        return match ending {
            "d" | "ll" | "m" | "re" | "s" | "ve" => common_key(base),
            "t" => common_key(base) || base.strip_suffix('n').is_some_and(common_key),
            _ => false,
        };
    }
    COMMON.contains(&key) || stems(&key).iter().any(|stem| COMMON.contains(stem))
}

/// The words that `word` can be an inflection of: "fixes", "fixed", and
/// "fixing" give "fix".
fn stems(word: &str) -> Vec<String> {
    const ENDINGS: [(&str, &[&str]); 5] = [
        ("ies", &["y"]),
        ("ied", &["y"]),
        ("ed", &["", "e"]),
        ("ing", &["", "e"]),
        ("s", &[""]),
    ];
    let mut stems = Vec::new();
    for (suffix, endings) in ENDINGS {
        let Some(stem) = word.strip_suffix(suffix) else {
            continue;
        };
        if stem.chars().count() < 2 {
            continue;
        }
        stems.extend(endings.iter().map(|ending| format!("{stem}{ending}")));
        let mut tail = stem.chars().rev();
        if let (Some(last), Some(previous)) = (tail.next(), tail.next()) {
            if last == previous {
                stems.push(stem[..stem.len() - last.len_utf8()].to_string());
            }
        }
    }
    if let Some(stem) = word.strip_suffix("es") {
        if ["s", "x", "z", "ch", "sh", "o"]
            .iter()
            .any(|end| stem.len() > end.len() && stem.ends_with(end))
        {
            stems.push(stem.to_string());
        }
    }
    stems
}

fn function_word(key: &str) -> bool {
    let base = key.split(['\'', '’']).next().unwrap_or(key);
    FUNCTION_WORDS.contains(&base)
}

/// "3rd", "21st", "1990s"
fn ordinal(key: &str) -> bool {
    let rest = key.trim_start_matches(|c: char| c.is_ascii_digit());
    rest.len() < key.len() && ["st", "nd", "rd", "th", "s"].contains(&rest)
}

/// A capital first letter and no other capital, as at the start of a
/// sentence.
fn title_case(word: &str) -> bool {
    let mut chars = word.chars();
    chars.next().is_some_and(char::is_uppercase) && !chars.any(char::is_uppercase)
}

fn tokens(text: &str) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut sentence_start = true;
    for raw in text.split_whitespace() {
        let (word, cut_start, cut_end) = bare(raw);
        if word.is_empty() {
            if let Some(last) = tokens.last_mut() {
                last.joins_next = false;
            }
        } else {
            tokens.push(Token {
                word: word.to_string(),
                key: word.to_lowercase(),
                sentence_start,
                joins_previous: !cut_start,
                joins_next: !cut_end,
            });
        }
        sentence_start = ends_sentence(raw) || (word.is_empty() && sentence_start);
    }
    tokens
}

/// The word inside `raw` without the punctuation around it or a possessive
/// "'s", and whether anything came off its start and its end. The word is
/// empty when `raw` has no letter or digit.
fn bare(raw: &str) -> (&str, bool, bool) {
    let end = raw.trim_end_matches(|c: char| is_punctuation(c) && c != '#' && c != '+');
    let core = end.trim_start_matches(is_punctuation);
    let lead = &end[..end.len() - core.len()];
    let mut word = core;
    let mut marks = lead.chars().rev();
    if let Some(mark @ ('.' | '@' | '#')) = marks.next() {
        if marks.next() != Some(mark) {
            word = &end[lead.len() - mark.len_utf8()..];
        }
    }
    let cut_start = word.len() < end.len();
    let stem = word
        .strip_suffix("'s")
        .or_else(|| word.strip_suffix("’s"))
        .filter(|stem| stem.chars().any(char::is_alphanumeric));
    let cut_end = end.len() < raw.len() || stem.is_some();
    let word = stem.unwrap_or(word);
    if !word.chars().any(char::is_alphanumeric) {
        return ("", true, true);
    }
    (word, cut_start, cut_end)
}

fn is_punctuation(c: char) -> bool {
    c.is_ascii_punctuation()
        || matches!(c, '¡' | '¿' | '«' | '»' | '·')
        || ('\u{2000}'..='\u{206f}').contains(&c)
        || ('\u{3000}'..='\u{303f}').contains(&c)
        || (('\u{ff01}'..='\u{ff65}').contains(&c) && !c.is_alphanumeric())
}

fn ends_sentence(raw: &str) -> bool {
    raw.trim_end_matches(['"', '\'', ')', ']', '}', '”', '’', '»', '」', '』', '）'])
        .ends_with(['.', '!', '?', '…', '。', '！', '？'])
}

/// For each word of `new`, the index of the word of `old` that it matches in
/// a longest common subsequence of the two texts, compared in lower case.
fn align(old: &[Token], new: &[Token]) -> Vec<Option<usize>> {
    let mut aligned = vec![None; new.len()];
    let prefix = old
        .iter()
        .zip(new)
        .take_while(|(a, b)| a.key == b.key)
        .count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a.key == b.key)
        .count();
    for (index, slot) in aligned.iter_mut().enumerate().take(prefix) {
        *slot = Some(index);
    }
    for back in 1..=suffix {
        aligned[new.len() - back] = Some(old.len() - back);
    }

    let a = &old[prefix..old.len() - suffix];
    let b = &new[prefix..new.len() - suffix];
    if a.is_empty() || b.is_empty() || a.len().saturating_mul(b.len()) > MAX_CELLS {
        return aligned;
    }
    let width = b.len() + 1;
    let mut table = vec![0u32; (a.len() + 1) * width];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            table[i * width + j] = if a[i].key == b[j].key {
                table[(i + 1) * width + j + 1] + 1
            } else {
                table[(i + 1) * width + j].max(table[i * width + j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i].key == b[j].key {
            aligned[prefix + j] = Some(prefix + i);
            i += 1;
            j += 1;
        } else if table[(i + 1) * width + j] >= table[i * width + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    aligned
}

fn entries() -> impl Iterator<Item = &'static str> {
    COMMON_WORDS
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn propose(before: &str, after: &str) -> Vec<String> {
        proposals(before, after, &DomainVocabulary::default())
    }

    #[test]
    fn words_align_so_an_added_word_does_not_shift_the_rest() {
        assert_eq!(
            propose(
                "ask routh about the deploy",
                "please ask Ralph about the deploy"
            ),
            ["Ralph"]
        );
        assert_eq!(
            propose(
                "we use cube cuddle for the cluster",
                "we use kubectl for the cluster"
            ),
            ["kubectl"]
        );
        assert_eq!(
            propose("deploy the zorbix now", "deploy now"),
            Vec::<String>::new(),
            "a removed word teaches nothing"
        );
        assert!(propose("same text", "same text").is_empty());
        assert!(propose("", "").is_empty());
    }

    #[test]
    fn a_change_of_case_is_a_term() {
        assert_eq!(
            propose("deploy to kubernetes today", "deploy to Kubernetes today"),
            ["Kubernetes"]
        );
        assert_eq!(
            propose("we write go and use apple", "we write Go and use Apple"),
            ["Go", "Apple"],
            "even for a common word"
        );
        assert_eq!(propose("Ship the zsh fix", "Ship the Zsh fix"), ["Zsh"]);
    }

    #[test]
    fn grammar_capitals_are_not_terms() {
        assert!(propose("the build passed", "The build passed").is_empty());
        assert!(propose("so. the build passed", "So. The build passed").is_empty());
        assert!(propose("The build passed", "the build passed").is_empty());
        assert!(propose("i think i'm done", "I think I'm done").is_empty());
        assert!(
            propose("we ship it. sleep now", "we ship it. Sleep now").is_empty(),
            "a capital at the start of a sentence"
        );
    }

    #[test]
    fn common_words_and_their_inflections_are_not_terms() {
        assert!(propose("the build failed", "the build broke").is_empty());
        assert!(propose("we fix it", "we fixed it").is_empty());
        assert!(propose("he fix it", "he fixes it").is_empty());
        assert!(propose("we are test it", "we are testing it").is_empty());
        assert!(propose("it is state of art", "it is state-of-the-art").is_empty());
        assert!(propose("they do not", "they don't").is_empty());
        assert!(propose("on the 3 rd in 2026", "on the 3rd in 2026").is_empty());
        assert_eq!(
            propose("the bild failed", "the build failed fast with deploys"),
            ["deploys"],
            "a word outside the list is a term"
        );
    }

    #[test]
    fn a_name_that_books_capitalise_is_a_term_only_with_its_capital() {
        assert_eq!(propose("ask routh", "ask Ralph"), ["Ralph"]);
        assert!(propose("ask routh", "ask ralph").is_empty());
    }

    #[test]
    fn adjacent_changed_words_make_one_term() {
        assert_eq!(
            propose("check new relic for errors", "check New Relic for errors"),
            ["New Relic"]
        );
        assert_eq!(
            propose("new relic is down", "New Relic is down"),
            ["New Relic"],
            "a capital at the start of a sentence joins the name after it"
        );
        assert_eq!(
            propose("the kubernetes cluster", "The Kubernetes cluster"),
            ["Kubernetes"],
            "but a grammar word does not"
        );
        assert_eq!(
            propose("tell siobhan and routh", "tell Siobhán and Ralph"),
            ["Siobhán", "Ralph"],
            "an unchanged word separates two names"
        );
        assert_eq!(
            propose("send it to siobhan routh", "send it to Siobhán, Ralph"),
            ["Siobhán", "Ralph"],
            "so does a comma"
        );
        assert_eq!(
            propose("ask zorg. blix too", "ask Zorg. Blix too"),
            ["Zorg", "Blix"],
            "and the end of a sentence"
        );
    }

    #[test]
    fn a_long_run_of_changed_words_gives_one_term_for_each() {
        assert_eq!(
            propose("x", "Zorg Blix Quax Vorn Tazz Plim"),
            ["Zorg", "Blix", "Quax", "Vorn", "Tazz", "Plim"]
        );
    }

    #[test]
    fn punctuation_around_a_word_comes_off() {
        assert_eq!(
            propose("ship it to get hub.", "Ship it to GitHub."),
            ["GitHub"]
        );
        assert_eq!(
            propose(
                "we use dot net and c sharp",
                "we use (.NET) and C#, “Siobhán”"
            ),
            [".NET", "C#", "Siobhán"]
        );
        assert_eq!(
            propose("ask siobhan's team", "ask Siobhán's team"),
            ["Siobhán"]
        );
        assert_eq!(propose("we use c++", "we use C++!"), ["C++"]);
        assert_eq!(propose("ping sam", "ping @sam..."), ["@sam"]);
        assert!(propose("well so", "well... so").is_empty());
    }

    #[test]
    fn terms_already_in_the_vocabulary_are_left_out() {
        let mut vocabulary = DomainVocabulary::default();
        learn::learn_into(&mut vocabulary, "Kubernetes");
        let fix = |before, after| proposals(before, after, &vocabulary);
        assert!(fix("deploy to kubernetes", "deploy to Kubernetes").is_empty());
        assert_eq!(
            fix("deploy to kubernetes", "deploy to KUBERNETES"),
            ["KUBERNETES"],
            "a new spelling of a known term is a term"
        );
        assert_eq!(
            propose(
                "kubernetes and kubernetes",
                "Kubernetes and Kubernetes and kubernetes"
            ),
            ["Kubernetes"],
            "each term once"
        );
    }

    #[test]
    fn words_in_other_scripts_are_terms_unless_common() {
        assert_eq!(propose("привет мир", "привет Мир"), ["Мир"]);
        assert_eq!(propose("我喜欢猫", "我喜欢狗。"), ["我喜欢狗"]);
        assert_eq!(propose("नमस्ते दोस्त", "नमस्ते जगत्"), ["जगत्"]);
        assert_eq!(propose("مرحبا", "مرحبا عالم"), ["عالم"]);
        assert!(propose("он сказал привет", "он сказал people").is_empty());
        assert!(propose("ok", "🚀 ✅").is_empty());
        let long = "这是一个非常长的句子".repeat(8);
        assert!(propose("短", &long).is_empty(), "too long to learn");
    }

    #[test]
    fn a_long_text_with_a_small_fix_aligns() {
        let words = "the quick fox jumps over the lazy dog ".repeat(2_000);
        let before = format!("{words} siobhan {words}");
        let after = format!("{words} Siobhán {words}");
        assert_eq!(propose(&before, &after), ["Siobhán"]);
    }

    #[test]
    fn the_word_list_loads() {
        assert_eq!(COMMON.len(), 5_000);
        assert!(COMMON.contains("the") && COMMON.contains("ralph"));
        assert!(NAMES.contains("ralph") && !NAMES.contains("the"));
        assert!(!COMMON.iter().any(|word| word.starts_with('#')));
    }
}
