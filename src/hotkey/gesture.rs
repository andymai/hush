//! Turns raw hotkey presses into recording actions.
//!
//! Hold mode records while the key is held. A press shorter than the tap
//! window is a tap: it discards the recording, and a second tap inside the
//! window locks hands-free recording until the next press. Toggle mode locks
//! on a single tap and still records while a longer press is held. The cancel
//! key discards whatever is in progress.
//!
//! A language chord or a style chord is a dictation key of its own. Only a
//! second tap on the same key locks, only the held key's release ends a
//! hold, and any dictation key stops a locked take.

use super::{HotkeyEvent, HotkeyMode};
use crate::text_processing::StyleName;
use crate::transcription::Language;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureAction {
    Start,
    Stop,
    Cancel,
}

/// The dictation key: the hotkey, a language chord, or a style chord.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Key {
    Hotkey,
    Language(Language),
    Style(StyleName),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Holding { since: Instant, key: Key },
    TapPending { until: Instant, key: Key },
    Locked { key: Key },
}

pub struct GestureDetector {
    mode: HotkeyMode,
    tap_window: Duration,
    state: State,
}

impl GestureDetector {
    pub fn new(mode: HotkeyMode, tap_window: Duration) -> Self {
        Self {
            mode,
            tap_window,
            state: State::Idle,
        }
    }

    /// Recording continues without the key held.
    pub fn is_locked(&self) -> bool {
        matches!(self.state, State::Locked { .. })
    }

    fn key(&self) -> Option<Key> {
        match self.state {
            State::Idle => None,
            State::Holding { key, .. } | State::TapPending { key, .. } | State::Locked { key } => {
                Some(key)
            },
        }
    }

    /// The language of the chord that started the gesture, or `None` for
    /// another key and when no gesture is in progress.
    pub fn language(&self) -> Option<Language> {
        match self.key() {
            Some(Key::Language(language)) => Some(language),
            _ => None,
        }
    }

    /// The style of the chord that started the gesture, or `None` for
    /// another key and when no gesture is in progress.
    pub fn style(&self) -> Option<StyleName> {
        match self.key() {
            Some(Key::Style(style)) => Some(style),
            _ => None,
        }
    }

    /// The key is down and the recording ends when it comes up.
    pub fn is_holding(&self) -> bool {
        matches!(self.state, State::Holding { .. })
    }

    /// When the tap window closes; the caller wakes up then and calls
    /// [`expire`](Self::expire).
    pub fn deadline(&self) -> Option<Instant> {
        match self.state {
            State::TapPending { until, .. } => Some(until),
            _ => None,
        }
    }

    pub fn expire(&mut self, now: Instant) {
        if let State::TapPending { until, .. } = self.state {
            if now >= until {
                self.state = State::Idle;
            }
        }
    }

    /// The session ended without the hotkey (an IPC stop, the recording cap,
    /// an error), so a lock no longer means anything.
    pub fn session_ended(&mut self) {
        if self.is_locked() {
            self.state = State::Idle;
        }
    }

    pub fn feed(&mut self, event: HotkeyEvent, now: Instant) -> Option<GestureAction> {
        let (key, pressed) = match event {
            HotkeyEvent::Action(_) | HotkeyEvent::Command(_) => return None,
            HotkeyEvent::Cancel if self.state == State::Idle => return None,
            HotkeyEvent::Cancel => {
                self.state = State::Idle;
                return Some(GestureAction::Cancel);
            },
            HotkeyEvent::Pressed => (Key::Hotkey, true),
            HotkeyEvent::Released => (Key::Hotkey, false),
            HotkeyEvent::Language(language, pressed) => (Key::Language(language), pressed),
            HotkeyEvent::Style(style, pressed) => (Key::Style(style), pressed),
        };
        if pressed {
            self.press(key, now)
        } else {
            self.release(key, now)
        }
    }

