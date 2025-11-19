use crate::config::settings::{HotkeyConfig, TranscriptionConfig};
use crate::core::types::{BufferSize, Channels, SampleRate};
/// Core trait abstractions for Hush components
///
/// This module defines the interfaces for all major components,
/// enabling dependency injection, testing, and extensibility.
use crate::Result;
use async_trait::async_trait;
use std::time::Duration;
use std::time::SystemTime;
use tokio::sync::mpsc;

// ============================================================================
// Audio Abstraction
// ============================================================================

/// Audio buffer with metadata
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration: Duration,
}

impl AudioBuffer {
    /// Create audio buffer from raw values (for backward compatibility)
    pub fn new(samples: Vec<f32>, sample_rate: u32, channels: u16) -> Self {
        let duration =
            Duration::from_secs_f32(samples.len() as f32 / (sample_rate as f32 * channels as f32));
        Self {
            samples,
            sample_rate,
            channels,
            duration,
        }
    }

    /// Create audio buffer from type-safe values
    pub fn from_config(samples: Vec<f32>, sample_rate: SampleRate, channels: Channels) -> Self {
        Self::new(samples, sample_rate.as_u32(), channels.as_u32() as u16)
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }
}

/// Audio device information
#[derive(Debug, Clone)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// Audio configuration with type-safe newtypes
#[derive(Debug, Clone, Copy)]
pub struct AudioConfig {
    pub sample_rate: SampleRate,
    pub channels: Channels,
    pub buffer_size: BufferSize,
}

/// Core trait for audio capture systems
///
/// Implementations: CpalAudioSource, SimulatedAudioSource, PulseAudioSource
pub trait AudioSource: Send + Sync {
    /// Start capturing audio from the source
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockAudioSource;
    /// use hush::core::traits::AudioSource;
    ///
    /// let mut audio = MockAudioSource::new();
    /// audio.start_recording()?;
    /// assert!(audio.is_recording());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Already recording
    /// - Audio device unavailable
    /// - Device configuration invalid
    fn start_recording(&mut self) -> Result<()>;

    /// Stop capturing and return all recorded audio samples
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockAudioSource;
    /// use hush::core::traits::AudioSource;
    ///
    /// let mut audio = MockAudioSource::new();
    /// audio.start_recording()?;
    ///
    /// // Record for some time...
    /// std::thread::sleep(std::time::Duration::from_millis(100));
    ///
    /// let buffer = audio.stop_recording()?;
    /// assert!(!buffer.is_empty());
    /// assert!(buffer.sample_rate > 0);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Not currently recording
    /// - Audio capture failed
    fn stop_recording(&mut self) -> Result<AudioBuffer>;

    /// Check if currently recording
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockAudioSource;
    /// use hush::core::traits::AudioSource;
    ///
    /// let mut audio = MockAudioSource::new();
    /// assert!(!audio.is_recording());
    ///
    /// audio.start_recording()?;
    /// assert!(audio.is_recording());
    ///
    /// audio.stop_recording()?;
    /// assert!(!audio.is_recording());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    fn is_recording(&self) -> bool;

    /// Get human-readable device name
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockAudioSource;
    /// use hush::core::traits::AudioSource;
    ///
    /// let audio = MockAudioSource::new();
    /// let name = audio.device_name();
    /// assert!(!name.is_empty());
    /// ```
    fn device_name(&self) -> &str;

    /// Get audio configuration (sample rate, channels)
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockAudioSource;
    /// use hush::core::traits::AudioSource;
    ///
    /// let audio = MockAudioSource::new();
    /// let config = audio.config();
    /// assert!(config.sample_rate.as_u32() > 0);
    /// assert!(config.channels.as_u32() > 0);
    /// ```
    fn config(&self) -> AudioConfig;
}

// ============================================================================
// Transcription Abstraction
// ============================================================================

/// Transcription result with metadata
#[derive(Debug, Clone)]
pub struct TranscriptionResult {
    pub text: String,
    pub confidence: f32,
    pub language: Option<String>,
    pub processing_time: Duration,
}

impl TranscriptionResult {
    pub fn simple(text: String, confidence: f32, duration: Duration) -> Self {
        Self {
            text,
            confidence,
            language: None,
            processing_time: duration,
        }
    }
}

