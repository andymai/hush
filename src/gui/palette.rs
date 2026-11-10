//! The command palette: Ctrl+K, or Commands in the sidebar, opens a search
//! over every settings row, the pages, and the actions of the window's
//! buttons. A row opens its page, scrolls into view, and gives the focus to
//! its first control. An action does what its button does, with the same
//! second click when the button asks for one.
//!
//! Ctrl+K opens the palette from anywhere, except in these cases:
//! - A key field waits for a key press. The key field gets the key.
//! - A text field with text has the focus. There, Ctrl+K deletes to the end
//!   of the line, so it keeps that meaning. An empty field opens the palette.
//!
//! In the palette, Ctrl+K and Escape close it.

use super::icons;
use super::jump;
use super::meter;
use super::pages::{self, desktop, hotkeys, speech, text, Page};
use super::theme::{self, palette};
use super::widgets::{self, text_focus_id, TextFocus};
use super::Gui;
use crate::config::settings::TranscriptionBackend;
use egui::text::{LayoutJob, TextWrapping};
use egui::{
    Align, Context, Event, Id, Key, Layout, Modifiers, Order, Pos2, Rect, Response, Sense, Stroke,
    Ui, UiBuilder, Vec2,
};
use std::sync::Arc;

const WIDTH: f32 = 560.0;
/// The space between the dialog and the sides of a narrow window
const SIDE: f32 = 24.0;
/// The dialog starts this far down the window, as a fraction of its height.
const TOP: f32 = 0.12;
const HEADER: f32 = 48.0;
const ITEM: f32 = 36.0;
const LIST_MAX: f32 = 400.0;
/// The list is at most this fraction of the window's height.
const LIST_SHARE: f32 = 0.6;
const PLACEHOLDER: &str = "Find a setting, a page, or an action";
const NOTHING: &str = "No setting, page, or action matches.";

/// A settings row in the palette.
pub struct Setting {
    /// The row's stable key, as the page draws it
    pub key: &'static str,
    pub title: &'static str,
    pub page: Page,
    pub section: &'static str,
    /// More words that a search finds: from the description, and synonyms
    pub words: &'static str,
    /// Whether the page shows the row now
    pub shown: fn(&Gui) -> bool,
}

const fn row(
    key: &'static str,
    title: &'static str,
    page: Page,
    section: &'static str,
    words: &'static str,
) -> Setting {
    Setting {
        key,
        title,
        page,
        section,
        words,
        shown: always,
    }
}

impl Setting {
    const fn when(self, shown: fn(&Gui) -> bool) -> Self {
        Setting { shown, ..self }
    }
}

