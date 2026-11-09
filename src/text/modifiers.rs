//! Wait for the user's hands to leave the modifier keys before Hush types or
//! pastes. A Ctrl or Alt that is still down turns typed letters into
//! shortcuts, and turns Ctrl+V into Ctrl+Alt+V.

use super::clock::Clock;
use std::time::Duration;

/// After the keys are up, the compositor still needs a moment to process
/// the release, and a new selection needs a moment to reach the target.
pub const SETTLE: Duration = Duration::from_millis(75);
/// A key that stays down longer than this is a stuck key or a deliberate
/// hold; insertion goes on.
pub const CAP: Duration = Duration::from_secs(2);
const POLL: Duration = Duration::from_millis(10);

pub trait ModifierSource {
    /// Whether Ctrl, Alt, Shift, or Super is down, or `None` when the state
    /// is unknown.
    fn held(&self) -> Option<bool>;
}

/// The keyboards the evdev hotkey listener reads.
pub struct PhysicalModifiers;

impl ModifierSource for PhysicalModifiers {
    fn held(&self) -> Option<bool> {
        crate::hotkey::modifiers::held()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifierWait {
    /// The keys were up, or came up within the cap.
    Released { waited: Duration },
    /// A key was still down at the cap.
    GaveUp,
    /// No keyboard state is available, for example with the X11 hotkey
    /// backend.
    Unknown,
}

/// Wait until no modifier is down, at most [`CAP`], and then [`SETTLE`].
pub fn wait_for_release(source: &dyn ModifierSource, clock: &dyn Clock) -> ModifierWait {
    let start = clock.now();
    let outcome = loop {
        match source.held() {
            None => break ModifierWait::Unknown,
            Some(false) => {
                break ModifierWait::Released {
                    waited: clock.now().saturating_duration_since(start),
                }
            },
            Some(true) if clock.now().saturating_duration_since(start) >= CAP => {
                break ModifierWait::GaveUp
            },
            Some(true) => clock.sleep(POLL),
        }
    };
    clock.sleep(SETTLE);
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::clock::fake::FakeClock;
    use std::cell::RefCell;

    /// Reports the scripted states in order, then repeats the last one.
    struct Script(RefCell<Vec<Option<bool>>>);

    impl Script {
        fn new(states: &[Option<bool>]) -> Self {
            let mut states = states.to_vec();
            states.reverse();
            Self(RefCell::new(states))
        }
    }

    impl ModifierSource for Script {
        fn held(&self) -> Option<bool> {
            let mut states = self.0.borrow_mut();
            if states.len() > 1 {
                states.pop().flatten()
            } else {
                states.last().copied().flatten()
            }
        }
    }

    #[test]
    fn released_keys_wait_only_the_settle_time() {
        let clock = FakeClock::new();
        let outcome = wait_for_release(&Script::new(&[Some(false)]), &clock);
        assert_eq!(
            outcome,
            ModifierWait::Released {
                waited: Duration::ZERO
            }
        );
        assert_eq!(clock.elapsed(), SETTLE);
    }

    #[test]
    fn a_held_key_is_waited_for() {
        let clock = FakeClock::new();
        let script = Script::new(&[Some(true), Some(true), Some(true), Some(false)]);
        let outcome = wait_for_release(&script, &clock);
        assert_eq!(outcome, ModifierWait::Released { waited: POLL * 3 });
        assert_eq!(clock.elapsed(), POLL * 3 + SETTLE);
    }

    #[test]
    fn a_stuck_key_gives_up_at_the_cap() {
        let clock = FakeClock::new();
        let outcome = wait_for_release(&Script::new(&[Some(true)]), &clock);
        assert_eq!(outcome, ModifierWait::GaveUp);
        assert!(clock.elapsed() >= CAP + SETTLE);
        assert!(clock.elapsed() < CAP + POLL + SETTLE);
    }

    #[test]
    fn unknown_state_does_not_block() {
        let clock = FakeClock::new();
        let outcome = wait_for_release(&Script::new(&[None]), &clock);
        assert_eq!(outcome, ModifierWait::Unknown);
        assert_eq!(clock.elapsed(), SETTLE);
    }
}