/// Transcriber capabilities and info
#[derive(Debug, Clone)]
pub struct TranscriberInfo {
    pub name: String,
    pub version: String,
    pub supports_languages: Vec<String>,
    pub max_audio_duration: Option<Duration>,
    pub requires_network: bool,
    pub hardware_accelerated: bool,
}

/// Speech-to-text transcription engine
///
/// Implementations: WhisperAdapter (local Whisper models), MockTranscriber (testing)
#[async_trait]
pub trait Transcriber: Send + Sync {
    /// Transcribe audio to text asynchronously
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockTranscriber;
    /// use hush::core::traits::{Transcriber, AudioBuffer};
    ///
    /// # tokio_test::block_on(async {
    /// let transcriber = MockTranscriber::new();
    /// let audio = AudioBuffer::new(vec![0.1f32; 16000], 16000, 1);
    ///
    /// let result = transcriber.transcribe(&audio).await?;
    /// assert!(!result.text.is_empty());
    /// assert!(result.confidence > 0.0);
    /// # Ok::<(), anyhow::Error>(())
    /// # })
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Audio format is incompatible
    /// - Transcription model fails
    /// - Audio duration exceeds limits
    async fn transcribe(&self, audio: &AudioBuffer) -> Result<TranscriptionResult>;

    /// Get transcriber capabilities/info
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockTranscriber;
    /// use hush::core::traits::Transcriber;
    ///
    /// let transcriber = MockTranscriber::new();
    /// let info = transcriber.info();
    /// assert!(!info.name.is_empty());
    /// assert!(!info.version.is_empty());
    /// ```
    fn info(&self) -> TranscriberInfo;

    /// Check if transcriber is ready (model loaded, etc.)
    ///
    /// Implementations must explicitly define readiness logic rather than
    /// defaulting to always-ready, which could mask initialization issues.
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockTranscriber;
    /// use hush::core::traits::Transcriber;
    ///
    /// # tokio_test::block_on(async {
    /// let transcriber = MockTranscriber::new();
    /// assert!(transcriber.is_ready().await);
    /// # })
    /// ```
    async fn is_ready(&self) -> bool;
}

// ============================================================================
// Text Output Abstraction
// ============================================================================

/// Window information (platform-agnostic)
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub title: String,
    pub class: String,
    pub app_name: String,
}

/// Text insertion and output
///
/// Implementations: X11TextInserter, WaylandTextInserter, ClipboardOutput, StdoutOutput
#[async_trait]
pub trait TextOutput: Send + Sync {
    /// Insert text at current cursor position
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockTextOutput;
    /// use hush::core::traits::TextOutput;
    ///
    /// # tokio_test::block_on(async {
    /// let mut output = MockTextOutput::new();
    /// output.insert_text("Hello, world!").await?;
    ///
    /// let inserted = output.get_inserted_texts();
    /// assert_eq!(inserted.len(), 1);
    /// assert_eq!(inserted[0], "Hello, world!");
    /// # Ok::<(), anyhow::Error>(())
    /// # })
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Text insertion fails
    /// - No window is focused
    /// - Platform-specific insertion unavailable
    async fn insert_text(&mut self, text: &str) -> Result<()>;

    /// Get currently focused window (if available)
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockTextOutput;
    /// use hush::core::traits::TextOutput;
    ///
    /// # tokio_test::block_on(async {
    /// let output = MockTextOutput::new();
    /// let window = output.focused_window().await?;
    ///
    /// if let Some(win) = window {
    ///     assert!(!win.title.is_empty());
    /// }
    /// # Ok::<(), anyhow::Error>(())
    /// # })
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Window information unavailable
    /// - Platform doesn't support window queries
    async fn focused_window(&self) -> Result<Option<WindowInfo>>;

    /// Check if text output is available/ready
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockTextOutput;
    /// use hush::core::traits::TextOutput;
    ///
    /// let output = MockTextOutput::new();
    /// assert!(output.is_available());
    /// ```
    fn is_available(&self) -> bool;

    /// Get output method name
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockTextOutput;
    /// use hush::core::traits::TextOutput;
    ///
    /// let output = MockTextOutput::new();
    /// let method = output.output_method();
    /// assert!(!method.is_empty());
    /// ```
    fn output_method(&self) -> &str;
}

// ============================================================================
// Input Trigger Abstraction
// ============================================================================

