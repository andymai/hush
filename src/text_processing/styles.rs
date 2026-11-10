//! Writing styles. A style has a name, a line for the polish prompt, and a
//! switch that turns the LLM polish on or off for its take. `verbatim` is
//! built in and turns polish off. A take gets its style from a style chord
//! or `--style`, then from `profiles.app_styles`; without either, only the
//! tone profile shapes the text. A style with polish on and a prompt can
//! also rewrite the selected text.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// The built-in style: no polish, so the take keeps the words as spoken.
pub const VERBATIM: StyleName = StyleName::from_static("verbatim");

pub const MAX_NAME_LEN: usize = 32;

/// What a style name may contain, for error messages.
pub const NAME_RULE: &str = "use lowercase letters, digits, - and _, start with a letter or a \
                             digit, and use at most 32 characters";

/// A style name: lowercase ASCII letters, digits, `-`, and `_`, at most
/// [`MAX_NAME_LEN`] bytes. The bytes live inline, so the name is `Copy` and
/// can travel in hotkey events and session commands.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct StyleName {
    len: u8,
    bytes: [u8; MAX_NAME_LEN],
}

impl StyleName {
    const fn from_static(name: &str) -> Self {
        let source = name.as_bytes();
        let mut bytes = [0u8; MAX_NAME_LEN];
        let mut i = 0;
        while i < source.len() && i < MAX_NAME_LEN {
            bytes[i] = source[i];
            i += 1;
        }
        Self {
            len: i as u8,
            bytes,
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        let name = name.trim();
        let allowed =
            |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_';
        let first_ok = name
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
        (first_ok && name.len() <= MAX_NAME_LEN && name.bytes().all(allowed))
            .then(|| Self::from_static(name))
    }

    /// [`Self::parse`] with an error that names the value.
    pub fn from_name(name: &str) -> anyhow::Result<Self> {
        Self::parse(name)
            .ok_or_else(|| anyhow::anyhow!("Invalid style name '{}': {}", name.trim(), NAME_RULE))
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..usize::from(self.len)]).unwrap_or_default()
    }
}

impl fmt::Display for StyleName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for StyleName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StyleName({})", self.as_str())
    }
}

/// One `[styles.<name>]` table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleConfig {
    /// A line for the polish prompt, such as "Formal English, no contractions"
    pub prompt: String,
    /// The LLM polishes takes in this style, whatever `llm.polish` says
    pub polish: bool,
}

impl Default for StyleConfig {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            polish: true,
        }
    }
}

impl StyleConfig {
    /// The style can rewrite the selection: polish is on, and its prompt is
    /// the instruction.
    pub fn can_rewrite(&self) -> bool {
        self.polish && !self.prompt.trim().is_empty()
    }
}

/// The style of one take, resolved from its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TakeStyle {
    pub name: StyleName,
    pub prompt: String,
    pub polish: bool,
}

impl TakeStyle {
    pub fn can_rewrite(&self) -> bool {
        self.polish && !self.prompt.is_empty()
    }
}

/// The style that `name` names: `verbatim`, or a table under `[styles]`.
pub fn resolve(styles: &BTreeMap<String, StyleConfig>, name: StyleName) -> Option<TakeStyle> {
    if name == VERBATIM {
        return Some(TakeStyle {
            name,
            prompt: String::new(),
            polish: false,
        });
    }
    styles.get(name.as_str()).map(|style| TakeStyle {
        name,
        prompt: style.prompt.trim().to_string(),
        polish: style.polish,
    })
}

/// `verbatim`, then each user style with a valid name.
pub fn names(styles: &BTreeMap<String, StyleConfig>) -> Vec<StyleName> {
    std::iter::once(VERBATIM)
        .chain(
            styles
                .keys()
                .filter_map(|name| StyleName::parse(name))
                .filter(|name| *name != VERBATIM),
        )
        .collect()
}

/// The style of one take: `once` (from `--style` or a style chord), else
/// the style for the app.
pub fn for_take(once: Option<StyleName>, app: Option<StyleName>) -> Option<StyleName> {
    once.or(app)
}

/// Each name under `[styles]` must be a valid name other than `verbatim`.
pub fn validate(styles: &BTreeMap<String, StyleConfig>) -> anyhow::Result<()> {
    for name in styles.keys() {
        let parsed = StyleName::parse(name).filter(|parsed| parsed.as_str() == name);
        match parsed {
            None => {
                return Err(anyhow::anyhow!(
                    "Invalid style name '{}' under [styles]: {}",
                    name,
                    NAME_RULE
                ))
            },
            Some(VERBATIM) => {
                return Err(anyhow::anyhow!(
                    "[styles.verbatim]: verbatim is a built-in style, so give your style \
                     another name"
                ))
            },
            Some(_) => {},
        }
    }
    Ok(())
}

/// The style that `name` names, or an error that says where the name is.
pub fn known(
    styles: &BTreeMap<String, StyleConfig>,
    name: &str,
    place: &str,
) -> anyhow::Result<TakeStyle> {
    StyleName::parse(name)
        .and_then(|parsed| resolve(styles, parsed))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Unknown style '{}' {}: use verbatim or add a [styles.{}] table",
                name.trim(),
                place,
                name.trim()
            )
        })
}

