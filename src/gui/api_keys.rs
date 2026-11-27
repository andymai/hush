//! The key rows of the Speech and Text pages: a field whose Save keeps the
//! key through [`keys::store`], Remove through [`keys::remove`], and where
//! the key is now, never its value. A save or a removal can show the
//! keyring's unlock prompt, so it runs on a thread of its own.

use super::icons;
use super::theme;
use super::widgets::{busy, submitted, Button, Field, Row, Variant};
use super::Gui;
use crate::config::keys::{self, Source};
use crate::config::settings::LlmSetting;
use crate::config::Config;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

/// The control of a key row: the field, Save, and Remove.
const KEY_COLUMN: f32 = theme::FIELD_WIDTH + 180.0;

/// The drafts of the key fields, and the save or removal that runs.
#[derive(Default)]
pub struct KeyRows {
    /// What the user typed into each key field, by variable
    drafts: HashMap<String, String>,
    shared: Arc<Mutex<Job>>,
}

#[derive(Default)]
struct Job {
    /// The variable whose save or removal runs
    running: Option<String>,
    /// What the last one did, for a toast, and the variable it changed
    finished: Option<(String, Option<String>)>,
}

/// Where the key in `var` is now, for the description of its row.
pub fn status(gui: &Gui, var: &str) -> String {
    describe(var, gui.machine.key_source(var), crate::portal::sandboxed())
}

fn describe(var: &str, source: Option<Source>, sandboxed: bool) -> String {
    match source {
        None if sandboxed => format!("{} is not set. Save puts it in the .env file of Hush.", var),
        None => format!("{} is not set. Save puts it in the keyring.", var),
        Some(Source::Environment) => format!(
            "{} comes from the environment, which wins over a saved key.",
            var
        ),
        Some(Source::Keyring) => format!("{} is in the keyring.", var),
        Some(Source::Locked) => format!(
            "{} is in the keyring, which is locked. Save the key again to unlock it.",
            var
        ),
        Some(Source::EnvFile) => format!("{} is in a .env file.", var),
    }
}

/// A row whose control saves the key in `var`, and removes a key that
/// Hush keeps.
pub fn key_row(gui: &mut Gui, ui: &mut egui::Ui, row: Row<'_>, var: &str) {
    let running = gui.api_keys.shared.lock().running.clone();
    if running.as_deref() == Some(var) {
        row.reserve(KEY_COLUMN)
            .show(ui, |ui| busy(ui, "Waiting for the keyring"));
        return;
    }
    let idle = running.is_none();
    let kept = matches!(
        gui.machine.key_source(var),
        Some(Source::Keyring | Source::Locked | Source::EnvFile)
    );
    let draft = gui.api_keys.drafts.entry(var.to_string()).or_default();
    let (save, remove) = row.reserve(KEY_COLUMN).show(ui, |ui| {
        let remove = kept
            && ui
                .add(Button::new("Remove").variant(Variant::Ghost).enabled(idle))
                .clicked();
        let typed = idle && !draft.trim().is_empty();
        let save = ui
            .add(Button::new("Save").icon(icons::CHECK).enabled(typed))
            .clicked();
        let response = ui.add(Field::new(draft, "Paste the key").password());
        (save || (typed && submitted(ui, &response)), remove)
    });
    if save {
        let value = gui.api_keys.drafts.remove(var).unwrap_or_default();
        start(gui, ui.ctx(), var, Some(value));
    } else if remove {
        start(gui, ui.ctx(), var, None);
    }
}

/// Save `value` as the key in `var`, or remove the key with `None`.
fn start(gui: &mut Gui, ctx: &egui::Context, var: &str, value: Option<String>) {
    {
        let mut job = gui.api_keys.shared.lock();
        if job.running.is_some() {
            return;
        }
        job.running = Some(var.to_string());
    }
    let shared = Arc::clone(&gui.api_keys.shared);
    let var = var.to_string();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let outcome = match value {
            Some(value) => keys::store(&var, &value).map(|()| format!("Saved {}.", var)),
            None => keys::remove(&var).map(|()| format!("Removed {}.", var)),
        };
        let finished = match outcome {
            Ok(message) => (message, Some(var)),
            Err(e) => (format!("{:#}", e), None),
        };
        {
            let mut job = shared.lock();
            job.running = None;
            job.finished = Some(finished);
        }
        ctx.request_repaint();
    });
}

/// Show what the last save or removal did, and read the keys again. A
/// daemon that uses the key restarts, so it reads the key.
pub fn finish(gui: &mut Gui, ctx: &egui::Context) {
    let Some((message, changed)) = gui.api_keys.shared.lock().finished.take() else {
        return;
    };
    gui.say(message);
    let Some(var) = changed else {
        return;
    };
    if daemon_uses(&gui.saved, &var) {
        gui.key_changed();
    }
    gui.request_refresh(ctx);
}

/// Whether the daemon of `config` reads the key in `var`.
fn daemon_uses(config: &Config, var: &str) -> bool {
    let llm = &config.llm;
    config.transcription.key_var() == Some(var)
        || (var == "ANTHROPIC_API_KEY"
            && matches!(llm.provider, LlmSetting::Auto | LlmSetting::Anthropic))
        || (llm.provider == LlmSetting::OpenAiCompatible && llm.server_key_var() == Some(var))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::TranscriptionBackend;

    #[test]
    fn the_status_says_where_the_key_is_and_never_its_value() {
        assert_eq!(
            describe("GROQ_API_KEY", None, false),
            "GROQ_API_KEY is not set. Save puts it in the keyring."
        );
        assert_eq!(
            describe("GROQ_API_KEY", None, true),
            "GROQ_API_KEY is not set. Save puts it in the .env file of Hush."
        );
        assert!(describe("GROQ_API_KEY", Some(Source::Locked), false).contains("locked"));
        assert!(describe("GROQ_API_KEY", Some(Source::Environment), false).contains("environment"));
        assert_eq!(
            describe("GROQ_API_KEY", Some(Source::Keyring), false),
            "GROQ_API_KEY is in the keyring."
        );
        assert_eq!(
            describe("GROQ_API_KEY", Some(Source::EnvFile), true),
            "GROQ_API_KEY is in a .env file."
        );
    }

    #[test]
    fn only_a_key_that_the_daemon_reads_restarts_it() {
        let mut config = Config::defaults().unwrap();
        assert!(daemon_uses(&config, "ANTHROPIC_API_KEY"), "auto polish");
        assert!(!daemon_uses(&config, "GROQ_API_KEY"), "local speech");
        config.transcription.backend = TranscriptionBackend::Groq;
        assert!(daemon_uses(&config, "GROQ_API_KEY"));
        assert!(!daemon_uses(&config, "OPENAI_API_KEY"));
        config.llm.provider = LlmSetting::None;
        assert!(!daemon_uses(&config, "ANTHROPIC_API_KEY"));
        config.llm.provider = LlmSetting::OpenAiCompatible;
        config.llm.server_key_env = "LLM_KEY".into();
        assert!(daemon_uses(&config, "LLM_KEY"));
        assert!(!daemon_uses(&config, "ANTHROPIC_API_KEY"));
    }
}
