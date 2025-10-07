/// Builder pattern for HushApp construction
///
/// Provides a fluent API for constructing HushApp with various configurations

use super::HushApp;
use crate::application::hush_app::AppMode;
use crate::core::traits::{AudioSource, Transcriber, TextOutput, InputTrigger};
use crate::Result;

/// Builder for HushApp with fluent API
pub struct HushAppBuilder {
    audio: Option<Box<dyn AudioSource>>,
    transcriber: Option<Box<dyn Transcriber>>,
    text_output: Option<Box<dyn TextOutput>>,
    input_trigger: Option<Box<dyn InputTrigger>>,
    mode: AppMode,
    notifications_enabled: bool,
}

impl HushAppBuilder {
    /// Create a new builder with default settings
    pub fn new() -> Self {
        Self {
            audio: None,
            transcriber: None,
            text_output: None,
            input_trigger: None,
            mode: AppMode::Daemon,
            notifications_enabled: true,
        }
    }

    /// Set the audio source
    pub fn with_audio(mut self, audio: Box<dyn AudioSource>) -> Self {
        self.audio = Some(audio);
        self
    }

    /// Set the transcriber
    pub fn with_transcriber(mut self, transcriber: Box<dyn Transcriber>) -> Self {
        self.transcriber = Some(transcriber);
        self
    }

    /// Set the text output
    pub fn with_text_output(mut self, text_output: Box<dyn TextOutput>) -> Self {
        self.text_output = Some(text_output);
        self
    }

    /// Set the input trigger
    pub fn with_input_trigger(mut self, input_trigger: Box<dyn InputTrigger>) -> Self {
        self.input_trigger = Some(input_trigger);
        self
    }

    /// Set application mode
    pub fn with_mode(mut self, mode: AppMode) -> Self {
        self.mode = mode;
        self
    }

    /// Enable or disable notifications
    pub fn with_notifications(mut self, enabled: bool) -> Self {
        self.notifications_enabled = enabled;
        self
    }

    /// Set daemon mode
    pub fn daemon_mode(self) -> Self {
        self.with_mode(AppMode::Daemon)
    }

    /// Set one-shot mode
    pub fn oneshot_mode(self, duration_secs: u64, print_only: bool) -> Self {
        self.with_mode(AppMode::OneShot { duration_secs, print_only })
    }

    /// Set manual mode
    pub fn manual_mode(self) -> Self {
        self.with_mode(AppMode::Manual)
    }

    /// Build the HushApp
    ///
    /// Returns an error if required components are not set
    pub fn build(self) -> Result<HushApp> {
        let audio = self.audio.ok_or_else(|| anyhow::anyhow!("Audio source not set"))?;
        let transcriber = self.transcriber.ok_or_else(|| anyhow::anyhow!("Transcriber not set"))?;
        let text_output = self.text_output.ok_or_else(|| anyhow::anyhow!("Text output not set"))?;
        let input_trigger = self.input_trigger.ok_or_else(|| anyhow::anyhow!("Input trigger not set"))?;

        Ok(HushApp::new(
            audio,
            transcriber,
            text_output,
            input_trigger,
            self.mode,
            self.notifications_enabled,
        ))
    }
}

impl Default for HushAppBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::mocks::{MockAudioSource, MockTranscriber, MockTextOutput, MockInputTrigger};

    #[test]
    fn test_builder_basic() {
        let app = HushAppBuilder::new()
            .with_audio(Box::new(MockAudioSource::new()))
            .with_transcriber(Box::new(MockTranscriber::new()))
            .with_text_output(Box::new(MockTextOutput::new()))
            .with_input_trigger(Box::new(MockInputTrigger::new()))
            .daemon_mode()
            .with_notifications(false)
            .build()
            .unwrap();

        assert!(!app.is_recording());
    }

    #[test]
    fn test_builder_missing_component() {
        let result = HushAppBuilder::new()
            .with_audio(Box::new(MockAudioSource::new()))
            // Missing other components
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_builder_modes() {
        // Daemon mode
        let app = HushAppBuilder::new()
            .with_audio(Box::new(MockAudioSource::new()))
            .with_transcriber(Box::new(MockTranscriber::new()))
            .with_text_output(Box::new(MockTextOutput::new()))
            .with_input_trigger(Box::new(MockInputTrigger::new()))
            .daemon_mode()
            .build()
            .unwrap();

        assert!(matches!(app.mode, AppMode::Daemon));

        // One-shot mode
        let app = HushAppBuilder::new()
            .with_audio(Box::new(MockAudioSource::new()))
            .with_transcriber(Box::new(MockTranscriber::new()))
            .with_text_output(Box::new(MockTextOutput::new()))
            .with_input_trigger(Box::new(MockInputTrigger::new()))
            .oneshot_mode(10, false)
            .build()
            .unwrap();

        assert!(matches!(app.mode, AppMode::OneShot { .. }));

        // Manual mode
        let app = HushAppBuilder::new()
            .with_audio(Box::new(MockAudioSource::new()))
            .with_transcriber(Box::new(MockTranscriber::new()))
            .with_text_output(Box::new(MockTextOutput::new()))
            .with_input_trigger(Box::new(MockInputTrigger::new()))
            .manual_mode()
            .build()
            .unwrap();

        assert!(matches!(app.mode, AppMode::Manual));
    }
}
