//! Puts transcribed text into the focused window.
//!
//! uinput types the text as key events, which works in every application,
//! VM, and password field. The keys come from the active layout's XKB
//! keymap, with dead keys for accents that have no key of their own. A take
//! with a character that no key types (CJK, emoji), a very long take, and
//! every take on a layout Hush cannot read go through the clipboard: the
//! previous clipboard is saved, the text is pasted with the chord the
//! focused window takes, and the clipboard is restored if it still holds the
//! text. Both paths first wait for the user to release Ctrl, Alt, Shift, and
//! Super.
//!
//! A terminal gets the text on one line, so a line break cannot run a
//! command. A chat application gets a typed line break as Shift+Enter, so
//! it cannot send the message. A `submit_apps` window gets Enter after a
//! dictation.

use super::clipboard::{self, ClipboardBackend, Restore, Selection, Transcript};
use super::clock::{Clock, SystemClock};
use super::keymap::KeyTable;
use super::modifiers::{self, ModifierWait, PhysicalModifiers};
use super::typing::Resolver;
use super::uinput_keyboard::{LineBreak, UinputKeyboard};
use super::window::{WindowInfo, WindowProvider};
use super::{class_matches, InsertionMethod, PasteChord};
use crate::config::settings::InsertionConfig;
use crate::portal::keyboard::PortalKeyboard;
use crate::portal::{keysym, ConsentRefused, InputMode};
use crate::text_processing::{AppKind, ProcessingContext, ProfilesConfig};
use crate::Result;
use anyhow::Context;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Whether Hush sends keys to a window now. The menu mask taps no Ctrl
/// then: on KWin, one XKB state spans every keyboard, so a Ctrl between
/// typed keys turns a letter into a shortcut such as Ctrl+W.
static SENDING: AtomicBool = AtomicBool::new(false);

pub fn sending_keys() -> bool {
    SENDING.load(Ordering::Acquire)
}

/// Marks the time that Hush sends keys, until it drops.
struct Sending;

impl Sending {
    fn start() -> Self {
        SENDING.store(true, Ordering::Release);
        Self
    }
}

impl Drop for Sending {
    fn drop(&mut self) {
        SENDING.store(false, Ordering::Release);
    }
}

/// Texts longer than this paste even when every character is typeable.
pub const PASTE_THRESHOLD_CHARS: usize = 2000;
/// A slow application reads the clipboard some time after the paste chord.
pub const AFTER_PASTE: Duration = Duration::from_millis(500);
/// Enter follows typed text by about this much, so the application handles
/// the text before the key that sends it.
pub const TYPED_SUBMIT_DELAY: Duration = Duration::from_millis(150);
/// A Wayland paste is asynchronous: the application fetches the data after
/// it gets the paste keys. Enter waits longer, so it does not arrive first.
pub const PASTED_SUBMIT_DELAY: Duration = Duration::from_millis(400);

/// Where the inserted text comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// A dictation, also when paste-last types it again
    Dictation,
    /// A Command Mode rewrite, which replaces text that the user may have
    /// sent already
    Rewrite,
}

/// Konsole (and Yakuake, which embeds it) drops a simulated Ctrl+Shift+V,
/// and the terminal inside VS Code, VSCodium, and Cursor takes Ctrl+V for
/// other things. Shift+Insert pastes in all of them.
const SHIFT_INSERT_CLASSES: [&str; 11] = [
    "konsole",
    "yakuake",
    "code",
    "code-oss",
    "code - oss",
    "code-insiders",
    "code - insiders",
    "code-url-handler",
    "codium",
    "vscodium",
    "cursor",
];

/// The path a given text will take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chosen {
    Uinput,
    Clipboard,
}

/// In portal mode, auto types printable ASCII through keysyms and pastes
/// other text, because the compositor drops a keysym that the layout lacks.
pub fn choose_portal_method(configured: InsertionMethod, text: &str) -> Chosen {
    match configured {
        InsertionMethod::Uinput => Chosen::Uinput,
        InsertionMethod::Clipboard => Chosen::Clipboard,
        InsertionMethod::Auto
            if crate::portal::keysym::covers(text)
                && text.chars().count() <= PASTE_THRESHOLD_CHARS =>
        {
            Chosen::Uinput
        },
        InsertionMethod::Auto => Chosen::Clipboard,
    }
}

