//! Checks on a transcript after Whisper returns it: text that Whisper invents
//! for silence or noise, and output that only repeats the prompt.

use std::collections::HashSet;

/// Whole transcripts that Whisper invents for silence or noise, most of them
/// from the video subtitles it learned from. Single common words such as
/// "you" are not in the list, because people dictate them too.
pub const BUILT_IN_DROP_PHRASES: &[&str] = &[
    "thank you for watching",
    "thank you very much for watching",
    "thank you so much for watching",
    "thanks for watching",
    "please subscribe",
    "please subscribe to my channel",
    "subscribe to my channel",
    "like and subscribe",
    "please like and subscribe",
    "don't forget to like and subscribe",
    "see you in the next video",
    "transcription by castingwords",
    "subtitles by the amara.org community",
    "untertitel der amara.org-community",
    "untertitel im auftrag des zdf für funk, 2017",
    "sous-titres réalisés par la communauté d'amara.org",
    "sous-titres réalisés para la communauté d'amara.org",
    "subtítulos realizados por la comunidad de amara.org",
    "sottotitoli creati dalla comunità amara.org",
];

/// A prompt term this many times in a row is an echo of the prompt.
const ECHO_RUN: usize = 3;

/// The share rule needs this many words, so that a dictated name or two is
/// never taken for an echo.
const ECHO_MIN_WORDS: usize = 3;

/// Prompt words too common to show an echo. Words under three letters are
/// skipped as well.
const COMMON_WORDS: &[&str] = &[
    "about", "after", "all", "and", "any", "are", "but", "can", "for", "from", "get", "had", "has",
    "have", "her", "his", "how", "its", "new", "not", "now", "one", "our", "out", "she", "that",
    "the", "their", "them", "then", "there", "they", "this", "was", "what", "when", "who", "why",
    "will", "with", "you", "your",
];

/// Lowercase words of letters and digits, one space apart. Apostrophes join
/// their word ("don't" becomes "dont"); other punctuation, brackets, and
/// symbols split words.
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '\'' | '\u{2019}') {
            continue;
        }
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn words(text: &str) -> Vec<String> {
    normalize(text)
        .split(' ')
        .filter(|word| !word.is_empty())
        .map(str::to_string)
        .collect()
}

/// The phrases to drop, normalized: `configured` when it is set, else the
/// built-in list.
pub fn drop_list(configured: Option<&[String]>) -> Vec<String> {
    let phrases: Vec<String> = match configured {
        Some(list) => list.iter().map(|phrase| normalize(phrase)).collect(),
        None => BUILT_IN_DROP_PHRASES
            .iter()
            .map(|phrase| normalize(phrase))
            .collect(),
    };
    phrases
        .into_iter()
        .filter(|phrase| !phrase.is_empty())
        .collect()
}

/// The transcript has no letter or digit.
pub fn is_punctuation_only(text: &str) -> bool {
    normalize(text).is_empty()
}

/// The whole transcript is one of `phrases`, which [`drop_list`] made.
pub fn is_drop_phrase(text: &str, phrases: &[String]) -> bool {
    let text = normalize(text);
    !text.is_empty() && phrases.contains(&text)
}

/// The transcript is not worth inserting: punctuation only, or a phrase
/// that Whisper invents.
pub fn is_invented(text: &str, phrases: &[String]) -> bool {
    is_punctuation_only(text) || is_drop_phrase(text, phrases)
}

/// The words of the prompt that can show an echo: three letters or more,
/// and not a common word.
fn prompt_terms(prompt: &str) -> HashSet<String> {
    words(prompt)
        .into_iter()
        .filter(|word| word.chars().count() >= 3 && !COMMON_WORDS.contains(&word.as_str()))
        .collect()
}

/// The output repeats the prompt instead of the speech: one prompt term
/// comes three times in a row, or prompt terms are most of three or more
/// words.
pub fn is_prompt_echo(output: &str, prompt: &str) -> bool {
    let terms = prompt_terms(prompt);
    if terms.is_empty() {
        return false;
    }
    let words = words(output);
    let mut run = 0;
    for (i, word) in words.iter().enumerate() {
        if !terms.contains(word) {
            run = 0;
            continue;
        }
        run = if i > 0 && words[i - 1] == *word {
            run + 1
        } else {
            1
        };
        if run >= ECHO_RUN {
            return true;
        }
    }
    let from_prompt = words.iter().filter(|word| terms.contains(*word)).count();
    words.len() >= ECHO_MIN_WORDS && from_prompt * 2 > words.len()
}

/// The words of `text` that are not prompt terms.
pub fn real_word_count(text: &str, prompt: &str) -> usize {
    let terms = prompt_terms(prompt);
    words(text)
        .iter()
        .filter(|word| !terms.contains(*word))
        .count()
}

