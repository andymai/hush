//! The languages Whisper knows and the language of each take.
//!
//! A take gets its language in this order: `hush start --lang` or a
//! language chord, then the choice from the tray menu, then
//! `transcription.language`. The tray choice stays in the state folder, so
//! it survives a restart of the daemon.

use crate::config::paths;
use anyhow::{Context, Result};
use std::fmt;
use std::path::{Path, PathBuf};

/// Every language Whisper knows, in Whisper's order: the index is
/// Whisper's language id.
pub const LANGUAGES: &[(&str, &str)] = &[
    ("en", "English"),
    ("zh", "Chinese"),
    ("de", "German"),
    ("es", "Spanish"),
    ("ru", "Russian"),
    ("ko", "Korean"),
    ("fr", "French"),
    ("ja", "Japanese"),
    ("pt", "Portuguese"),
    ("tr", "Turkish"),
    ("pl", "Polish"),
    ("ca", "Catalan"),
    ("nl", "Dutch"),
    ("ar", "Arabic"),
    ("sv", "Swedish"),
    ("it", "Italian"),
    ("id", "Indonesian"),
    ("hi", "Hindi"),
    ("fi", "Finnish"),
    ("vi", "Vietnamese"),
    ("he", "Hebrew"),
    ("uk", "Ukrainian"),
    ("el", "Greek"),
    ("ms", "Malay"),
    ("cs", "Czech"),
    ("ro", "Romanian"),
    ("da", "Danish"),
    ("hu", "Hungarian"),
    ("ta", "Tamil"),
    ("no", "Norwegian"),
    ("th", "Thai"),
    ("ur", "Urdu"),
    ("hr", "Croatian"),
    ("bg", "Bulgarian"),
    ("lt", "Lithuanian"),
    ("la", "Latin"),
    ("mi", "Maori"),
    ("ml", "Malayalam"),
    ("cy", "Welsh"),
    ("sk", "Slovak"),
    ("te", "Telugu"),
    ("fa", "Persian"),
    ("lv", "Latvian"),
    ("bn", "Bengali"),
    ("sr", "Serbian"),
    ("az", "Azerbaijani"),
    ("sl", "Slovenian"),
    ("kn", "Kannada"),
    ("et", "Estonian"),
    ("mk", "Macedonian"),
    ("br", "Breton"),
    ("eu", "Basque"),
    ("is", "Icelandic"),
    ("hy", "Armenian"),
    ("ne", "Nepali"),
    ("mn", "Mongolian"),
    ("bs", "Bosnian"),
    ("kk", "Kazakh"),
    ("sq", "Albanian"),
    ("sw", "Swahili"),
    ("gl", "Galician"),
    ("mr", "Marathi"),
    ("pa", "Punjabi"),
    ("si", "Sinhala"),
    ("km", "Khmer"),
    ("sn", "Shona"),
    ("yo", "Yoruba"),
    ("so", "Somali"),
    ("af", "Afrikaans"),
    ("oc", "Occitan"),
    ("ka", "Georgian"),
    ("be", "Belarusian"),
    ("tg", "Tajik"),
    ("sd", "Sindhi"),
    ("gu", "Gujarati"),
    ("am", "Amharic"),
    ("yi", "Yiddish"),
    ("lo", "Lao"),
    ("uz", "Uzbek"),
    ("fo", "Faroese"),
    ("ht", "Haitian Creole"),
    ("ps", "Pashto"),
    ("tk", "Turkmen"),
    ("nn", "Nynorsk"),
    ("mt", "Maltese"),
    ("sa", "Sanskrit"),
    ("lb", "Luxembourgish"),
    ("my", "Myanmar"),
    ("bo", "Tibetan"),
    ("tl", "Tagalog"),
    ("mg", "Malagasy"),
    ("as", "Assamese"),
    ("tt", "Tatar"),
    ("haw", "Hawaiian"),
    ("ln", "Lingala"),
    ("ha", "Hausa"),
    ("ba", "Bashkir"),
    ("jw", "Javanese"),
    ("su", "Sundanese"),
    ("yue", "Cantonese"),
];

const AUTO: &str = "auto";

/// A language code that Whisper knows, or `auto` to let Whisper detect the
/// language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Language(&'static str);

impl Language {
    pub const AUTO: Language = Language(AUTO);