/// `table` is the keys for the active layout, or `None` when Hush cannot
/// type on it. One take uses one method: when any character has no keys,
/// the whole take pastes.
pub fn choose_method(configured: InsertionMethod, text: &str, table: Option<&KeyTable>) -> Chosen {
    match configured {
        InsertionMethod::Uinput => Chosen::Uinput,
        InsertionMethod::Clipboard => Chosen::Clipboard,
        InsertionMethod::Auto => {
            if !table.is_some_and(|table| table.covers(text))
                || text.chars().count() > PASTE_THRESHOLD_CHARS
            {
                Chosen::Clipboard
            } else {
                Chosen::Uinput
            }
        },
    }
}

/// The paste chord for a window: a `paste_keys` rule, else Shift+Insert for
/// Konsole and the Electron editors, else Ctrl+Shift+V for the windows the
/// terminal profile matches, else Ctrl+V.
pub fn choose_chord(
    window: Option<&WindowInfo>,
    config: &InsertionConfig,
    profiles: &ProfilesConfig,
) -> PasteChord {
    let Some(window) = window else {
        return PasteChord::CtrlV;
    };
    if let Some(chord) = config.paste_rule(&window.class) {
        return chord;
    }
    if SHIFT_INSERT_CLASSES
        .iter()
        .any(|class| class_matches(class, &window.class))
    {
        return PasteChord::ShiftInsert;
    }
    chord_without_primary(Some(window), profiles)
}

/// Shift+Insert pastes the primary selection in some applications, so it
/// is only safe when Hush can put the text there too.
fn chord_without_primary(window: Option<&WindowInfo>, profiles: &ProfilesConfig) -> PasteChord {
    match window {
        Some(window) if AppKind::classify(window, profiles) == AppKind::Terminal => {
            PasteChord::CtrlShiftV
        },
        _ => PasteChord::CtrlV,
    }
}

/// The chord when the primary selection cannot take the text, or `None`
/// when no chord pastes it. Under Shift Lock, Ctrl+V arrives as
/// Ctrl+Shift+V, so a window that pastes the clipboard on Shift+Insert
/// keeps Shift+Insert. A terminal pastes the primary selection on it, so
/// no chord is left, and the take falls back to typing.
fn fallback_without_primary(
    window: Option<&WindowInfo>,
    profiles: &ProfilesConfig,
    shift_locked: bool,
) -> Option<PasteChord> {
    if !shift_locked {
        return Some(chord_without_primary(window, profiles));
    }
    match window {
        Some(window) if AppKind::classify(window, profiles) == AppKind::Terminal => None,
        _ => Some(PasteChord::ShiftInsert),
    }
}

/// Text for a terminal, where a line break runs the command. Each run of
/// line breaks (`\n`, `\r\n`, `\r`) and the spaces next to them becomes one
/// space, or nothing at the start or end of the text. Other control
/// characters, such as Tab and Escape, go.
pub fn one_line(text: &str) -> String {
    let is_break = |ch: char| matches!(ch, '\n' | '\r');
    let mut line = String::with_capacity(text.len());
    let mut run = String::new();
    let mut run_breaks = false;
    for ch in text.chars() {
        if ch == ' ' || is_break(ch) {
            run.push(ch);
            run_breaks |= is_break(ch);
            continue;
        }
        if ch.is_control() {
            continue;
        }
        if !run_breaks {
            line.push_str(&run);
        } else if !line.is_empty() {
            line.push(' ');
        }
        run.clear();
        run_breaks = false;
        line.push(ch);
    }
    if !run_breaks {
        line.push_str(&run);
    }
    line
}

/// Where a paste goes, and what the keyboard's locks do to its chord.
#[derive(Clone, Copy)]
struct PasteTarget<'a> {
    window: Option<&'a WindowInfo>,
    table: Option<&'a KeyTable>,
    shift_locked: bool,
}

/// Under Shift Lock, every key Hush sends gets Shift, so Ctrl+V arrives as
/// Ctrl+Shift+V, which is Paste Special in LibreOffice and the clipboard
/// history in JetBrains editors. Shift+Insert pastes in every layout.
pub fn paste_chord(chosen: PasteChord, shift_locked: bool) -> PasteChord {
    if shift_locked && chosen != PasteChord::ShiftInsert {
        debug!("Shift can be locked, so the paste uses Shift+Insert");
        PasteChord::ShiftInsert
    } else {
        chosen
    }
}

/// Under Shift Lock, Enter arrives as Shift+Enter, which starts a new line
/// in chat instead of sending the message, so no Enter follows the take.
pub fn submits(submit_app: bool, shift_locked: bool) -> bool {
    if submit_app && shift_locked {
        debug!("Shift can be locked, so no Enter follows the text");
    }
    submit_app && !shift_locked
}

