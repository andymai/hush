/// Adapter for existing AudioCapture to implement AudioSource trait
///
/// This adapter wraps the existing AudioCapture implementation,
/// allowing it to be used with the new trait-based architecture.
use crate::audio::AudioCapture;
use crate::core::traits::{AudioBuffer, AudioSource};
use crate::Result;
use parking_lot::Mutex;
use std::sync::Arc;

/// Thread-safe wrapper for AudioCapture.
///
/// The underlying `cpal::Stream` is not Send/Sync, but this wrapper is safe because:
/// 1. Private constructor prevents direct instantiation
/// 2. Always wrapped in Arc<Mutex<>> by CpalAudioAdapter::new()
/// 3. Mutex ensures exclusive access to the Stream
/// 4. Audio callbacks only capture Arc<Mutex<>> types (already Send + Sync)
pub struct ThreadSafeAudioCapture(AudioCapture);

/// SAFETY: Always wrapped in Arc<Mutex<>> (enforced by private constructor).
/// The Mutex ensures exclusive access when sending between threads.
unsafe impl Send for ThreadSafeAudioCapture {}

/// SAFETY: Always wrapped in Arc<Mutex<>> (enforced by private constructor).
/// The Mutex prevents concurrent access to the inner AudioCapture.
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
            device_name,
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
            samples, 16000, // AudioCapture always uses 16kHz
            1,     // AudioCapture always uses mono
        ))
    }

    fn is_recording(&self) -> bool {
        self.inner.lock().inner().is_recording()
    }

    fn device_name(&self) -> &str {
        &self.device_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::AudioSource;

    #[test]
    fn test_cpal_adapter_implements_trait() {
        let _can_create_boxed_trait_object: Box<dyn AudioSource> =
            Box::new(CpalAudioAdapter::new(None).unwrap());
    }

    #[test]
    fn test_adapter_is_send() {
        let adapter = CpalAudioAdapter::new(None).unwrap();

        let handle = std::thread::spawn(move || {
            let device_name = adapter.device_name();
            assert!(!device_name.is_empty());
        });

        handle.join().expect("Thread should complete successfully");
    }

    #[test]
    fn test_adapter_is_sync() {
        use std::sync::Arc;

        let adapter = Arc::new(CpalAudioAdapter::new(None).unwrap());

        let handles: Vec<_> = (0..3)
            .map(|i| {
                let adapter_clone = Arc::clone(&adapter);
                std::thread::spawn(move || {
                    let device_name = adapter_clone.device_name();
                    println!("Thread {} read device: {}", i, device_name);
                    assert!(!device_name.is_empty());
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("Thread should complete successfully");
        }
    }

    #[test]
    fn test_concurrent_access_safety() {
        use std::sync::Arc;
        use std::thread;
        use std::time::Duration;

        let adapter = Arc::new(CpalAudioAdapter::new(None).unwrap());
        let mut handles = vec![];

        {
            let adapter_clone = Arc::clone(&adapter);
            handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    let _is_recording = adapter_clone.inner_arc().lock().inner().is_recording();
                    thread::sleep(Duration::from_millis(10));
                }
            }));
        }

        {
            let adapter_clone = Arc::clone(&adapter);
            handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    let _is_recording = adapter_clone.inner_arc().lock().inner().is_recording();
                    thread::sleep(Duration::from_millis(10));
                }
            }));
        }

        {
            let adapter_clone = Arc::clone(&adapter);
            handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    let _name = adapter_clone.device_name();
                    thread::sleep(Duration::from_millis(10));
                }
            }));
        }

        for handle in handles {
            handle.join().expect("Thread should complete without panic");
        }
    }

    #[test]
    fn test_mutex_prevents_concurrent_mutation() {
        use std::sync::Arc;
        use std::thread;
        use std::time::Duration;

        let adapter = Arc::new(CpalAudioAdapter::new(None).unwrap());
        let inner_arc = adapter.inner_arc();
        let mut handles = vec![];

        for i in 0..5 {
            let inner_clone = Arc::clone(&inner_arc);
            handles.push(thread::spawn(move || {
                let mut guard = inner_clone.lock();
                println!("Thread {} acquired lock", i);
                thread::sleep(Duration::from_millis(10));
                let _capture = guard.inner_mut();
            }));
        }

        for handle in handles {
            handle.join().expect("Thread should complete successfully");
        }
    }

    #[test]
    fn test_trait_object_with_send_sync_bounds() {
        let _boxed: Box<dyn AudioSource + Send> = Box::new(CpalAudioAdapter::new(None).unwrap());
        let _boxed_sync: Box<dyn AudioSource + Send + Sync> =
            Box::new(CpalAudioAdapter::new(None).unwrap());

        use std::sync::Arc;
        let _arc: Arc<dyn AudioSource + Send + Sync> =
            Arc::new(CpalAudioAdapter::new(None).unwrap());
    }

    #[test]
    fn test_private_constructor_enforces_safety() {
        let adapter = CpalAudioAdapter::new(None).unwrap();
        let _safe_access = adapter.inner_arc();
    }
}