/// Every settings row that has a key of its own, in page order. The chord
/// rows of the Hotkeys page come from the configuration. Rows whose only
/// control is a button come from [`Action`].
pub const SETTINGS: &[Setting] = &[
    row(
        "overview.daemon",
        "Hush status",
        Page::Overview,
        "Status",
        "daemon running start stop restart safe mode",
    ),
    row(
        "overview.autostart",
        "Start Hush when I log in",
        Page::Overview,
        "Status",
        "autostart login session boot",
    ),
    row(
        "overview.access",
        "Keyboard access",
        Page::Overview,
        "Keyboard access",
        "permissions grant udev uinput input group password read type",
    ),
    row(
        "overview.model",
        "Speech model status",
        Page::Overview,
        "Speech model",
        "installed downloaded ready whisper",
    ),
    row(
        "hotkeys.dictate",
        "Dictate key",
        Page::Hotkeys,
        "Dictation",
        "hotkey hold talk record push",
    ),
    row(
        "hotkeys.mode",
        "Mode",
        Page::Hotkeys,
        "Dictation",
        "hold toggle tap talk hands-free double-tap lock",
    ),
    row(
        "hotkeys.cancel",
        "Cancel key",
        Page::Hotkeys,
        "Dictation",
        "hotkey discard escape abort",
    ),
    row(
        "hotkeys.command",
        "Command key",
        Page::Hotkeys,
        "Dictation",
        "hotkey command mode rewrite selection",
    ),
    row(
        "hotkeys.pastelast",
        "Type last again key",
        Page::Hotkeys,
        "More keys",
        "hotkey paste last transcript repeat",
    ),
    row(
        "hotkeys.learn",
        "Learn selection key",
        Page::Hotkeys,
        "More keys",
        "hotkey vocabulary word spelling",
    ),
    row(
        "hotkeys.backend",
        "Read keys through",
        Page::Hotkeys,
        "More keys",
        "evdev x11 input devices wayland backend",
    ),
    row(
        "hotkeys.tap_ms",
        "Tap is shorter than",
        Page::Hotkeys,
        "More keys",
        "tap hold milliseconds ms double-tap",
    ),
    row(
        "hotkeys.exclusive",
        "Keep the key from other applications",
        Page::Hotkeys,
        "More keys",
        "grab exclusive mouse button back browser",
    ),
    row(
        "hotkeys.mask_menu",
        "Keep menus closed",
        Page::Hotkeys,
        "More keys",
        "alt super menu launcher ctrl tap",
    ),
    row(
        "hotkeys.languages.new",
        "Add a language key",
        Page::Hotkeys,
        "Language keys",
        "hotkey chord dictate language",
    ),
    row(
        "hotkeys.styles.new",
        "Add a style key",
        Page::Hotkeys,
        "Style keys",
        "hotkey chord dictate writing style",
    ),
    row(
        "hotkeys.rewrite.new",
        "Add a rewrite key",
        Page::Hotkeys,
        "Rewrite keys",
        "hotkey chord selection style",
    ),
    row(
        "speech.backend",
        "Transcribe on",
        Page::Speech,
        "Transcription",
        "backend local cloud groq openai server whisper service privacy",
    ),
    row(
        "speech.key",
        "API key of the service",
        Page::Speech,
        "Transcription",
        "groq openai key env privacy audio",
    )
    .when(hosted),
    row(
        "speech.groq_model",
        "Groq model",
        Page::Speech,
        "Transcription",
        "whisper turbo",
    )
    .when(groq),
    row(
        "speech.openai_model",
        "OpenAI model",
        Page::Speech,
        "Transcription",
        "gpt transcribe",
    )
    .when(openai),
    row(
        "speech.server",
        "Server status",
        Page::Speech,
        "Transcription",
        "openai-compatible server key",
    )
    .when(server),
    row(
        "speech.server_url",
        "Server address",
        Page::Speech,
        "Transcription",
        "openai-compatible url endpoint",
    )
    .when(server),
    row(
        "speech.server_model",
        "Server model",
        Page::Speech,
        "Transcription",
        "openai-compatible whisper",
    )
    .when(server),
    row(
        "speech.models_dir",
        "The models directory is not readable",
        Page::Speech,
        "Model",
        "folder error",
    )
    .when(models_unreadable),
    row(
        "speech.model",
        "Speech model",
        Page::Speech,
        "Model",
        "whisper tiny base small medium large parakeet size accuracy",
    )
    .when(model_list),
    row(
        "speech.language",
        "Language",
        Page::Speech,
        "Listening",
        "detect auto english german french",
    ),
    row(
        "speech.detect_languages",
        "Detect only",
        Page::Speech,
        "Listening",
        "language detection identification list",
    )
    .when(detecting),
    row(
        "speech.gpu",
        "Use the GPU",
        Page::Speech,
        "Listening",
        "cuda vulkan graphics cpu acceleration",
    )
    .when(on_this_machine),
    row(
        "speech.vad",
        "Trim silence",
        Page::Speech,
        "Listening",
        "vad voice activity detection silero",
    )
    .when(on_this_machine),
    row(
        "speech.context",
        "Spell names from context",
        Page::Speech,
        "Listening",
        "window title vocabulary prompt",
    ),
    row(
        "speech.max_recording",
        "Stop recording after",
        Page::Speech,
        "Listening",
        "cap limit minutes hands-free maximum",
    ),
    row(
        "speech.silence_stop",
        "Stop after a pause",
        Page::Speech,
        "Listening",
        "silence seconds hands-free quiet",
    ),
    row(
        "speech.microphone",
        "Input device",
        Page::Speech,
        "Microphone",
        "microphone mic audio device input source",
    ),
    row(
        "speech.level",
        "Level",
        Page::Speech,
        "Microphone",
        "meter volume microphone test loudness",
    ),
    row(
        "text.profiles",
        "Match the application you are typing into",
        Page::Text,
        "Tone",
        "profiles tone casual terminal chat mail",
    ),
    row(
        "text.llm_provider",
        "Polish with",
        Page::Text,
        "Polish",
        "llm provider ollama anthropic claude openai-compatible model ai",
    ),
    row(
        "text.polish",
        "Tidy up every transcript",
        Page::Text,
        "Polish",
        "polish llm cleanup grammar rewrite",
    ),
    row(
        "text.ollama_url",
        "Ollama server",
        Page::Text,
        "Language models",
        "llm url address local",
    ),
    row(
        "text.ollama_model",
        "Ollama model",
        Page::Text,
        "Language models",
        "llm llama",
    ),
    row(
        "text.anthropic_model",
        "Anthropic model",
        Page::Text,
        "Language models",
        "llm claude haiku",
    ),
    row(
        "text.server_url",
        "OpenAI-compatible server",
        Page::Text,
        "Language models",
        "llm url address",
    ),
    row(
        "text.server_model",
        "OpenAI-compatible model",
        Page::Text,
        "Language models",
        "llm",
    ),
    row(
        "text.style_form.name",
        "Style name",
        Page::Text,
        "Add a style",
        "writing styles new",
    ),
    row(
        "text.style_form.prompt",
        "Style prompt",
        Page::Text,
        "Add a style",
        "writing styles instruction",
    ),
    row(
        "text.style_form.polish",
        "Style polish",
        Page::Text,
        "Add a style",
        "writing styles llm",
    ),
    row(
        "text.vocabulary.add",
        "Add a word",
        Page::Text,
        "Vocabulary",
        "vocabulary term spelling learn names",
    ),
    row(
        "text.vocabulary.find",
        "Find a word",
        Page::Text,
        "Vocabulary",
        "vocabulary search filter",
    )
    .when(text::many_words),
    row(
        "text.vocabulary.word_list",
        "Word list",
        Page::Text,
        "Vocabulary",
        "import export file text",
    ),
    row(
        "output.method",
        "Insert text by",
        Page::Output,
        "Typing",
        "type paste clipboard uinput insertion method",
    ),
    row(
        "output.typing_delay",
        "Between keystrokes",
        Page::Output,
        "Typing",
        "delay typing speed milliseconds drops letters",
    ),
    row(
        "output.uinput_apps",
        "Always type in",
        Page::Output,
        "By application",
        "rules window class applications",
    ),
    row(
        "output.clipboard_apps",
        "Always paste in",
        Page::Output,
        "By application",
        "rules window class applications clipboard",
    ),
    row(
        "desktop.tray",
        "Show the panel icon",
        Page::Desktop,
        "General",
        "tray indicator appindicator status icon",
    ),
    row(
        "desktop.sounds",
        "Play a sound when recording starts and stops",
        Page::Desktop,
        "General",
        "audio feedback beep tone",
    ),
    row(
        "desktop.pause_media",
        "Pause music and video while you record",
        Page::Desktop,
        "General",
        "mpris media players spotify",
    ),
    row(
        "desktop.updates",
        "Check for new releases",
        Page::Desktop,
        "General",
        "updates version github notification",
    ),
    row(
        "desktop.appearance",
        "Theme",
        Page::Desktop,
        "Appearance",
        "dark light system appearance colours colors mode",
    ),
    row(
        "desktop.config_file",
        "Configuration file",
        Page::Desktop,
        "Configuration file",
        "config toml path settings",
    ),
    row(
        "history.enabled",
        "Keep a history",
        Page::History,
        "Storage",
        "transcripts save store",
    ),
    row(
        "history.keep_audio",
        "Keep the audio",
        Page::History,
        "Storage",
        "audio recordings takes days retention flac",
    ),
    row(
        "history.toolbar",
        "Search the transcripts",
        Page::History,
        "Transcripts",
        "find filter history",
    )
    .when(transcripts),
];

fn always(_: &Gui) -> bool {
    true
}

/// The backend that the Speech page shows, which can wait to save.
fn backend(gui: &Gui) -> TranscriptionBackend {
    gui.pending
        .backend
        .unwrap_or(gui.config.transcription.backend)
}

fn hosted(gui: &Gui) -> bool {
    matches!(
        backend(gui),
        TranscriptionBackend::Groq | TranscriptionBackend::OpenAi
    )
}

fn groq(gui: &Gui) -> bool {
    backend(gui) == TranscriptionBackend::Groq
}

fn openai(gui: &Gui) -> bool {
    backend(gui) == TranscriptionBackend::OpenAi
}

fn server(gui: &Gui) -> bool {
    backend(gui) == TranscriptionBackend::OpenAiCompatible
}

fn on_this_machine(gui: &Gui) -> bool {
    !gui.config.transcription.backend.is_cloud()
}

fn models_unreadable(gui: &Gui) -> bool {
    on_this_machine(gui) && gui.catalogue.is_empty()
}