/// Whether Enter follows an insertion: only after a dictation in a
/// `submit_apps` window, only when the keys went out, and only when they
/// carried text. A printed take and a hook that typed nothing never reach
/// the inserter.
pub fn presses_enter(origin: Origin, submit_app: bool, text: &str, sent: &Result<()>) -> bool {
    origin == Origin::Dictation && submit_app && sent.is_ok() && !text.trim().is_empty()
}

/// How keys reach the focused window.
enum Keys {
    /// A uinput keyboard, with the key table of the active layout.
    Uinput(UinputKeyboard),
    /// The RemoteDesktop portal, with keysyms.
    Portal(PortalKeyboard),
}

pub struct TextInserter {
    keys: Keys,
    clipboard: Option<Box<dyn ClipboardBackend>>,
    windows: WindowProvider,
    config: InsertionConfig,
    profiles: ProfilesConfig,
    typing: Resolver,
    /// What the last take found, to log a change once.
    last_typing: String,
}

impl TextInserter {
    /// In the input mode of the user's configuration.
    pub fn new() -> Result<Self> {
        Self::with_mode(InputMode::current())
    }

    /// Native mode needs a working uinput device; both paths type through
    /// it. Portal mode types through the RemoteDesktop portal.
    pub fn with_mode(mode: InputMode) -> Result<Self> {
        info!("Initializing text insertion");
        let mut typing = Resolver::new();
        let (keys, last_typing) = match mode {
            InputMode::Native => {
                let mut uinput = UinputKeyboard::new()?;
                if !uinput.is_ready() {
                    return Err(anyhow::anyhow!(
                        "uinput is not available; run `hush setup permissions` and try again"
                    ));
                }
                let typing = typing.resolve(&mut uinput).to_string();
                (Keys::Uinput(uinput), typing)
            },
            InputMode::Portal => (
                Keys::Portal(PortalKeyboard::new()?),
                "types printable ASCII through the RemoteDesktop portal and pastes the rest"
                    .to_string(),
            ),
        };
        let clipboard = clipboard::open();
        let windows = WindowProvider::detect();
        info!(
            "Text insertion ready: {}, clipboard restore {}, window detection via {}",
            last_typing,
            clipboard
                .as_ref()
                .map_or("off", |clipboard| clipboard.describe()),
            windows.name()
        );
        Ok(Self {
            keys,
            clipboard,
            windows,
            config: InsertionConfig::default(),
            profiles: ProfilesConfig::defaults(),
            typing,
            last_typing,
        })
    }

    /// evdev codes that typing must never press. The X11 hotkey backend
    /// grabs its key on every keyboard, Hush's own included, so a typed
    /// AltGr on that key would start a dictation.
    pub fn reserve_keys(&mut self, keys: Vec<u16>) {
        if !keys.is_empty() {
            debug!("Typing never presses the key codes {:?}", keys);
        }
        self.typing.reserve(keys);
    }

    pub fn configure(&mut self, config: &InsertionConfig) {
        self.config = config.clone();
        if let Keys::Uinput(uinput) = &mut self.keys {
            uinput.set_typing_delay(config.typing_delay_ms);
        }
    }

    /// The profiles' window classes. Terminals paste with Ctrl+Shift+V and
    /// get one line, and chat applications get Shift+Enter line breaks.
    pub fn set_profiles(&mut self, profiles: &ProfilesConfig) {
        self.profiles = profiles.clone();
    }

    pub fn get_focused_window(&self) -> Option<WindowInfo> {
        self.windows.focused()
    }

    /// Returns the text as the window got it, which differs from `text`
    /// when the terminal cleanup changed it.
    pub fn insert_text(&mut self, text: &str) -> Result<String> {
        self.insert(text, Origin::Dictation)
    }

    /// Like [`Self::insert_text`], but never presses Enter.
    pub fn insert_rewrite(&mut self, text: &str) -> Result<String> {
        self.insert(text, Origin::Rewrite)
    }

