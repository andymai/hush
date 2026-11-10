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

/// One prompt term comes three times in a row in the output.
pub fn has_prompt_term_run(output: &str, prompt: &str) -> bool {
    let terms = prompt_terms(prompt);
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
    false
}

/// Prompt terms are most of the output, which has three words or more.
pub fn is_mostly_prompt_terms(output: &str, prompt: &str) -> bool {
    let terms = prompt_terms(prompt);
    let words = words(output);
    let from_prompt = words.iter().filter(|word| terms.contains(*word)).count();
    words.len() >= ECHO_MIN_WORDS && from_prompt * 2 > words.len()
}

/// The output can repeat the prompt instead of the speech: one prompt term
/// comes three times in a row, or prompt terms are most of the words.
pub fn is_prompt_echo(output: &str, prompt: &str) -> bool {
    has_prompt_term_run(output, prompt) || is_mostly_prompt_terms(output, prompt)
}

/// Which transcript to keep after the first one looked like an echo of the
/// prompt and the retry ran without the prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EchoChoice {
    /// Neither: the retry heard no words, or it echoes the prompt too.
    Drop,
    /// The retry: the first transcript looped on one prompt term.
    Retry,
    /// The first transcript: the retry heard speech, so the prompt terms
    /// were real, and the prompt spells them better.
    Original,
}

pub fn after_echo(first: &str, retry: &str, prompt: &str) -> EchoChoice {
    if is_punctuation_only(retry) || is_prompt_echo(retry, prompt) {
        EchoChoice::Drop
    } else if has_prompt_term_run(first, prompt) {
        EchoChoice::Retry
    } else {
        EchoChoice::Original
    }
}

/// Why a transcript gets a retry without the prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Echo<'a> {
    /// The output echoes `terms`, the window title and learned terms.
    Terms(&'a str),
    /// One word of the prompt loops in the output.
    Loop,
}

impl Echo<'_> {
    /// Which transcript to keep after the retry.
    pub fn choose(self, first: &str, retry: &str) -> EchoChoice {
        match self {
            Echo::Terms(terms) => after_echo(first, retry, terms),
            Echo::Loop if is_punctuation_only(retry) => EchoChoice::Drop,
            Echo::Loop => EchoChoice::Retry,
        }
    }
}

/// Whether `output` repeats the prompt instead of the speech. `terms` is
/// the start of `prompt` from the window and the learned terms; the rest of
/// `prompt` is the text before a part of a take. Speech can repeat that
/// text, so it counts only when one of its words loops.
pub fn echo_in<'a>(output: &str, prompt: Option<&str>, terms: Option<&'a str>) -> Option<Echo<'a>> {
    if let Some(terms) = terms.filter(|terms| is_prompt_echo(output, terms)) {
        return Some(Echo::Terms(terms));
    }
    prompt
        .filter(|prompt| has_prompt_term_run(output, prompt))
        .map(|_| Echo::Loop)
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
    fn the_two_echo_signs_are_told_apart() {
        let run = "Kubernetes, Kubernetes, Kubernetes.";
        assert!(has_prompt_term_run(run, PROMPT));
        assert!(!has_prompt_term_run(PROMPT, PROMPT));
        assert!(is_mostly_prompt_terms(PROMPT, PROMPT));
        assert!(!is_mostly_prompt_terms("Ship the Kubernetes fix.", PROMPT));
        assert!(!has_prompt_term_run("anything at all", ""));
    }

    #[test]
    fn a_retry_with_no_words_or_another_echo_drops_the_take() {
        for first in [PROMPT, "PostgreSQL PostgreSQL PostgreSQL"] {
            for retry in [
                "",
                "  ",
                "...",
                "PostgreSQL, PostgreSQL, PostgreSQL, the end",
            ] {
                assert_eq!(
                    after_echo(first, retry, PROMPT),
                    EchoChoice::Drop,
                    "{first:?} then {retry:?}"
                );
            }
            assert_eq!(
                after_echo(first, "Kubernetes, PostgreSQL, main code.", PROMPT),
                EchoChoice::Drop,
                "the retry is mostly prompt terms too"
            );
        }
    }

    #[test]
    fn a_loop_on_one_term_gives_way_to_the_retry() {
        assert_eq!(
            after_echo(
                "So Kubernetes Kubernetes Kubernetes Kubernetes",
                "So the cluster is up.",
                PROMPT
            ),
            EchoChoice::Retry
        );
        assert_eq!(
            after_echo(
                "Kubernetes, Kubernetes, Kubernetes now",
                "Kubernetes now",
                PROMPT
            ),
            EchoChoice::Retry
        );
    }

    #[test]
    fn a_part_may_repeat_the_text_before_it() {
        let carried = "Als Gregor Samsa eines Morgens erwachte.";
        let prompt = format!("{} {}", PROMPT, carried);
        let repeat = "Als Gregor Samsa eines Morgens erwachte.";
        assert!(
            is_prompt_echo(repeat, &prompt),
            "the whole prompt would drop it"
        );
        assert_eq!(echo_in(repeat, Some(&prompt), Some(PROMPT)), None);
        assert_eq!(echo_in(repeat, Some(carried), None), None);

        let looped = "Samsa Samsa Samsa Samsa";
        let echo = echo_in(looped, Some(&prompt), Some(PROMPT));
        assert_eq!(
            echo,
            Some(Echo::Loop),
            "a loop on a carried word still counts"
        );
        assert_eq!(
            Echo::Loop.choose(looped, repeat),
            EchoChoice::Retry,
            "the retry had no prompt to echo"
        );
        assert_eq!(Echo::Loop.choose(looped, " . "), EchoChoice::Drop);

        assert_eq!(
            echo_in(PROMPT, Some(&prompt), Some(PROMPT)),
            Some(Echo::Terms(PROMPT)),
            "an echo of the window and terms counts as before"
        );
    }

    #[test]
    fn one_pass_keeps_the_rules_of_the_whole_prompt() {
        for output in [
            PROMPT,
            "So Kubernetes, Kubernetes, Kubernetes and then more words follow here.",
            "Move the PostgreSQL backup to the new Kubernetes cluster.",
        ] {
            let echo = echo_in(output, Some(PROMPT), Some(PROMPT));
            assert_eq!(echo.is_some(), is_prompt_echo(output, PROMPT), "{output:?}");
            if let Some(echo) = echo {
                assert_eq!(
                    echo.choose(output, "the cluster is up"),
                    after_echo(output, "the cluster is up", PROMPT)
                );
            }
        }
        assert_eq!(echo_in("anything", None, None), None);
    }

    #[test]
    fn a_real_list_of_terms_keeps_the_prompted_spelling() {
        assert_eq!(
            after_echo(
                "Kubernetes, PostgreSQL, and code review.",
                "Cooper Netties, Postgres QL, and code review.",
                PROMPT
            ),
            EchoChoice::Original
        );
    }
}