fn model_list(gui: &Gui) -> bool {
    on_this_machine(gui) && !gui.catalogue.is_empty()
}

fn detecting(gui: &Gui) -> bool {
    let transcription = &gui.config.transcription;
    transcription.language == "auto" || !transcription.detect_languages.is_empty()
}

fn transcripts(gui: &Gui) -> bool {
    gui.history.has_entries()
}

/// What a button of the window does, offered while the window shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    StartHush,
    RestartHush,
    StopHush,
    GrantAccess,
    Download,
    FindMicrophones,
    OpenConfigFile,
    Reset,
    ImportWords,
    ExportWords,
    ClearHistory,
    ReplaceConfig,
    ReplaceVocabulary,
    Undo,
}

impl Action {
    pub const ALL: [Action; 14] = [
        Action::StartHush,
        Action::RestartHush,
        Action::StopHush,
        Action::GrantAccess,
        Action::Download,
        Action::FindMicrophones,
        Action::OpenConfigFile,
        Action::Reset,
        Action::ImportWords,
        Action::ExportWords,
        Action::ClearHistory,
        Action::ReplaceConfig,
        Action::ReplaceVocabulary,
        Action::Undo,
    ];

    /// The row that holds the button, and the page of the row: none for
    /// the file rows above every page.
    pub fn row(self) -> Option<(&'static str, Option<Page>)> {
        let found = match self {
            Action::StartHush | Action::RestartHush | Action::StopHush => {
                ("overview.daemon", Some(Page::Overview))
            },
            Action::GrantAccess => ("overview.access", Some(Page::Overview)),
            Action::Download => ("speech.download", Some(Page::Speech)),
            Action::FindMicrophones => ("speech.microphone", Some(Page::Speech)),
            Action::OpenConfigFile => ("desktop.config_file", Some(Page::Desktop)),
            Action::Reset => ("desktop.reset", Some(Page::Desktop)),
            Action::ImportWords | Action::ExportWords => {
                ("text.vocabulary.word_list", Some(Page::Text))
            },
            Action::ClearHistory => ("history.toolbar", Some(Page::History)),
            Action::ReplaceConfig => ("files.config", None),
            Action::ReplaceVocabulary => ("files.vocabulary", None),
            Action::Undo => return None,
        };
        Some(found)
    }

    fn section(self) -> &'static str {
        match self {
            Action::StartHush | Action::RestartHush | Action::StopHush => "Status",
            Action::GrantAccess => "Keyboard access",
            Action::Download => "Model",
            Action::FindMicrophones => "Microphone",
            Action::OpenConfigFile | Action::Reset => "Configuration file",
            Action::ImportWords | Action::ExportWords => "Vocabulary",
            Action::ClearHistory => "Transcripts",
            Action::ReplaceConfig | Action::ReplaceVocabulary => "Files",
            Action::Undo => "Footer",
        }
    }

    fn icon(self) -> char {
        match self {
            Action::StartHush => icons::POWER,
            Action::RestartHush => icons::ROTATE_CW,
            Action::StopHush => icons::CIRCLE_STOP,
            Action::GrantAccess => icons::SHIELD_CHECK,
            Action::Download => icons::DOWNLOAD,
            Action::FindMicrophones => icons::REFRESH_CW,
            Action::OpenConfigFile => icons::FILE_TEXT,
            Action::ImportWords => icons::FILE_INPUT,
            Action::ExportWords => icons::FILE_OUTPUT,
            Action::ClearHistory => icons::TRASH,
            Action::Reset | Action::ReplaceConfig | Action::ReplaceVocabulary | Action::Undo => {
                icons::ROTATE_CCW
            },
        }
    }

    fn words(self) -> &'static str {
        match self {
            Action::StartHush => "daemon run launch on",
            Action::RestartHush => "daemon reload apply",
            Action::StopHush => "daemon quit off",
            Action::GrantAccess => "permissions udev uinput input group keyboard",
            Action::Download => "model whisper fetch install voice detection vad",
            Action::FindMicrophones => "refresh microphones input devices rescan",
            Action::OpenConfigFile => "config toml edit editor",
            Action::Reset => "defaults restore settings",
            Action::ImportWords => "vocabulary words file load",
            Action::ExportWords => "vocabulary words file save",
            Action::ClearHistory => "history delete transcripts audio remove",
            Action::ReplaceConfig => "config toml broken error defaults load",
            Action::ReplaceVocabulary => "vocabulary json broken error load",
            Action::Undo => "restore removal back",
        }
    }

    fn title(self, gui: &Gui) -> String {
        let title = match self {
            Action::StartHush => "Start Hush",
            Action::RestartHush => "Restart Hush",
            Action::StopHush => "Stop Hush",
            Action::GrantAccess => "Grant keyboard access",
            Action::Download => {
                return match speech::download_offer(gui) {
                    Some(size) if !gui.machine.installed_models.contains(&size) => {
                        format!("Download {}", speech::size_key(size))
                    },
                    _ => "Download voice detection".to_string(),
                }
            },
            Action::FindMicrophones => "Look for microphones again",
            Action::OpenConfigFile => "Open the configuration file",
            Action::Reset => "Reset everything",
            Action::ImportWords => "Import the word list",
            Action::ExportWords => "Export the word list",
            Action::ClearHistory => "Clear all transcripts",
            Action::ReplaceConfig => "Replace the configuration file with defaults",
            Action::ReplaceVocabulary => "Replace the vocabulary file",
            Action::Undo => "Undo",
        };
        title.to_string()
    }

    /// The window shows the button now, and the button can act.
    fn offered(self, gui: &Gui) -> bool {
        match self {
            Action::StartHush => !gui.machine.running(),
            Action::RestartHush | Action::StopHush => gui.machine.running(),
            Action::GrantAccess => !gui.machine.permissions.ready(),
            Action::Download => speech::download_offer(gui).is_some(),
            Action::FindMicrophones | Action::OpenConfigFile | Action::Reset => true,
            Action::ImportWords => text::can_import(gui),
            Action::ExportWords => text::can_export(gui),
            Action::ClearHistory => gui.history.has_entries(),
            Action::ReplaceConfig => gui.files.config_error.is_some(),
            Action::ReplaceVocabulary => gui.files.vocabulary_error.is_some(),
            Action::Undo => gui.undo.is_some(),
        }
    }

    fn entry(self, gui: &Gui) -> Entry {
        let detail = match (self, self.row()) {
            (Action::Undo, _) => gui
                .undo
                .as_ref()
                .map(|undo| undo.label.clone())
                .unwrap_or_default(),
            (_, Some((_, Some(page)))) => format!("{} · {}", page.title(), self.section()),
            _ => self.section().to_string(),
        };
        Entry {
            title: self.title(gui),
            context: detail.clone(),
            detail,
            icon: self.icon(),
            group: Group::Actions,
            words: self.words().to_string(),
            pick: Pick::Action(self),
        }
    }

    /// Do what the button does. A button that asks first opens its page and
    /// asks there; a button whose result shows on its page opens that page.
    fn run(self, gui: &mut Gui, ctx: &Context) {
        match self {
            Action::StartHush => gui.run_hush(&["daemon", "start"], "Starting…"),
            Action::RestartHush => gui.run_hush(&["daemon", "restart"], "Restarting…"),
            Action::StopHush => gui.run_hush(&["daemon", "stop"], "Stopping…"),
            Action::GrantAccess => {
                gui.run_hush(&["setup", "permissions"], "Asking for permission…")
            },
            Action::Download => {
                if let Some(size) = speech::download_offer(gui) {
                    speech::start_download(gui, size);
                    self.show_row(gui, ctx);
                }
            },
            Action::FindMicrophones => {
                gui.microphones = meter::input_devices();
                self.show_row(gui, ctx);
            },
            Action::OpenConfigFile => desktop::open_config_file(gui),
            Action::Reset => {
                self.show_row(gui, ctx);
                gui.confirm_reset = true;
            },
            Action::ImportWords => text::import_word_list(gui),
            Action::ExportWords => text::export_word_list(gui),
            Action::ClearHistory => {
                self.show_row(gui, ctx);
                gui.history.ask_to_clear();
            },
            Action::ReplaceConfig => {
                if pages::replace_loses_audio(gui, ctx) {
                    self.show_row(gui, ctx);
                    gui.confirm_replace = true;
                } else {
                    gui.replace_config();
                }
            },
            Action::ReplaceVocabulary => gui.replace_vocabulary(),
            Action::Undo => gui.undo(),
        }
    }

    fn show_row(self, gui: &mut Gui, ctx: &Context) {
        if let Some((key, page)) = self.row() {
            if let Some(page) = page {
                gui.go_to(page);
            }
            jump::start(ctx, key);
        }
    }
}

