/// Adapter for existing AudioCapture to implement AudioSource trait
///
/// This adapter wraps the existing AudioCapture implementation,
/// allowing it to be used with the new trait-based architecture.

use crate::audio::AudioCapture;
use crate::core::traits::{AudioBuffer, AudioConfig, AudioSource};
use crate::Result;
use parking_lot::Mutex;
use std::sync::Arc;

/// Wrapper that makes AudioCapture Send + Sync (unsafe but necessary for trait bounds)
/// SAFETY: We ensure single-threaded access through Arc<Mutex<>> and only use
/// this in contexts where the audio stream operations happen on the same thread.
struct ThreadSafeAudioCapture(AudioCapture);

// SAFETY: We use Arc<Mutex<>> to ensure only one thread can access at a time
unsafe impl Send for ThreadSafeAudioCapture {}
unsafe impl Sync for ThreadSafeAudioCapture {}

impl ThreadSafeAudioCapture {
    fn new(device_name: Option<&str>) -> Result<Self> {
        Ok(ThreadSafeAudioCapture(AudioCapture::new(device_name)?))
    }
    
    fn inner_mut(&mut self) -> &mut AudioCapture {
        &mut self.0
    }
    
    fn inner(&self) -> &AudioCapture {
        &self.0
    }
}

/// Adapter that wraps AudioCapture to implement AudioSource trait
/// Uses Arc<Mutex<>> for thread safety since cpal::Stream is not Send + Sync
pub struct CpalAudioAdapter {
    inner: Arc<Mutex<ThreadSafeAudioCapture>>,
    device_name: String,
}

impl CpalAudioAdapter {
    /// Create new adapter wrapping an AudioCapture instance
    pub fn new(device_name: Option<&str>) -> Result<Self> {
        let inner = ThreadSafeAudioCapture::new(device_name)?;
        let device_name = inner.inner().get_device_name();
        Ok(Self { 
            inner: Arc::new(Mutex::new(inner)), 
            device_name 
        })
    }

    /// Get cloned Arc to inner AudioCapture (for migration period)
    pub fn inner_arc(&self) -> Arc<Mutex<ThreadSafeAudioCapture>> {
        Arc::clone(&self.inner)
    }
}

impl AudioSource for CpalAudioAdapter {
    fn start_recording(&mut self) -> Result<()> {
        self.inner.lock().inner_mut().start_recording()
    }

    fn stop_recording(&mut self) -> Result<AudioBuffer> {
        let samples = self.inner.lock().inner_mut().stop_recording()?;

        // Convert to AudioBuffer
        Ok(AudioBuffer::new(
            samples,
            16000, // AudioCapture always uses 16kHz
            1,     // AudioCapture always uses mono
        ))
    }

    fn is_recording(&self) -> bool {
        self.inner.lock().inner().is_recording()
    }

    fn device_name(&self) -> &str {
        &self.device_name
    }

    fn config(&self) -> AudioConfig {
        use crate::core::types::*;
        AudioConfig {
            sample_rate: SampleRate::WHISPER_OPTIMAL,
            channels: Channels::MONO,
            buffer_size: BufferSize::STANDARD,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::AudioSource;

    #[test]
    fn test_cpal_adapter_implements_trait() {
        // This test verifies the adapter compiles and implements the trait
        // Actual functionality is tested via integration tests with real hardware
        let _can_create_boxed_trait_object: Box<dyn AudioSource> =
            Box::new(CpalAudioAdapter::new(None).unwrap());
    }

    #[test]
    fn test_adapter_config() {
        use crate::core::types::*;
        let adapter = CpalAudioAdapter::new(None).unwrap();
        let config = adapter.config();

        assert_eq!(config.sample_rate, SampleRate::WHISPER_OPTIMAL);
        assert_eq!(config.channels, Channels::MONO);
        assert_eq!(config.buffer_size, BufferSize::STANDARD);
    }
}
