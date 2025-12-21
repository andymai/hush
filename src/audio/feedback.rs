use crate::Result;
use anyhow::Context;
use rodio::{OutputStream, Sink, Source};
use std::sync::Arc;
use std::time::Duration;
use tracing::warn;

/// Audio feedback system for providing sound cues
pub struct AudioFeedback {
    _stream: Option<Arc<OutputStream>>,
    enabled: bool,
}

impl AudioFeedback {
    /// Create a new audio feedback system
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn new() -> Result<Self> {
        // Try to initialize audio output, but don't fail if unavailable
        let (_stream, enabled) = match OutputStream::try_default() {
            Ok((stream, _handle)) => (Some(Arc::new(stream)), true),
            Err(e) => {
                warn!(
                    "Audio feedback unavailable: {}. Continuing without sound feedback.",
                    e
                );
                (None, false)
            },
        };

        Ok(Self { _stream, enabled })
    }

    /// Play recording start sound (ascending tone, 440 Hz -> 880 Hz)
    pub fn play_start(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // Use tokio's blocking task pool instead of spawning a new thread
        tokio::task::spawn_blocking(|| {
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

        // Use tokio's blocking task pool instead of spawning a new thread
        tokio::task::spawn_blocking(|| {
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

        // Use tokio's blocking task pool instead of spawning a new thread
        tokio::task::spawn_blocking(|| {
            if let Err(e) = play_double_beep(220.0, 150, 50) {
                warn!("Failed to play error sound: {}", e);
            }
        });

        Ok(())
    }

    /// Play success sound (high frequency chime, 1320 Hz)
    pub fn play_success(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // Use tokio's blocking task pool instead of spawning a new thread
        tokio::task::spawn_blocking(|| {
            if let Err(e) = play_tone(1320.0, 80) {
                warn!("Failed to play success sound: {}", e);
            }
        });

        Ok(())
    }
}

impl Default for AudioFeedback {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            _stream: None,
            enabled: false,
        })
    }
}

/// Play a single tone at the specified frequency and duration
fn play_tone(frequency: f32, duration_ms: u64) -> Result<()> {
    let (_stream, stream_handle) =
        OutputStream::try_default().context("Failed to get audio output stream")?;

    let sink = Sink::try_new(&stream_handle).context("Failed to create audio sink")?;

    let source = SineWave::new(frequency)
        .take_duration(Duration::from_millis(duration_ms))
        .amplify(0.20); // 20% volume to avoid being too loud

    sink.append(source);
    sink.sleep_until_end();

    Ok(())
}

/// Play two tones in sequence (for error sound)
fn play_double_beep(frequency: f32, duration_ms: u64, gap_ms: u64) -> Result<()> {
    let (_stream, stream_handle) =
        OutputStream::try_default().context("Failed to get audio output stream")?;

    let sink = Sink::try_new(&stream_handle).context("Failed to create audio sink")?;

    // First beep
    let source1 = SineWave::new(frequency)
        .take_duration(Duration::from_millis(duration_ms))
        .amplify(0.20);
    sink.append(source1);

    // Gap (silence)
    let silence =
        rodio::source::Zero::<f32>::new(1, 48000).take_duration(Duration::from_millis(gap_ms));
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
    fn current_frame_len(&self) -> Option<usize> {
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
