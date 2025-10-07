pub mod dashboard;
pub mod mode_selector;
pub mod audio_device_selector;
pub mod model_selector;
pub mod config_file_selector;
pub mod hotkey_configurator;
pub mod recording_interface;
pub mod config_editor;

pub use dashboard::Dashboard;

use ratatui::{prelude::*, widgets::*};

/// Common UI helpers and utilities for components
pub struct ComponentHelpers;

impl ComponentHelpers {
    /// Create a bordered block with title
    pub fn create_block(title: &str, focused: bool) -> Block {
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(if focused {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            })
    }

    /// Get status color based on condition
    pub fn status_color(is_active: bool, has_error: bool) -> Color {
        if has_error {
            Color::Red
        } else if is_active {
            Color::Green
        } else {
            Color::Gray
        }
    }

}