/// What picking an entry does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick {
    Action(Action),
    Page(Page),
    Row { key: String, page: Page },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Actions,
    Pages,
    Settings,
}

impl Group {
    fn label(self) -> &'static str {
        match self {
            Group::Actions => "Actions",
            Group::Pages => "Pages",
            Group::Settings => "Settings",
        }
    }
}

/// One line of the palette.
#[derive(Clone, Debug)]
pub struct Entry {
    pub title: String,
    /// Where the entry is, at the right of the line
    pub detail: String,
    pub icon: char,
    pub group: Group,
    /// The section and the page, which a search ranks below the title
    pub context: String,
    /// More words that a search finds, ranked last
    pub words: String,
    pub pick: Pick,
}

impl Setting {
    pub fn entry(&self) -> Entry {
        setting_entry(
            self.key.to_string(),
            self.title.to_string(),
            self.page,
            self.section,
            self.words,
        )
    }
}

fn setting_entry(key: String, title: String, page: Page, section: &str, words: &str) -> Entry {
    Entry {
        title,
        detail: format!("{} · {}", page.title(), section),
        icon: page.icon(),
        group: Group::Settings,
        context: format!("{} {}", page.title(), section),
        words: words.to_string(),
        pick: Pick::Row { key, page },
    }
}

fn page_entry(page: Page) -> Entry {
    Entry {
        title: page.title().to_string(),
        detail: "Page".to_string(),
        icon: page.icon(),
        group: Group::Pages,
        context: "page".to_string(),
        words: page.words().to_string(),
        pick: Pick::Page(page),
    }
}

/// Every entry that the window can act on now: the actions of the buttons
/// it shows, the pages, then the rows that the pages show, in page order.
pub fn catalogue(gui: &Gui) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Action::ALL
        .into_iter()
        .filter(|action| action.offered(gui))
        .map(|action| action.entry(gui))
        .collect();
    entries.extend(Page::ALL.into_iter().map(page_entry));
    let chords = hotkeys::chord_rows(&gui.config.hotkey);
    for setting in SETTINGS.iter().filter(|setting| (setting.shown)(gui)) {
        if setting.page == Page::Hotkeys && setting.key.ends_with(".new") {
            entries.extend(
                chords
                    .iter()
                    .filter(|(_, _, section)| *section == setting.section)
                    .map(|(key, title, section)| {
                        setting_entry(
                            key.clone(),
                            title.clone(),
                            Page::Hotkeys,
                            section,
                            "hotkey chord",
                        )
                    }),
            );
        }
        entries.push(setting.entry());
    }
    entries
}

/// Lowercase words: the runs of letters and digits.
fn split(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// How well an entry matches a query: first the query words found in the
/// title, then a score. A better rank sorts later.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Rank {
    title_words: usize,
    score: u32,
}

/// The rank of `entry` for the query words, or none when a word matches
/// nothing. Each word scores by where it matches best: as a whole word of
/// the title, at the start of a title word, inside the title, at the start
/// of a section or page word, at the start of another word, or as letters
/// in order through one title word, then through the title.
fn rank(query: &[String], entry: &Entry) -> Option<Rank> {
    let title = entry.title.to_lowercase();
    let title_words = split(&entry.title);
    let context = split(&entry.context);
    let words = split(&entry.words);
    let starts = |list: &[String], word: &str| list.iter().any(|w| w.starts_with(word));
    let mut rank = Rank {
        title_words: 0,
        score: 0,
    };
    for word in query {
        let score = if title_words.iter().any(|w| w == word) {
            110
        } else if starts(&title_words, word) {
            100
        } else if title.contains(word.as_str()) {
            60
        } else if starts(&context, word) {
            20
        } else if starts(&words, word) {
            10
        } else if title_words
            .iter()
            .any(|w| fuzzy(word, std::slice::from_ref(w)))
        {
            8
        } else if fuzzy(word, &title_words) {
            5
        } else {
            return None;
        };
        if score >= 60 {
            rank.title_words += 1;
        }
        rank.score += score;
    }
    if title.starts_with(&query.join(" ")) {
        rank.score += 200;
    }
    Some(rank)
}

/// The letters of `word` in order through the title, from the start of a
/// title word: "dwnld" finds "Download". Words of fewer than three letters
/// never match this way, because too much would match.
fn fuzzy(word: &str, title_words: &[String]) -> bool {
    let mut letters = word.chars();
    let Some(first) = letters.next() else {
        return false;
    };
    if word.chars().count() < 3 {
        return false;
    }
    let rest: Vec<char> = letters.collect();
    (0..title_words.len()).any(|start| {
        if !title_words[start].starts_with(first) {
            return false;
        }
        let tail = title_words[start..].join(" ");
        let mut wanted = rest.iter().peekable();
        for c in tail.chars().skip(1) {
            if wanted.peek() == Some(&&c) {
                wanted.next();
            }
        }
        wanted.peek().is_none()
    })
}

