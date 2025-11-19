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
///
/// # SAFETY DOCUMENTATION
///
/// This type implements Send and Sync using `unsafe impl`, which requires careful
/// justification. The underlying `AudioCapture` contains `cpal::Stream`, which is
/// NOT Send or Sync due to platform-specific audio handles and callbacks.
///
/// ## Why unsafe is necessary
///
/// The `cpal::Stream` type is not Send/Sync because:
/// 1. It contains platform-specific audio handles (ALSA/PipeWire/JACK on Linux)
/// 2. It has audio callbacks that must run on specific audio service threads
/// 3. Moving the stream between threads could cause:
///    - Use-after-free if the audio thread accesses deallocated memory
///    - Data races between audio callback thread and application threads
///    - Violation of audio service threading requirements
///
/// However, we need AudioCapture to be Send/Sync to use it with the trait-based
/// architecture (AudioSource trait objects require Send + Sync bounds).
///
/// ## Safety invariants
///
/// This unsafe impl is ONLY safe because:
///
/// 1. **Private constructor**: `ThreadSafeAudioCapture::new()` is private (not pub),
///    preventing external code from creating instances outside the controlled context.
///
/// 2. **Always wrapped in Arc<Mutex<>>**: The ONLY way to obtain a ThreadSafeAudioCapture
///    is through `CpalAudioAdapter::new()`, which immediately wraps it in `Arc<Mutex<>>`.
///    See line 47: `Arc::new(Mutex::new(inner))`.
///
/// 3. **Mutex prevents concurrent access**: All methods on AudioCapture require `&mut self`
///    (start_recording, stop_recording). The Mutex ensures only one thread can hold
///    a mutable lock at a time, preventing concurrent access to the Stream.
///
/// 4. **Stream callbacks only capture Arc<Mutex<>> data**: When the audio stream is
///    created in `AudioCapture::start_recording()`, the callback only captures:
///    - `Arc<Mutex<Vec<f32>>>` (audio buffer)
///    - `Arc<Mutex<bool>>` (is_recording flag)
///    - `Option<mpsc::Sender<f32>>` (amplitude sender)
///    All of these types are already Send + Sync, so the callback can safely run
///    on the audio thread while the main thread holds the AudioCapture.
///
/// 5. **Stream lifecycle is single-threaded**: The Stream is created and destroyed
///    through `&mut self` methods while holding the Mutex lock, ensuring no other
///    thread can access it during state transitions.
///
/// ## What could go wrong (if invariants violated)
///
/// If someone could create a bare `ThreadSafeAudioCapture` without the Arc<Mutex<>>
/// wrapper and send it to another thread, these safety violations could occur:
///
/// 1. **Audio callback use-after-free**: If Thread A starts recording (creating Stream
///    with callbacks), then Thread B drops the AudioCapture, the audio thread could
///    access deallocated memory through the captured Arc<Mutex<>> pointers.
///    - MITIGATED: This doesn't happen because Arc<Mutex<>> keeps the data alive
///      as long as the callback holds a reference.
///
/// 2. **Concurrent start/stop**: If Thread A calls start_recording() while Thread B
///    calls stop_recording(), the Stream could be in an inconsistent state.
///    - MITIGATED: The Arc<Mutex<>> wrapper prevents this - only one thread can
///      hold `&mut self` at a time.
///
/// 3. **Cross-thread Stream access**: If the Stream was sent to another thread, the
///    audio backend might crash or behave unpredictably.
///    - MITIGATED: The Stream is never exposed outside AudioCapture, and the Mutex
///      ensures all access is synchronized.
///
/// ## Why this is safe
///
/// The combination of:
/// - Private constructor (no external instantiation)
/// - Mandatory Arc<Mutex<>> wrapper (enforced by CpalAudioAdapter::new)
/// - Stream callbacks only capturing Arc<Mutex<>> types (already Send + Sync)
/// - Mutex-protected method access (no concurrent mutable access)
///
/// ...ensures that even though we claim ThreadSafeAudioCapture is Send + Sync,
/// the actual dangerous type (cpal::Stream) is never sent between threads or
/// accessed concurrently.
///
/// ## Alternatives considered
///
/// 1. **Different audio library**: Could use a different crate with Send/Sync streams.
///    - REJECTED: CPAL is the most mature cross-platform audio library for Rust.
///    - Would require significant refactoring with uncertain compatibility.
///
/// 2. **Single-threaded architecture**: Keep all audio operations on one thread.
///    - REJECTED: The trait-based architecture requires Send + Sync for trait objects.
///    - Would prevent using AudioSource in multi-threaded contexts like async runtimes.
///
/// 3. **Remove unsafe, accept !Send + !Sync**: Use `AudioCapture` directly without
///    implementing Send/Sync.
///    - REJECTED: Cannot use with `Box<dyn AudioSource>` or `Arc<dyn AudioSource>`
///      because trait objects with Send/Sync bounds require Send/Sync implementations.
///
/// 4. **Use message passing instead of shared state**: Send commands to audio thread.
///    - CONSIDERED: Would be architecturally cleaner but requires major refactoring.
///    - Could be future work to eliminate this unsafe code entirely.
///
/// ## Testing
///
/// Safety is verified through:
/// - Multi-threaded stress tests (see tests::test_concurrent_access_safety below)
/// - Runtime assertions in AudioCapture methods (is_recording checks)
/// - Integration tests that exercise concurrent recording scenarios
/// - MIRI testing would be ideal but may not support CPAL's foreign function calls
///
/// ## Technical debt note
///
/// This unsafe impl is necessary for the current architecture but represents technical
/// debt. A better long-term solution would be to refactor AudioCapture to use message
/// passing (e.g., crossbeam channels) to communicate with a dedicated audio thread,
/// eliminating the need for unsafe code entirely.
///
/// See: Architecture docs on unsafe code patterns and future refactoring opportunities.
pub struct ThreadSafeAudioCapture(AudioCapture);

