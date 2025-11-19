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
mod entities;
mod context;
mod session;
mod intent;
mod vocabulary;

pub use config::{ProcessingConfig, EditingMode, LlmProvider};
pub use filler_words::FillerWordRemover;
pub use llm::LlmProcessor;
pub use commands::{VoiceCommand, ParsedCommand, CommandParser};
pub use executor::{CommandExecutor, ExecutionResult};
pub use history::{InsertionHistory, HistoryEntry};
pub use entities::EntityRecognizer;
pub use context::{ApplicationContext, ContextDetector, CodeContext, CommunicationType, BrowserContext};
pub use session::{SessionMemory, SessionEntry};
pub use intent::{UserIntent, IntentDetector, CommunicationStyle};
pub use vocabulary::{DomainVocabulary, DomainVocabularies, VocabularyManager};

use anyhow::Result;
use tracing::{debug, info};

/// Main text processor that combines rule-based and LLM processing
pub struct TextProcessor {
    filler_remover: FillerWordRemover,
    entity_recognizer: EntityRecognizer,
    llm_processor: Option<LlmProcessor>,
    context_detector: ContextDetector,
    intent_detector: IntentDetector,
    vocabulary_manager: VocabularyManager,
    session_memory: SessionMemory,
    config: ProcessingConfig,
}

impl TextProcessor {
    /// Create a new text processor
    pub fn new(config: ProcessingConfig) -> Result<Self> {
        info!("Initializing text processor with mode: {:?}", config.mode);

        let filler_remover = FillerWordRemover::new();
        let entity_recognizer = EntityRecognizer::new();
        let context_detector = ContextDetector::new();
        let intent_detector = IntentDetector::new();
        let session_memory = SessionMemory::new();

        // Initialize vocabulary manager with default vocabularies
        let mut vocabulary_manager = VocabularyManager::new();
        vocabulary_manager.add(DomainVocabularies::general_programming());

        // Try to load user-defined vocabulary
        if let Err(e) = vocabulary_manager.load_if_exists(DomainVocabulary::default_path()) {
            debug!("Could not load user vocabulary: {}", e);
        }

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

        info!("✅ Text processor initialized with {} vocabulary entries",
              vocabulary_manager.total_entries());

        Ok(Self {
            filler_remover,
            entity_recognizer,
            llm_processor,
            context_detector,
            intent_detector,
            vocabulary_manager,
            session_memory,
            config,
        })
    }

    /// Process raw transcription text with full context awareness
    pub async fn process(&self, raw_text: &str) -> Result<String> {
        info!("🔄 Processing: '{}'", raw_text);

        // Detect application context and intent
        let app_context = self.context_detector.last_context();
        let intent = self.intent_detector.detect(raw_text);

        debug!("Application context: {:?}", app_context);
        debug!("Detected intent: {:?}", intent);

        // Stage 1: Rule-based cleanup (fast, always applied)
        let cleaned = self.filler_remover.remove(raw_text, self.config.mode);
        debug!("After filler removal: '{}'", cleaned);

        // Stage 2: Apply domain vocabularies
        let with_vocab = self.vocabulary_manager.apply(&cleaned);
        debug!("After vocabulary: '{}'", with_vocab);

        // Stage 3: Smart capitalization for tech terms
        let capitalized = self.entity_recognizer.capitalize(&with_vocab);
        debug!("After entity capitalization: '{}'", capitalized);

        // Stage 4: Context-aware LLM polishing (optional)
        let polished = if let Some(ref llm) = self.llm_processor {
            debug!("Applying context-aware LLM polishing...");

            // Build context from session memory
            let session_context = self.session_memory.build_context(3);

            // Enhance system prompt with context and intent
            let enhanced_text = self.build_context_enhanced_prompt(
                &capitalized,
                app_context,
                &intent,
                session_context.as_deref(),
            );

            llm.polish(&enhanced_text, self.config.mode).await?
        } else {
            capitalized
        };

        info!("✅ Processing complete: '{}'", polished);
        debug!("  Input:  '{}'", raw_text);
        debug!("  Output: '{}'", polished);
        debug!("  Intent: {}", intent.description());

        Ok(polished)
    }

    /// Build context-enhanced prompt for LLM
    fn build_context_enhanced_prompt(
        &self,
        text: &str,
        app_context: Option<&ApplicationContext>,
        intent: &UserIntent,
        session_context: Option<&str>,
    ) -> String {
        let prompt = text.to_string();

        // Add context hints to guide LLM processing
        let mut hints = Vec::new();

        if let Some(context) = app_context {
            hints.push(format!("Context: {}", context.description()));
        }

        hints.push(format!("Intent: {}", intent.description()));

        if let Some(session) = session_context {
            hints.push(format!("Recent: {}", session));
        }

        if !hints.is_empty() {
            debug!("Processing hints: {:?}", hints);
        }

        prompt
    }

    /// Add custom entity for capitalization
    pub fn add_custom_entity(&mut self, lowercase: String, proper: String) {
        self.entity_recognizer.add_custom_entity(lowercase, proper);
    }

    /// Record processed text in session memory
    pub fn record_in_session(&mut self, text: String) {
        let context = self.context_detector.last_context()
            .map(|c| c.description());
        self.session_memory.add(text, context);
    }

    /// Update application context (call periodically or on focus change)
    pub fn update_context(&mut self) {
        self.context_detector.detect();
    }

    /// Get session memory reference
    pub fn session_memory(&self) -> &SessionMemory {
        &self.session_memory
    }

    /// Get current application context
    pub fn current_context(&self) -> Option<&ApplicationContext> {
        self.context_detector.last_context()
    }

    /// Clear session memory
    pub fn clear_session(&mut self) {
        self.session_memory.clear();
    }

    /// Add a custom vocabulary
    pub fn add_vocabulary(&mut self, vocab: DomainVocabulary) {
        self.vocabulary_manager.add(vocab);
    }

    /// Check if LLM is available and ready
    pub fn is_llm_ready(&self) -> bool {
        self.llm_processor.is_some()
    }

    /// Get processing statistics
    pub fn get_stats(&self) -> ProcessingStats {
        ProcessingStats {
            vocabulary_entries: self.vocabulary_manager.total_entries(),
            session_entries: self.session_memory.len(),
            session_active: self.session_memory.is_active(),
            llm_enabled: self.is_llm_ready(),
            current_context: self.context_detector.last_context()
                .map(|c| c.description()),
        }
    }
}

/// Processing statistics
#[derive(Debug, Clone)]
pub struct ProcessingStats {
    pub vocabulary_entries: usize,
    pub session_entries: usize,
    pub session_active: bool,
    pub llm_enabled: bool,
    pub current_context: Option<String>,
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
