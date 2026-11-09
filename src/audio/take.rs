//! When a take really starts.
//!
//! The input stream pauses between takes, and PipeWire then suspends the idle
//! source. The source can need seconds to wake, so a take starts at its first
//! audio frame: the start cue, the recording look, and every timer that
//! measures the take wait for that frame.

use std::time::{Duration, Instant};

/// A take that gets no audio frame in this time fails.
pub const START_TIMEOUT: Duration = Duration::from_secs(5);

/// A first frame later than this is a slow start, worth a warning in the log.
pub const SLOW_START: Duration = Duration::from_millis(200);

pub fn is_slow_start(delay: Duration) -> bool {
    delay > SLOW_START
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartEvent {
    /// The first frame arrived `delay` after the stream started. Give the
    /// start cue now.
    Live { delay: Duration },
    /// No frame arrived in [`START_TIMEOUT`]. Cancel the take.
    TimedOut,
}

/// What a stop request does to the take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopAction {
    /// The take has audio: transcribe it.
    Transcribe,
    /// The start cue never played: discard the take without a message.
    Discard,
}

/// The timing of one take, from the start of the stream to its first frame
/// and after.
#[derive(Debug, Clone, Copy)]
pub struct TakeClock {
    started: Instant,
    live_since: Option<Instant>,
}

impl TakeClock {
    /// `started` is the time the input stream started.
    pub fn new(started: Instant) -> Self {
        Self {
            started,
            live_since: None,
        }
    }

    /// The first frame arrived, so the start cue played.
    pub fn is_live(&self) -> bool {
        self.live_since.is_some()
    }

    /// The time of the first frame, after the take is live.
    pub fn live_since(&self) -> Option<Instant> {
        self.live_since
    }

    /// Takes the time of the first frame from the capture, if one arrived.
    /// Returns an event once: when the take becomes live, or when it times out.
    pub fn poll(&mut self, first_frame: Option<Instant>, now: Instant) -> Option<StartEvent> {
        if self.is_live() {
            return None;
        }
        let deadline = self.started + START_TIMEOUT;
        match first_frame {
            Some(at) if at <= deadline => {
                self.live_since = Some(at);
                Some(StartEvent::Live {
                    delay: at.saturating_duration_since(self.started),
                })
            },
            _ if now >= deadline => Some(StartEvent::TimedOut),
            _ => None,
        }
    }

    /// How long the take recorded audio. Zero until the first frame.
    pub fn elapsed(&self, now: Instant) -> Duration {
        self.live_since
            .map(|since| now.saturating_duration_since(since))
            .unwrap_or_default()
    }

