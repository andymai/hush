use crate::Result;
use anyhow::Context;
use rodio::{OutputStreamBuilder, Sink, Source};
use std::time::Duration;
use tracing::warn;

/// Sound cues for recording state changes. Each cue opens its own output
/// stream on a short-lived thread, so the value is cheap to copy anywhere.
#[derive(Debug, Clone, Copy)]
pub struct AudioFeedback {
    enabled: bool,
}

impl AudioFeedback {
    /// Probe the default output; without one the cues are silent.
    pub fn new() -> Result<Self> {
        let enabled = match OutputStreamBuilder::open_default_stream() {
            Ok(_stream) => true,
            Err(e) => {
                warn!(
                    "Audio feedback unavailable: {}. Continuing without sound feedback.",
                    e
                );
                false
            },
        };
        Ok(Self { enabled })
    }

    pub fn silent() -> Self {
        Self { enabled: false }
    }

    /// Play recording start sound (ascending tone, 440 Hz -> 880 Hz)
    pub fn play_start(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        std::thread::spawn(|| {
            if let Err(e) = play_tone(440.0, 100) {
                warn!("Failed to play start sound: {}", e);
            }
        });

        Ok(())
    }

    /// Play recording stop sound (descending tone, 880 Hz -> 440 Hz)
    pub fn play_stop(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        std::thread::spawn(|| {
            if let Err(e) = play_tone(880.0, 100) {
                warn!("Failed to play stop sound: {}", e);
            }
        });

        Ok(())
    }

    /// Play error sound (low frequency buzz, 220 Hz)
    pub fn play_error(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        std::thread::spawn(|| {
            if let Err(e) = play_double_beep(220.0, 150, 50) {
                warn!("Failed to play error sound: {}", e);
            }
        });

        Ok(())
    }

    /// A recording was discarded: one low tone.
    pub fn play_cancel(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        std::thread::spawn(|| {
            if let Err(e) = play_tone(330.0, 120) {
                warn!("Failed to play cancel sound: {}", e);
            }
        });
        Ok(())
    }

    /// The recording cap is close: two mid tones.
    pub fn play_warning(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        std::thread::spawn(|| {
            if let Err(e) = play_double_beep(660.0, 120, 60) {
                warn!("Failed to play warning sound: {}", e);
            }
        });
        Ok(())
    }

    /// Play success sound (high frequency chime, 1320 Hz)
    pub fn play_success(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        std::thread::spawn(|| {
            if let Err(e) = play_tone(1320.0, 80) {
                warn!("Failed to play success sound: {}", e);
            }
        });

        Ok(())
    }
}

impl Default for AudioFeedback {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self::silent())
    }
}

/// Play a single tone at the specified frequency and duration
fn play_tone(frequency: f32, duration_ms: u64) -> Result<()> {
    let stream =
        OutputStreamBuilder::open_default_stream().context("Failed to get audio output stream")?;

    let sink = Sink::connect_new(stream.mixer());

    let source = SineWave::new(frequency)
        .take_duration(Duration::from_millis(duration_ms))
        .amplify(0.20); // 20% volume to avoid being too loud

    sink.append(source);
    sink.sleep_until_end();

    Ok(())
}

/// Play two tones in sequence (for error sound)
fn play_double_beep(frequency: f32, duration_ms: u64, gap_ms: u64) -> Result<()> {
    let stream =
        OutputStreamBuilder::open_default_stream().context("Failed to get audio output stream")?;

    let sink = Sink::connect_new(stream.mixer());

    // First beep
    let source1 = SineWave::new(frequency)
        .take_duration(Duration::from_millis(duration_ms))
        .amplify(0.20);
    sink.append(source1);

    // Gap (silence)
    let silence = rodio::source::Zero::new(1, 48000).take_duration(Duration::from_millis(gap_ms));
    sink.append(silence);

    // Second beep
    let source2 = SineWave::new(frequency)
        .take_duration(Duration::from_millis(duration_ms))
        .amplify(0.20);
    sink.append(source2);

    sink.sleep_until_end();

    Ok(())
}

/// Simple sine wave generator
struct SineWave {
    frequency: f32,
    sample_rate: u32,
    current_sample: u64,
}

impl SineWave {
    fn new(frequency: f32) -> Self {
        Self {
            frequency,
            sample_rate: 48000,
            current_sample: 0,
        }
    }
}

impl Iterator for SineWave {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = (self.current_sample as f32 * self.frequency * 2.0 * std::f32::consts::PI
            / self.sample_rate as f32)
            .sin();
        self.current_sample = self.current_sample.wrapping_add(1);
        Some(sample)
    }
}

impl Source for SineWave {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        1
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