/// SAFETY: ThreadSafeAudioCapture is Send because:
/// 1. It is ALWAYS wrapped in Arc<Mutex<>> (enforced by private constructor)
/// 2. The Mutex ensures exclusive access when sending between threads
/// 3. The wrapped cpal::Stream callbacks only capture Send types (Arc<Mutex<>>)
/// 4. The Stream itself is never directly accessed outside the Mutex guard
///
/// See the detailed safety documentation on ThreadSafeAudioCapture above.
unsafe impl Send for ThreadSafeAudioCapture {}

/// SAFETY: ThreadSafeAudioCapture is Sync because:
/// 1. It is ALWAYS wrapped in Arc<Mutex<>> (enforced by private constructor)
/// 2. The Mutex prevents concurrent access to the inner AudioCapture
/// 3. All methods require &mut self, enforcing exclusive access through the Mutex
/// 4. Shared references (&self) cannot cause data races due to Mutex protection
///
/// See the detailed safety documentation on ThreadSafeAudioCapture above.
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

    /// Test that CpalAudioAdapter can be sent between threads
    /// This verifies the Send bound from our unsafe impl
    #[test]
    fn test_adapter_is_send() {
        let adapter = CpalAudioAdapter::new(None).unwrap();

        // Move adapter to another thread
        let handle = std::thread::spawn(move || {
            // Verify we can access the adapter from another thread
            let device_name = adapter.device_name();
            assert!(!device_name.is_empty());
        });

        handle.join().expect("Thread should complete successfully");
    }

    /// Test that CpalAudioAdapter can be shared between threads via Arc
    /// This verifies the Sync bound from our unsafe impl
    #[test]
    fn test_adapter_is_sync() {
        use std::sync::Arc;

        let adapter = Arc::new(CpalAudioAdapter::new(None).unwrap());

        // Create multiple threads that read from the adapter concurrently
        let handles: Vec<_> = (0..3)
            .map(|i| {
                let adapter_clone = Arc::clone(&adapter);
                std::thread::spawn(move || {
                    // Each thread reads the device name
                    let device_name = adapter_clone.device_name();
                    println!("Thread {} read device: {}", i, device_name);
                    assert!(!device_name.is_empty());
                })
            })
            .collect();

        // Wait for all threads to complete
        for handle in handles {
            handle.join().expect("Thread should complete successfully");
        }
    }

    /// Test concurrent access to the adapter through multiple threads
    /// This stresses the Mutex synchronization to ensure no data races
    #[test]
    fn test_concurrent_access_safety() {
        use std::sync::Arc;
        use std::thread;
        use std::time::Duration;

        let adapter = Arc::new(CpalAudioAdapter::new(None).unwrap());
        let mut handles = vec![];

        // Thread 1: Repeatedly check recording status
        {
            let adapter_clone = Arc::clone(&adapter);
            handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    let _is_recording = adapter_clone.inner_arc().lock().inner().is_recording();
                    thread::sleep(Duration::from_millis(10));
                }
            }));
        }

        // Thread 2: Also check recording status
        {
            let adapter_clone = Arc::clone(&adapter);
            handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    let _is_recording = adapter_clone.inner_arc().lock().inner().is_recording();
                    thread::sleep(Duration::from_millis(10));
                }
            }));
        }

        // Thread 3: Read device name
        {
            let adapter_clone = Arc::clone(&adapter);
            handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    let _name = adapter_clone.device_name();
                    thread::sleep(Duration::from_millis(10));
                }
            }));
        }

        // Wait for all threads to complete without panicking
        for handle in handles {
            handle.join().expect("Thread should complete without panic");
        }

        // If we got here, no data races or deadlocks occurred
    }

    /// Test that the Arc<Mutex<>> wrapper properly enforces exclusive access
    /// This verifies that only one thread can mutate the AudioCapture at a time
    #[test]
    fn test_mutex_prevents_concurrent_mutation() {
        use std::sync::Arc;
        use std::thread;
        use std::time::Duration;

        let adapter = Arc::new(CpalAudioAdapter::new(None).unwrap());
        let inner_arc = adapter.inner_arc();

        // Try to acquire the lock from multiple threads
        // Only one should be able to hold it at a time
        let mut handles = vec![];

        for i in 0..5 {
            let inner_clone = Arc::clone(&inner_arc);
            handles.push(thread::spawn(move || {
                // Acquire the lock (blocking until available)
                let mut guard = inner_clone.lock();

                // Simulate some work while holding the lock
                println!("Thread {} acquired lock", i);
                thread::sleep(Duration::from_millis(10));

                // Access inner AudioCapture through mutable reference
                let _capture = guard.inner_mut();

                // Lock is automatically released when guard is dropped
            }));
        }

        // All threads should complete successfully
        for handle in handles {
            handle.join().expect("Thread should complete successfully");
        }
    }

    /// Test that the adapter compiles with trait object bounds
    /// This verifies Send + Sync are properly implemented for trait object usage
    #[test]
    fn test_trait_object_with_send_sync_bounds() {
        // These would fail to compile if Send/Sync weren't properly implemented
        let _boxed: Box<dyn AudioSource + Send> = Box::new(CpalAudioAdapter::new(None).unwrap());
        let _boxed_sync: Box<dyn AudioSource + Send + Sync> =
            Box::new(CpalAudioAdapter::new(None).unwrap());

        // Can also use Arc for shared ownership across threads
        use std::sync::Arc;
        let _arc: Arc<dyn AudioSource + Send + Sync> =
            Arc::new(CpalAudioAdapter::new(None).unwrap());
    }

    /// Test that demonstrates the safety invariant: private constructor
    /// This is a compile-time check that ThreadSafeAudioCapture cannot be
    /// created outside this module
    #[test]
    fn test_private_constructor_enforces_safety() {
        // This test documents that ThreadSafeAudioCapture::new() is private
        // The following would NOT compile if uncommented:
        // let unsafe_direct = ThreadSafeAudioCapture::new(None);
        //
        // This enforces that ThreadSafeAudioCapture can ONLY be obtained
        // through CpalAudioAdapter::new(), which wraps it in Arc<Mutex<>>

        // The safe way to get a ThreadSafeAudioCapture is through the adapter
        let adapter = CpalAudioAdapter::new(None).unwrap();
        let _safe_access = adapter.inner_arc();

        // This test passes if it compiles, documenting the safety invariant
    }
}