    fn press(&mut self, key: Key, now: Instant) -> Option<GestureAction> {
        match self.state {
            State::Idle => {
                self.state = State::Holding { since: now, key };
                Some(GestureAction::Start)
            },
            State::TapPending { until, key: tapped } => {
                self.state = if now <= until && key == tapped {
                    State::Locked { key }
                } else {
                    State::Holding { since: now, key }
                };
                Some(GestureAction::Start)
            },
            State::Locked { .. } => {
                self.state = State::Idle;
                Some(GestureAction::Stop)
            },
            State::Holding { .. } => None,
        }
    }

    fn release(&mut self, key: Key, now: Instant) -> Option<GestureAction> {
        let State::Holding { since, key: held } = self.state else {
            return None;
        };
        if key != held {
            return None;
        }
        let tap = now.duration_since(since) < self.tap_window;
        match (self.mode, tap) {
            (HotkeyMode::Hold, true) => {
                self.state = State::TapPending {
                    until: now + self.tap_window,
                    key,
                };
                Some(GestureAction::Cancel)
            },
            (HotkeyMode::Toggle, true) => {
                self.state = State::Locked { key };
                None
            },
            (_, false) => {
                self.state = State::Idle;
                Some(GestureAction::Stop)
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAP: Duration = Duration::from_millis(300);

    fn at(base: Instant, ms: u64) -> Instant {
        base + Duration::from_millis(ms)
    }

    #[test]
    fn holding_is_only_a_key_down_session() {
        let base = Instant::now();
        let mut g = GestureDetector::new(HotkeyMode::Toggle, TAP);
        g.feed(HotkeyEvent::Pressed, at(base, 0));
        assert!(g.is_holding());
        g.feed(HotkeyEvent::Released, at(base, 100));
        assert!(g.is_locked());
        assert!(!g.is_holding());

        let mut g = GestureDetector::new(HotkeyMode::Hold, TAP);
        g.feed(HotkeyEvent::Pressed, at(base, 0));
        g.feed(HotkeyEvent::Released, at(base, 100));
        g.feed(HotkeyEvent::Pressed, at(base, 200));
        assert!(g.is_locked());
        assert!(!g.is_holding());
    }

    #[test]
    fn hold_records_while_held() {
        let base = Instant::now();
        let mut g = GestureDetector::new(HotkeyMode::Hold, TAP);
        assert_eq!(
            g.feed(HotkeyEvent::Pressed, at(base, 0)),
            Some(GestureAction::Start)
        );
        assert_eq!(
            g.feed(HotkeyEvent::Released, at(base, 800)),
            Some(GestureAction::Stop)
        );
        assert!(!g.is_locked());
    }

    #[test]
    fn a_short_press_is_discarded_and_a_double_tap_locks() {
        let base = Instant::now();
        let mut g = GestureDetector::new(HotkeyMode::Hold, TAP);
        assert_eq!(
            g.feed(HotkeyEvent::Pressed, at(base, 0)),
            Some(GestureAction::Start)
        );
        assert_eq!(
            g.feed(HotkeyEvent::Released, at(base, 100)),
            Some(GestureAction::Cancel)
        );
        assert_eq!(g.deadline(), Some(at(base, 400)));
        assert_eq!(
            g.feed(HotkeyEvent::Pressed, at(base, 250)),
            Some(GestureAction::Start)
        );
        assert!(g.is_locked());
        assert_eq!(g.feed(HotkeyEvent::Released, at(base, 300)), None);
        assert_eq!(
            g.feed(HotkeyEvent::Pressed, at(base, 5000)),
            Some(GestureAction::Stop)
        );
        assert!(!g.is_locked());
        assert_eq!(g.feed(HotkeyEvent::Released, at(base, 5100)), None);
    }

    #[test]
    fn a_late_second_press_is_a_new_hold() {
        let base = Instant::now();
        let mut g = GestureDetector::new(HotkeyMode::Hold, TAP);
        g.feed(HotkeyEvent::Pressed, at(base, 0));
        g.feed(HotkeyEvent::Released, at(base, 100));
        g.expire(at(base, 500));
        assert_eq!(g.deadline(), None);
        assert_eq!(
            g.feed(HotkeyEvent::Pressed, at(base, 600)),
            Some(GestureAction::Start)
        );
        assert!(!g.is_locked());
        assert_eq!(
            g.feed(HotkeyEvent::Released, at(base, 1500)),
            Some(GestureAction::Stop)
        );
    }

    #[test]
    fn toggle_locks_on_a_tap_and_still_holds() {
        let base = Instant::now();
        let mut g = GestureDetector::new(HotkeyMode::Toggle, TAP);
        assert_eq!(
            g.feed(HotkeyEvent::Pressed, at(base, 0)),
            Some(GestureAction::Start)
        );
        assert_eq!(g.feed(HotkeyEvent::Released, at(base, 100)), None);
        assert!(g.is_locked());
        assert_eq!(
            g.feed(HotkeyEvent::Pressed, at(base, 3000)),
            Some(GestureAction::Stop)
        );
        g.feed(HotkeyEvent::Released, at(base, 3100));

        assert_eq!(
            g.feed(HotkeyEvent::Pressed, at(base, 4000)),
            Some(GestureAction::Start)
        );
        assert_eq!(
            g.feed(HotkeyEvent::Released, at(base, 6000)),
            Some(GestureAction::Stop)
        );
    }

    fn german() -> Language {
        Language::parse("de").unwrap()
    }

    #[test]
    fn a_language_chord_holds_locks_and_cancels_like_the_hotkey() {
        let base = Instant::now();
        let de = german();
        let mut g = GestureDetector::new(HotkeyMode::Hold, TAP);
        assert_eq!(
            g.feed(HotkeyEvent::Language(de, true), at(base, 0)),
            Some(GestureAction::Start)
        );
        assert_eq!(g.language(), Some(de));
        assert!(g.is_holding());
        assert_eq!(
            g.feed(HotkeyEvent::Language(de, false), at(base, 900)),
            Some(GestureAction::Stop)
        );
        assert_eq!(g.language(), None);

        g.feed(HotkeyEvent::Language(de, true), at(base, 1000));
        assert_eq!(
            g.feed(HotkeyEvent::Language(de, false), at(base, 1100)),
            Some(GestureAction::Cancel),
            "a lone tap records nothing"
        );
        assert_eq!(
            g.feed(HotkeyEvent::Language(de, true), at(base, 1200)),
            Some(GestureAction::Start)
        );
        assert!(g.is_locked(), "a double-tap locks");
        assert_eq!(g.language(), Some(de));
        assert_eq!(
            g.feed(HotkeyEvent::Cancel, at(base, 3000)),
            Some(GestureAction::Cancel)
        );

        let mut g = GestureDetector::new(HotkeyMode::Toggle, TAP);
        g.feed(HotkeyEvent::Language(de, true), at(base, 0));
        assert_eq!(
            g.feed(HotkeyEvent::Language(de, false), at(base, 100)),
            None
        );
        assert!(g.is_locked(), "a tap locks in toggle mode");
        assert_eq!(g.language(), Some(de));
    }

    #[test]
    fn keys_do_not_mix_inside_one_gesture() {
        let base = Instant::now();
        let de = german();
        let mut g = GestureDetector::new(HotkeyMode::Hold, TAP);
        g.feed(HotkeyEvent::Language(de, true), at(base, 0));
        assert_eq!(g.feed(HotkeyEvent::Pressed, at(base, 100)), None);
        assert_eq!(
            g.feed(HotkeyEvent::Released, at(base, 900)),
            None,
            "only the held key ends the hold"
        );
        assert!(g.is_holding());
        assert_eq!(
            g.feed(HotkeyEvent::Language(de, false), at(base, 1000)),
            Some(GestureAction::Stop)
        );

        g.feed(HotkeyEvent::Pressed, at(base, 2000));
        g.feed(HotkeyEvent::Released, at(base, 2100));
        assert_eq!(
            g.feed(HotkeyEvent::Language(de, true), at(base, 2200)),
            Some(GestureAction::Start)
        );
        assert!(
            !g.is_locked(),
            "a tap on one key and a press on another is a new hold"
        );
        assert_eq!(g.language(), Some(de));
        g.feed(HotkeyEvent::Language(de, false), at(base, 3000));

        g.feed(HotkeyEvent::Language(de, true), at(base, 4000));
        g.feed(HotkeyEvent::Language(de, false), at(base, 4100));
        g.feed(HotkeyEvent::Language(de, true), at(base, 4200));
        assert!(g.is_locked());
        assert_eq!(
            g.feed(HotkeyEvent::Pressed, at(base, 6000)),
            Some(GestureAction::Stop),
            "any dictation key stops a locked take"
        );
    }

    #[test]
    fn a_style_chord_is_a_dictation_key_of_its_own() {
        let base = Instant::now();
        let verbatim = crate::text_processing::styles::VERBATIM;
        let de = german();
        let mut g = GestureDetector::new(HotkeyMode::Hold, TAP);
        assert_eq!(
            g.feed(HotkeyEvent::Style(verbatim, true), at(base, 0)),
            Some(GestureAction::Start)
        );
        assert_eq!(g.style(), Some(verbatim));
        assert_eq!(g.language(), None);
        assert_eq!(
            g.feed(HotkeyEvent::Language(de, false), at(base, 800)),
            None,
            "only the held key ends the hold"
        );
        assert_eq!(
            g.feed(HotkeyEvent::Style(verbatim, false), at(base, 900)),
            Some(GestureAction::Stop)
        );
        assert_eq!(g.style(), None);

        g.feed(HotkeyEvent::Style(verbatim, true), at(base, 1000));
        g.feed(HotkeyEvent::Style(verbatim, false), at(base, 1100));
        g.feed(HotkeyEvent::Style(verbatim, true), at(base, 1200));
        assert!(g.is_locked(), "a double-tap locks");
        assert_eq!(g.style(), Some(verbatim));
        assert_eq!(
            g.feed(HotkeyEvent::Pressed, at(base, 3000)),
            Some(GestureAction::Stop),
            "any dictation key stops a locked take"
        );

        g.feed(HotkeyEvent::Language(de, true), at(base, 4000));
        g.feed(HotkeyEvent::Language(de, false), at(base, 4100));
        g.feed(HotkeyEvent::Style(verbatim, true), at(base, 4200));
        assert!(!g.is_locked(), "a tap on another key is a new hold");
        assert_eq!(g.style(), Some(verbatim));
        assert_eq!(g.language(), None);
    }

    #[test]
    fn cancel_discards_and_a_session_end_clears_a_lock() {
        let base = Instant::now();
        let mut g = GestureDetector::new(HotkeyMode::Hold, TAP);
        assert_eq!(g.feed(HotkeyEvent::Cancel, at(base, 0)), None);
        g.feed(HotkeyEvent::Pressed, at(base, 0));
        assert_eq!(
            g.feed(HotkeyEvent::Cancel, at(base, 1000)),
            Some(GestureAction::Cancel)
        );
        assert_eq!(g.feed(HotkeyEvent::Released, at(base, 1100)), None);

        let mut g = GestureDetector::new(HotkeyMode::Toggle, TAP);
        g.feed(HotkeyEvent::Pressed, at(base, 0));
        g.feed(HotkeyEvent::Released, at(base, 50));
        assert!(g.is_locked());
        g.session_ended();
        assert!(!g.is_locked());
        assert_eq!(
            g.feed(HotkeyEvent::Pressed, at(base, 2000)),
            Some(GestureAction::Start)
        );
    }
}
