/// LLM-based text polishing using llama.cpp
///
/// NOTE: This is a placeholder implementation. Full LLM integration
/// will be added in a future update after API stabilization.
use anyhow::Result;
use std::path::Path;
use tracing::info;
use super::config::EditingMode;

/// LLM processor for grammar correction and polishing
pub struct LlmProcessor {
    _model_path: std::path::PathBuf,
}

impl LlmProcessor {
    /// Initialize the LLM processor
    pub fn new(model_path: &Path) -> Result<Self> {
        info!("Loading LLM model from: {}", model_path.display());

        if !model_path.exists() {
            anyhow::bail!("Model file not found: {}", model_path.display());
        }

        // TODO: Implement full llama.cpp integration
        // For now, return an error to indicate LLM is not yet available
        anyhow::bail!("LLM integration not yet implemented - use rule-based processing only")
    }

    /// Polish text using LLM
    ///
    /// This is a placeholder that will be implemented in a future update
    #[allow(dead_code)]
    pub async fn polish(&self, text: &str, _mode: EditingMode) -> Result<String> {
        // For now, just return the input unchanged
        // In the future, this will call the LLM to polish the text
        Ok(text.to_string())
    }

}

// Tests removed - will be re-added when LLM integration is complete
