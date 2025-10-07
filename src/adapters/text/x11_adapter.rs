/// Adapter for TextInserter to implement TextOutput trait

use crate::text::TextInserter;
use crate::core::traits::{TextOutput, WindowInfo};
use crate::Result;
use async_trait::async_trait;
use parking_lot::Mutex;
use std::sync::Arc;

/// Adapter that wraps TextInserter to implement TextOutput trait
/// Uses Arc<Mutex<>> for thread safety since Enigo is not Send + Sync
pub struct X11TextAdapter {
    inner: Arc<Mutex<TextInserter>>,
}

impl X11TextAdapter {
    /// Create new adapter wrapping a TextInserter instance
    pub fn new() -> Result<Self> {
        let inner = TextInserter::new()?;
        Ok(Self { 
            inner: Arc::new(Mutex::new(inner))
        })
    }

    /// Get cloned Arc to inner TextInserter (for migration period)
    pub fn inner_arc(&self) -> Arc<Mutex<TextInserter>> {
        Arc::clone(&self.inner)
    }
}

#[async_trait]
impl TextOutput for X11TextAdapter {
    async fn insert_text(&mut self, text: &str) -> Result<()> {
        // TextInserter's insert_text is synchronous
        self.inner.lock().insert_text(text)
    }

    async fn focused_window(&self) -> Result<Option<WindowInfo>> {
        // Get focused window info
        match self.inner.lock().get_focused_window() {
            Ok(window) => {
                let class = window.class.clone();
                Ok(Some(WindowInfo {
                    title: window.title,
                    class: class.clone(),
                    app_name: class, // Use class as app_name for now
                }))
            }
            Err(_) => Ok(None),
        }
    }

    fn is_available(&self) -> bool {
        // X11 is available if we successfully created the adapter
        true
    }

    fn output_method(&self) -> &str {
        "X11"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::TextOutput;

    #[test]
    fn test_x11_adapter_creation() {
        // This will only work if X11 is available
        if let Ok(adapter) = X11TextAdapter::new() {
            assert_eq!(adapter.output_method(), "X11");
            assert!(adapter.is_available());
        }
    }

    #[test]
    fn test_adapter_implements_trait() {
        // Verify the adapter compiles as trait object
        let _can_box: Result<Box<dyn TextOutput>> = X11TextAdapter::new()
            .map(|adapter| Box::new(adapter) as Box<dyn TextOutput>);
    }
}