/// Trigger events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerEvent {
    /// Start recording (hotkey pressed, wake word detected, etc.)
    StartRecording,
    /// Stop recording (hotkey released, timeout, etc.)
    StopRecording,
    /// Cancel recording
    Cancel,
}

/// User input trigger for recording
///
/// Implementations: HotkeyTrigger, CLITrigger, DBusTrigger, WakeWordTrigger
#[async_trait]
pub trait InputTrigger: Send + Sync {
    /// Start listening for trigger events
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockInputTrigger;
    /// use hush::core::traits::InputTrigger;
    ///
    /// # tokio_test::block_on(async {
    /// let mut trigger = MockInputTrigger::new();
    /// trigger.start_listening().await?;
    /// # Ok::<(), anyhow::Error>(())
    /// # })
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Already listening
    /// - Platform-specific registration fails
    /// - Hotkey conflicts with system hotkeys
    async fn start_listening(&mut self) -> Result<()>;

    /// Stop listening
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockInputTrigger;
    /// use hush::core::traits::InputTrigger;
    ///
    /// # tokio_test::block_on(async {
    /// let mut trigger = MockInputTrigger::new();
    /// trigger.start_listening().await?;
    /// trigger.stop_listening().await?;
    /// # Ok::<(), anyhow::Error>(())
    /// # })
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Not currently listening
    /// - Platform-specific deregistration fails
    async fn stop_listening(&mut self) -> Result<()>;

    /// Wait for next trigger event
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockInputTrigger;
    /// use hush::core::traits::{InputTrigger, TriggerEvent};
    ///
    /// # tokio_test::block_on(async {
    /// let mut trigger = MockInputTrigger::new();
    /// trigger.start_listening().await?;
    ///
    /// // Simulate a trigger event
    /// trigger.trigger(TriggerEvent::StartRecording)?;
    ///
    /// // Wait for the event
    /// let event = trigger.next_event().await;
    /// assert_eq!(event, Some(TriggerEvent::StartRecording));
    /// # Ok::<(), anyhow::Error>(())
    /// # })
    /// ```
    async fn next_event(&mut self) -> Option<TriggerEvent>;

    /// Get trigger description (e.g., "Ctrl+Shift+Space")
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockInputTrigger;
    /// use hush::core::traits::InputTrigger;
    ///
    /// let trigger = MockInputTrigger::new();
    /// let description = trigger.description();
    /// assert!(!description.is_empty());
    /// ```
    fn description(&self) -> String;
}

// ============================================================================
// Configuration Abstraction
// ============================================================================

/// Configuration access
pub trait ConfigProvider: Send + Sync {
    fn audio_config(&self) -> AudioConfig;
    fn transcription_config(&self) -> &TranscriptionConfig;
    fn hotkey_config(&self) -> &HotkeyConfig;
}

// ============================================================================
// System Tray Abstraction
// ============================================================================

/// Transcription entry for history storage
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TranscriptionEntry {
    pub id: String,
    pub timestamp: SystemTime,
    pub text: String,
    pub confidence: f32,
    pub duration: Duration,
}

/// System tray icon state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayIconState {
    Idle,
    Recording,
    Processing,
    Error,
}

/// System tray menu item
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayMenuItem {
    Show,
    Hide,
    Settings,
    ClearHistory,
    StartRecording,
    StopRecording,
    About,
    Quit,
}

/// System tray events
#[derive(Debug, Clone)]
pub enum TrayEvent {
    MenuClicked(TrayMenuItem),
    HistoryItemSelected(String), // entry id
    SettingsRequested,
    QuitRequested,
}

/// Menu transcription item
#[derive(Debug, Clone)]
pub struct MenuTranscriptionItem {
    pub id: String,
    pub text: String,
    pub preview: String, // truncated text for display
}

/// System tray menu structure
#[derive(Debug, Clone)]
pub struct TrayMenu {
    pub recent_transcriptions: Vec<MenuTranscriptionItem>,
    pub recording_state: bool,
}

/// System tray interface
#[async_trait]
pub trait SystemTray: Send + Sync {
    /// Show the system tray icon
    ///
    /// # Example
    ///
    /// ```
    /// use hush::tray::mocks::MockSystemTray;
    /// use hush::core::traits::SystemTray;
    ///
    /// # tokio_test::block_on(async {
    /// let mut tray = MockSystemTray::new();
    /// tray.show().await?;
    /// assert!(tray.is_visible);
    /// # Ok::<(), anyhow::Error>(())
    /// # })
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - System tray not available
    /// - Platform initialization fails
    async fn show(&mut self) -> Result<()>;

