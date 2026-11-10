//! Cuts a hands-free take into parts while it records, so that each part
//! can be transcribed before the take stops.
//!
//! A part ends in the middle of a pause of [`PAUSE`] after speech, once it
//! is [`MIN_PART`] long. A part that reaches [`HARD_CUT`] without such a
//! pause ends at the quietest block of its last seconds, so that a part
//! never outgrows the 30 seconds that Whisper hears at once.

use crate::audio::capture::calculate_rms_amplitude;
use crate::audio::SilenceDetector;
use crate::transcription::WHISPER_SAMPLE_RATE;
use std::ops::Range;
use std::time::{Duration, Instant};

const RATE: usize = WHISPER_SAMPLE_RATE as usize;
/// One level per block, the size of a capture buffer (64 ms).
const BLOCK: usize = 1024;
/// The silence after speech that ends a part.
pub const PAUSE: Duration = Duration::from_millis(800);
/// A pause cuts only a part at least this long.
pub const MIN_PART: Duration = Duration::from_secs(3);
/// A part without a pause ends at this length. The silence that pads each
/// part keeps it under 30 seconds.
pub const HARD_CUT: Duration = Duration::from_secs(29);
/// A hard cut looks for the quietest block in this last stretch of the part.
pub const HARD_CUT_SEARCH: Duration = Duration::from_secs(5);
/// Silence before and after each part, so Whisper hears its first and last
/// sounds in full.
pub const PAD: Duration = Duration::from_millis(200);

fn samples(duration: Duration) -> usize {
    (duration.as_secs_f64() * RATE as f64) as usize
}

/// Takes the samples of a take as they arrive, at 16 kHz, and returns the
/// parts that end.
pub struct PartCutter {
    detector: SilenceDetector,
    clock: Instant,
    /// The samples after the last cut
    pending: Vec<f32>,
    /// How much of `pending` the detector heard, in whole blocks
    scanned: usize,
    /// The start in `pending` and the level of each block it heard
    levels: Vec<(usize, f32)>,
    /// The samples in the parts so far
    emitted: usize,
}

impl Default for PartCutter {
    fn default() -> Self {
        Self::new()
    }
}

impl PartCutter {
    pub fn new() -> Self {
        Self {
            detector: SilenceDetector::new(PAUSE),
            clock: Instant::now(),
            pending: Vec::new(),
            scanned: 0,
            levels: Vec::new(),
            emitted: 0,
        }
    }

    /// The samples of the take in the parts that ended. The rest of the take
    /// starts there.
    pub fn emitted(&self) -> usize {
        self.emitted
    }

    /// Add the next samples of the take. Returns each part that ended.
    pub fn feed(&mut self, samples_in: &[f32]) -> Vec<Vec<f32>> {
        self.pending.extend_from_slice(samples_in);
        let mut parts = Vec::new();
        while self.pending.len() - self.scanned >= BLOCK {
            let start = self.scanned;
            let level = calculate_rms_amplitude(&self.pending[start..start + BLOCK]);
            self.scanned += BLOCK;
            self.levels.push((start, level));
            let heard = self.emitted + self.scanned;
            let at = self.clock + Duration::from_secs_f64(heard as f64 / RATE as f64);
            let paused = self.detector.feed(level, at);
            let cut = if paused && self.scanned >= samples(MIN_PART) {
                Some(self.scanned - samples(PAUSE) / 2)
            } else if self.scanned >= samples(HARD_CUT) {
                Some(self.quietest())
            } else {
                None
            };
            if let Some(cut) = cut {
                parts.push(self.cut(cut));
            }
        }
        parts
    }

    /// The middle of the quietest block in the last stretch of the part.
    fn quietest(&self) -> usize {
        let from = samples(HARD_CUT - HARD_CUT_SEARCH);
        quietest_of(
            self.levels
                .iter()
                .copied()
                .filter(|(start, _)| *start >= from),
        )
        .unwrap_or(self.scanned)
    }

    fn cut(&mut self, at: usize) -> Vec<f32> {
        let rest = self.pending.split_off(at);
        let part = std::mem::replace(&mut self.pending, rest);
        self.scanned -= at;
        self.emitted += at;
        self.levels = self
            .levels
            .iter()
            .filter(|(start, _)| *start >= at)
            .map(|(start, level)| (start - at, *level))
            .collect();
        self.detector.rearm();
        part
    }
}

/// The middle of the quietest of `blocks`, each a start and a level. Of
/// blocks equally quiet, the first wins.
fn quietest_of(blocks: impl IntoIterator<Item = (usize, f32)>) -> Option<usize> {
    blocks
        .into_iter()
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(start, _)| start + BLOCK / 2)
}

/// Where a hard cut falls in `take[range]`: the middle of its quietest block,
/// as an index into `take`. `None` when the range holds no whole block.
pub fn quietest_in(take: &[f32], range: Range<usize>) -> Option<usize> {
    let end = range.end.min(take.len());
    quietest_of(
        (range.start..end.saturating_sub(BLOCK - 1))
            .step_by(BLOCK)
            .map(|start| (start, calculate_rms_amplitude(&take[start..start + BLOCK]))),
    )
}

