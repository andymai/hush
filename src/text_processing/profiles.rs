//! Per-application tone. The focused window's class (and, inside a browser,
//! its title) picks an [`AppKind`]; each kind carries a [`TextProfile`] that
//! shapes the offline cleanup and, when polishing is on, the LLM prompt.

use crate::text::WindowInfo;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AppKind {
    Terminal,
    Editor,
    Chat,
    Mail,
    Docs,
    Browser,
    #[default]
    Other,
}

/// How dictated text should read in a kind of application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextProfile {
    pub kind: AppKind,
    /// Uppercase the first letter
    pub capitalize_first: bool,
    /// Add a period when the text ends without punctuation
    pub ending_punctuation: bool,
    /// One line for the polish prompt
    pub tone: &'static str,
}

/// Extra window classes per kind, from `[profiles]` in the config.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ProfilesConfig {
    /// Adapt the output to the focused application
    pub enabled: bool,
    pub terminal: Vec<String>,
    pub editor: Vec<String>,
    pub chat: Vec<String>,
    pub mail: Vec<String>,
    pub docs: Vec<String>,
    pub browser: Vec<String>,
}

impl ProfilesConfig {
    pub fn defaults() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }

    fn patterns(&self, kind: AppKind) -> &[String] {
        match kind {
            AppKind::Terminal => &self.terminal,
            AppKind::Editor => &self.editor,
            AppKind::Chat => &self.chat,
            AppKind::Mail => &self.mail,
            AppKind::Docs => &self.docs,
            AppKind::Browser => &self.browser,
            AppKind::Other => &[],
        }
    }
}

const KINDS: [AppKind; 6] = [
    AppKind::Terminal,
    AppKind::Editor,
    AppKind::Chat,
    AppKind::Mail,
    AppKind::Docs,
    AppKind::Browser,
];

/// Lowercase substrings matched against the window class.
fn builtin_class_patterns(kind: AppKind) -> &'static [&'static str] {
    match kind {
        AppKind::Terminal => &[
            "konsole",
            "alacritty",
            "kitty",
            "foot",
            "wezterm",
            "gnome-terminal",
            "gnome.terminal",
            "ptyxis",
            "ghostty",
            "xterm",
            "urxvt",
            "terminator",
            "tilix",
            "terminal",
            "yakuake",
            "st-256color",
            "warp",
            "rio",
            "contour",
            "tabby",
            "blackbox",
        ],
        AppKind::Editor => &[
            "code",
            "codium",
            "cursor",
            "zed",
            "jetbrains",
            "idea",
            "pycharm",
            "clion",
            "rustrover",
            "webstorm",
            "goland",
            "sublime",
            "neovide",
            "emacs",
            "gvim",
            "kate",
            "gedit",
            "helix",
            "lapce",
        ],
        AppKind::Chat => &[
            "slack",
            "discord",
            "telegram",
            "signal",
            "element",
            "whatsapp",
            "teams",
            "zulip",
            "mattermost",
            "rocket.chat",
            "neochat",
            "konversation",
            "weechat",
            "fractal",
            "beeper",
        ],
        AppKind::Mail => &[
            "thunderbird",
            "betterbird",
            "evolution",
            "kmail",
            "geary",
            "mailspring",
            "kontact",
        ],
        AppKind::Docs => &[
            "libreoffice",
            "soffice",
            "obsidian",
            "logseq",
            "notion",
            "joplin",
            "typora",
            "onlyoffice",
            "zettlr",
            "anytype",
        ],
        AppKind::Browser => &[
            "firefox",
            "librewolf",
            "zen",
            "chromium",
            "chrome",
            "brave",
            "vivaldi",
            "edge",
            "opera",
            "epiphany",
            "falkon",
        ],
        AppKind::Other => &[],
    }
}

/// Web apps recognised from a browser tab's title.
const BROWSER_TITLE_PATTERNS: &[(&str, AppKind)] = &[
    ("gmail", AppKind::Mail),
    ("outlook", AppKind::Mail),
    ("proton mail", AppKind::Mail),
    ("fastmail", AppKind::Mail),
    ("slack", AppKind::Chat),
    ("discord", AppKind::Chat),
    ("teams", AppKind::Chat),
    ("google chat", AppKind::Chat),
    ("whatsapp", AppKind::Chat),
    ("telegram", AppKind::Chat),
    ("google docs", AppKind::Docs),
    ("notion", AppKind::Docs),
    ("confluence", AppKind::Docs),
    ("linear", AppKind::Docs),
    ("jira", AppKind::Docs),
];