/// After `first` echoed the prompt, keep `second` (transcribed without the
/// prompt) only when it has more real words and does not echo the prompt.
pub fn keep_second_pass(first: &str, second: &str, prompt: &str) -> bool {
    real_word_count(second, prompt) > real_word_count(first, prompt)
        && !is_prompt_echo(second, prompt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn built_in() -> Vec<String> {
        drop_list(None)
    }

    #[test]
    fn normalizing_ignores_case_punctuation_brackets_and_spaces() {
        assert_eq!(
            normalize("  Thanks   for WATCHING!! "),
            "thanks for watching"
        );
        assert_eq!(normalize("[Thanks for watching.]"), "thanks for watching");
        assert_eq!(normalize("Don’t forget"), "dont forget");
        assert_eq!(normalize("Amara.org-Community"), "amara org community");
        assert_eq!(normalize("...?!"), "");
    }

    #[test]
    fn punctuation_and_whitespace_alone_are_dropped() {
        for text in ["", "   ", ".", "...", " - ", "?!", "♪ ♪", "( )"] {
            assert!(is_punctuation_only(text), "{text:?}");
            assert!(is_invented(text, &built_in()), "{text:?}");
        }
        for text in ["ok", "42", "Ja.", "日本"] {
            assert!(!is_punctuation_only(text), "{text:?}");
        }
    }

    #[test]
    fn the_whole_transcript_must_match_a_phrase() {
        let phrases = built_in();
        for text in [
            "Thank you for watching.",
            "THANKS FOR WATCHING!",
            "(Thanks for watching)",
            "  please   subscribe ",
            "Don't forget to like and subscribe!",
            "Subtitles by the Amara.org community",
            "Untertitel der Amara.org-Community",
        ] {
            assert!(is_drop_phrase(text, &phrases), "{text:?}");
        }
        for text in [
            "Thanks for watching the build for me.",
            "Please subscribe me to the list.",
            "you",
            "Thank you.",
            "Thank you",
            "Bye.",
        ] {
            assert!(!is_drop_phrase(text, &phrases), "{text:?}");
        }
    }

    #[test]
    fn the_built_in_list_holds_no_single_word() {
        for phrase in built_in() {
            assert!(phrase.contains(' '), "{phrase:?} is a single word");
        }
    }

    #[test]
    fn a_configured_list_replaces_the_built_in_one() {
        let phrases = drop_list(Some(&["Over and out.".to_string(), " ".to_string()]));
        assert_eq!(phrases, vec!["over and out".to_string()]);
        assert!(is_drop_phrase("over, and OUT", &phrases));
        assert!(!is_drop_phrase("Thanks for watching", &phrases));

        let nothing = drop_list(Some(&[]));
        assert!(!is_invented("Thanks for watching", &nothing));
        assert!(is_invented("...", &nothing), "punctuation still goes");
    }

    const PROMPT: &str = "hush/src/main.rs - Code. Kubernetes, PostgreSQL.";

    #[test]
    fn a_term_three_times_in_a_row_is_an_echo() {
        assert!(is_prompt_echo(
            "So Kubernetes, Kubernetes, Kubernetes and then more words follow here.",
            PROMPT
        ));
        assert!(!is_prompt_echo(
            "Kubernetes runs pods, Kubernetes runs jobs, and Kubernetes scales them.",
            PROMPT
        ));
        assert!(
            !is_prompt_echo("the the the the", "The Rust Book"),
            "a common word is not a prompt term"
        );
    }

    #[test]
    fn mostly_prompt_terms_is_an_echo() {
        assert!(is_prompt_echo(PROMPT, PROMPT), "the prompt itself");
        assert!(is_prompt_echo("Kubernetes, PostgreSQL, main code.", PROMPT));
        assert!(!is_prompt_echo(
            "Move the PostgreSQL backup to the new Kubernetes cluster.",
            PROMPT
        ));
    }

    #[test]
    fn a_dictated_name_or_two_is_not_an_echo() {
        assert!(!is_prompt_echo("PostgreSQL.", PROMPT));
        assert!(!is_prompt_echo("Kubernetes, PostgreSQL.", PROMPT));
        assert!(!is_prompt_echo("anything at all", ""));
        assert!(!is_prompt_echo("", PROMPT));
    }

    #[test]
    fn real_words_are_the_words_outside_the_prompt() {
        assert_eq!(real_word_count(PROMPT, PROMPT), 1, "only 'rs' is too short");
        assert_eq!(real_word_count("Ship the Kubernetes fix.", PROMPT), 3);
        assert_eq!(real_word_count("", PROMPT), 0);
    }

    #[test]
    fn the_second_pass_wins_only_with_more_real_text_and_no_echo() {
        let echo = "Kubernetes, PostgreSQL, Kubernetes, PostgreSQL.";
        assert!(keep_second_pass(echo, "Ship the fix today.", PROMPT));
        assert!(!keep_second_pass(echo, "", PROMPT), "nothing is not more");
        assert!(
            !keep_second_pass(echo, "PostgreSQL, PostgreSQL, PostgreSQL, the end", PROMPT),
            "an echo again"
        );
        assert!(
            !keep_second_pass(
                "Kubernetes Kubernetes Kubernetes now",
                "Kubernetes now",
                PROMPT
            ),
            "no more real words"
        );
    }
}
