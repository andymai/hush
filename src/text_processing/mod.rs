/// Text processing module for intelligent auto-editing
///
/// This module provides post-processing of raw transcriptions to produce
/// polished, professional text suitable for direct insertion.
///
/// Features:
/// - Filler word removal (um, uh, like, you know, etc.)
/// - Grammar correction
/// - Punctuation normalization
/// - Capitalization fixes
/// - Optional LLM-based polishing

mod filler_words;
mod llm;
mod config;

pub use config::{ProcessingConfig, EditingMode};
pub use filler_words::FillerWordRemover;
pub use llm::LlmProcessor;

use anyhow::Result;
use tracing::{debug, info};

/// Main text processor that combines rule-based and LLM processing
pub struct TextProcessor {
    filler_remover: FillerWordRemover,
    llm_processor: Option<LlmProcessor>,
    config: ProcessingConfig,
}

impl TextProcessor {
    /// Create a new text processor
    pub fn new(config: ProcessingConfig) -> Result<Self> {
        info!("Initializing text processor with mode: {:?}", config.mode);

        let filler_remover = FillerWordRemover::new();

        let llm_processor = if config.use_llm {
            match LlmProcessor::new(&config.llm_model_path) {
                Ok(processor) => {
                    info!("✅ LLM processor initialized");
                    Some(processor)
                }
                Err(e) => {
                    info!("⚠️  LLM processor not available: {}", e);
                    info!("Will use rule-based processing only");
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            filler_remover,
            llm_processor,
            config,
        })
    }

    /// Process raw transcription text
    pub async fn process(&self, raw_text: &str) -> Result<String> {
        debug!("Processing raw text: '{}'", raw_text);

        // Stage 1: Rule-based cleanup (fast, always applied)
        let cleaned = self.filler_remover.remove(raw_text, self.config.mode);
        debug!("After filler removal: '{}'", cleaned);

        // Stage 2: LLM polishing (optional, slower but higher quality)
        let polished = if let Some(ref llm) = self.llm_processor {
            if self.config.use_llm {
                debug!("Applying LLM polishing...");
                llm.polish(&cleaned, self.config.mode).await?
            } else {
                cleaned
            }
        } else {
            cleaned
        };

        info!("Text processing complete");
        debug!("  Input:  '{}'", raw_text);
        debug!("  Output: '{}'", polished);

        Ok(polished)
    }

    /// Check if LLM is available and ready
    pub fn is_llm_ready(&self) -> bool {
        self.llm_processor.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_processing() {
        let config = ProcessingConfig {
            mode: EditingMode::Medium,
            use_llm: false,
            llm_model_path: Default::default(),
        };

        let processor = TextProcessor::new(config).unwrap();

        let raw = "um so like I think um you know we should uh do this";
        let result = processor.process(raw).await.unwrap();

        // Should remove filler words
        assert!(!result.contains("um"));
        assert!(!result.contains("uh"));
        assert!(!result.contains("like"));
    }
}
