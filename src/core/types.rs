/// Domain-specific newtype wrappers for type safety
///
/// These newtypes prevent mixing up parameters at compile time and provide
/// semantic meaning to primitive types (following Effective Rust best practices).

use crate::core::error::{AudioError, HushError};
use std::fmt;

// ============================================================================
// Audio Domain Types
// ============================================================================

/// Sample rate in Hertz (Hz)
///
/// Enforces valid sample rates and provides semantic constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SampleRate(u32);

impl SampleRate {
    /// Optimal sample rate for Whisper transcription (16kHz)
    pub const WHISPER_OPTIMAL: Self = Self(16000);

    /// Standard telephone quality (8kHz)
    pub const TELEPHONE: Self = Self(8000);

    /// CD quality (44.1kHz)
    pub const CD_QUALITY: Self = Self(44100);

    /// Professional audio (48kHz)
    pub const PROFESSIONAL: Self = Self(48000);

    /// Minimum supported sample rate
    pub const MIN: u32 = 8000;

    /// Maximum supported sample rate
    pub const MAX: u32 = 48000;

    /// Create a new sample rate with validation
    pub fn new(hz: u32) -> Result<Self, HushError> {
        if hz < Self::MIN || hz > Self::MAX {
            return Err(HushError::Audio(AudioError::InvalidSampleRate {
                hz,
                min: Self::MIN,
                max: Self::MAX,
            }));
        }
        Ok(Self(hz))
    }

    /// Create without validation (use for constants only)
    pub const fn new_unchecked(hz: u32) -> Self {
        Self(hz)
    }

    /// Get the raw Hz value
    pub const fn as_u32(self) -> u32 {
        self.0
    }

    /// Get as Hz value
    pub const fn hz(self) -> u32 {
        self.0
    }
}

impl fmt::Display for SampleRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}Hz", self.0)
    }
}

impl TryFrom<u32> for SampleRate {
    type Error = HushError;

    fn try_from(hz: u32) -> Result<Self, Self::Error> {
        Self::new(hz)
    }
}

impl From<SampleRate> for u32 {
    fn from(rate: SampleRate) -> u32 {
        rate.0
    }
}

// ============================================================================
// Channel Count
// ============================================================================

/// Number of audio channels
///
/// Enforces valid channel counts with semantic constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Channels(u32);

impl Channels {
    /// Mono audio (1 channel) - optimal for speech recognition
    pub const MONO: Self = Self(1);

    /// Stereo audio (2 channels)
    pub const STEREO: Self = Self(2);

    /// Quadraphonic (4 channels)
    pub const QUAD: Self = Self(4);

    /// 5.1 Surround (6 channels)
    pub const SURROUND_5_1: Self = Self(6);

    /// 7.1 Surround (8 channels)
    pub const SURROUND_7_1: Self = Self(8);

    /// Maximum supported channels
    pub const MAX: u32 = 8;

    /// Create a new channel count with validation
    pub fn new(count: u32) -> Result<Self, HushError> {
        if count == 0 || count > Self::MAX {
            return Err(HushError::Audio(AudioError::InvalidChannelCount {
                count,
                max: Self::MAX,
            }));
        }
        Ok(Self(count))
    }

    /// Create without validation (use for constants only)
    pub const fn new_unchecked(count: u32) -> Self {
        Self(count)
    }

    /// Get the raw channel count
    pub const fn as_u32(self) -> u32 {
        self.0
    }

    /// Get channel count
    pub const fn count(self) -> u32 {
        self.0
    }

    /// Check if mono
    pub const fn is_mono(self) -> bool {
        self.0 == 1
    }

    /// Check if stereo
    pub const fn is_stereo(self) -> bool {
        self.0 == 2
    }
}

impl fmt::Display for Channels {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            1 => write!(f, "Mono"),
            2 => write!(f, "Stereo"),
            n => write!(f, "{} channels", n),
        }
    }
}

impl TryFrom<u32> for Channels {
    type Error = HushError;

    fn try_from(count: u32) -> Result<Self, Self::Error> {
        Self::new(count)
    }
}