impl AppKind {
    pub fn classify(window: &WindowInfo, overrides: &ProfilesConfig) -> Self {
        let class = window.class.to_lowercase();
        let title = window.title.to_lowercase();
        let user_match = KINDS.iter().copied().find(|kind| {
            overrides
                .patterns(*kind)
                .iter()
                .any(|pattern| !pattern.is_empty() && class.contains(&pattern.to_lowercase()))
        });
        let kind = user_match.or_else(|| {
            KINDS.iter().copied().find(|kind| {
                builtin_class_patterns(*kind)
                    .iter()
                    .any(|pattern| class.contains(pattern))
            })
        });
        match kind {
            Some(AppKind::Browser) => BROWSER_TITLE_PATTERNS
                .iter()
                .find(|(pattern, _)| title.contains(pattern))
                .map(|(_, kind)| *kind)
                .unwrap_or(AppKind::Browser),
            Some(kind) => kind,
            None => AppKind::Other,
        }
    }

    pub fn profile(self) -> TextProfile {
        match self {
            AppKind::Terminal => TextProfile {
                kind: self,
                capitalize_first: false,
                ending_punctuation: false,
                tone: "The text goes into a terminal: keep it lowercase where dictated, no trailing period, keep commands, paths, and flags verbatim.",
            },
            AppKind::Editor => TextProfile {
                kind: self,
                capitalize_first: false,
                ending_punctuation: false,
                tone: "The text goes into a code editor: no trailing period, keep identifiers, code words, and symbols verbatim, do not reformat as prose.",
            },
            AppKind::Chat => TextProfile {
                kind: self,
                capitalize_first: true,
                ending_punctuation: false,
                tone: "The text is a chat message: conversational and brief, no trailing period on a single sentence.",
            },
            AppKind::Mail => TextProfile {
                kind: self,
                capitalize_first: true,
                ending_punctuation: true,
                tone: "The text is part of an email: complete sentences, proper punctuation, paragraphs where the speaker pauses, polite and clear.",
            },
            AppKind::Docs => TextProfile {
                kind: self,
                capitalize_first: true,
                ending_punctuation: true,
                tone: "The text goes into a document: complete sentences, proper punctuation, paragraphs where the speaker pauses.",
            },
            AppKind::Browser | AppKind::Other => TextProfile {
                kind: self,
                capitalize_first: true,
                ending_punctuation: true,
                tone: "Natural written prose with proper punctuation.",
            },
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AppKind::Terminal => "terminal",
            AppKind::Editor => "code editor",
            AppKind::Chat => "chat",
            AppKind::Mail => "email",
            AppKind::Docs => "document",
            AppKind::Browser => "browser",
            AppKind::Other => "application",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(class: &str, title: &str) -> WindowInfo {
        WindowInfo {
            class: class.to_string(),
            title: title.to_string(),
        }
    }

    #[test]
    fn classes_map_to_kinds() {
        let cfg = ProfilesConfig::defaults();
        assert_eq!(
            AppKind::classify(&window("org.kde.konsole", "~ : bash"), &cfg),
            AppKind::Terminal
        );
        assert_eq!(
            AppKind::classify(&window("Code", "main.rs - hush"), &cfg),
            AppKind::Editor
        );
        assert_eq!(
            AppKind::classify(&window("thunderbird", "Inbox"), &cfg),
            AppKind::Mail
        );
        assert_eq!(
            AppKind::classify(&window("obsidian", "notes"), &cfg),
            AppKind::Docs
        );
        assert_eq!(
            AppKind::classify(&window("org.gnome.Nautilus", "Home"), &cfg),
            AppKind::Other
        );
        assert_eq!(
            AppKind::classify(&window("dev.warp.Warp", "Aurora"), &cfg),
            AppKind::Terminal
        );
    }

    #[test]
    fn browser_tabs_take_the_web_app_kind() {
        let cfg = ProfilesConfig::defaults();
        assert_eq!(
            AppKind::classify(&window("firefox", "Inbox (3) - Gmail"), &cfg),
            AppKind::Mail
        );
        assert_eq!(
            AppKind::classify(&window("Google-chrome", "general - hush - Slack"), &cfg),
            AppKind::Chat
        );
        assert_eq!(
            AppKind::classify(&window("firefox", "Rust docs"), &cfg),
            AppKind::Browser
        );
    }

    #[test]
    fn user_patterns_win() {
        let cfg = ProfilesConfig {
            chat: vec!["MyCompanyApp".to_string()],
            ..ProfilesConfig::defaults()
        };
        assert_eq!(
            AppKind::classify(&window("mycompanyapp", ""), &cfg),
            AppKind::Chat
        );
        let cfg = ProfilesConfig {
            terminal: vec!["code".to_string()],
            ..ProfilesConfig::defaults()
        };
        assert_eq!(
            AppKind::classify(&window("Code", ""), &cfg),
            AppKind::Terminal
        );
    }

    #[test]
    fn profiles_shape_punctuation() {
        assert!(!AppKind::Terminal.profile().capitalize_first);
        assert!(!AppKind::Chat.profile().ending_punctuation);
        assert!(AppKind::Mail.profile().ending_punctuation);
        assert!(AppKind::Other.profile().capitalize_first);
    }
}