/// The styles that can rewrite the selection.
pub fn rewriters(styles: &BTreeMap<String, StyleConfig>) -> Vec<StyleName> {
    styles
        .iter()
        .filter(|(_, style)| style.can_rewrite())
        .filter_map(|(name, _)| StyleName::parse(name))
        .filter(|name| *name != VERBATIM)
        .collect()
}

/// The style that `name` names, when it can rewrite the selection, or an
/// error that says where the name is and why it cannot.
pub fn rewriter(
    styles: &BTreeMap<String, StyleConfig>,
    name: &str,
    place: &str,
) -> anyhow::Result<TakeStyle> {
    let style = known(styles, name, place)?;
    if !style.polish {
        return Err(anyhow::anyhow!(
            "The style {} {} turns polish off, so it cannot rewrite text: use a style with \
             polish = true and a prompt",
            style.name,
            place
        ));
    }
    if style.prompt.is_empty() {
        return Err(anyhow::anyhow!(
            "The style {} {} has no prompt, so it cannot rewrite text: give it a prompt",
            style.name,
            place
        ));
    }
    Ok(style)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(entries: &[(&str, &str, bool)]) -> BTreeMap<String, StyleConfig> {
        entries
            .iter()
            .map(|(name, prompt, polish)| {
                (
                    name.to_string(),
                    StyleConfig {
                        prompt: prompt.to_string(),
                        polish: *polish,
                    },
                )
            })
            .collect()
    }

    #[test]
    fn names_are_short_lowercase_words() {
        for good in ["casual", "formal-2", "my_style", "9lives", " slack "] {
            let name = StyleName::parse(good).unwrap();
            assert_eq!(name.as_str(), good.trim());
            assert_eq!(name.to_string(), good.trim());
        }
        for bad in [
            "",
            "Casual",
            "-x",
            "_x",
            "with space",
            "émoji",
            &"a".repeat(33),
        ] {
            assert!(StyleName::parse(bad).is_none(), "{bad:?}");
        }
        assert!(StyleName::parse(&"a".repeat(32)).is_some());
        assert_eq!(VERBATIM.as_str(), "verbatim");
        assert_eq!(StyleName::parse("verbatim"), Some(VERBATIM));
        let err = StyleName::from_name("Bad Name").unwrap_err().to_string();
        assert!(err.contains("'Bad Name'"), "{err}");
    }

    #[test]
    fn the_chord_or_flag_wins_over_the_app_style() {
        let casual = StyleName::parse("casual");
        let formal = StyleName::parse("formal");
        assert_eq!(for_take(formal, casual), formal);
        assert_eq!(for_take(None, casual), casual);
        assert_eq!(for_take(formal, None), formal);
        assert_eq!(for_take(None, None), None, "the tone profile alone");
    }

    #[test]
    fn verbatim_is_built_in_and_turns_polish_off() {
        let styles = user(&[("formal", "  Formal English. ", true)]);
        let verbatim = resolve(&styles, VERBATIM).unwrap();
        assert!(!verbatim.polish);
        assert!(verbatim.prompt.is_empty());
        let formal = resolve(&styles, StyleName::parse("formal").unwrap()).unwrap();
        assert!(formal.polish);
        assert_eq!(formal.prompt, "Formal English.");
        assert_eq!(resolve(&styles, StyleName::parse("nosuch").unwrap()), None);
        assert_eq!(
            names(&styles),
            vec![VERBATIM, StyleName::parse("formal").unwrap()]
        );
    }

    #[test]
    fn user_names_are_checked() {
        assert!(validate(&user(&[("casual", "Brief.", true)])).is_ok());
        let err = validate(&user(&[("verbatim", "x", true)]))
            .unwrap_err()
            .to_string();
        assert!(err.contains("built-in"), "{err}");
        let err = validate(&user(&[("Casual", "x", true)]))
            .unwrap_err()
            .to_string();
        assert!(err.contains("'Casual'"), "{err}");
        assert!(validate(&user(&[(" casual", "x", true)])).is_err());

        let styles = user(&[("casual", "Brief.", true)]);
        assert!(known(&styles, "verbatim", "in hotkey.styles").is_ok());
        let err = known(&styles, "nosuch", "for F13 in hotkey.styles")
            .unwrap_err()
            .to_string();
        assert!(err.contains("'nosuch' for F13 in hotkey.styles"), "{err}");
    }

    #[test]
    fn only_a_polished_style_with_a_prompt_can_rewrite() {
        let styles = user(&[
            ("formal", "Formal English.", true),
            ("raw", "Formal English.", false),
            ("empty", "  ", true),
        ]);
        assert!(styles["formal"].can_rewrite());
        assert!(!styles["raw"].can_rewrite());
        assert!(!styles["empty"].can_rewrite());
        assert_eq!(
            rewriters(&styles),
            vec![StyleName::parse("formal").unwrap()]
        );

        let formal = rewriter(&styles, "formal", "for hush rewrite").unwrap();
        assert_eq!(formal.prompt, "Formal English.");
        assert!(formal.can_rewrite());
        for (name, needle) in [
            ("verbatim", "turns polish off"),
            ("raw", "turns polish off"),
            ("empty", "has no prompt"),
            ("nosuch", "Unknown style 'nosuch'"),
        ] {
            let err = rewriter(&styles, name, "for Super+F in hotkey.rewrite")
                .unwrap_err()
                .to_string();
            assert!(err.contains(needle), "{name}: {err}");
            assert!(
                err.contains("for Super+F in hotkey.rewrite"),
                "{name}: {err}"
            );
        }
    }
}