/// `part` with [`PAD`] of silence before and after it.
pub fn pad(part: &[f32]) -> Vec<f32> {
    let pad = samples(PAD);
    let mut padded = Vec::with_capacity(part.len() + 2 * pad);
    padded.resize(pad, 0.0);
    padded.extend_from_slice(part);
    padded.resize(padded.len() + pad, 0.0);
    padded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn speech(length: Duration) -> Vec<f32> {
        (0..samples(length))
            .map(|i| (i as f32 * 0.07).sin() * 0.3 * if (i / 4000) % 4 == 3 { 0.2 } else { 1.0 })
            .collect()
    }

    fn quiet(length: Duration) -> Vec<f32> {
        (0..samples(length))
            .map(|i| if i % 2 == 0 { 0.001 } else { -0.001 })
            .collect()
    }

    fn secs(n: f64) -> Duration {
        Duration::from_secs_f64(n)
    }

    /// Feed `audio` in capture-sized pieces, as the session does.
    fn feed_all(cutter: &mut PartCutter, audio: &[f32]) -> Vec<Vec<f32>> {
        audio
            .chunks(341)
            .flat_map(|piece| cutter.feed(piece))
            .collect()
    }

    #[test]
    fn a_pause_after_speech_ends_a_part_in_its_middle() {
        let mut take = quiet(secs(0.5));
        take.extend(speech(secs(4.0)));
        take.extend(quiet(secs(1.2)));
        take.extend(speech(secs(2.0)));
        let mut cutter = PartCutter::new();
        let parts = feed_all(&mut cutter, &take);
        assert_eq!(parts.len(), 1);
        let end = parts[0].len() as f64 / RATE as f64;
        assert!(
            (4.6..5.2).contains(&end),
            "the cut lies in the pause: {end}"
        );
        assert_eq!(cutter.emitted(), parts[0].len());
        assert_eq!(
            &take[..parts[0].len()],
            &parts[0][..],
            "a part is the take as it is"
        );
    }

    #[test]
    fn a_short_pause_or_a_short_part_does_not_cut() {
        let mut take = speech(secs(4.0));
        take.extend(quiet(secs(0.5)));
        take.extend(speech(secs(4.0)));
        assert!(feed_all(&mut PartCutter::new(), &take).is_empty());

        let mut take = speech(secs(1.0));
        take.extend(quiet(secs(1.0)));
        take.extend(speech(secs(1.0)));
        assert!(
            feed_all(&mut PartCutter::new(), &take).is_empty(),
            "a part shorter than {:?} waits for the next pause",
            MIN_PART
        );
    }

    #[test]
    fn speech_without_a_pause_is_cut_before_30_seconds() {
        let take = speech(secs(70.0));
        let mut cutter = PartCutter::new();
        let parts = feed_all(&mut cutter, &take);
        assert_eq!(parts.len(), 2);
        for part in &parts {
            let length = Duration::from_secs_f64(part.len() as f64 / RATE as f64);
            assert!(length <= HARD_CUT, "{length:?}");
            assert!(length >= HARD_CUT - HARD_CUT_SEARCH, "{length:?}");
            assert!(pad(part).len() < 30 * RATE);
        }
        let joined: Vec<f32> = parts.concat();
        assert_eq!(
            &take[..joined.len()],
            &joined[..],
            "no sample is lost or repeated"
        );
    }

    #[test]
    fn a_hard_cut_takes_the_quietest_block() {
        let mut take = speech(secs(26.0));
        take.extend(vec![0.0; samples(secs(0.2))]);
        take.extend(speech(secs(10.0)));
        let parts = feed_all(&mut PartCutter::new(), &take);
        assert_eq!(parts.len(), 1);
        let end = parts[0].len() as f64 / RATE as f64;
        assert!((26.0..26.2).contains(&end), "{end}");
    }

    #[test]
    fn each_part_gets_its_own_pause() {
        let mut take = Vec::new();
        for _ in 0..3 {
            take.extend(speech(secs(3.5)));
            take.extend(quiet(secs(1.0)));
        }
        let parts = feed_all(&mut PartCutter::new(), &take);
        assert_eq!(parts.len(), 3);
    }

    #[test]
    fn the_quietest_whole_block_in_the_range_wins() {
        let mut take = vec![0.3; samples(secs(3.0))];
        take[20 * BLOCK..21 * BLOCK].fill(0.0);
        take[30 * BLOCK..31 * BLOCK].fill(0.0);
        assert_eq!(
            quietest_in(&take, 0..40_000),
            Some(20 * BLOCK + BLOCK / 2),
            "the first of two silent blocks"
        );
        let start = 25_000;
        take[start + 5 * BLOCK..start + 6 * BLOCK].fill(0.01);
        assert_eq!(
            quietest_in(&take, start..40_000),
            Some(start + 5 * BLOCK + BLOCK / 2),
            "the blocks start at the range"
        );
        assert_eq!(quietest_in(&take, 0..BLOCK - 1), None, "no whole block");
        assert_eq!(quietest_in(&take, 47_500..60_000), None, "past the take");
    }

    #[test]
    fn padding_adds_silence_on_both_sides() {
        let padded = pad(&[0.5; 10]);
        let pad = samples(PAD);
        assert_eq!(padded.len(), 10 + 2 * pad);
        assert!(padded[..pad].iter().all(|s| *s == 0.0));
        assert!(padded[pad..pad + 10].iter().all(|s| *s == 0.5));
        assert!(padded[pad + 10..].iter().all(|s| *s == 0.0));
    }
}