/// The entries that match `query`, best first, with ties in their order.
/// An empty query keeps every entry in its group.
pub fn filter(entries: Vec<Entry>, query: &str) -> Vec<Entry> {
    let words = split(query);
    if words.is_empty() {
        return entries;
    }
    let mut ranked: Vec<(Rank, usize, Entry)> = entries
        .into_iter()
        .enumerate()
        .filter_map(|(index, entry)| Some((rank(&words, &entry)?, index, entry)))
        .collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    ranked.into_iter().map(|(_, _, entry)| entry).collect()
}

/// What has the keyboard focus, for Ctrl+K.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Nothing,
    Palette,
    EmptyText,
    Text,
    Control,
}

/// What Ctrl+K does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shortcut {
    Open,
    Close,
    /// The key goes on to the window: to a key field that waits for a key,
    /// or to a text field with text
    Pass,
}

pub fn shortcut(open: bool, capturing: bool, focus: Focus) -> Shortcut {
    if open {
        Shortcut::Close
    } else if capturing || focus == Focus::Text {
        Shortcut::Pass
    } else {
        Shortcut::Open
    }
}

pub fn field_id() -> Id {
    Id::new("hush-palette-field")
}

fn focus(ctx: &Context) -> Focus {
    let Some(focused) = ctx.memory(|memory| memory.focused()) else {
        return Focus::Nothing;
    };
    if focused == field_id() {
        return Focus::Palette;
    }
    match ctx.data(|data| data.get_temp::<TextFocus>(text_focus_id())) {
        Some(text) if text.id == focused && text.empty => Focus::EmptyText,
        Some(text) if text.id == focused => Focus::Text,
        _ => Focus::Control,
    }
}

fn ctrl_k(event: &Event) -> bool {
    matches!(
        event,
        Event::Key {
            key: Key::K,
            pressed: true,
            modifiers,
            ..
        } if modifiers.matches_logically(Modifiers::COMMAND)
    )
}

/// The palette's state between frames.
#[derive(Default)]
pub struct Palette {
    open: bool,
    query: String,
    selected: usize,
    /// Moves down the list, or up when negative, that the keys asked for
    step: i64,
    /// Enter: pick the selected entry
    choose: bool,
    /// Scroll the selected entry into view
    follow: bool,
    /// The query changed, so the list starts at the top
    restart: bool,
    /// The widget that had the focus before the palette opened
    restore: Option<Id>,
}

