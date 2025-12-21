use crate::core::state::{AppState, StateMachine};
/// Refactored HushApp using trait-based dependency injection
///
/// This is the new implementation that accepts trait objects instead of
/// concrete types, enabling testing, mocking, and extensibility.
use crate::core::traits::{AudioSource, InputTrigger, TextOutput, Transcriber, TriggerEvent};
use crate::Result;
use anyhow::Context;
use std::time::Instant;
use tracing::{error, info, warn};

/// Main application orchestrator (trait-based version)
pub struct HushApp {
    // Trait objects enable dependency injection ✅
    audio: Box<dyn AudioSource>,
    transcriber: Box<dyn Transcriber>,
    text_output: Box<dyn TextOutput>,
    input_trigger: Box<dyn InputTrigger>,

    // Centralized state machine
    state: StateMachine,

    // Configuration
    notifications_enabled: bool,

    // Mode
    mode: AppMode,
}

#[derive(Debug, Clone)]
pub enum AppMode {
    Daemon,
    OneShot {
        duration_secs: u64,
        print_only: bool,
    },
    Manual,
}

impl HushApp {
    /// Create new HushApp with dependency injection
    ///
    /// This constructor accepts trait objects, enabling:
    /// - Mock implementations for testing
    /// - Runtime component swapping
    /// - Platform-specific implementations
    pub fn new(
        audio: Box<dyn AudioSource>,
        transcriber: Box<dyn Transcriber>,
        text_output: Box<dyn TextOutput>,
        input_trigger: Box<dyn InputTrigger>,
        mode: AppMode,
        notifications_enabled: bool,
    ) -> Self {
        info!("🤫 Initializing Hush Application (trait-based architecture)");
        info!("  Audio: {}", audio.device_name());
        info!("  Transcriber: {}", transcriber.info().name);
        info!("  Text Output: {}", text_output.output_method());
        info!("  Input Trigger: {}", input_trigger.description());
        info!("  Mode: {:?}", mode);

        let state = StateMachine::new();

        Self {
            audio,
            transcriber,
            text_output,
            input_trigger,
            state,
            notifications_enabled,
            mode,
        }
    }

    /// Run the application based on its mode
    pub async fn run(&mut self) -> Result<()> {
        match &self.mode {
            AppMode::Daemon => self.run_daemon_mode().await,
            AppMode::OneShot {
                duration_secs,
                print_only,
            } => self.run_oneshot_mode(*duration_secs, *print_only).await,
            AppMode::Manual => self.run_manual_mode().await,
        }
    }

    /// Run in daemon mode with input trigger
    async fn run_daemon_mode(&mut self) -> Result<()> {
        info!("🚀 Starting Hush in daemon mode");
        info!("📝 Instructions:");
        info!(
            "   • {} to start recording",
            self.input_trigger.description()
        );
        info!("   • Release to stop and transcribe");
        info!("   • Text will be inserted at cursor");
        info!("   • Press Ctrl+C to quit");

        // Start listening for input trigger
        self.input_trigger
            .start_listening()
            .await
            .context("Failed to start input trigger listener")?;
        info!("🎯 Input trigger active - waiting for events...");

        if self.notifications_enabled {
            self.show_notification(
                "Hush Started",
                "Voice-to-text is active. Use trigger to record.",
                NotificationUrgency::Normal,
            );
        }

        // Main event loop
        loop {
            match self.input_trigger.next_event().await {
                Some(TriggerEvent::StartRecording) => {
                    if let Err(e) = self.handle_recording_start().await {
                        error!("Error starting recording: {:?}", e);
                        self.handle_error(e).await;
                    }
                },
                Some(TriggerEvent::StopRecording) => {
                    if let Err(e) = self.handle_recording_stop().await {
                        error!("Error stopping recording: {:?}", e);
                        self.handle_error(e).await;
                    }
                },
                Some(TriggerEvent::Cancel) => {
                    info!("Recording cancelled");
                    self.state.transition(AppState::Idle)?;
                },
                None => {
                    // Trigger closed, exit
                    info!("Input trigger closed, shutting down");
                    break;
                },
            }
        }

        info!("👋 Hush daemon shutting down");
        Ok(())
    }