    pub fn stop_action(&self) -> StopAction {
        if self.is_live() {
            StopAction::Transcribe
        } else {
            StopAction::Discard
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapCheck {
    Under,
    /// The cap is near: warn the user.
    Warn {
        remaining_secs: u64,
    },
    /// Stop the take and transcribe it.
    Reached,
}

/// The length cap of a take and the warning before it.
#[derive(Debug, Clone, Copy)]
pub struct LengthCap {
    max_secs: u64,
    warn_secs: u64,
}

impl LengthCap {
    /// `max_secs` of 0 turns the cap off.
    pub fn new(max_secs: u64) -> Self {
        let warn_secs = if max_secs > 90 {
            max_secs - 60
        } else {
            max_secs * 2 / 3
        };
        Self {
            max_secs,
            warn_secs,
        }
    }

    pub fn max_secs(&self) -> u64 {
        self.max_secs
    }

    /// `recorded` comes from [`TakeClock::elapsed`].
    pub fn check(&self, recorded: Duration) -> CapCheck {
        if self.max_secs == 0 {
            return CapCheck::Under;
        }
        let elapsed = recorded.as_secs();
        if elapsed >= self.max_secs {
            CapCheck::Reached
        } else if elapsed >= self.warn_secs {
            CapCheck::Warn {
                remaining_secs: self.max_secs - elapsed,
            }
        } else {
            CapCheck::Under
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(base: Instant, ms: u64) -> Instant {
        base + Duration::from_millis(ms)
    }

    #[test]
    fn the_cue_waits_for_the_first_frame() {
        let base = Instant::now();
        let mut clock = TakeClock::new(base);
        assert_eq!(clock.poll(None, at(base, 16)), None);
        assert_eq!(clock.poll(None, at(base, 600)), None);
        assert!(!clock.is_live());

        assert_eq!(
            clock.poll(Some(at(base, 650)), at(base, 660)),
            Some(StartEvent::Live {
                delay: Duration::from_millis(650)
            })
        );
        assert!(clock.is_live());
        assert_eq!(clock.live_since(), Some(at(base, 650)));
        assert_eq!(
            clock.poll(Some(at(base, 650)), at(base, 700)),
            None,
            "the cue plays once"
        );
    }

    #[test]
    fn a_warm_source_goes_live_at_once() {
        let base = Instant::now();
        let mut clock = TakeClock::new(base);
        assert_eq!(
            clock.poll(Some(at(base, 5)), at(base, 16)),
            Some(StartEvent::Live {
                delay: Duration::from_millis(5)
            })
        );
    }

    #[test]
    fn no_frame_in_five_seconds_cancels_the_take() {
        let base = Instant::now();
        let mut clock = TakeClock::new(base);
        assert_eq!(clock.poll(None, at(base, 4999)), None);
        assert_eq!(clock.poll(None, at(base, 5000)), Some(StartEvent::TimedOut));
        assert!(!clock.is_live());
        assert_eq!(clock.stop_action(), StopAction::Discard);
    }

    #[test]
    fn a_frame_after_the_timeout_does_not_start_the_take() {
        let base = Instant::now();
        let mut clock = TakeClock::new(base);
        assert_eq!(
            clock.poll(Some(at(base, 5010)), at(base, 5016)),
            Some(StartEvent::TimedOut)
        );
        assert!(!clock.is_live());
    }

    #[test]
    fn a_frame_just_inside_the_timeout_still_counts() {
        let base = Instant::now();
        let mut clock = TakeClock::new(base);
        assert_eq!(
            clock.poll(Some(at(base, 4995)), at(base, 5011)),
            Some(StartEvent::Live {
                delay: Duration::from_millis(4995)
            })
        );
    }

    #[test]
    fn a_four_second_wake_still_starts_the_take() {
        let base = Instant::now();
        let mut clock = TakeClock::new(base);
        assert_eq!(clock.poll(None, at(base, 3990)), None);
        assert_eq!(
            clock.poll(Some(at(base, 4000)), at(base, 4006)),
            Some(StartEvent::Live {
                delay: Duration::from_secs(4)
            })
        );
    }

    #[test]
    fn a_release_before_the_first_frame_discards_the_take() {
        let base = Instant::now();
        let mut clock = TakeClock::new(base);
        clock.poll(None, at(base, 400));
        assert_eq!(clock.stop_action(), StopAction::Discard);

        clock.poll(Some(at(base, 700)), at(base, 710));
        assert_eq!(clock.stop_action(), StopAction::Transcribe);
    }

    #[test]
    fn a_slow_start_is_over_200_ms() {
        assert!(!is_slow_start(Duration::ZERO));
        assert!(!is_slow_start(Duration::from_millis(200)));
        assert!(is_slow_start(Duration::from_millis(201)));
        assert!(is_slow_start(Duration::from_secs(4)));
    }

    #[test]
    fn take_time_counts_from_the_first_frame() {
        let base = Instant::now();
        let mut clock = TakeClock::new(base);
        assert_eq!(clock.elapsed(at(base, 1500)), Duration::ZERO);
        clock.poll(Some(at(base, 1500)), at(base, 1510));
        assert_eq!(clock.elapsed(at(base, 1500)), Duration::ZERO);
        assert_eq!(clock.elapsed(at(base, 2500)), Duration::from_secs(1));
    }

    #[test]
    fn the_cap_and_its_warning_count_from_the_first_frame() {
        let base = Instant::now();
        let cap = LengthCap::new(600);
        let mut clock = TakeClock::new(base);
        clock.poll(Some(at(base, 1500)), at(base, 1510));

        assert_eq!(
            cap.check(clock.elapsed(at(base, 541_000))),
            CapCheck::Under,
            "the warning waits for 540 s of audio"
        );
        assert_eq!(
            cap.check(clock.elapsed(at(base, 541_500))),
            CapCheck::Warn { remaining_secs: 60 }
        );
        assert_eq!(
            cap.check(clock.elapsed(at(base, 600_000))),
            CapCheck::Warn { remaining_secs: 2 },
            "the stream start does not count"
        );
        assert_eq!(
            cap.check(clock.elapsed(at(base, 601_500))),
            CapCheck::Reached
        );
    }

    #[test]
    fn a_short_cap_warns_at_two_thirds_and_zero_turns_it_off() {
        let cap = LengthCap::new(60);
        assert_eq!(cap.check(Duration::from_secs(39)), CapCheck::Under);
        assert_eq!(
            cap.check(Duration::from_secs(40)),
            CapCheck::Warn { remaining_secs: 20 }
        );
        assert_eq!(cap.check(Duration::from_secs(60)), CapCheck::Reached);

        let off = LengthCap::new(0);
        assert_eq!(off.check(Duration::from_secs(100_000)), CapCheck::Under);
    }
}
