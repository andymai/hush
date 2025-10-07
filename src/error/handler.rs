use thiserror::Error;

#[derive(Error, Debug)]
pub enum HushError {
    #[error("Audio capture failed: {0}")]
    AudioCapture(String),
    
    #[error("Transcription failed: {0}")]
    Transcription(String),
    
    #[error("Text insertion failed: {0}")]
    TextInsertion(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Hotkey error: {0}")]
    Hotkey(String),
}

pub struct ErrorHandler {
    // Will be implemented with proper error handling
}

impl ErrorHandler {
    pub fn new() -> Self {
        todo!("Implement error handler")
    }

    pub fn handle_error(&self, _error: HushError) {
        todo!("Implement error handling with audio feedback")
    }
}