    /// Run in one-shot mode (record once and exit)
    async fn run_oneshot_mode(&mut self, duration_secs: u64, print_only: bool) -> Result<()> {
        info!("🎤 Running in one-shot mode ({}s)", duration_secs);

        if self.notifications_enabled {
            self.show_notification(
                "Hush Recording",
                "Recording started. Speak now...",
                NotificationUrgency::Normal,
            );
        }

        // Start recording
        self.state.transition(AppState::Recording {
            started_at: Instant::now(),
        })?;
        self.audio.start_recording()?;
        info!("🎤 Recording started");

        // Record for specified duration
        tokio::time::sleep(tokio::time::Duration::from_secs(duration_secs)).await;

        // Stop and process
        let audio_buffer = self.audio.stop_recording()?;
        let duration = audio_buffer.duration;

        self.state.transition(AppState::Transcribing {
            audio_duration: duration,
        })?;

        if audio_buffer.is_empty() {
            warn!("No audio captured");
            self.state.transition(AppState::Idle)?;
            return Ok(());
        }

        info!("🔄 Transcribing audio ({:.2}s)...", duration.as_secs_f32());
        let transcription_start = Instant::now();
        let result = self.transcriber.transcribe(&audio_buffer).await?;
        let transcription_time = transcription_start.elapsed();

        info!(
            "✅ Transcription completed in {:.2}s",
            transcription_time.as_secs_f32()
        );

        let text = result.text.trim();
        if text.is_empty() {
            info!("🤐 No speech detected");
            self.state.transition(AppState::Idle)?;

            if self.notifications_enabled {
                self.show_notification(
                    "Hush Complete",
                    "No speech detected",
                    NotificationUrgency::Low,
                );
            }
            return Ok(());
        }

        if print_only {
            // Just print to stdout
            println!("{}", text);
        } else {
            // Insert text
            info!("📝 Transcribed: '{}'", text);
            self.state.transition(AppState::Inserting {
                text_length: text.len(),
            })?;

            self.text_output.insert_text(text).await?;
            info!("✅ Text inserted successfully");
        }

        self.state.transition(AppState::Idle)?;

        if self.notifications_enabled {
            self.show_notification(
                "Hush Complete",
                &format!("Transcribed: {}", text),
                NotificationUrgency::Normal,
            );
        }

        Ok(())
    }

    /// Run in manual mode (stdin control)
    async fn run_manual_mode(&mut self) -> Result<()> {
        info!("🔧 Running in manual mode");
        info!("Press Enter to start/stop recording, 'q' to quit");

        use tokio::io::{self, AsyncBufReadExt, BufReader};
        let stdin = io::stdin();
        let mut reader = BufReader::new(stdin).lines();

        while let Ok(Some(line)) = reader.next_line().await {
            if line.trim() == "q" {
                break;
            }

            match self.state.current() {
                AppState::Idle => {
                    // Start recording
                    if let Err(e) = self.handle_recording_start().await {
                        error!("Failed to start recording: {:?}", e);
                        continue;
                    }
                    info!("🎤 Recording... Press Enter to stop.");
                },
                AppState::Recording { .. } => {
                    // Stop and transcribe
                    if let Err(e) = self.handle_recording_stop().await {
                        error!("Failed to stop recording: {:?}", e);
                        self.state.transition(AppState::Idle)?;
                        continue;
                    }
                    info!("Press Enter to record again, 'q' to quit.");
                },
                _ => {
                    warn!("Cannot record in current state: {:?}", self.state.current());
                },
            }
        }

        info!("👋 Manual mode ended");
        Ok(())
    }

    /// Handle recording start
    pub(crate) async fn handle_recording_start(&mut self) -> Result<()> {
        if self.state.current().is_recording() {
            warn!("Already recording");
            return Ok(());
        }

        info!("🎙️  Starting recording...");

        // Transition state
        self.state.transition(AppState::Recording {
            started_at: Instant::now(),
        })?;

        // Start audio capture
        self.audio.start_recording().with_context(|| {
            format!(
                "Failed to start recording on device '{}'",
                self.audio.device_name()
            )
        })?;

        // Log target window info if available
        if let Ok(Some(window)) = self.text_output.focused_window().await {
            info!("📝 Target window: '{}' ({})", window.title, window.class);
        }

        info!("✅ Recording started");
        Ok(())
    }

