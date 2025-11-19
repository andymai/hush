/// Hotkey input trigger adapters
mod hotkey_adapter;
mod macos;

pub use hotkey_adapter::HotkeyTriggerAdapter;
pub use macos::{ensure_main_thread, is_main_thread, log_thread_info};