    fn insert(&mut self, text: &str, origin: Origin) -> Result<String> {
        let window = self.windows.focused();
        let kind = ProcessingContext::new(window.clone(), &self.profiles).kind;
        let text = if kind == AppKind::Terminal {
            let line = one_line(text);
            if line != text {
                debug!(
                    "Put the text on one line for the terminal ({} chars, {} before)",
                    line.chars().count(),
                    text.chars().count()
                );
            }
            line
        } else {
            text.to_string()
        };
        let line_break = if kind == AppKind::Chat {
            LineBreak::ShiftEnter
        } else {
            LineBreak::Enter
        };
        let submit_app = window
            .as_ref()
            .is_some_and(|window| self.config.submits(&window.class));
        info!("Inserting {} chars", text.chars().count());
        debug!(
            "Inserting text: '{}'",
            text.chars().take(50).collect::<String>()
        );
        let method = match &window {
            Some(window) => {
                let method = self.config.method_for(&window.class);
                debug!(
                    "Target window: {} ({}), method {:?}",
                    window.title, window.class, method
                );
                method
            },
            None => self.config.method,
        };

        if matches!(self.keys, Keys::Portal(_)) {
            return self.insert_with_portal(
                text,
                window.as_ref(),
                line_break,
                origin,
                submit_app,
                method,
            );
        }
        let typing = match &mut self.keys {
            Keys::Uinput(uinput) => self.typing.resolve(uinput),
            Keys::Portal(_) => return Err(anyhow::anyhow!("uinput is off in portal mode")),
        };
        // The typing for the active layout and the Caps Lock state of now. A
        // change since the last take is logged once.
        let summary = typing.to_string();
        if summary != self.last_typing {
            info!("Keyboard state changed: Hush now {}", summary);
            self.last_typing = summary;
        }
        let shift_locked = self.typing.shift_locked();
        let submit_app = submits(submit_app, shift_locked);
        // An explicit uinput choice, and a failed paste, type even when no
        // table fits the layout, with the US table as before.
        let table = typing.table().unwrap_or(KeyTable::us());
        match choose_method(method, &text, typing.table()) {
            Chosen::Uinput => {
                self.type_with_uinput(&text, table, line_break, origin, submit_app)?
            },
            Chosen::Clipboard => {
                let target = PasteTarget {
                    window: window.as_ref(),
                    table: typing.table(),
                    shift_locked,
                };
                let pasted = self.paste(&text, target, origin, submit_app);
                if let Err(e) = pasted {
                    warn!("Paste failed ({}); typing what uinput can", e);
                    // Typing can skip characters, so Hush does not send the result.
                    self.type_with_uinput(&text, table, line_break, origin, false)?;
                }
            },
        }
        Ok(text)
    }

    fn type_with_uinput(
        &mut self,
        text: &str,
        table: &KeyTable,
        line_break: LineBreak,
        origin: Origin,
        submit_app: bool,
    ) -> Result<()> {
        let Keys::Uinput(uinput) = &mut self.keys else {
            return Err(anyhow::anyhow!("uinput is off in portal mode"));
        };
        wait_for_modifiers();
        let _sending = Sending::start();
        uinput.set_typing_delay(self.config.typing_delay_ms);
        let typed = uinput.type_text(text, table, line_break);
        if presses_enter(origin, submit_app, text, &typed) {
            press_enter(uinput, &SystemClock, TYPED_SUBMIT_DELAY);
        }
        typed?;
        info!("Text inserted via uinput ({} chars)", text.chars().count());
        Ok(())
    }

    /// Portal mode: one RemoteDesktop session types the text, or pastes it,
    /// and presses the submit Enter.
    fn insert_with_portal(
        &mut self,
        text: String,
        window: Option<&WindowInfo>,
        line_break: LineBreak,
        origin: Origin,
        submit_app: bool,
        method: InsertionMethod,
    ) -> Result<String> {
        match choose_portal_method(method, &text) {
            Chosen::Uinput => self.type_with_portal(&text, line_break, origin, submit_app)?,
            Chosen::Clipboard => {
                let target = PasteTarget {
                    window,
                    table: None,
                    shift_locked: false,
                };
                if let Err(e) = self.paste(&text, target, origin, submit_app) {
                    if e.downcast_ref::<ConsentRefused>().is_some() {
                        return Err(e);
                    }
                    warn!("Paste failed ({}); typing what the portal can", e);
                    self.type_with_portal(&text, line_break, origin, false)?;
                }
            },
        }
        Ok(text)
    }

    fn type_with_portal(
        &mut self,
        text: &str,
        line_break: LineBreak,
        origin: Origin,
        submit_app: bool,
    ) -> Result<()> {
        let Keys::Portal(portal) = &self.keys else {
            return Err(anyhow::anyhow!("the portal is off in native mode"));
        };
        let delay = Duration::from_millis(self.config.typing_delay_ms);
        let mut steps = keysym::text_steps(text, line_break, delay);
        if presses_enter(origin, submit_app, text, &Ok(())) {
            steps.extend(keysym::enter_steps(TYPED_SUBMIT_DELAY));
        }
        portal.send(steps)?;
        info!(
            "Text inserted via the RemoteDesktop portal ({} chars)",
            text.chars().count()
        );
        Ok(())
    }

