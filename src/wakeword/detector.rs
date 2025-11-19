use crate::Result;
use std::path::Path;

pub struct WakeWordDetector {
    // Will be implemented in future phase
}

impl WakeWordDetector {
    pub fn new(_model_path: &Path, _wake_phrase: &str, _sensitivity: f32) -> Result<Self> {
        todo!("Implement in Phase 3: Wake word detection")
    }

    pub fn process_audio(&mut self, _audio: &[f32]) -> bool {
        todo!("Implement in Phase 3")
    }
}