    /// Handle recording stop and full transcription pipeline
    pub(crate) async fn handle_recording_stop(&mut self) -> Result<()> {
        if !self.state.current().is_recording() {
            warn!("Not recording");
            return Ok(());
        }

        let duration = self
            .state
            .current()
            .recording_duration()
            .unwrap_or_default();

        info!(
            "🛑 Stopping recording (duration: {:.2}s)...",
            duration.as_secs_f32()
        );

        // Stop audio capture
        let audio_buffer = self.audio.stop_recording().with_context(|| {
            format!(
                "Failed to stop recording on device '{}'",
                self.audio.device_name()
            )
        })?;

        self.state.transition(AppState::Transcribing {
            audio_duration: audio_buffer.duration,
        })?;

        if audio_buffer.is_empty() {
            warn!("No audio data captured");
            self.state.transition(AppState::Idle)?;
            return Ok(());
        }

        info!(
            "📊 Audio captured: {} samples ({:.2}s)",
            audio_buffer.len(),
            audio_buffer.duration.as_secs_f32()
        );

        // Transcribe
        info!("🔄 Transcribing audio...");
        let transcription_start = Instant::now();
        let result = self
            .transcriber
            .transcribe(&audio_buffer)
            .await
            .with_context(|| {
                format!(
                    "Failed to transcribe {:.2}s of audio using {}",
                    audio_buffer.duration.as_secs_f32(),
                    self.transcriber.info().name
                )
            })?;
        let transcription_time = transcription_start.elapsed();

        info!(
            "✅ Transcription completed in {:.2}s",
            transcription_time.as_secs_f32()
        );

        let text = result.text.trim();
        if text.is_empty() {
            info!("🤐 No speech detected");
            self.state.transition(AppState::Idle)?;
            return Ok(());
        }

        info!("📝 Transcribed text: '{}'", text);
        info!("🎯 Confidence: {:.2}", result.confidence);

        // Insert text
        self.state.transition(AppState::Inserting {
            text_length: text.len(),
        })?;

        info!("⌨️  Inserting text...");
        let insertion_start = Instant::now();
        self.text_output.insert_text(text).await.with_context(|| {
            format!(
                "Failed to insert text ({} chars) using {}",
                text.len(),
                self.text_output.output_method()
            )
        })?;
        let insertion_time = insertion_start.elapsed();

        info!("✅ Text inserted in {:.2}ms", insertion_time.as_millis());

        // Back to idle
        self.state.transition(AppState::Idle)?;

        // Performance metrics
        let total_time = transcription_time + insertion_time;
        info!("⚡ Total processing: {:.2}ms", total_time.as_millis());

        let words = text.split_whitespace().count();
        if words > 0 {
            info!(
                "📈 Performance: {} words, {:.0} chars/sec",
                words,
                text.len() as f32 / transcription_time.as_secs_f32()
            );
        }

        Ok(())
    }

    /// Handle errors with appropriate recovery
    async fn handle_error(&mut self, error: anyhow::Error) {
        error!("Error occurred: {:?}", error);

        // Try to transition to error state
        if let Err(e) = self.state.transition(AppState::Error { recoverable: true }) {
            error!("Failed to transition to error state: {:?}", e);
        }

        // Show notification
        if self.notifications_enabled {
            let message = format!("Error: {}", error);
            self.show_notification("Hush Error", &message, NotificationUrgency::Critical);
        }

        // Try to recover to idle
        if let Err(e) = self.state.transition(AppState::Idle) {
            error!("Failed to recover to idle state: {:?}", e);
        }
    }

    /// Show notification (logs to console)
    fn show_notification(&self, title: &str, message: &str, _urgency: NotificationUrgency) {
        info!("📢 {} - {}", title, message);
    }

    /// Get application statistics
    pub fn get_stats(&self) -> AppStats {
        AppStats {
            state: self.state.current(),
            audio_device: self.audio.device_name().to_string(),
            transcriber_name: self.transcriber.info().name.clone(),
            text_output_method: self.text_output.output_method().to_string(),
            input_trigger_desc: self.input_trigger.description(),
        }
    }

    /// Get current state
    pub fn current_state(&self) -> AppState {
        self.state.current()
    }

    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        self.state.current().is_recording()
    }

    /// Get the application mode
    pub fn mode(&self) -> &AppMode {
        &self.mode
    }
}

/// Application statistics
#[derive(Debug, Clone)]
pub struct AppStats {
    pub state: AppState,
    pub audio_device: String,
    pub transcriber_name: String,
    pub text_output_method: String,
    pub input_trigger_desc: String,
}

/// Notification urgency
#[derive(Debug, Clone, Copy)]
enum NotificationUrgency {
    Low,
    Normal,
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::mocks::{MockAudioSource, MockInputTrigger, MockTextOutput, MockTranscriber};

    fn create_test_app() -> HushApp {
        HushApp::new(
            Box::new(MockAudioSource::new()),
            Box::new(MockTranscriber::new()),
            Box::new(MockTextOutput::new()),
            Box::new(MockInputTrigger::new()),
            AppMode::Manual,
            false,
        )
    }

    #[test]
    fn test_app_initialization() {
        let app = create_test_app();
        assert_eq!(app.current_state(), AppState::Idle);
        assert!(!app.is_recording());
    }

    #[tokio::test]
    async fn test_recording_start_stop() {
        let mut app = create_test_app();

        // Start recording
        app.handle_recording_start().await.unwrap();
        assert!(app.is_recording());

        // Stop recording
        app.handle_recording_stop().await.unwrap();
        assert!(!app.is_recording());
        assert_eq!(app.current_state(), AppState::Idle);
    }

    #[test]
    fn test_get_stats() {
        let app = create_test_app();
        let stats = app.get_stats();

        assert_eq!(stats.audio_device, "Mock Audio Device");
        assert_eq!(stats.transcriber_name, "MockTranscriber");
        assert_eq!(stats.text_output_method, "Mock");
    }
}