    /// The language for a code such as `de`, or `auto`. Case matters, as it
    /// does for Whisper.
    pub fn parse(code: &str) -> Option<Self> {
        let code = code.trim();
        if code == AUTO {
            return Some(Self::AUTO);
        }
        LANGUAGES
            .iter()
            .find(|(known, _)| *known == code)
            .map(|(known, _)| Language(known))
    }

    /// [`Self::parse`] with an error that names the code.
    pub fn from_code(code: &str) -> Result<Self> {
        Self::parse(code).with_context(|| {
            format!(
                "Unknown language '{}': use a Whisper language code such as en or de, or auto",
                code.trim()
            )
        })
    }

    pub fn code(self) -> &'static str {
        self.0
    }

    pub fn is_auto(self) -> bool {
        self.0 == AUTO
    }

    /// The English name, such as `German`, or `Auto`.
    pub fn name(self) -> &'static str {
        if self.is_auto() {
            return "Auto";
        }
        LANGUAGES
            .iter()
            .find(|(code, _)| *code == self.0)
            .map_or(self.0, |(_, name)| name)
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// The language of one take: `once` (from `--lang` or a language chord),
/// else the tray choice, else the config language.
pub fn for_take(once: Option<Language>, chosen: Option<Language>, config: Language) -> Language {
    once.or(chosen).unwrap_or(config)
}

/// `$XDG_STATE_HOME/hush/language`: the language that the tray menu chose.
pub fn choice_path() -> PathBuf {
    paths::state_dir().join("language")
}

/// The saved tray choice, or `None` when there is none or the file holds a
/// code that Whisper does not know.
pub fn load_choice(path: &Path) -> Option<Language> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| Language::parse(&text))
}

/// Save the tray choice. `None` removes the file, so takes follow the config
/// language again.
pub fn save_choice(path: &Path, choice: Option<Language>) -> Result<()> {
    let Some(language) = choice else {
        return match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                Err(e).with_context(|| format!("Failed to remove {}", path.display()))
            },
            _ => Ok(()),
        };
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    std::fs::write(path, format!("{}\n", language.code()))
        .with_context(|| format!("Failed to write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lang(code: &str) -> Language {
        Language::parse(code).unwrap()
    }

    #[test]
    fn whisper_codes_and_auto_parse() {
        for code in ["en", "de", "haw", "yue", "jw", "auto", " de "] {
            assert!(Language::parse(code).is_some(), "{code}");
        }
        for code in ["", "EN", "english", "xx", "jv", "de-DE"] {
            assert!(Language::parse(code).is_none(), "{code}");
        }
        assert_eq!(LANGUAGES.len(), 100);
        assert_eq!(lang("de").name(), "German");
        assert_eq!(Language::AUTO.name(), "Auto");
        assert!(lang("auto").is_auto());
        assert_eq!(lang("yue").to_string(), "yue");
        let err = Language::from_code("klingon").unwrap_err().to_string();
        assert!(err.contains("'klingon'"), "{err}");
    }

    #[test]
    fn a_take_prefers_its_own_language_then_the_tray_then_the_config() {
        let (en, de, fr) = (lang("en"), lang("de"), lang("fr"));
        assert_eq!(for_take(None, None, en), en);
        assert_eq!(
            for_take(None, Some(de), en),
            de,
            "the tray beats the config"
        );
        assert_eq!(
            for_take(Some(fr), Some(de), en),
            fr,
            "--lang or a chord wins"
        );
        assert_eq!(for_take(Some(fr), None, en), fr);
        assert_eq!(
            for_take(None, Some(Language::AUTO), en),
            Language::AUTO,
            "the tray can choose detection"
        );
    }

    #[test]
    fn the_tray_choice_survives_in_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state/language");
        assert_eq!(load_choice(&path), None);
        save_choice(&path, Some(lang("de"))).unwrap();
        assert_eq!(load_choice(&path), Some(lang("de")));
        save_choice(&path, Some(Language::AUTO)).unwrap();
        assert_eq!(load_choice(&path), Some(Language::AUTO));
        save_choice(&path, None).unwrap();
        assert!(!path.exists());
        save_choice(&path, None).unwrap();
        std::fs::write(&path, "xx\n").unwrap();
        assert_eq!(load_choice(&path), None, "an unknown code is no choice");
    }
}
