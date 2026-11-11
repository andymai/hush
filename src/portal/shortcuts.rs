//! The hotkeys through the GlobalShortcuts portal.
//!
//! Hush binds one portal shortcut for each action. The desktop owns the
//! triggers: Hush proposes a trigger for the dictation key, and the user
//! sets or changes every trigger in the desktop's settings. `Activated` and
//! `Deactivated` become key presses and releases, with the portal's
//! timestamps, for the gesture detector, so hold, tap, double-tap lock, and
//! toggle mode work as with evdev.

use super::PortalError;
use crate::config::settings::HotkeyConfig;
use crate::hotkey::{HotkeyAction, HotkeyEvent, KeyCombination, Stamped};
use crate::text_processing::StyleName;
use crate::transcription::Language;
use async_trait::async_trait;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc as async_mpsc, oneshot};
use tracing::{debug, info, warn};

/// The dictation trigger when the config chord has none: a bare modifier
/// such as Right Alt cannot be a portal trigger.
pub const DEFAULT_DICTATE: &str = "LOGO+space";

/// A second `Activated` with no `Deactivated` between, sooner than this, is
/// key repeat. Later, the desktop lost the release, so it counts as one.
const REPEAT_GAP: Duration = Duration::from_secs(1);

/// A portal timestamp further behind the clock than this is stale.
const MAX_LAG: Duration = Duration::from_secs(2);

/// What a portal shortcut does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Meaning {
    Dictate,
    Cancel,
    Command,
    Action(HotkeyAction),
    Language(Language),
    Style(StyleName),
}

impl Meaning {
    /// The hotkey event for a press (`true`) or a release.
    pub fn event(self, active: bool) -> Option<HotkeyEvent> {
        match (self, active) {
            (Self::Dictate, true) => Some(HotkeyEvent::Pressed),
            (Self::Dictate, false) => Some(HotkeyEvent::Released),
            (Self::Cancel, true) => Some(HotkeyEvent::Cancel),
            (Self::Command, active) => Some(HotkeyEvent::Command(active)),
            (Self::Action(action), true) => Some(HotkeyEvent::Action(action)),
            (Self::Language(language), active) => Some(HotkeyEvent::Language(language, active)),
            (Self::Style(style), active) => Some(HotkeyEvent::Style(style, active)),
            (Self::Cancel | Self::Action(_), false) => None,
        }
    }
}

/// A shortcut for the portal to bind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutSpec {
    pub id: String,
    pub description: String,
    /// In the XDG shortcuts format, such as `LOGO+space`.
    pub preferred: Option<String>,
    pub meaning: Meaning,
}

/// A shortcut as the desktop reports it.
pub use crate::ipc::protocol::PortalShortcut as Binding;

/// The portal trigger for a config chord. Only a chord with a modifier and
/// a key that is not one: a bare key as a global shortcut never reaches
/// applications, and Escape or a letter must reach them.
pub fn portal_trigger(chord: &str) -> Option<String> {
    let chord = KeyCombination::parse(chord).ok()?;
    let key = chord.xdg_key_name()?;
    let modifiers = chord.modifiers;
    let mut parts = Vec::new();
    if modifiers.ctrl {
        parts.push("CTRL");
    }
    if modifiers.alt {
        parts.push("ALT");
    }
    if modifiers.shift {
        parts.push("SHIFT");
    }
    if modifiers.super_key {
        parts.push("LOGO");
    }
    if parts.is_empty() {
        return None;
    }
    parts.push(key);
    Some(parts.join("+"))
}