    fn paste(
        &mut self,
        text: &str,
        target: PasteTarget,
        origin: Origin,
        submit_app: bool,
    ) -> Result<()> {
        let PasteTarget {
            window,
            table,
            shift_locked,
        } = target;
        let clock = SystemClock;
        let mut chord = paste_chord(
            choose_chord(window, &self.config, &self.profiles),
            shift_locked,
        );
        let clipboard = self
            .clipboard
            .as_deref_mut()
            .ok_or_else(|| anyhow::anyhow!("clipboard unavailable"))?;
        let transcript = Transcript::new(text);
        let mut selections = vec![Selection::Clipboard];
        if chord == PasteChord::ShiftInsert {
            selections.push(Selection::Primary);
        }
        let mut saved = Vec::with_capacity(selections.len());
        let mut no_chord = None;
        for selection in selections {
            let snapshot = clipboard::snapshot(clipboard, selection, &clock);
            match clipboard.offer(selection, transcript.formats()) {
                Ok(()) => saved.push((selection, snapshot)),
                Err(e) if selection == Selection::Clipboard => {
                    return Err(e).context("failed to set clipboard");
                },
                Err(e) => match fallback_without_primary(window, &self.profiles, shift_locked) {
                    Some(fallback) => {
                        chord = fallback;
                        warn!(
                            "Could not set the {} ({:#}); pasting with {} instead",
                            selection, e, chord
                        );
                    },
                    None => {
                        no_chord = Some(anyhow::anyhow!(
                            "could not set the {} ({:#}), and under Shift Lock no other \
                             chord pastes in a terminal",
                            selection,
                            e
                        ));
                        break;
                    },
                },
            }
        }
        if let Some(e) = no_chord {
            for (selection, snapshot) in saved {
                let outcome =
                    clipboard::restore(clipboard, selection, snapshot, &transcript, &clock);
                log_restore(selection, &outcome);
            }
            return Err(e);
        }
        debug!("Pasting with {}", chord);
        let sent;
        let pasted_at;
        match &mut self.keys {
            Keys::Uinput(uinput) => {
                wait_for_modifiers();
                let sending = Sending::start();
                sent = uinput.send_paste_shortcut(chord, table, self.typing.chords());
                pasted_at = clock.now();
                if presses_enter(origin, submit_app, text, &sent) {
                    press_enter(uinput, &clock, PASTED_SUBMIT_DELAY);
                }
                drop(sending);
            },
            Keys::Portal(portal) => {
                let mut steps = keysym::chord_steps(chord);
                if presses_enter(origin, submit_app, text, &Ok(())) {
                    steps.extend(keysym::enter_steps(PASTED_SUBMIT_DELAY));
                }
                sent = portal.send(steps);
                pasted_at = clock.now();
            },
        }
        if sent.is_ok() {
            let waited = clock.now().saturating_duration_since(pasted_at);
            clock.sleep(AFTER_PASTE.saturating_sub(waited));
        }
        for (selection, snapshot) in saved {
            let outcome = clipboard::restore(clipboard, selection, snapshot, &transcript, &clock);
            log_restore(selection, &outcome);
        }
        sent?;
        info!(
            "Text inserted via clipboard with {} ({} chars)",
            chord,
            text.chars().count()
        );
        Ok(())
    }

    /// Leave text on the clipboard for the user to paste by hand.
    pub fn copy_to_clipboard(&mut self, text: &str) -> Result<()> {
        let clipboard = self
            .clipboard
            .as_deref_mut()
            .ok_or_else(|| anyhow::anyhow!("clipboard unavailable"))?;
        clipboard
            .offer(Selection::Clipboard, clipboard::text_formats(text))
            .context("failed to set clipboard")
    }

    /// Undo the last insertion by sending backspaces.
    pub fn undo_last_insertion(&mut self, char_count: usize) -> Result<()> {
        info!("Undoing last insertion ({} chars)", char_count);
        let delay = Duration::from_millis(self.config.typing_delay_ms);
        match &mut self.keys {
            Keys::Uinput(uinput) => {
                wait_for_modifiers();
                let _sending = Sending::start();
                for _ in 0..char_count {
                    uinput.send_backspace()?;
                    thread::sleep(delay);
                }
                Ok(())
            },
            Keys::Portal(portal) => portal.send(keysym::backspace_steps(char_count, delay)),
        }
    }

    pub fn test_insertion(&mut self) -> Result<()> {
        self.insert_text("Hello from Hush voice-to-text!")?;
        thread::sleep(Duration::from_millis(500));
        self.insert_text("\nThis includes: special-chars, numbers123, and symbols!@#")?;
        Ok(())
    }
}