impl Palette {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self, ctx: &Context) {
        if self.open {
            return;
        }
        self.open = true;
        self.query.clear();
        self.selected = 0;
        self.restart = true;
        self.restore = ctx
            .memory(|memory| memory.focused())
            .filter(|id| *id != field_id());
        ctx.memory_mut(|memory| {
            memory.close_popup();
            memory.request_focus(field_id());
        });
        ctx.request_repaint();
    }

    /// Open with `query` typed, as for a screenshot.
    pub fn open_with(&mut self, query: &str) {
        self.open = true;
        self.query = query.to_string();
        self.restart = true;
    }

    /// Close without a pick, and leave the focus free, as before a close of
    /// the window.
    pub fn dismiss(&mut self, ctx: &Context) {
        if self.open {
            self.close(ctx, false);
        }
    }

    fn close(&mut self, ctx: &Context, restore: bool) {
        self.open = false;
        self.step = 0;
        self.choose = false;
        let restore = self.restore.take().filter(|_| restore);
        ctx.memory_mut(|memory| {
            memory.surrender_focus(field_id());
            if let Some(id) = restore {
                memory.request_focus(id);
            }
        });
        ctx.request_repaint();
    }

    /// Before the window draws: Ctrl+K, and while the palette is open, the
    /// keys that move through the list, pick, and close. They never reach
    /// the page under the palette.
    pub fn keys(&mut self, ctx: &Context, capturing: bool) {
        if ctx.input(|input| input.events.iter().any(ctrl_k)) {
            match shortcut(self.open, capturing, focus(ctx)) {
                Shortcut::Open => {
                    ctx.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::K));
                    self.open(ctx);
                    return;
                },
                Shortcut::Close => {
                    ctx.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::K));
                    self.close(ctx, true);
                    return;
                },
                Shortcut::Pass => {},
            }
        }
        if !self.open {
            return;
        }
        let (escape, step, choose) = ctx.input_mut(|input| {
            let escape = input.consume_key(Modifiers::NONE, Key::Escape);
            let back = input.count_and_consume_key(Modifiers::SHIFT, Key::Tab);
            let next = input.count_and_consume_key(Modifiers::NONE, Key::Tab);
            let down = input.count_and_consume_key(Modifiers::NONE, Key::ArrowDown);
            let up = input.count_and_consume_key(Modifiers::NONE, Key::ArrowUp);
            let choose = input.consume_key(Modifiers::NONE, Key::Enter);
            let step = (down + next) as i64 - (up + back) as i64;
            (escape, step, choose)
        });
        if escape {
            self.close(ctx, true);
            return;
        }
        if step != 0 {
            self.step += step;
            self.follow = true;
        }
        self.choose |= choose;
    }

    /// Draw the palette over the window while it is open, and return what
    /// the user picked.
    pub fn show(&mut self, ctx: &Context, gui: &Gui) -> Option<Pick> {
        self.draw(ctx, |query| filter(catalogue(gui), query))
    }

    fn draw(&mut self, ctx: &Context, entries: impl FnOnce(&str) -> Vec<Entry>) -> Option<Pick> {
        if !self.open {
            return None;
        }
        if ctx.memory(|memory| memory.focused()) != Some(field_id()) {
            ctx.memory_mut(|memory| memory.request_focus(field_id()));
        }
        let screen = ctx.screen_rect();
        let width = (screen.width() - 2.0 * SIDE).min(WIDTH);
        let corner = Pos2::new(
            screen.center().x - width / 2.0,
            screen.top() + screen.height() * TOP,
        );
        let list_height = (screen.height() * LIST_SHARE).min(LIST_MAX);
        let mut pick = None;
        let mut dismiss = false;
        let area = egui::Area::new(Id::new("hush-palette"))
            .order(Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let colours = palette(ctx);
                let (_, backdrop) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(screen, 0.0, theme::scrim(ctx));
                let shadows = [
                    ui.painter().add(egui::Shape::Noop),
                    ui.painter().add(egui::Shape::Noop),
                ];
                let frame = ui.painter().add(egui::Shape::Noop);
                let mut dialog = ui.new_child(
                    UiBuilder::new()
                        .max_rect(Rect::from_min_size(corner, Vec2::new(width, f32::INFINITY)))
                        .layout(Layout::top_down(Align::Min)),
                );
                dialog.set_width(width);
                dialog.spacing_mut().item_spacing = Vec2::ZERO;
                self.header(&mut dialog);
                let grouped = split(&self.query).is_empty();
                let entries = entries(&self.query);
                self.settle(entries.len());
                if std::mem::take(&mut self.choose) {
                    pick = entries.get(self.selected).map(|entry| entry.pick.clone());
                }
                if let Some(clicked) = self.list(&mut dialog, &entries, grouped, list_height) {
                    pick = Some(clicked);
                }
                let rect = Rect::from_min_max(
                    corner,
                    Pos2::new(corner.x + width, dialog.min_rect().bottom()),
                );
                for (index, shadow) in shadows.into_iter().zip(theme::dialog_shadows(ctx)) {
                    ui.painter()
                        .set(index, shadow.as_shape(rect, theme::RADIUS_LG));
                }
                ui.painter().set(
                    frame,
                    egui::epaint::RectShape::new(
                        rect,
                        theme::RADIUS_LG,
                        colours.raised,
                        Stroke::new(1.0_f32, colours.line),
                    ),
                );
                let outside = ui
                    .input(|input| input.pointer.interact_pos())
                    .is_some_and(|at| !rect.contains(at));
                dismiss = backdrop.clicked() && outside;
            });
        ctx.move_to_top(area.response.layer_id);
        if pick.is_some() {
            self.close(ctx, false);
        } else if dismiss {
            self.close(ctx, true);
        }
        pick
    }

    /// Keep the selection on an entry, after the moves the keys asked for.
    fn settle(&mut self, count: usize) {
        if count == 0 {
            self.selected = 0;
        } else {
            let at = self.selected.min(count - 1) as i64 + self.step;
            self.selected = at.rem_euclid(count as i64) as usize;
        }
        self.step = 0;
    }

    /// The search icon and the field, with a line under them.
    fn header(&mut self, ui: &mut Ui) {
        let colours = palette(ui.ctx());
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, HEADER), Sense::hover());
        widgets::paint_icon(
            ui,
            Pos2::new(rect.left() + 16.0 + theme::ICON / 2.0, rect.center().y),
            icons::SEARCH,
            theme::ICON,
            colours.muted,
        );
        let left = rect.left() + 16.0 + theme::ICON + 12.0;
        let field = Rect::from_min_max(
            Pos2::new(left, rect.top()),
            Pos2::new(rect.right() - 16.0, rect.bottom() - 1.0),
        );
        let mut inside = ui.new_child(
            UiBuilder::new()
                .max_rect(field)
                .layout(Layout::left_to_right(Align::Center)),
        );
        let hint = egui::RichText::new(PLACEHOLDER)
            .font(theme::sans(theme::BODY))
            .color(colours.muted);
        let response = inside.add(
            egui::TextEdit::singleline(&mut self.query)
                .id(field_id())
                .font(theme::sans(theme::BODY))
                .text_color(colours.ink)
                .hint_text(hint)
                .frame(false)
                .margin(egui::Margin::ZERO)
                .vertical_align(Align::Center)
                .min_size(field.size())
                .desired_width(field.width())
                .lock_focus(true),
        );
        if response.changed() {
            self.selected = 0;
            self.restart = true;
        }
        ui.painter().hline(
            rect.left() + 1.0..=rect.right() - 1.0,
            rect.bottom() - 0.5,
            Stroke::new(1.0_f32, colours.line),
        );
    }

    /// The entries, in their groups when nothing is typed. The pointer
    /// selects the entry it moves over, and a click picks it.
    fn list(
        &mut self,
        ui: &mut Ui,
        entries: &[Entry],
        grouped: bool,
        max_height: f32,
    ) -> Option<Pick> {
        let mut area = egui::ScrollArea::vertical()
            .id_salt("hush-palette-list")
            .max_height(max_height)
            .auto_shrink([false, true]);
        if std::mem::take(&mut self.restart) {
            area = area.vertical_scroll_offset(0.0);
        }
        let follow = std::mem::take(&mut self.follow);
        let mut picked = None;
        area.show(ui, |ui| {
            egui::Frame::none()
                .inner_margin(egui::Margin::same(8.0))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    if entries.is_empty() {
                        nothing(ui);
                        return;
                    }
                    let moved = ui.input(|input| input.pointer.delta() != Vec2::ZERO);
                    let mut group = None;
                    for (index, entry) in entries.iter().enumerate() {
                        if grouped && group != Some(entry.group) {
                            group = Some(entry.group);
                            group_label(ui, entry.group.label());
                        }
                        let response = item(ui, entry, index == self.selected);
                        if response.hovered() && moved {
                            self.selected = index;
                        }
                        if response.clicked() {
                            picked = Some(entry.pick.clone());
                        }
                        if follow && index == self.selected {
                            ui.scroll_to_rect(response.rect, None);
                        }
                    }
                });
        });
        picked
    }
}

fn nothing(ui: &mut Ui) {
    let colours = palette(ui.ctx());
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 40.0), Sense::hover());
    ui.painter().text(
        Pos2::new(rect.left() + 8.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        NOTHING,
        theme::sans(theme::SMALL),
        colours.muted,
    );
}

fn group_label(ui: &mut Ui, label: &str) {
    let colours = palette(ui.ctx());
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 26.0), Sense::hover());
    ui.painter().text(
        Pos2::new(rect.left() + 8.0, rect.top() + 8.0 + 7.0),
        egui::Align2::LEFT_CENTER,
        label,
        theme::medium(theme::LABEL),
        colours.muted,
    );
}

/// `text` on one line, cut with an ellipsis at `width`.
fn line(
    ui: &Ui,
    text: &str,
    font: egui::FontId,
    colour: egui::Color32,
    width: f32,
) -> Arc<egui::Galley> {
    let mut job = LayoutJob::simple_singleline(text.to_owned(), font, colour);
    job.wrap = TextWrapping::truncate_at_width(width.max(1.0));
    ui.fonts(|fonts| fonts.layout_job(job))
}

