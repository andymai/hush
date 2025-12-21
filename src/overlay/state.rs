use std::time::{Duration, Instant};

/// The current state of the overlay
#[derive(Debug, Clone, PartialEq, Default)]
pub enum OverlayState {
    /// Idle state - showing button or waiting for hotkey
    #[default]
    Idle,
    /// Recording audio - capturing voice input
    Recording {
        start_time: Instant,
        /// Current audio amplitude (0.0 to 1.0)
        amplitude: f32,
        /// Smoothed amplitude for animation (with decay)
        smoothed_amplitude: f32,
    },
    /// Processing - transcribing the audio
    Processing { message: String },
    /// Success - text was inserted successfully
    Success { text: String, show_until: Instant },
    /// Error - something went wrong
    Error {
        message: String,
        show_until: Instant,
    },
    /// Settings panel
    Settings,
}

impl OverlayState {
    /// Create a new idle state
    pub fn idle() -> Self {
        Self::Idle
    }

    /// Start recording
    pub fn start_recording() -> Self {
        Self::Recording {
            start_time: Instant::now(),
            amplitude: 0.0,
            smoothed_amplitude: 0.0,
        }
    }

    /// Update recording amplitude (immutable - creates new state)
    pub fn with_amplitude(self, new_amplitude: f32) -> Self {
        match self {
            Self::Recording {
                start_time,
                smoothed_amplitude,
                ..
            } => Self::Recording {
                start_time,
                amplitude: new_amplitude.clamp(0.0, 1.0),
                smoothed_amplitude,
            },
            _ => self,
        }
    }

    /// Update recording amplitude in-place (more efficient - avoids clone)
    /// Also updates smoothed_amplitude with attack/decay for smooth animations
    pub fn update_amplitude(&mut self, new_amplitude: f32) {
        if let Self::Recording {
            amplitude,
            smoothed_amplitude,
            ..
        } = self
        {
            let clamped = new_amplitude.clamp(0.0, 1.0);
            *amplitude = clamped;

            // Apply asymmetric smoothing: fast attack, slow decay
            // Attack factor ~0.4 (responds quickly to increases)
            // Decay factor ~0.85 (falls off slowly for smooth animation)
            if clamped > *smoothed_amplitude {
                // Fast attack
                *smoothed_amplitude = *smoothed_amplitude * 0.6 + clamped * 0.4;
            } else {
                // Slow decay
                *smoothed_amplitude = *smoothed_amplitude * 0.85 + clamped * 0.15;
            }
        }
    }

    /// Get recording duration if in recording state
    pub fn recording_duration(&self) -> Option<Duration> {
        match self {
            Self::Recording { start_time, .. } => Some(start_time.elapsed()),
            _ => None,
        }
    }

    /// Get smoothed amplitude for animation (if recording)
    pub fn smoothed_amplitude(&self) -> Option<f32> {
        match self {
            Self::Recording {
                smoothed_amplitude, ..
            } => Some(*smoothed_amplitude),
            _ => None,
        }
    }

    /// Transition to processing state
    pub fn processing(message: impl Into<String>) -> Self {
        Self::Processing {
            message: message.into(),
        }
    }

    /// Transition to success state with auto-hide
    pub fn success(text: impl Into<String>, auto_hide_after: Duration) -> Self {
        Self::Success {
            text: text.into(),
            show_until: Instant::now() + auto_hide_after,
        }
    }

    /// Transition to error state with auto-hide
    pub fn error(message: impl Into<String>, auto_hide_after: Duration) -> Self {
        Self::Error {
            message: message.into(),
            show_until: Instant::now() + auto_hide_after,
        }
    }

    /// Check if this is a temporary state that should auto-hide
    pub fn should_hide(&self) -> bool {
        match self {
            Self::Success { show_until, .. } | Self::Error { show_until, .. } => {
                Instant::now() >= *show_until
            },
            _ => false,
        }
    }

    /// Check if state is idle
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    /// Check if state is recording
    pub fn is_recording(&self) -> bool {
        matches!(self, Self::Recording { .. })
    }

    /// Check if state is processing
    pub fn is_processing(&self) -> bool {
        matches!(self, Self::Processing { .. })
    }

    /// Check if state is showing settings
    pub fn is_settings(&self) -> bool {
        matches!(self, Self::Settings)
    }

    /// Create a settings state
    pub fn settings() -> Self {
        Self::Settings
    }
}

/// Configuration for the overlay appearance
#[derive(Debug, Clone)]
pub struct OverlayConfig {
    /// Width of the overlay window
    pub width: f32,
    /// Height of the overlay window
    pub height: f32,
    /// Position of the overlay (e.g., "bottom-right")
    pub position: OverlayPosition,
    /// Opacity (0.0 - 1.0)
    pub opacity: f32,
    /// Auto-hide duration for success/error messages
    pub auto_hide_duration: Duration,
    /// Show the button when idle
    pub show_button_when_idle: bool,
    /// Theme
    pub theme: OverlayTheme,
    /// Hotkey combination to display
    pub hotkey: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverlayPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    BottomCenter,
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverlayTheme {
    Dark,
    Light,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            width: 80.0,  // Recording width
            height: 24.0, // Recording height (idle is 60x4, recording is 80x24)
            position: OverlayPosition::BottomCenter,
            opacity: 1.0, // Fully opaque for clean look
            auto_hide_duration: Duration::from_secs(2),
            show_button_when_idle: true,
            theme: OverlayTheme::Dark,
            hotkey: "Ctrl+Shift+Space".to_string(),
        }
    }
}

impl OverlayConfig {
    /// Get window position in screen coordinates
    pub fn get_window_position(&self, screen_width: f32, screen_height: f32) -> (f32, f32) {
        let padding = 20.0; // pixels from edge

        match self.position {
            OverlayPosition::TopLeft => (padding, padding),
            OverlayPosition::TopRight => (screen_width - self.width - padding, padding),
            OverlayPosition::BottomLeft => (padding, screen_height - self.height - padding),
            OverlayPosition::BottomRight => (
                screen_width - self.width - padding,
                screen_height - self.height - padding,
            ),
            OverlayPosition::BottomCenter => (
                (screen_width - self.width) / 2.0,
                screen_height - self.height - padding,
            ),
            OverlayPosition::Center => (
                (screen_width - self.width) / 2.0,
                (screen_height - self.height) / 2.0,
            ),
        }
    }
}