    /// Hide the system tray icon
    ///
    /// # Example
    ///
    /// ```
    /// use hush::tray::mocks::MockSystemTray;
    /// use hush::core::traits::SystemTray;
    ///
    /// # tokio_test::block_on(async {
    /// let mut tray = MockSystemTray::new();
    /// tray.show().await?;
    /// tray.hide().await?;
    /// assert!(!tray.is_visible);
    /// # Ok::<(), anyhow::Error>(())
    /// # })
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Tray hide operation fails
    async fn hide(&mut self) -> Result<()>;

    /// Set the tray icon state
    ///
    /// # Example
    ///
    /// ```
    /// use hush::tray::mocks::MockSystemTray;
    /// use hush::core::traits::{SystemTray, TrayIconState};
    ///
    /// let mut tray = MockSystemTray::new();
    /// tray.set_icon(TrayIconState::Recording);
    /// assert_eq!(tray.icon_state, TrayIconState::Recording);
    /// ```
    fn set_icon(&mut self, state: TrayIconState);

    /// Set the tooltip text
    ///
    /// # Example
    ///
    /// ```
    /// use hush::tray::mocks::MockSystemTray;
    /// use hush::core::traits::SystemTray;
    ///
    /// let mut tray = MockSystemTray::new();
    /// tray.set_tooltip("Recording in progress...");
    /// assert_eq!(tray.tooltip, "Recording in progress...");
    /// ```
    fn set_tooltip(&mut self, text: &str);

    /// Update the context menu
    ///
    /// # Example
    ///
    /// ```
    /// use hush::tray::mocks::MockSystemTray;
    /// use hush::core::traits::{SystemTray, TrayMenu};
    ///
    /// let mut tray = MockSystemTray::new();
    /// let menu = TrayMenu {
    ///     recent_transcriptions: vec![],
    ///     recording_state: false,
    /// };
    /// tray.update_menu(menu);
    /// assert!(tray.menu.is_some());
    /// ```
    fn update_menu(&mut self, menu: TrayMenu);

    /// Get event receiver for tray interactions
    ///
    /// # Example
    ///
    /// ```
    /// use hush::tray::mocks::MockSystemTray;
    /// use hush::core::traits::{SystemTray, TrayMenuItem};
    ///
    /// # tokio_test::block_on(async {
    /// let tray = MockSystemTray::new();
    ///
    /// // Simulate a menu click
    /// tray.simulate_menu_click(TrayMenuItem::Quit);
    ///
    /// // Events can be received from the receiver
    /// // Note: In real usage, this would be done in a separate task
    /// # })
    /// ```
    fn event_receiver(&self) -> &mpsc::UnboundedReceiver<TrayEvent>;
}

/// Transcription history storage
#[async_trait]
pub trait HistoryStore: Send + Sync {
    /// Add a new transcription entry
    ///
    /// # Example
    ///
    /// ```
    /// use hush::tray::mocks::MockHistoryStore;
    /// use hush::core::traits::{HistoryStore, TranscriptionEntry};
    /// use std::time::{SystemTime, Duration};
    ///
    /// # tokio_test::block_on(async {
    /// let mut store = MockHistoryStore::new();
    ///
    /// let entry = TranscriptionEntry {
    ///     id: "test_1".to_string(),
    ///     timestamp: SystemTime::now(),
    ///     text: "Hello world".to_string(),
    ///     confidence: 0.95,
    ///     duration: Duration::from_secs(2),
    /// };
    ///
    /// store.add_entry(entry).await?;
    /// assert_eq!(store.len(), 1);
    /// # Ok::<(), anyhow::Error>(())
    /// # })
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Storage backend unavailable
    /// - Entry serialization fails
    async fn add_entry(&mut self, entry: TranscriptionEntry) -> Result<()>;

    /// Get recent transcription entries (newest first)
    ///
    /// # Example
    ///
    /// ```
    /// use hush::tray::mocks::MockHistoryStore;
    /// use hush::core::traits::HistoryStore;
    ///
    /// # tokio_test::block_on(async {
    /// let mut store = MockHistoryStore::new();
    /// store.add_sample_entries(5);
    ///
    /// let recent = store.get_recent(3).await?;
    /// assert_eq!(recent.len(), 3);
    /// # Ok::<(), anyhow::Error>(())
    /// # })
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Storage backend unavailable
    /// - Entry deserialization fails
    async fn get_recent(&self, limit: usize) -> Result<Vec<TranscriptionEntry>>;

