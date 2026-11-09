//! How loud a take is, measured before it goes to transcription.

/// A peak under this, -60 dBFS, is digital silence or a muted microphone.
pub const NO_SOUND_PEAK: f32 = 0.001;

/// A take with a root mean square under this, about -46 dBFS, holds no
/// speech worth transcribing.
pub const QUIET_RMS: f32 = 0.005;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TakeLevel {
    /// The microphone sent no sound: it may be muted.
    NoSound,
    /// Sound too faint to hold speech.
    Quiet,
    /// Loud enough to transcribe.
    Sound,
}

impl TakeLevel {
    pub fn of(samples: &[f32]) -> Self {
        if peak(samples) < NO_SOUND_PEAK {
            return Self::NoSound;
        }
        if rms(samples) < QUIET_RMS {
            Self::Quiet
        } else {
            Self::Sound
        }
    }
}

/// The largest absolute sample, from 0.0 to 1.0 for audio in range.
pub fn peak(samples: &[f32]) -> f32 {
    samples
        .iter()
        .fold(0.0f32, |peak, sample| peak.max(sample.abs()))
}

pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f32 = samples.iter().map(|sample| sample * sample).sum();
    (sum / samples.len() as f32).sqrt()
}

/// An amplitude in decibels relative to full scale, for logs.
pub fn dbfs(amplitude: f32) -> f32 {
    20.0 * amplitude.max(1e-10).log10()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digital_silence_and_a_faint_floor_are_no_sound() {
        assert_eq!(TakeLevel::of(&[]), TakeLevel::NoSound);
        assert_eq!(TakeLevel::of(&[0.0; 16_000]), TakeLevel::NoSound);
        let floor: Vec<f32> = (0..16_000)
            .map(|i| if i % 2 == 0 { 0.0005 } else { -0.0005 })
            .collect();
        assert_eq!(TakeLevel::of(&floor), TakeLevel::NoSound);
    }

    #[test]
    fn a_quiet_room_is_sound_without_speech() {
        let room: Vec<f32> = (0..16_000)
            .map(|i| if i % 2 == 0 { 0.002 } else { -0.002 })
            .collect();
        assert_eq!(TakeLevel::of(&room), TakeLevel::Quiet);
    }

    #[test]
    fn speech_is_sound() {
        let speech: Vec<f32> = (0..16_000).map(|i| (i as f32 * 0.05).sin() * 0.2).collect();
        assert_eq!(TakeLevel::of(&speech), TakeLevel::Sound);
    }

    #[test]
    fn one_click_is_not_enough_to_count_as_speech() {
        let mut click = vec![0.0f32; 16_000];
        click[8_000] = 0.5;
        assert_eq!(TakeLevel::of(&click), TakeLevel::Quiet);
    }

    #[test]
    fn levels_convert_to_dbfs() {
        assert!((dbfs(NO_SOUND_PEAK) + 60.0).abs() < 0.01);
        assert!(dbfs(1.0).abs() < 0.01);
        assert!(dbfs(0.0) < -150.0);
        assert!((peak(&[0.1, -0.7, 0.3]) - 0.7).abs() < f32::EPSILON);
    }
}