/// One entry: its icon, its title, and where it is, at the right.
fn item(ui: &mut Ui, entry: &Entry, selected: bool) -> Response {
    let colours = palette(ui.ctx());
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, ITEM), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            true,
            selected,
            &entry.title,
        )
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    if selected {
        ui.painter()
            .rect_filled(rect, theme::RADIUS_SM, colours.selection);
    }
    widgets::paint_icon(
        ui,
        Pos2::new(rect.left() + 8.0 + theme::ICON / 2.0, rect.center().y),
        entry.icon,
        theme::ICON,
        colours.muted,
    );
    let left = rect.left() + 8.0 + theme::ICON + 12.0;
    let right = rect.right() - 8.0;
    let detail = line(
        ui,
        &entry.detail,
        theme::sans(theme::SMALL),
        colours.muted,
        (right - left) * 0.45,
    );
    let detail_left = right - detail.size().x;
    let title = line(
        ui,
        &entry.title,
        theme::sans(theme::BODY),
        colours.ink,
        detail_left - 12.0 - left,
    );
    ui.painter().galley(
        Pos2::new(left, rect.center().y - title.size().y / 2.0),
        title,
        colours.ink,
    );
    ui.painter().galley(
        Pos2::new(detail_left, rect.center().y - detail.size().y / 2.0),
        detail,
        colours.muted,
    );
    response
}

