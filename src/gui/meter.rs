//! The microphones CPAL can open, and a level meter on the chosen one so the
//! Speech page shows that Hush hears you.

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, StreamConfig};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

pub const DEFAULT: &str = "System default";

fn name_of(device: &Device) -> Option<String> {
    device.description().ok().map(|d| d.name().to_string())
}

/// Every input device by name, the usual system-wide ones first.
pub fn input_devices() -> Vec<String> {
    let host = cpal::default_host();
    let mut names: Vec<String> = host
        .input_devices()
        .map(|devices| devices.filter_map(|d| name_of(&d)).collect())
        .unwrap_or_default();
    let rank = |name: &str| match name {
        "default" => 0,
        "pipewire" | "pulse" => 1,
        n if n.starts_with("sysdefault") => 2,
        _ => 3,
    };
    names.sort_by_key(|name| (rank(name), name.clone()));
    names.dedup();
    names
}

pub struct Meter {
    _stream: cpal::Stream,
    level: Arc<AtomicU32>,
    pub device: Option<String>,
}

impl Meter {
    /// Open the named microphone, or the default one, with the same 16 kHz
    /// mono stream the daemon records with. Without a name, a default that
    /// will not open gives way to the PipeWire or Pulse device, as the daemon's
    /// own fallback does.
    pub fn open(device: Option<&str>) -> Result<Self> {
        let host = cpal::default_host();
        let candidates: Vec<Device> = match device {
            Some(name) => vec![host
                .input_devices()
                .map_err(|e| anyhow!("Could not list microphones: {}", e))?
                .find(|d| name_of(d).is_some_and(|n| n.contains(name)))
                .ok_or_else(|| anyhow!("Microphone '{}' not found", name))?],
            None => {
                let shared = host
                    .input_devices()
                    .map(|devices| {
                        devices
                            .filter(|d| {
                                name_of(d).is_some_and(|n| {
                                    n.starts_with("pipewire") || n.starts_with("pulse")
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                host.default_input_device()
                    .into_iter()
                    .chain(shared)
                    .collect()
            },
        };
        if candidates.is_empty() {
            return Err(anyhow!("No microphone found"));
        }
        let level = Arc::new(AtomicU32::new(0));
        let mut last_error = anyhow!("No microphone found");
        for candidate in candidates {
            match start(&candidate, Arc::clone(&level)) {
                Ok(stream) => {
                    return Ok(Self {
                        _stream: stream,
                        level,
                        device: device.map(str::to_string),
                    })
                },
                Err(e) => last_error = e,
            }
        }
        Err(last_error)
    }

    /// The latest loudness, 0 to 1.
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }
}

fn start(device: &Device, level: Arc<AtomicU32>) -> Result<cpal::Stream> {
    let config = StreamConfig {
        channels: 1,
        sample_rate: 16000,
        buffer_size: cpal::BufferSize::Fixed(1024),
    };
    let stream = device
        .build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                level.store(rms(data).to_bits(), Ordering::Relaxed);
            },
            |_| {},
            None,
        )
        .map_err(|e| anyhow!("Could not open the microphone: {}", e))?;
    stream
        .play()
        .map_err(|e| anyhow!("Could not start the microphone: {}", e))?;
    Ok(stream)
}

/// Root mean square, exaggerated so speech fills the bar.
fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f32 = samples.iter().map(|s| s * s).sum();
    ((sum / samples.len() as f32).sqrt() * 8.0).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_is_zero_and_loud_input_saturates() {
        assert_eq!(rms(&[]), 0.0);
        assert_eq!(rms(&[0.0; 64]), 0.0);
        assert_eq!(rms(&[1.0; 64]), 1.0);
        let quiet = rms(&[0.01; 64]);
        assert!(quiet > 0.0 && quiet < 0.2);
    }
}
