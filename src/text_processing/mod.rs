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
mod commands;
mod executor;
mod history;

pub use config::{ProcessingConfig, EditingMode, LlmProvider};
pub use filler_words::FillerWordRemover;
pub use llm::LlmProcessor;
pub use commands::{VoiceCommand, ParsedCommand, CommandParser};
pub use executor::{CommandExecutor, ExecutionResult};
pub use history::{InsertionHistory, HistoryEntry};

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

        let llm_processor = match &config.llm_provider {
            LlmProvider::None => {
                info!("LLM processing disabled - using rule-based only");
                None
            }
            provider => {
                match LlmProcessor::new(provider.clone(), config.max_tokens, config.temperature) {
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
            }
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
            debug!("Applying LLM polishing...");
            llm.polish(&cleaned, self.config.mode).await?
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