impl From<Channels> for u32 {
    fn from(channels: Channels) -> u32 {
        channels.0
    }
}

// ============================================================================
// Buffer Size
// ============================================================================

/// Audio buffer size in samples
///
/// Must be a power of 2 for efficient processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BufferSize(usize);

impl BufferSize {
    /// Minimum buffer size (good for low latency)
    pub const MIN: Self = Self(128);

    /// Standard buffer size (balanced)
    pub const STANDARD: Self = Self(1024);

    /// Large buffer size (good for stability)
    pub const LARGE: Self = Self(4096);

    /// Create a new buffer size with validation
    pub fn new(size: usize) -> Result<Self, HushError> {
        // Must be power of 2 and >= 128
        if size < 128 || !size.is_power_of_two() {
            return Err(HushError::Audio(AudioError::InvalidBufferSize {
                size,
                reason: "Buffer size must be a power of 2 and >= 128".to_string(),
            }));
        }
        Ok(Self(size))
    }

    /// Create without validation (use for constants only)
    pub const fn new_unchecked(size: usize) -> Self {
        Self(size)
    }

    /// Get the raw buffer size
    pub const fn as_usize(self) -> usize {
        self.0
    }

    /// Get buffer size in samples
    pub const fn samples(self) -> usize {
        self.0
    }
}

impl fmt::Display for BufferSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} samples", self.0)
    }
}

impl TryFrom<usize> for BufferSize {
    type Error = HushError;

    fn try_from(size: usize) -> Result<Self, Self::Error> {
        Self::new(size)
    }
}

impl From<BufferSize> for usize {
    fn from(size: BufferSize) -> usize {
        size.0
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sample_rate_validation() {
        // Valid rates
        assert!(SampleRate::new(16000).is_ok());
        assert!(SampleRate::new(44100).is_ok());

        // Invalid rates
        assert!(SampleRate::new(7999).is_err());
        assert!(SampleRate::new(48001).is_err());
    }

    #[test]
    fn test_sample_rate_constants() {
        assert_eq!(SampleRate::WHISPER_OPTIMAL.as_u32(), 16000);
        assert_eq!(SampleRate::CD_QUALITY.as_u32(), 44100);
    }

    #[test]
    fn test_channels_validation() {
        // Valid counts
        assert!(Channels::new(1).is_ok());
        assert!(Channels::new(2).is_ok());
        assert!(Channels::new(8).is_ok());

        // Invalid counts
        assert!(Channels::new(0).is_err());
        assert!(Channels::new(9).is_err());
    }

    #[test]
    fn test_channels_constants() {
        assert_eq!(Channels::MONO.as_u32(), 1);
        assert_eq!(Channels::STEREO.as_u32(), 2);
        assert!(Channels::MONO.is_mono());
        assert!(!Channels::STEREO.is_mono());
    }

    #[test]
    fn test_buffer_size_validation() {
        // Valid sizes (powers of 2)
        assert!(BufferSize::new(128).is_ok());
        assert!(BufferSize::new(1024).is_ok());
        assert!(BufferSize::new(4096).is_ok());

        // Invalid sizes
        assert!(BufferSize::new(100).is_err()); // Not power of 2
        assert!(BufferSize::new(1000).is_err()); // Not power of 2
        assert!(BufferSize::new(64).is_err()); // Too small
    }

    #[test]
    fn test_type_safety() {
        let sample_rate = SampleRate::WHISPER_OPTIMAL;
        let channels = Channels::MONO;

        // This wouldn't compile if we accidentally swapped them:
        // fn takes_sample_rate(rate: SampleRate) {}
        // takes_sample_rate(channels); // ← Compile error! ✅

        // But primitives could be mixed:
        // fn takes_u32(x: u32) {}
        // takes_u32(16000); // sample rate
        // takes_u32(1);     // channels - compiler can't catch this bug!

        assert_eq!(sample_rate.as_u32(), 16000);
        assert_eq!(channels.as_u32(), 1);
    }
}
