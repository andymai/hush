/// Desktop notification provider using notify-rust
///
/// Implements NotificationProvider trait for Linux desktop notifications.

use crate::Result;
use crate::core::traits::{NotificationProvider, NotificationLevel, TranscriptionResult};
use async_trait::async_trait;
use notify_rust::Notification;

#[cfg(target_os = "linux")]
use notify_rust::Urgency;

/// Desktop notification provider using libnotify
pub struct LibnotifyProvider {
    app_name: String,
    enabled: bool,
    timeout_ms: u32,
}

impl LibnotifyProvider {
    /// Create new libnotify provider
    pub fn new(app_name: impl Into<String>) -> Self {
        Self {
            app_name: app_name.into(),
            enabled: true,
            timeout_ms: 3000,
        }
    }
    
    /// Create provider with custom timeout
    pub fn with_timeout(app_name: impl Into<String>, timeout_ms: u32) -> Self {
        Self {
            app_name: app_name.into(),
            enabled: true,
            timeout_ms,
        }
    }
    
    /// Truncate text for notification preview
    fn truncate_text(text: &str, max_length: usize) -> String {
        if text.len() <= max_length {
            text.to_string()
        } else {
            let truncated = &text[..max_length.saturating_sub(3)];
            // Try to break at word boundary
            if let Some(last_space) = truncated.rfind(' ') {
                format!("{}...", &truncated[..last_space])
            } else {
                format!("{}...", truncated)
            }
        }
    }
}

#[async_trait]
impl NotificationProvider for LibnotifyProvider {
    async fn show_transcription(&self, result: &TranscriptionResult) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        
        let preview = Self::truncate_text(&result.text, 60);
        let confidence_percent = (result.confidence * 100.0) as u8;
        
        let body = if result.text.len() > 60 {
            format!("{} ({}% confidence)", preview, confidence_percent)
        } else {
            format!("{} ({}% confidence)", result.text, confidence_percent)
        };

        #[cfg(target_os = "linux")]
        let urgency = if result.confidence < 0.7 {
            Urgency::Normal
        } else {
            Urgency::Low
        };

        let mut notification = Notification::new();
        notification
            .summary("🎙️ Transcription Complete")
            .body(&body)
            .icon("microphone-sensitivity-high")
            .timeout(self.timeout_ms as i32);

        #[cfg(target_os = "linux")]
        notification.urgency(urgency);

        notification.show()
            .map_err(|e| anyhow::anyhow!("Failed to show transcription notification: {}", e))?;
        
        Ok(())
    }
    
    async fn show_status(&self, message: &str, level: NotificationLevel) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        #[cfg(target_os = "linux")]
        let (icon, urgency, prefix) = match level {
            NotificationLevel::Info => ("dialog-information", Urgency::Normal, "ℹ️"),
            NotificationLevel::Warning => ("dialog-warning", Urgency::Normal, "⚠️"),
            NotificationLevel::Error => ("dialog-error", Urgency::Critical, "❌"),
        };

        #[cfg(not(target_os = "linux"))]
        let (icon, prefix) = match level {
            NotificationLevel::Info => ("dialog-information", "ℹ️"),
            NotificationLevel::Warning => ("dialog-warning", "⚠️"),
            NotificationLevel::Error => ("dialog-error", "❌"),
        };

        let timeout = match level {
            NotificationLevel::Error => self.timeout_ms * 2, // Show errors longer
            _ => self.timeout_ms,
        };

        let mut notification = Notification::new();
        notification
            .summary(&format!("{} {}", prefix, self.app_name))
            .body(message)
            .icon(icon)
            .timeout(timeout as i32);

        #[cfg(target_os = "linux")]
        notification.urgency(urgency);

        notification.show()
            .map_err(|e| anyhow::anyhow!("Failed to show status notification: {}", e))?;

        Ok(())
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}