/// The shortcuts for the hotkey config: dictation, cancel, Command Mode,
/// paste-last, and learn always, and a shortcut for each configured
/// language, style, and rewrite chord. A chord that converts gives the
/// preferred trigger; the rest wait for the user to set them.
pub fn specs(hotkey: &HotkeyConfig) -> Vec<ShortcutSpec> {
    let spec = |id: String, description: String, chord: &str, meaning| ShortcutSpec {
        id,
        description,
        preferred: portal_trigger(chord),
        meaning,
    };
    let mut specs = vec![ShortcutSpec {
        preferred: portal_trigger(&hotkey.combination).or_else(|| Some(DEFAULT_DICTATE.into())),
        ..spec(
            "dictate".into(),
            "Dictate: hold to talk, double-tap to lock".into(),
            "",
            Meaning::Dictate,
        )
    }];
    specs.push(spec(
        "cancel".into(),
        "Discard the recording".into(),
        &hotkey.cancel,
        Meaning::Cancel,
    ));
    specs.push(spec(
        "command".into(),
        "Command Mode: speak an instruction for the selected text".into(),
        &hotkey.command,
        Meaning::Command,
    ));
    specs.push(spec(
        "paste-last".into(),
        "Type the last transcript again".into(),
        &hotkey.paste_last,
        Meaning::Action(HotkeyAction::PasteLast),
    ));
    specs.push(spec(
        "learn".into(),
        "Learn the selected text".into(),
        &hotkey.learn,
        Meaning::Action(HotkeyAction::Learn),
    ));
    for (language, chord) in hotkey.language_keys() {
        specs.push(spec(
            format!("language-{}", language.code()),
            format!("Dictate in {}", language.name()),
            chord,
            Meaning::Language(language),
        ));
    }
    for (style, chord) in hotkey.style_keys() {
        specs.push(spec(
            format!("style-{}", style.as_str()),
            format!("Dictate in the {} style", style.as_str()),
            chord,
            Meaning::Style(style),
        ));
    }
    for (style, chord) in hotkey.rewrite_keys() {
        specs.push(spec(
            format!("rewrite-{}", style.as_str()),
            format!("Rewrite the selection in the {} style", style.as_str()),
            chord,
            Meaning::Action(HotkeyAction::Rewrite(style)),
        ));
    }
    specs
}

/// A signal from the portal, with its timestamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    Activated { id: String, stamp: Duration },
    Deactivated { id: String, stamp: Duration },
    Changed(Vec<Binding>),
}

/// Maps the portal's timestamps, whose start the spec leaves open, to the
/// clock of the gesture detector. The first timestamp marks the start. A
/// timestamp in the future, from the past of the mark, or far behind the
/// clock sets a new mark. The times never go back.
#[derive(Debug, Default)]
pub struct PortalClock {
    mark: Option<(Duration, Instant)>,
    last: Option<Instant>,
}

impl PortalClock {
    pub fn at(&mut self, stamp: Duration, now: Instant) -> Instant {
        let at = match self.mark {
            _ if stamp.is_zero() => now,
            Some((mark, then)) if stamp >= mark => {
                let at = then + (stamp - mark);
                if at > now || now.duration_since(at) > MAX_LAG {
                    self.mark = Some((stamp, now));
                    now
                } else {
                    at
                }
            },
            _ => {
                self.mark = Some((stamp, now));
                now
            },
        };
        let at = self.last.map_or(at, |last| at.max(last));
        self.last = Some(at);
        at
    }
}

/// Turns portal signals into stamped hotkey events.
pub struct Translator {
    meanings: HashMap<String, Meaning>,
    /// The shortcuts that are down, with the time of their last signal.
    active: HashMap<String, Instant>,
    clock: PortalClock,
}

impl Translator {
    pub fn new(specs: &[ShortcutSpec]) -> Self {
        Self {
            meanings: specs
                .iter()
                .map(|spec| (spec.id.clone(), spec.meaning))
                .collect(),
            active: HashMap::new(),
            clock: PortalClock::default(),
        }
    }

    pub fn signal(&mut self, signal: Signal, now: Instant) -> Option<Stamped> {
        let (id, stamp, pressed) = match signal {
            Signal::Activated { id, stamp } => (id, stamp, true),
            Signal::Deactivated { id, stamp } => (id, stamp, false),
            Signal::Changed(_) => return None,
        };
        let Some(&meaning) = self.meanings.get(&id) else {
            debug!("Portal shortcut {} is not Hush's", id);
            return None;
        };
        let at = self.clock.at(stamp, now);
        let release = |meaning: Meaning| meaning.event(false).map(|event| Stamped { event, at });
        if !pressed {
            return self.active.remove(&id).and_then(|_| release(meaning));
        }
        match self.active.get(&id).copied() {
            Some(last) if at.saturating_duration_since(last) < REPEAT_GAP => {
                self.active.insert(id, at);
                None
            },
            Some(_) => {
                debug!(
                    "Portal shortcut {} came again with no release; it counts as one",
                    id
                );
                self.active.remove(&id);
                release(meaning)
            },
            None => {
                self.active.insert(id, at);
                meaning.event(true).map(|event| Stamped { event, at })
            },
        }
    }
}

