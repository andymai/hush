//! Ends a hands-free recording once the speaker stops talking.
//!
//! The input is the per-block level from [`AudioCapture`](super::AudioCapture)
//! amplitude monitoring. A block counts as voice when it is well above the
//! noise floor, which is the quiet end of the last few seconds of levels, so
//! the detector follows the room and the microphone gain without a setting.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Levels below this never count as voice, however quiet the room is.
const MIN_VOICE: f32 = 0.06;
/// Voice is at least this many times the noise floor (about 10 dB).
const VOICE_RATIO: f32 = 3.0;
/// Levels clip at 1.0, so a loud room must not push the threshold past them.
const MAX_THRESHOLD: f32 = 0.9;
/// The noise floor comes from this much recent input.
const FLOOR_WINDOW: Duration = Duration::from_secs(4);
/// The noise floor is this quantile of the recent levels.
const FLOOR_QUANTILE: f32 = 0.1;
/// A key click or a cough must not arm the detector.
const ARM_AFTER: Duration = Duration::from_millis(300);
/// One block never counts for more than this, so a stalled stream cannot arm
/// the detector in one step.
const MAX_BLOCK: Duration = Duration::from_millis(200);

pub struct SilenceDetector {
    pause: Duration,
    recent: VecDeque<(Instant, f32)>,
    previous: Option<Instant>,
    voiced: Duration,
    quiet_since: Option<Instant>,
}

impl SilenceDetector {
    pub fn new(pause: Duration) -> Self {
        Self {
            pause,
            recent: VecDeque::new(),
            previous: None,
            voiced: Duration::ZERO,
            quiet_since: None,
        }
    }

    /// Takes the level of one block. Returns true once the speaker talked and
    /// then stayed quiet for the whole pause.
    pub fn feed(&mut self, level: f32, now: Instant) -> bool {
        let block = self
            .previous
            .map(|previous| now.saturating_duration_since(previous).min(MAX_BLOCK))
            .unwrap_or(Duration::ZERO);
        self.previous = Some(now);

        self.recent.push_back((now, level));
        while self
            .recent
            .front()
            .is_some_and(|(at, _)| now.saturating_duration_since(*at) > FLOOR_WINDOW)
        {
            self.recent.pop_front();
        }

        if level >= self.threshold() {
            self.voiced += block;
            self.quiet_since = None;
        } else if self.quiet_since.is_none() {
            self.quiet_since = Some(now);
        }

        self.voiced >= ARM_AFTER
            && self
                .quiet_since
                .is_some_and(|since| now.saturating_duration_since(since) >= self.pause)
    }

    fn threshold(&self) -> f32 {
        let mut levels: Vec<f32> = self.recent.iter().map(|(_, level)| *level).collect();
        levels.sort_by(f32::total_cmp);
        let index = ((levels.len() as f32 - 1.0) * FLOOR_QUANTILE) as usize;
        let floor = levels.get(index).copied().unwrap_or(0.0);
        (floor * VOICE_RATIO).clamp(MIN_VOICE, MAX_THRESHOLD)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLOCK: Duration = Duration::from_millis(64);
    const PAUSE: Duration = Duration::from_secs(2);

    /// Feeds `level` for `length`, one block at a time, and reports whether
    /// the detector fired on any block.
    fn feed_for(
        detector: &mut SilenceDetector,
        clock: &mut Instant,
        level: f32,
        length: Duration,
    ) -> bool {
        let mut fired = false;
        let end = *clock + length;
        while *clock < end {
            *clock += BLOCK;
            fired |= detector.feed(level, *clock);
        }
        fired
    }

    /// Speech that rises and falls with each syllable, as real speech does.
    fn speak(detector: &mut SilenceDetector, clock: &mut Instant, peak: f32, length: Duration) {
        let end = *clock + length;
        let mut step = 0;
        while *clock < end {
            *clock += BLOCK;
            let level = if step % 4 == 3 { peak / 6.0 } else { peak };
            assert!(!detector.feed(level, *clock), "fired while speaking");
            step += 1;
        }
    }

    #[test]
    fn stops_after_speech_then_a_pause() {
        let mut detector = SilenceDetector::new(PAUSE);
        let mut clock = Instant::now();
        feed_for(&mut detector, &mut clock, 0.01, Duration::from_secs(1));
        speak(&mut detector, &mut clock, 0.5, Duration::from_secs(3));
        assert!(!feed_for(
            &mut detector,
            &mut clock,
            0.01,
            Duration::from_millis(1800)
        ));
        assert!(feed_for(
            &mut detector,
            &mut clock,
            0.01,
            Duration::from_millis(400)
        ));
    }

    #[test]
    fn silence_alone_never_stops() {
        let mut detector = SilenceDetector::new(PAUSE);
        let mut clock = Instant::now();
        assert!(!feed_for(
            &mut detector,
            &mut clock,
            0.01,
            Duration::from_secs(30)
        ));
    }

    #[test]
    fn a_click_does_not_arm() {
        let mut detector = SilenceDetector::new(PAUSE);
        let mut clock = Instant::now();
        feed_for(&mut detector, &mut clock, 0.01, Duration::from_secs(1));
        feed_for(&mut detector, &mut clock, 0.8, BLOCK);
        assert!(!feed_for(
            &mut detector,
            &mut clock,
            0.01,
            Duration::from_secs(10)
        ));
    }

    #[test]
    fn speech_resets_the_pause() {
        let mut detector = SilenceDetector::new(PAUSE);
        let mut clock = Instant::now();
        speak(&mut detector, &mut clock, 0.5, Duration::from_secs(2));
        feed_for(&mut detector, &mut clock, 0.01, Duration::from_millis(1500));
        speak(&mut detector, &mut clock, 0.5, Duration::from_secs(1));
        assert!(!feed_for(
            &mut detector,
            &mut clock,
            0.01,
            Duration::from_millis(1500)
        ));
        assert!(feed_for(
            &mut detector,
            &mut clock,
            0.01,
            Duration::from_secs(1)
        ));
    }

    #[test]
    fn follows_a_noisy_room() {
        let mut detector = SilenceDetector::new(PAUSE);
        let mut clock = Instant::now();
        feed_for(&mut detector, &mut clock, 0.15, Duration::from_secs(2));
        speak(&mut detector, &mut clock, 0.9, Duration::from_secs(2));
        assert!(feed_for(
            &mut detector,
            &mut clock,
            0.15,
            Duration::from_secs(3)
        ));
    }

    #[test]
    fn long_speech_never_stops() {
        let mut detector = SilenceDetector::new(PAUSE);
        let mut clock = Instant::now();
        speak(&mut detector, &mut clock, 0.4, Duration::from_secs(60));
    }

    #[test]
    fn a_quiet_microphone_still_arms() {
        let mut detector = SilenceDetector::new(PAUSE);
        let mut clock = Instant::now();
        feed_for(&mut detector, &mut clock, 0.004, Duration::from_secs(1));
        speak(&mut detector, &mut clock, 0.08, Duration::from_secs(2));
        assert!(feed_for(
            &mut detector,
            &mut clock,
            0.004,
            Duration::from_secs(3)
        ));
    }

    #[test]
    fn a_stalled_stream_counts_as_one_short_block() {
        let mut detector = SilenceDetector::new(PAUSE);
        let mut clock = Instant::now();
        feed_for(&mut detector, &mut clock, 0.01, Duration::from_secs(1));
        clock += Duration::from_secs(5);
        detector.feed(0.8, clock);
        assert!(!feed_for(
            &mut detector,
            &mut clock,
            0.01,
            Duration::from_secs(10)
        ));
    }
}