    /// Clear all history
    ///
    /// # Example
    ///
    /// ```
    /// use hush::tray::mocks::MockHistoryStore;
    /// use hush::core::traits::HistoryStore;
    ///
    /// # tokio_test::block_on(async {
    /// let mut store = MockHistoryStore::new();
    /// store.add_sample_entries(10);
    /// assert_eq!(store.len(), 10);
    ///
    /// store.clear().await?;
    /// assert_eq!(store.len(), 0);
    /// # Ok::<(), anyhow::Error>(())
    /// # })
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Storage backend unavailable
    /// - Clear operation fails
    async fn clear(&mut self) -> Result<()>;

    /// Get total number of stored entries
    ///
    /// # Example
    ///
    /// ```
    /// use hush::tray::mocks::MockHistoryStore;
    /// use hush::core::traits::HistoryStore;
    ///
    /// let mut store = MockHistoryStore::new();
    /// assert_eq!(store.len(), 0);
    ///
    /// store.add_sample_entries(5);
    /// assert_eq!(store.len(), 5);
    /// ```
    fn len(&self) -> usize;

    /// Check if history is empty
    ///
    /// # Example
    ///
    /// ```
    /// use hush::tray::mocks::MockHistoryStore;
    /// use hush::core::traits::HistoryStore;
    ///
    /// let mut store = MockHistoryStore::new();
    /// assert!(store.is_empty());
    ///
    /// store.add_sample_entries(1);
    /// assert!(!store.is_empty());
    /// ```
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Notification urgency level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    Info,
    Warning,
    Error,
}

/// Desktop notification provider
#[async_trait]
pub trait NotificationProvider: Send + Sync {
    /// Show a notification for completed transcription
    ///
    /// # Example
    ///
    /// ```
    /// use hush::tray::mocks::MockNotificationProvider;
    /// use hush::core::traits::{NotificationProvider, TranscriptionResult};
    /// use std::time::Duration;
    ///
    /// # tokio_test::block_on(async {
    /// let provider = MockNotificationProvider::new();
    ///
    /// let result = TranscriptionResult {
    ///     text: "Hello, world!".to_string(),
    ///     confidence: 0.95,
    ///     language: Some("en".to_string()),
    ///     processing_time: Duration::from_millis(500),
    /// };
    ///
    /// provider.show_transcription(&result).await?;
    /// # Ok::<(), anyhow::Error>(())
    /// # })
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Notification system unavailable
    /// - Platform notification fails
    async fn show_transcription(&self, result: &TranscriptionResult) -> Result<()>;

    /// Show a general status notification
    ///
    /// # Example
    ///
    /// ```
    /// use hush::tray::mocks::MockNotificationProvider;
    /// use hush::core::traits::{NotificationProvider, NotificationLevel};
    ///
    /// # tokio_test::block_on(async {
    /// let provider = MockNotificationProvider::new();
    ///
    /// provider.show_status("Recording started", NotificationLevel::Info).await?;
    /// provider.show_status("Model loading...", NotificationLevel::Warning).await?;
    /// # Ok::<(), anyhow::Error>(())
    /// # })
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Notification system unavailable
    /// - Platform notification fails
    async fn show_status(&self, message: &str, level: NotificationLevel) -> Result<()>;

    /// Check if notifications are enabled
    ///
    /// # Example
    ///
    /// ```
    /// use hush::tray::mocks::MockNotificationProvider;
    /// use hush::core::traits::NotificationProvider;
    ///
    /// let provider = MockNotificationProvider::new();
    /// assert!(provider.is_enabled());
    /// ```
    fn is_enabled(&self) -> bool;

    /// Enable or disable notifications
    ///
    /// # Example
    ///
    /// ```
    /// use hush::tray::mocks::MockNotificationProvider;
    /// use hush::core::traits::NotificationProvider;
    ///
    /// let mut provider = MockNotificationProvider::new();
    /// assert!(provider.is_enabled());
    ///
    /// provider.set_enabled(false);
    /// assert!(!provider.is_enabled());
    ///
    /// provider.set_enabled(true);
    /// assert!(provider.is_enabled());
    /// ```
    fn set_enabled(&mut self, enabled: bool);
}