fn wait_for_modifiers() {
    match modifiers::wait_for_release(&PhysicalModifiers, &SystemClock) {
        ModifierWait::Released { waited } if !waited.is_zero() => {
            debug!("Waited {:?} for the modifier keys to come up", waited)
        },
        ModifierWait::Released { .. } => {},
        ModifierWait::GaveUp => warn!(
            "A modifier key is still down after {:?}; inserting anyway",
            modifiers::CAP
        ),
        ModifierWait::Unknown => debug!("Modifier key state unknown"),
    }
}

/// A failure only logs, because the text is already in the window.
fn press_enter(uinput: &mut UinputKeyboard, clock: &dyn Clock, delay: Duration) {
    // The modifier wait ends with SETTLE, which counts toward the delay.
    clock.sleep(delay.saturating_sub(modifiers::SETTLE));
    wait_for_modifiers();
    match uinput.send_enter() {
        Ok(()) => info!("Pressed Enter after the text"),
        Err(e) => warn!("Could not press Enter after the text: {}", e),
    }
}

fn log_restore(selection: Selection, outcome: &Restore) {
    match outcome {
        Restore::Restored => debug!("Restored the previous {}", selection),
        Restore::Cleared => debug!("Cleared the {}, which was empty before", selection),
        Restore::Kept(why) => warn!("The {} keeps the transcript: {}", selection, why),
        Restore::Replaced => info!(
            "The {} changed after the paste; Hush keeps the new content",
            selection
        ),
        Restore::CheckFailed(e) => warn!(
            "Could not check the {} after the paste ({}); it stays as it is",
            selection, e
        ),
        Restore::Failed(e) => warn!("Could not restore the {}: {}", selection, e),
    }
}

/// Explain how device access is granted.
pub fn print_uinput_setup_guidance() {
    println!("\n🔧 Device access for Hush");
    println!("═══════════════════════════");
    println!("Hush types through /dev/uinput and reads hotkeys from /dev/input.");
    println!("`hush setup permissions` installs one udev rule that grants both to the");
    println!("logged-in user through a polkit prompt, with no logout.\n");
    println!("The rule it installs, for reference or manual use:\n");
    print!("{}", crate::permissions::install_script());
}

