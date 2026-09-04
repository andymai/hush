//! Turns raw hotkey presses into recording actions.
//!
//! Hold mode records while the key is held. A press shorter than the tap
//! window is a tap: it discards the recording, and a second tap inside the
//! window locks hands-free recording until the next press. Toggle mode locks
//! on a single tap and still records while a longer press is held. The cancel
//! key discards whatever is in progress.

use super::{HotkeyEvent, HotkeyMode};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureAction {
    Start,
    Stop,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Holding { since: Instant },
    TapPending { until: Instant },
    Locked,
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
        matches!(self.state, State::Locked)
    }

    /// When the tap window closes; the caller wakes up then and calls
    /// [`expire`](Self::expire).
    pub fn deadline(&self) -> Option<Instant> {
        match self.state {
            State::TapPending { until } => Some(until),
            _ => None,
        }
    }

    pub fn expire(&mut self, now: Instant) {
        if let State::TapPending { until } = self.state {
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
        match (event, self.state) {
            (HotkeyEvent::Action(_), _) | (HotkeyEvent::Command(_), _) => None,
            (HotkeyEvent::Cancel, State::Idle) => None,
            (HotkeyEvent::Cancel, _) => {
                self.state = State::Idle;
                Some(GestureAction::Cancel)
            },
            (HotkeyEvent::Pressed, State::Idle) => {
                self.state = State::Holding { since: now };
                Some(GestureAction::Start)
            },
            (HotkeyEvent::Pressed, State::TapPending { until }) => {
                self.state = if now <= until {
                    State::Locked
                } else {
                    State::Holding { since: now }
                };
                Some(GestureAction::Start)
            },
            (HotkeyEvent::Pressed, State::Locked) => {
                self.state = State::Idle;
                Some(GestureAction::Stop)
            },
            (HotkeyEvent::Pressed, State::Holding { .. }) => None,
            (HotkeyEvent::Released, State::Holding { since }) => {
                let tap = now.duration_since(since) < self.tap_window;
                match (self.mode, tap) {
                    (HotkeyMode::Hold, true) => {
                        self.state = State::TapPending {
                            until: now + self.tap_window,
                        };
                        Some(GestureAction::Cancel)
                    },
                    (HotkeyMode::Toggle, true) => {
                        self.state = State::Locked;
                        None
                    },
                    (_, false) => {
                        self.state = State::Idle;
                        Some(GestureAction::Stop)
                    },
                }
            },
            (HotkeyEvent::Released, _) => None,
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