/// Run what the user picked: open a page at its top, jump to a row, or do
/// an action.
pub fn run(gui: &mut Gui, ctx: &Context, pick: Pick) {
    match pick {
        Pick::Page(page) => {
            gui.go_to(page);
            gui.scroll_top = true;
        },
        Pick::Row { key, page } => {
            gui.go_to(page);
            jump::start(ctx, &key);
        },
        Pick::Action(action) => action.run(gui, ctx),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::pages::hotkeys::ChordMap;
    use crate::gui::widgets::Field;
    use crate::hotkey::Role;
    use std::collections::BTreeSet;

    const PAGE_SOURCES: [(Option<Page>, &str); 8] = [
        (None, include_str!("pages/mod.rs")),
        (Some(Page::Overview), include_str!("pages/overview.rs")),
        (Some(Page::Hotkeys), include_str!("pages/hotkeys.rs")),
        (Some(Page::Speech), include_str!("pages/speech.rs")),
        (Some(Page::Text), include_str!("pages/text.rs")),
        (Some(Page::Output), include_str!("pages/output.rs")),
        (Some(Page::Desktop), include_str!("pages/desktop.rs")),
        (Some(Page::History), include_str!("pages/history.rs")),
    ];

    /// Keys of list items, which are no settings rows.
    const LIST_ITEMS: [&str; 1] = ["text.styles.verbatim"];

    /// Every row key that the pages draw: the literal keys, and the keys
    /// that the Hotkeys page makes for its actions and its chord tables.
    fn keys_in_pages() -> BTreeSet<String> {
        let pattern = regex::Regex::new(
            r#""((?:overview|hotkeys|speech|text|output|desktop|history|files)\.[a-z_.]+)""#,
        )
        .unwrap();
        PAGE_SOURCES
            .iter()
            .flat_map(|(_, source)| {
                pattern
                    .captures_iter(source)
                    .map(|found| found[1].to_string())
                    .collect::<Vec<_>>()
            })
            .chain(Role::ALL.map(hotkeys::role_key))
            .chain(ChordMap::ALL.map(hotkeys::new_chord_key))
            .filter(|key| !LIST_ITEMS.contains(&key.as_str()))
            .collect()
    }

    #[test]
    fn every_row_of_every_page_is_in_the_palette_and_nothing_else() {
        let indexed: BTreeSet<String> = SETTINGS
            .iter()
            .map(|setting| setting.key.to_string())
            .chain(
                Action::ALL
                    .iter()
                    .filter_map(|action| action.row())
                    .map(|(key, _)| key.to_string()),
            )
            .collect();
        let in_pages = keys_in_pages();
        assert!(in_pages.len() > 50, "the scan found the rows: {in_pages:?}");
        assert_eq!(indexed, in_pages);
    }

    #[test]
    fn each_row_is_in_the_palette_once_with_a_title_of_its_own() {
        let keys: BTreeSet<&str> = SETTINGS.iter().map(|setting| setting.key).collect();
        assert_eq!(keys.len(), SETTINGS.len(), "a key is listed twice");
        let titles: BTreeSet<&str> = SETTINGS.iter().map(|setting| setting.title).collect();
        assert_eq!(titles.len(), SETTINGS.len(), "two rows share a title");
    }

    #[test]
    fn each_row_names_a_section_of_its_own_page() {
        for setting in SETTINGS {
            let (_, source) = PAGE_SOURCES
                .iter()
                .find(|(page, _)| *page == Some(setting.page))
                .unwrap();
            assert!(
                source.contains(&format!("\"{}\"", setting.section)),
                "{} is not a section of the {} page",
                setting.section,
                setting.page.title()
            );
            assert!(
                setting
                    .key
                    .starts_with(&setting.page.title().to_lowercase()),
                "{} is not on the {} page",
                setting.key,
                setting.page.title()
            );
        }
    }

    fn sample() -> Vec<Entry> {
        Page::ALL
            .into_iter()
            .map(page_entry)
            .chain(SETTINGS.iter().map(Setting::entry))
            .collect()
    }

    fn titles(query: &str) -> Vec<String> {
        filter(sample(), query)
            .into_iter()
            .map(|entry| entry.title)
            .collect()
    }

    #[test]
    fn title_matches_rank_before_matches_of_the_section_or_the_words() {
        let found = filter(sample(), "model");
        let in_title: Vec<bool> = found
            .iter()
            .map(|entry| entry.title.to_lowercase().contains("model"))
            .collect();
        assert!(in_title[0], "{:?}", titles("model"));
        assert!(in_title.contains(&false), "the Model section matches too");
        let first_other = in_title.iter().position(|title| !title).unwrap();
        assert!(
            in_title[first_other..].iter().all(|title| !title),
            "{:?}",
            titles("model")
        );
    }

    #[test]
    fn a_search_matches_word_starts_descriptions_and_letters_in_order() {
        assert_eq!(titles("trim")[0], "Trim silence");
        assert_eq!(
            titles("spe nam")[0],
            "Spell names from context",
            "word starts"
        );
        assert_eq!(titles("dark")[0], "Theme", "a word of the description");
        assert_eq!(titles("thme")[0], "Theme", "letters in order");
        assert_eq!(titles("keep audio")[0], "Keep the audio", "every word");
        assert_eq!(titles("HOTKEYS")[0], "Hotkeys", "in any case");
        assert_eq!(titles("level")[0], "Level", "a whole word first");
        assert!(titles("zzzz").is_empty());
        assert!(titles("keep zzzz").is_empty(), "each word must match");
    }

    #[test]
    fn an_empty_query_keeps_every_entry_in_order() {
        let all: Vec<String> = sample().into_iter().map(|entry| entry.title).collect();
        assert_eq!(titles(""), all);
        assert_eq!(titles("  "), all);
    }

    #[test]
    fn words_are_runs_of_letters_and_digits() {
        assert_eq!(
            split("OpenAI-compatible server"),
            ["openai", "compatible", "server"]
        );
        assert_eq!(split("  "), Vec::<String>::new());
        assert!(!fuzzy("ab", &split("about")), "too short to match in order");
        assert!(fuzzy("dwnld", &split("Download the model")));
        assert!(!fuzzy("wnld", &split("Download")), "not from a word start");
    }

    #[test]
    fn ctrl_k_opens_except_in_a_key_field_or_a_field_with_text() {
        assert_eq!(shortcut(false, false, Focus::Nothing), Shortcut::Open);
        assert_eq!(shortcut(false, false, Focus::Control), Shortcut::Open);
        assert_eq!(shortcut(false, false, Focus::EmptyText), Shortcut::Open);
        assert_eq!(shortcut(false, false, Focus::Text), Shortcut::Pass);
        assert_eq!(shortcut(false, true, Focus::Nothing), Shortcut::Pass);
        assert_eq!(shortcut(true, false, Focus::Palette), Shortcut::Close);
    }

    /// The window's frames: a page with one text field, and the palette.
    struct Window {
        ctx: Context,
        palette: Palette,
        text: String,
        field: Option<Id>,
        picked: Option<Pick>,
    }

    impl Window {
        fn new(text: &str) -> Self {
            let ctx = Context::default();
            theme::install(&ctx);
            Self {
                ctx,
                palette: Palette::default(),
                text: text.to_string(),
                field: None,
                picked: None,
            }
        }

        fn frame(&mut self, events: Vec<Event>) {
            self.frame_with(events, false, false);
        }

        fn frame_with(&mut self, events: Vec<Event>, capturing: bool, focus_field: bool) {
            let input = egui::RawInput {
                events,
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 720.0))),
                ..Default::default()
            };
            let ctx = self.ctx.clone();
            let _ = ctx.run(input, |ctx| {
                self.palette.keys(ctx, capturing);
                egui::CentralPanel::default().show(ctx, |ui| {
                    let response = ui.add(Field::new(&mut self.text, ""));
                    if focus_field {
                        response.request_focus();
                    }
                    self.field = Some(response.id);
                });
                if let Some(pick) = self.palette.draw(ctx, |query| filter(sample(), query)) {
                    self.picked = Some(pick);
                }
            });
        }

        /// Frames that give the page's field the focus.
        fn focus_field(&mut self) {
            self.frame_with(Vec::new(), false, true);
            self.frame(Vec::new());
        }

        fn focused(&self) -> Option<Id> {
            self.ctx.memory(|memory| memory.focused())
        }
    }

    fn press(key: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn ctrl_k_press() -> Event {
        press(Key::K, Modifiers::CTRL | Modifiers::COMMAND)
    }

    #[test]
    fn ctrl_k_opens_the_palette_with_the_focus_in_its_field_and_closes_it() {
        let mut window = Window::new("");
        window.frame(Vec::new());
        window.frame(vec![ctrl_k_press()]);
        assert!(window.palette.is_open());
        window.frame(Vec::new());
        assert_eq!(window.focused(), Some(field_id()));
        window.frame(vec![ctrl_k_press()]);
        assert!(!window.palette.is_open());
        assert_eq!(window.picked, None);
    }

    #[test]
    fn ctrl_k_goes_to_a_key_field_that_listens() {
        let mut window = Window::new("");
        window.frame_with(vec![ctrl_k_press()], true, false);
        assert!(!window.palette.is_open());
    }

    #[test]
    fn ctrl_k_stays_with_a_field_that_has_text() {
        let mut window = Window::new("llama");
        window.focus_field();
        assert_eq!(window.focused(), window.field);
        window.frame(vec![ctrl_k_press()]);
        assert!(!window.palette.is_open());
        assert_eq!(window.focused(), window.field, "the field keeps the focus");

        let mut empty = Window::new("");
        empty.focus_field();
        empty.frame(vec![ctrl_k_press()]);
        assert!(empty.palette.is_open(), "an empty field opens the palette");
    }

    #[test]
    fn escape_closes_the_palette_and_gives_the_focus_back() {
        let mut window = Window::new("");
        window.focus_field();
        window.frame(vec![ctrl_k_press()]);
        window.frame(Vec::new());
        assert_eq!(window.focused(), Some(field_id()));
        window.frame(vec![press(Key::Escape, Modifiers::NONE)]);
        assert!(!window.palette.is_open());
        assert_eq!(window.picked, None);
        window.frame(Vec::new());
        assert_eq!(window.focused(), window.field);
    }

    #[test]
    fn the_arrow_keys_move_through_the_list_and_enter_picks() {
        let mut window = Window::new("");
        window.frame(vec![ctrl_k_press()]);
        let down = press(Key::ArrowDown, Modifiers::NONE);
        window.frame(vec![down.clone(), down.clone(), down]);
        window.frame(vec![press(Key::ArrowUp, Modifiers::NONE)]);
        window.frame(vec![press(Key::Enter, Modifiers::NONE)]);
        assert_eq!(window.picked, Some(sample()[2].pick.clone()));
        assert!(!window.palette.is_open());

        let mut wrap = Window::new("");
        wrap.frame(vec![ctrl_k_press()]);
        wrap.frame(vec![press(Key::ArrowUp, Modifiers::NONE)]);
        wrap.frame(vec![press(Key::Enter, Modifiers::NONE)]);
        let last = sample().last().unwrap().pick.clone();
        assert_eq!(wrap.picked, Some(last), "up from the top goes to the end");
    }

    #[test]
    fn typing_narrows_the_list_and_enter_picks_the_best_match() {
        let mut window = Window::new("");
        window.frame(vec![ctrl_k_press()]);
        window.frame(Vec::new());
        window.frame(vec![Event::Text("gpu".to_string())]);
        window.frame(vec![press(Key::Enter, Modifiers::NONE)]);
        let gpu = Pick::Row {
            key: "speech.gpu".to_string(),
            page: Page::Speech,
        };
        assert_eq!(window.picked, Some(gpu));
        assert_eq!(window.text, "", "the page's field gets none of the keys");
    }

    #[test]
    fn a_click_outside_the_palette_closes_it() {
        let mut window = Window::new("");
        window.frame(vec![ctrl_k_press()]);
        window.frame(Vec::new());
        let at = Pos2::new(10.0, 700.0);
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        window.frame(vec![Event::PointerMoved(at), button(true)]);
        window.frame(vec![button(false)]);
        assert!(!window.palette.is_open());
        assert_eq!(window.picked, None);
    }
}