/// Report the current device access state.
pub fn diagnose_uinput_issues() -> Result<()> {
    crate::permissions::print_status(&crate::permissions::status());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn us() -> Option<&'static KeyTable> {
        Some(KeyTable::us())
    }

    #[test]
    fn the_sending_flag_covers_the_guard_only() {
        assert!(!sending_keys());
        let sending = Sending::start();
        assert!(sending_keys(), "the menu mask sees that Hush types");
        drop(sending);
        assert!(!sending_keys());
    }

    #[test]
    fn auto_types_plain_text_and_pastes_the_rest() {
        assert_eq!(
            choose_method(InsertionMethod::Auto, "hello, world!\n", us()),
            Chosen::Uinput
        );
        assert_eq!(
            choose_method(InsertionMethod::Auto, "café", us()),
            Chosen::Clipboard,
            "one character without keys pastes the whole take"
        );
        assert_eq!(
            choose_method(InsertionMethod::Auto, "日本語", us()),
            Chosen::Clipboard
        );
        let long = "a".repeat(PASTE_THRESHOLD_CHARS + 1);
        assert_eq!(
            choose_method(InsertionMethod::Auto, &long, us()),
            Chosen::Clipboard
        );
    }

    #[test]
    fn portal_mode_types_ascii_and_pastes_the_rest() {
        assert_eq!(
            choose_portal_method(InsertionMethod::Auto, "hello, world!\n"),
            Chosen::Uinput
        );
        assert_eq!(
            choose_portal_method(InsertionMethod::Auto, "café"),
            Chosen::Clipboard,
            "the compositor drops a keysym that the layout lacks"
        );
        let long = "a".repeat(PASTE_THRESHOLD_CHARS + 1);
        assert_eq!(
            choose_portal_method(InsertionMethod::Auto, &long),
            Chosen::Clipboard
        );
        assert_eq!(
            choose_portal_method(InsertionMethod::Uinput, "café"),
            Chosen::Uinput,
            "an explicit choice types every keysym"
        );
        assert_eq!(
            choose_portal_method(InsertionMethod::Clipboard, "abc"),
            Chosen::Clipboard
        );
    }

    #[test]
    fn explicit_methods_win() {
        assert_eq!(
            choose_method(InsertionMethod::Uinput, "café", us()),
            Chosen::Uinput
        );
        assert_eq!(
            choose_method(InsertionMethod::Clipboard, "abc", us()),
            Chosen::Clipboard
        );
    }

    #[test]
    fn a_layout_table_types_its_own_characters() {
        use super::super::keymap::Tap;
        use input_linux::Key;
        let chars = std::collections::HashMap::from([
            ('a', vec![Tap::plain(Key::Q)]),
            ('é', vec![Tap::plain(Key::Equal), Tap::plain(Key::E)]),
            (' ', vec![Tap::plain(Key::Space)]),
        ]);
        let table = KeyTable::new(chars, Key::LeftShift);
        assert_eq!(
            choose_method(InsertionMethod::Auto, "é a\n", Some(&table)),
            Chosen::Uinput
        );
        assert_eq!(
            choose_method(InsertionMethod::Auto, "é b", Some(&table)),
            Chosen::Clipboard
        );
    }

    fn window(class: &str) -> WindowInfo {
        WindowInfo {
            title: String::new(),
            class: class.to_string(),
        }
    }

    fn chord(class: &str, config: &InsertionConfig, profiles: &ProfilesConfig) -> PasteChord {
        choose_chord(Some(&window(class)), config, profiles)
    }

    #[test]
    fn terminals_paste_with_ctrl_shift_v() {
        let config = InsertionConfig::default();
        let profiles = ProfilesConfig::defaults();
        for class in [
            "kitty",
            "Alacritty",
            "org.gnome.Ptyxis",
            "foot",
            "st-256color",
        ] {
            assert_eq!(
                chord(class, &config, &profiles),
                PasteChord::CtrlShiftV,
                "{class}"
            );
        }
        for class in [
            "firefox",
            "org.gnome.Nautilus",
            "steam",
            "libreoffice-writer",
        ] {
            assert_eq!(
                chord(class, &config, &profiles),
                PasteChord::CtrlV,
                "{class}"
            );
        }
        assert_eq!(
            choose_chord(None, &config, &profiles),
            PasteChord::CtrlV,
            "an unknown window gets the common chord"
        );
    }

    #[test]
    fn konsole_and_electron_editors_paste_with_shift_insert() {
        let config = InsertionConfig::default();
        let profiles = ProfilesConfig::defaults();
        for class in [
            "org.kde.konsole",
            "konsole\0konsole",
            "org.kde.yakuake",
            "code",
            "code\0Code",
            "com.visualstudio.code",
            "code-oss",
            "VSCodium",
            "com.vscodium.codium",
            "Cursor",
        ] {
            assert_eq!(
                chord(class, &config, &profiles),
                PasteChord::ShiftInsert,
                "{class}"
            );
        }
        assert_eq!(
            chord("codeblocks", &config, &profiles),
            PasteChord::CtrlV,
            "only whole names count"
        );
    }

    #[test]
    fn the_terminal_profile_extends_to_user_classes() {
        let config = InsertionConfig::default();
        let profiles = ProfilesConfig {
            terminal: vec!["my-term".to_string()],
            ..ProfilesConfig::defaults()
        };
        assert_eq!(chord("my-term", &config, &profiles), PasteChord::CtrlShiftV);
        assert_eq!(
            chord("my-term", &config, &ProfilesConfig::defaults()),
            PasteChord::CtrlV
        );
    }

    #[test]
    fn a_paste_keys_rule_wins() {
        let mut config = InsertionConfig::default();
        config
            .paste_keys
            .insert("kitty".to_string(), PasteChord::CtrlV);
        config
            .paste_keys
            .insert("konsole".to_string(), PasteChord::CtrlShiftV);
        config
            .paste_keys
            .insert("st".to_string(), PasteChord::ShiftInsert);
        let profiles = ProfilesConfig::defaults();
        assert_eq!(chord("kitty", &config, &profiles), PasteChord::CtrlV);
        assert_eq!(
            chord("org.kde.konsole", &config, &profiles),
            PasteChord::CtrlShiftV
        );
        assert_eq!(chord("st", &config, &profiles), PasteChord::ShiftInsert);
        assert_eq!(
            chord("steam", &config, &profiles),
            PasteChord::CtrlV,
            "a rule for st does not touch steam"
        );
    }

    #[test]
    fn without_the_primary_selection_shift_insert_falls_back() {
        let profiles = ProfilesConfig::defaults();
        assert_eq!(
            chord_without_primary(Some(&window("org.kde.konsole")), &profiles),
            PasteChord::CtrlShiftV
        );
        assert_eq!(
            chord_without_primary(Some(&window("code")), &profiles),
            PasteChord::CtrlV
        );
    }

    #[test]
    fn a_layout_without_a_table_pastes_even_plain_text() {
        assert_eq!(
            choose_method(InsertionMethod::Auto, "hello", None),
            Chosen::Clipboard,
            "key codes could come out as other letters"
        );
        assert_eq!(
            choose_method(InsertionMethod::Uinput, "hello", None),
            Chosen::Uinput,
            "an explicit choice is still honoured"
        );
    }

    #[test]
    fn terminal_text_keeps_to_one_line() {
        assert_eq!(one_line("ls -la\ncd /tmp"), "ls -la cd /tmp");
        assert_eq!(
            one_line("a\r\nb\rc\n\nd"),
            "a b c d",
            "every line break style, and a paragraph break, is one space"
        );
        assert_eq!(one_line("echo hi \n  there"), "echo hi there");
        assert_eq!(
            one_line("\n\ngit status \n"),
            "git status",
            "a break at either end leaves no space"
        );
        assert_eq!(one_line("\n"), "");
    }

    #[test]
    fn terminal_text_loses_control_characters() {
        assert_eq!(one_line("a\tb\x1b[31mc\x07"), "ab[31mc");
        assert_eq!(
            one_line("a \t\n b"),
            "a b",
            "a removed character does not split a run of breaks and spaces"
        );
        assert_eq!(one_line("x\u{7f}\u{85}y"), "xy");
    }

    #[test]
    fn terminal_text_keeps_spaces_without_a_break() {
        assert_eq!(one_line("keep  two spaces "), "keep  two spaces ");
        assert_eq!(one_line(" lead"), " lead");
        assert_eq!(one_line("plain"), "plain");
        assert_eq!(one_line("café ⌘"), "café ⌘");
    }

    #[test]
    fn enter_follows_only_a_successful_insertion_with_text() {
        let dictation = Origin::Dictation;
        let sent: Result<()> = Ok(());
        let failed: Result<()> = Err(anyhow::anyhow!("uinput write failed"));
        assert!(presses_enter(dictation, true, "ls -la", &sent));
        assert!(
            !presses_enter(dictation, true, "ls -la", &failed),
            "a failed insertion"
        );
        assert!(
            !presses_enter(dictation, true, "", &sent),
            "an empty insertion"
        );
        assert!(
            !presses_enter(dictation, true, " \n ", &sent),
            "an insertion of only white space"
        );
        assert!(
            !presses_enter(dictation, false, "ls -la", &sent),
            "a window outside submit_apps"
        );
    }

    #[test]
    fn under_shift_lock_a_failed_primary_selection_keeps_shift_insert_or_types() {
        let profiles = ProfilesConfig::defaults();
        let editor = window("firefox");
        let terminal = window("kitty");
        assert_eq!(
            fallback_without_primary(Some(&editor), &profiles, false),
            Some(PasteChord::CtrlV)
        );
        assert_eq!(
            fallback_without_primary(Some(&terminal), &profiles, false),
            Some(PasteChord::CtrlShiftV)
        );
        assert_eq!(
            fallback_without_primary(Some(&editor), &profiles, true),
            Some(PasteChord::ShiftInsert),
            "Ctrl+V would arrive as Ctrl+Shift+V; Shift+Insert pastes the clipboard here"
        );
        assert_eq!(
            fallback_without_primary(None, &profiles, true),
            Some(PasteChord::ShiftInsert)
        );
        assert_eq!(
            fallback_without_primary(Some(&terminal), &profiles, true),
            None,
            "a terminal pastes the primary selection on Shift+Insert, so the take types"
        );
    }

    #[test]
    fn shift_lock_pastes_with_shift_insert_and_sends_no_enter() {
        for chord in [
            PasteChord::CtrlV,
            PasteChord::CtrlShiftV,
            PasteChord::ShiftInsert,
        ] {
            assert_eq!(paste_chord(chord, true), PasteChord::ShiftInsert, "{chord}");
            assert_eq!(paste_chord(chord, false), chord, "{chord}");
        }
        assert!(submits(true, false));
        assert!(
            !submits(true, true),
            "Enter would arrive as Shift+Enter, a new line in chat"
        );
        assert!(!submits(false, false));
    }

    #[test]
    fn a_command_mode_rewrite_never_presses_enter() {
        let sent: Result<()> = Ok(());
        assert!(
            presses_enter(Origin::Dictation, true, "see you then", &sent),
            "a dictation, and paste-last"
        );
        assert!(
            !presses_enter(Origin::Rewrite, true, "see you then", &sent),
            "the rewritten text can replace a message that is already sent"
        );
    }

    #[test]
    fn enter_waits_longer_after_a_paste() {
        assert!(PASTED_SUBMIT_DELAY > TYPED_SUBMIT_DELAY);
        assert!(
            PASTED_SUBMIT_DELAY < AFTER_PASTE,
            "Enter comes before the clipboard restore"
        );
    }
}