/// The GlobalShortcuts portal, one session.
#[async_trait]
pub trait ShortcutsPortal: Send {
    async fn bind(&mut self, specs: &[ShortcutSpec]) -> Result<Vec<Binding>, PortalError>;
    /// The next signal of the session, or `None` when the portal goes.
    async fn next_signal(&mut self) -> Option<Signal>;
    /// Opens the desktop's dialog for the shortcuts (version 2).
    async fn configure(&mut self) -> Result<(), PortalError>;
    async fn close(&mut self);
}

/// A request from the daemon to the portal thread.
pub enum Request {
    Configure(oneshot::Sender<Result<(), PortalError>>),
}

/// The part of the portal thread that IPC and the daemon hold.
#[derive(Clone)]
pub struct ShortcutsHandle {
    requests: async_mpsc::UnboundedSender<Request>,
    bindings: Arc<Mutex<Vec<Binding>>>,
    version: u32,
}

impl ShortcutsHandle {
    pub fn bindings(&self) -> Vec<Binding> {
        self.bindings.lock().clone()
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    /// Opens the desktop's shortcut dialog. Needs GlobalShortcuts version 2.
    pub async fn configure(&self) -> Result<(), PortalError> {
        if self.version < 2 {
            return Err(PortalError::Unavailable(
                "GlobalShortcuts version 2 (ConfigureShortcuts)".into(),
            ));
        }
        let (reply, answer) = oneshot::channel();
        self.requests
            .send(Request::Configure(reply))
            .map_err(|_| PortalError::Failed("the shortcut thread stopped".into()))?;
        answer
            .await
            .map_err(|_| PortalError::Failed("the shortcut thread stopped".into()))?
    }
}

/// Feeds the portal's signals to `events` and answers requests, until the
/// portal goes or the daemon drops its handle.
pub async fn serve<P: ShortcutsPortal>(
    mut portal: P,
    specs: Vec<ShortcutSpec>,
    events: mpsc::Sender<Stamped>,
    mut requests: async_mpsc::UnboundedReceiver<Request>,
    bindings: Arc<Mutex<Vec<Binding>>>,
) {
    let mut translator = Translator::new(&specs);
    loop {
        tokio::select! {
            signal = portal.next_signal() => match signal {
                None => {
                    warn!("The GlobalShortcuts portal went away; the hotkeys stop");
                    break;
                },
                Some(Signal::Changed(list)) => {
                    info!("The desktop changed Hush's shortcuts: {}", describe(&list));
                    *bindings.lock() = list;
                },
                Some(signal) => {
                    if let Some(stamped) = translator.signal(signal, Instant::now()) {
                        if events.send(stamped).is_err() {
                            break;
                        }
                    }
                },
            },
            request = requests.recv() => match request {
                None => break,
                Some(Request::Configure(reply)) => {
                    let _ = reply.send(portal.configure().await);
                },
            },
        }
    }
    portal.close().await;
}

/// `Dictate: Meta+Space, Cancel: none`, for logs.
pub fn describe(bindings: &[Binding]) -> String {
    bindings
        .iter()
        .map(|binding| {
            let trigger = if binding.trigger.is_empty() {
                "none"
            } else {
                binding.trigger.as_str()
            };
            format!("{}: {}", binding.id, trigger)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Starts the portal thread: it connects, binds the shortcuts, and then
/// feeds the signals. The call returns when the session is open, so a
/// missing portal is an error here. The binding goes on in the thread,
/// because GNOME asks the user first, and the daemon must not wait for
/// that answer.
pub fn start<P, F, Fut>(
    connect: F,
    specs: Vec<ShortcutSpec>,
) -> anyhow::Result<(ShortcutsHandle, mpsc::Receiver<Stamped>)>
where
    P: ShortcutsPortal + 'static,
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<(P, u32), PortalError>>,
{
    let (events, receiver) = mpsc::channel();
    let (requests, inbox) = async_mpsc::unbounded_channel();
    let bindings = Arc::new(Mutex::new(Vec::new()));
    let (ready, outcome) = mpsc::channel::<Result<u32, PortalError>>();
    let thread_bindings = Arc::clone(&bindings);
    thread::Builder::new()
        .name("hush-global-shortcuts".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(e) => {
                    let _ = ready.send(Err(PortalError::Failed(e.to_string())));
                    return;
                },
            };
            runtime.block_on(async move {
                let (mut portal, version) = match connect().await {
                    Ok(connected) => connected,
                    Err(e) => {
                        let _ = ready.send(Err(e));
                        return;
                    },
                };
                let _ = ready.send(Ok(version));
                match portal.bind(&specs).await {
                    Ok(list) => {
                        info!("Portal shortcuts bound: {}", describe(&list));
                        *thread_bindings.lock() = list;
                    },
                    Err(e) => {
                        warn!("Cannot bind the portal shortcuts: {}", e);
                        crate::notify::notify(
                            crate::notify::NoticeKind::Problem,
                            "Hush has no shortcuts",
                            "The desktop did not bind the shortcuts of Hush. Run \
                             `hush daemon restart` to ask again, or bind `hush toggle` to a key \
                             in the desktop settings.",
                        );
                        portal.close().await;
                        return;
                    },
                }
                serve(portal, specs, events, inbox, thread_bindings).await;
            });
        })
        .map_err(|e| anyhow::anyhow!("cannot start the shortcut thread: {}", e))?;
    let version = outcome
        .recv()
        .map_err(|_| anyhow::anyhow!("the shortcut thread stopped"))?
        .map_err(|e| anyhow::anyhow!("cannot open the GlobalShortcuts portal: {}", e))?;
    Ok((
        ShortcutsHandle {
            requests,
            bindings,
            version,
        },
        receiver,
    ))
}

/// The GlobalShortcuts portal through ashpd.
pub struct AshpdShortcuts {
    proxy: ashpd::desktop::global_shortcuts::GlobalShortcuts,
    session: ashpd::desktop::Session<ashpd::desktop::global_shortcuts::GlobalShortcuts>,
    signals: futures_util::stream::BoxStream<'static, Signal>,
}

impl AshpdShortcuts {
    pub async fn connect() -> Result<(Self, u32), PortalError> {
        use ashpd::desktop::global_shortcuts::GlobalShortcuts;
        use futures_util::StreamExt;
        let error = |e| PortalError::from_ashpd("GlobalShortcuts", e);
        let proxy = GlobalShortcuts::new().await.map_err(error)?;
        let version = proxy.version();
        let session = proxy
            .create_session(Default::default())
            .await
            .map_err(error)?;
        let activated = proxy
            .receive_activated()
            .await
            .map_err(error)?
            .map(|signal| Signal::Activated {
                id: signal.shortcut_id().to_string(),
                stamp: signal.timestamp(),
            });
        let deactivated = proxy
            .receive_deactivated()
            .await
            .map_err(error)?
            .map(|signal| Signal::Deactivated {
                id: signal.shortcut_id().to_string(),
                stamp: signal.timestamp(),
            });
        let changed = proxy
            .receive_shortcuts_changed()
            .await
            .map_err(error)?
            .map(|signal| Signal::Changed(bindings_of(signal.shortcuts())));
        let signals = futures_util::stream::select(
            futures_util::stream::select(activated, deactivated),
            changed,
        )
        .boxed();
        Ok((
            Self {
                proxy,
                session,
                signals,
            },
            version,
        ))
    }
}

fn bindings_of(shortcuts: &[ashpd::desktop::global_shortcuts::Shortcut]) -> Vec<Binding> {
    shortcuts
        .iter()
        .map(|shortcut| Binding {
            id: shortcut.id().to_string(),
            description: shortcut.description().to_string(),
            trigger: shortcut.trigger_description().to_string(),
        })
        .collect()
}

#[async_trait]
impl ShortcutsPortal for AshpdShortcuts {
    async fn bind(&mut self, specs: &[ShortcutSpec]) -> Result<Vec<Binding>, PortalError> {
        use ashpd::desktop::global_shortcuts::NewShortcut;
        let error = |e| PortalError::from_ashpd("GlobalShortcuts", e);
        let shortcuts: Vec<NewShortcut> = specs
            .iter()
            .map(|spec| {
                NewShortcut::new(spec.id.as_str(), spec.description.as_str())
                    .preferred_trigger(spec.preferred.as_deref())
            })
            .collect();
        let bound = self
            .proxy
            .bind_shortcuts(&self.session, &shortcuts, None, Default::default())
            .await
            .map_err(error)?
            .response()
            .map_err(error)?;
        Ok(bindings_of(bound.shortcuts()))
    }

    async fn next_signal(&mut self) -> Option<Signal> {
        use futures_util::StreamExt;
        self.signals.next().await
    }

    async fn configure(&mut self) -> Result<(), PortalError> {
        self.proxy
            .configure_shortcuts(&self.session, None, Default::default())
            .await
            .map_err(|e| PortalError::from_ashpd("GlobalShortcuts", e))
    }

    async fn close(&mut self) {
        if let Err(e) = self.session.close().await {
            debug!("Cannot close the GlobalShortcuts session: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(toml_text: &str) -> HotkeyConfig {
        crate::Config::from_user_table(toml::from_str(toml_text).unwrap())
            .unwrap()
            .hotkey
    }

    #[test]
    fn only_a_chord_with_a_modifier_and_a_key_becomes_a_trigger() {
        assert_eq!(
            portal_trigger("Ctrl+Shift+Space").as_deref(),
            Some("CTRL+SHIFT+space")
        );
        assert_eq!(portal_trigger("Super+F9").as_deref(), Some("LOGO+F9"));
        assert_eq!(portal_trigger("Alt+a").as_deref(), Some("ALT+a"));
        assert_eq!(
            portal_trigger("Ctrl+PageUp").as_deref(),
            Some("CTRL+Page_Up")
        );
        assert_eq!(portal_trigger("RightAlt"), None, "a bare modifier");
        assert_eq!(
            portal_trigger("Ctrl+RightAlt"),
            None,
            "a modifier as the key"
        );
        assert_eq!(
            portal_trigger("Escape"),
            None,
            "Escape must reach applications"
        );
        assert_eq!(portal_trigger("F13"), None, "a bare key");
        assert_eq!(portal_trigger("Ctrl+Mouse4"), None);
        assert_eq!(portal_trigger(""), None);
    }

    #[test]
    fn the_default_config_binds_dictation_to_meta_space_and_leaves_the_rest() {
        let specs = specs(&crate::Config::defaults().unwrap().hotkey);
        let ids: Vec<&str> = specs.iter().map(|spec| spec.id.as_str()).collect();
        assert_eq!(ids, ["dictate", "cancel", "command", "paste-last", "learn"]);
        assert_eq!(specs[0].preferred.as_deref(), Some(DEFAULT_DICTATE));
        assert!(
            specs[1..].iter().all(|spec| spec.preferred.is_none()),
            "Escape and Ctrl+RightAlt make no trigger: {specs:?}"
        );
    }

    #[test]
    fn configured_chords_add_shortcuts_with_their_triggers() {
        let hotkey = config(
            "[hotkey]\ncombination = \"Ctrl+Shift+Space\"\npaste_last = \"Super+V\"\n\
             languages = { \"Shift+F9\" = \"de\" }\nstyles = { \"Super+F10\" = \"verbatim\" }",
        );
        let specs = specs(&hotkey);
        let find = |id: &str| specs.iter().find(|spec| spec.id == id).cloned();
        assert_eq!(
            find("dictate").unwrap().preferred.as_deref(),
            Some("CTRL+SHIFT+space")
        );
        assert_eq!(
            find("paste-last").unwrap().preferred.as_deref(),
            Some("LOGO+v")
        );
        let german = find("language-de").unwrap();
        assert_eq!(german.preferred.as_deref(), Some("SHIFT+F9"));
        assert_eq!(german.description, "Dictate in German");
        assert!(find("style-verbatim").is_some());
    }

    fn spec(id: &str, meaning: Meaning) -> ShortcutSpec {
        ShortcutSpec {
            id: id.into(),
            description: String::new(),
            preferred: None,
            meaning,
        }
    }

    fn ms(ms: u64) -> Duration {
        Duration::from_millis(ms)
    }

    #[test]
    fn the_clock_keeps_the_portal_gaps() {
        let now = Instant::now();
        let mut clock = PortalClock::default();
        assert_eq!(clock.at(ms(10_000), now), now);
        let press = clock.at(ms(10_050), now + ms(80));
        assert_eq!(press, now + ms(50), "the portal time, not the arrival time");
        let late = clock.at(ms(10_200), now + ms(5_000));
        assert_eq!(late, now + ms(5_000), "far behind the clock: a new mark");
        let back = clock.at(ms(1), now + ms(5_010));
        assert_eq!(back, now + ms(5_010), "a timestamp from before the mark");
        let zero = clock.at(Duration::ZERO, now + ms(5_020));
        assert_eq!(zero, now + ms(5_020), "no timestamp: the arrival time");
        let ahead = clock.at(ms(99_999), now + ms(5_030));
        assert!(ahead <= now + ms(5_030), "never in the future");
    }

    #[test]
    fn activated_and_deactivated_are_press_and_release_at_portal_time() {
        let now = Instant::now();
        let mut translator = Translator::new(&[spec("dictate", Meaning::Dictate)]);
        let press = translator
            .signal(
                Signal::Activated {
                    id: "dictate".into(),
                    stamp: ms(1_000),
                },
                now,
            )
            .unwrap();
        assert_eq!(press.event, HotkeyEvent::Pressed);
        let release = translator
            .signal(
                Signal::Deactivated {
                    id: "dictate".into(),
                    stamp: ms(1_120),
                },
                now + ms(400),
            )
            .unwrap();
        assert_eq!(release.event, HotkeyEvent::Released);
        assert_eq!(
            release.at.duration_since(press.at),
            ms(120),
            "a 120 ms tap stays a tap, though the release came late"
        );
    }

    #[test]
    fn the_gesture_detector_locks_on_a_portal_double_tap() {
        use crate::hotkey::{GestureAction, GestureDetector, HotkeyMode};
        let base = Instant::now();
        let mut translator = Translator::new(&[spec("dictate", Meaning::Dictate)]);
        let mut gesture = GestureDetector::new(HotkeyMode::Hold, ms(300));
        let mut actions = Vec::new();
        for (stamp, active, arrives) in [
            (100, true, 105),
            (180, false, 185),
            (300, true, 305),
            (380, false, 385),
            (2_000, true, 2_005),
        ] {
            let signal = if active {
                Signal::Activated {
                    id: "dictate".into(),
                    stamp: ms(stamp),
                }
            } else {
                Signal::Deactivated {
                    id: "dictate".into(),
                    stamp: ms(stamp),
                }
            };
            if let Some(stamped) = translator.signal(signal, base + ms(arrives)) {
                actions.push(gesture.feed(stamped.event, stamped.at));
            }
        }
        assert_eq!(
            actions,
            [
                Some(GestureAction::Start),
                Some(GestureAction::Cancel),
                Some(GestureAction::Start),
                None,
                Some(GestureAction::Stop),
            ],
            "a tap, a second tap that locks, and a press that stops"
        );
    }

    #[test]
    fn repeats_are_dropped_and_a_lost_release_is_restored() {
        let now = Instant::now();
        let mut translator = Translator::new(&[
            spec("dictate", Meaning::Dictate),
            spec("cancel", Meaning::Cancel),
            spec("learn", Meaning::Action(HotkeyAction::Learn)),
        ]);
        let activated = |id: &str, at: u64| Signal::Activated {
            id: id.into(),
            stamp: ms(at),
        };
        assert!(translator
            .signal(activated("dictate", 1_000), now)
            .is_some());
        assert_eq!(
            translator.signal(activated("dictate", 1_600), now + ms(600)),
            None,
            "key repeat while held"
        );
        let lost = translator
            .signal(activated("dictate", 4_000), now + ms(3_000))
            .unwrap();
        assert_eq!(
            lost.event,
            HotkeyEvent::Released,
            "a desktop that sends no release still stops the take"
        );
        assert_eq!(
            translator
                .signal(activated("cancel", 5_000), now + ms(4_000))
                .map(|s| s.event),
            Some(HotkeyEvent::Cancel)
        );
        assert_eq!(
            translator
                .signal(
                    Signal::Deactivated {
                        id: "cancel".into(),
                        stamp: ms(5_100)
                    },
                    now + ms(4_100)
                )
                .map(|s| s.event),
            None,
            "cancel has no release"
        );
        assert_eq!(
            translator
                .signal(activated("learn", 6_000), now + ms(5_000))
                .map(|s| s.event),
            Some(HotkeyEvent::Action(HotkeyAction::Learn))
        );
        assert_eq!(
            translator.signal(activated("other", 7_000), now + ms(6_000)),
            None
        );
    }

    /// A portal that plays a script of signals.
    struct ScriptedPortal {
        bound: Vec<Binding>,
        signals: std::collections::VecDeque<Signal>,
        configured: Arc<Mutex<u32>>,
        closed: Arc<Mutex<bool>>,
    }

    #[async_trait]
    impl ShortcutsPortal for ScriptedPortal {
        async fn bind(&mut self, _specs: &[ShortcutSpec]) -> Result<Vec<Binding>, PortalError> {
            Ok(self.bound.clone())
        }

        async fn next_signal(&mut self) -> Option<Signal> {
            match self.signals.pop_front() {
                Some(signal) => Some(signal),
                None => std::future::pending().await,
            }
        }

        async fn configure(&mut self) -> Result<(), PortalError> {
            *self.configured.lock() += 1;
            Ok(())
        }

        async fn close(&mut self) {
            *self.closed.lock() = true;
        }
    }

    #[test]
    fn the_portal_thread_binds_feeds_and_configures() {
        let binding = |trigger: &str| Binding {
            id: "dictate".into(),
            description: "Dictate".into(),
            trigger: trigger.into(),
        };
        let configured = Arc::new(Mutex::new(0));
        let closed = Arc::new(Mutex::new(false));
        let portal = ScriptedPortal {
            bound: vec![binding("Meta+Space")],
            signals: [
                Signal::Activated {
                    id: "dictate".into(),
                    stamp: ms(10),
                },
                Signal::Changed(vec![binding("Meta+D")]),
                Signal::Deactivated {
                    id: "dictate".into(),
                    stamp: ms(900),
                },
            ]
            .into(),
            configured: Arc::clone(&configured),
            closed: Arc::clone(&closed),
        };
        let (handle, events) = start(
            move || async move { Ok((portal, 2)) },
            vec![spec("dictate", Meaning::Dictate)],
        )
        .unwrap();
        let first = events.recv_timeout(Duration::from_secs(5)).unwrap();
        let second = events.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(
            (first.event, second.event),
            (HotkeyEvent::Pressed, HotkeyEvent::Released)
        );
        assert!(
            second.at >= first.at,
            "the signals came at once, so no time lies in the future"
        );
        assert_eq!(
            handle.bindings(),
            [binding("Meta+D")],
            "the desktop changed it"
        );
        crate::portal::block_on({
            let handle = handle.clone();
            async move { handle.configure().await }
        })
        .unwrap()
        .unwrap();
        assert_eq!(*configured.lock(), 1);
        drop(handle);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !*closed.lock() && Instant::now() < deadline {
            thread::sleep(ms(10));
        }
        assert!(*closed.lock(), "dropping the handle closes the session");
    }

    #[test]
    fn a_missing_portal_fails_the_start_and_version_one_cannot_configure() {
        let failed = start(
            || async {
                Err::<(ScriptedPortal, u32), _>(PortalError::Unavailable("GlobalShortcuts".into()))
            },
            Vec::new(),
        );
        assert!(failed.is_err());
        let portal = ScriptedPortal {
            bound: Vec::new(),
            signals: Default::default(),
            configured: Arc::default(),
            closed: Arc::default(),
        };
        let (handle, _events) = start(move || async move { Ok((portal, 1)) }, Vec::new()).unwrap();
        let outcome = crate::portal::block_on(async move { handle.configure().await }).unwrap();
        assert!(matches!(outcome, Err(PortalError::Unavailable(_))));
    }
}
