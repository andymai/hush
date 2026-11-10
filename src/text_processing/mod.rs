mod commands;
mod config;
mod context;
mod entities;
mod executor;
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
mod history;
pub mod hook;
mod intent;
pub mod llm;
mod session;
mod vocabulary;

pub use commands::{CommandParser, ParsedCommand, VoiceCommand};
pub use config::{EditingMode, LlmProvider, ProcessingConfig};
pub use context::{
    ApplicationContext, BrowserContext, CodeContext, CommunicationType, ContextDetector,
};
pub use entities::EntityRecognizer;
pub use executor::{CommandExecutor, ExecutionResult};
pub use filler_words::FillerWordRemover;
pub use history::{HistoryEntry, InsertionHistory};
pub use hook::{HookEnv, TranscriptHook, Trigger};
pub use intent::{CommunicationStyle, IntentDetector, UserIntent};
pub mod corrections;
pub mod learn;
pub mod profiles;
pub mod prompt;
pub mod styles;
pub mod transcripts;
pub use llm::LlmProcessor;
pub use profiles::{AppKind, ProfilesConfig, TextProfile};
pub use session::{SessionEntry, SessionMemory};
pub use styles::{StyleConfig, StyleName, TakeStyle};
pub use vocabulary::{DomainVocabularies, DomainVocabulary, VocabularyManager};

use anyhow::Result;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Where the text is going: the focused window, the kind of app it is, and
/// the style of the take.
#[derive(Debug, Clone, Default)]
pub struct ProcessingContext {
    pub window: Option<crate::text::WindowInfo>,
    pub kind: AppKind,
    pub style: Option<TakeStyle>,
}

impl ProcessingContext {
    pub fn new(window: Option<crate::text::WindowInfo>, profiles: &ProfilesConfig) -> Self {
        let kind = match &window {
            Some(info) if profiles.enabled => AppKind::classify(info, profiles),
            _ => AppKind::Other,
        };
        Self {
            window,
            kind,
            style: None,
        }
    }

    pub fn with_style(mut self, style: Option<TakeStyle>) -> Self {
        self.style = style;
        self
    }

    /// The style decides whether the LLM polishes the take; without a
    /// style, `llm.polish` decides.
    fn polishes(&self, configured: bool) -> bool {
        self.style.as_ref().map_or(configured, |style| style.polish)
    }
}

/// Main text processor that combines rule-based and LLM processing
pub struct TextProcessor {
    filler_remover: FillerWordRemover,
    entity_recognizer: EntityRecognizer,
    llm_processor: Option<LlmProcessor>,
    context_detector: ContextDetector,
    intent_detector: IntentDetector,
    vocabulary_manager: parking_lot::RwLock<VocabularyManager>,
    user_terms: parking_lot::RwLock<Vec<String>>,
    session_memory: SessionMemory,
    config: ProcessingConfig,
    vocabulary_path: Option<PathBuf>,
}

impl TextProcessor {
    /// `vocabulary_path` is the user's vocabulary file, normally
    /// `DomainVocabulary::default_path()`. `None` uses only the built-in terms.
    pub fn new(config: ProcessingConfig, vocabulary_path: Option<PathBuf>) -> Result<Self> {
        info!("Initializing text processor with mode: {:?}", config.mode);

        let filler_remover = FillerWordRemover::new();
        let entity_recognizer = EntityRecognizer::new();
        let context_detector = ContextDetector::new();
        let intent_detector = IntentDetector::new();
        let session_memory = SessionMemory::new();

        let (vocabulary_manager, user_terms) = Self::build_vocabulary(vocabulary_path.as_deref());

        let llm_processor = match &config.llm_provider {
            LlmProvider::None => {
                info!("LLM processing disabled - using rule-based only");
                None
            },
            provider => {
                match LlmProcessor::new(provider.clone(), config.max_tokens, config.temperature) {
                    Ok(processor) => {
                        info!("✅ LLM processor initialized");
                        Some(processor)
                    },
                    Err(e) => {
                        info!("⚠️  LLM processor not available: {}", e);
                        info!("Will use rule-based processing only");
                        None
                    },
                }
            },
        };

        info!(
            "✅ Text processor initialized with {} vocabulary entries",
            vocabulary_manager.total_entries()
        );

        Ok(Self {
            filler_remover,
            entity_recognizer,
            llm_processor,
            context_detector,
            intent_detector,
            vocabulary_manager: parking_lot::RwLock::new(vocabulary_manager),
            user_terms: parking_lot::RwLock::new(user_terms),
            session_memory,
            config,
            vocabulary_path,
        })
    }

    /// The built-in programming vocabulary plus the user's vocabulary file,
    /// and the user's own terms for the transcription prompt.
    fn build_vocabulary(path: Option<&Path>) -> (VocabularyManager, Vec<String>) {
        let mut manager = VocabularyManager::new();
        manager.add(DomainVocabularies::general_programming());
        let mut terms = Vec::new();
        if let Some(path) = path.filter(|path| path.exists()) {
            match DomainVocabulary::load_from_file(path) {
                Ok(vocab) => {
                    let mut values: Vec<String> = vocab.technical_terms.values().cloned().collect();
                    values.sort();
                    terms = values;
                    manager.add(vocab);
                },
                Err(e) => warn!("Could not load the vocabulary file: {}", e),
            }
        }
        (manager, terms)
    }

    /// Re-read the vocabulary file after `hush learn`; returns the entry count.
    pub fn reload_vocabulary(&self) -> usize {
        let (manager, terms) = Self::build_vocabulary(self.vocabulary_path.as_deref());
        let total = manager.total_entries();
        *self.vocabulary_manager.write() = manager;
        *self.user_terms.write() = terms;
        info!("Vocabulary reloaded: {} entries", total);
        total
    }

    /// The user's learned terms, for Whisper's initial prompt.
    pub fn user_terms(&self) -> Vec<String> {
        self.user_terms.read().clone()
    }

    /// Process raw transcription text for a generic application.
    pub async fn process(&self, raw_text: &str) -> Result<String> {
        self.process_for(raw_text, &ProcessingContext::default())
            .await
    }

    /// Process raw transcription text the way the focused application wants it.
    pub async fn process_for(&self, raw_text: &str, context: &ProcessingContext) -> Result<String> {
        info!(
            "🔄 Processing {} chars ({})",
            raw_text.chars().count(),
            context.kind.label()
        );
        debug!("Processing: '{}'", raw_text);
        let profile = context.kind.profile();

        // Detect application context and intent
        let app_context = self.context_detector.last_context();
        let intent = self.intent_detector.detect(raw_text);

        debug!("Application context: {:?}", app_context);
        debug!("Detected intent: {:?}", intent);

        // Stage 1: Rule-based cleanup (fast, always applied)
        let cleaned = self.filler_remover.remove_with(
            raw_text,
            self.config.mode,
            profile.capitalize_first,
            profile.ending_punctuation,
        );
        debug!("After filler removal: '{}'", cleaned);

        // Stage 2: Apply domain vocabularies
        let with_vocab = self.vocabulary_manager.read().apply(&cleaned);
        debug!("After vocabulary: '{}'", with_vocab);

        // Stage 3: Smart capitalization for tech terms
        let capitalized = self.entity_recognizer.capitalize(&with_vocab);
        debug!("After entity capitalization: '{}'", capitalized);

        // Stage 4: Context-aware LLM polishing (optional)
        let polish = context.polishes(self.config.polish);
        let polished = if let Some(llm) = self.llm_processor.as_ref().filter(|_| polish) {
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

            let place = context.window.as_ref().map(|w| {
                format!(
                    "The text goes into {} (window title: {}).",
                    context.kind.label(),
                    w.title.chars().take(80).collect::<String>()
                )
            });
            let style = context
                .style
                .as_ref()
                .map(|style| style.prompt.as_str())
                .filter(|prompt| !prompt.is_empty());
            match llm
                .polish_with_context(
                    &enhanced_text,
                    self.config.mode,
                    Some(profile.tone),
                    style,
                    place.as_deref(),
                )
                .await
            {
                Ok(output) => match llm::reject_polish(&capitalized, &output) {
                    Some(reason) => {
                        info!("Polish discarded ({}), using the unpolished text", reason);
                        capitalized
                    },
                    None => output,
                },
                Err(e) => {
                    warn!("Polish failed ({:#}), using the unpolished text", e);
                    capitalized
                },
            }
        } else {
            capitalized
        };

        info!(
            "✅ Processing complete ({} chars)",
            polished.chars().count()
        );
        debug!("Processing result: '{}'", polished);
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
        let context = self
            .context_detector
            .last_context()
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
        self.vocabulary_manager.write().add(vocab);
    }

    /// Command Mode: apply a spoken instruction to some text with the LLM.
    pub async fn rewrite(&self, instruction: &str, target: &str) -> Result<String> {
        let llm = self
            .llm_processor
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No LLM is configured"))?;
        llm.rewrite(instruction, target).await
    }

    pub fn llm_name(&self) -> Option<String> {
        self.llm_processor.as_ref().map(|llm| llm.provider_name())
    }

    /// Check if LLM is available and ready
    pub fn is_llm_ready(&self) -> bool {
        self.llm_processor.is_some()
    }

    /// Get processing statistics
    pub fn get_stats(&self) -> ProcessingStats {
        ProcessingStats {
            vocabulary_entries: self.vocabulary_manager.read().total_entries(),
            session_entries: self.session_memory.len(),
            session_active: self.session_memory.is_active(),
            llm_enabled: self.is_llm_ready(),
            current_context: self
                .context_detector
                .last_context()
                .map(|c| c.description()),
        }
    }
}

/// What a style rewrite of the selection comes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RewriteOutcome {
    /// Type this text over the selection
    Replace(String),
    /// Nothing is selected, so no LLM call was made
    NothingSelected,
    /// No LLM is configured
    NoLlm,
    /// The LLM call failed, and the selection stays as it is
    Failed(String),
    /// The LLM gave back nothing or the same text
    Unchanged,
}

/// Rewrite the selected text with the prompt of `style` as the instruction,
/// through the Command Mode rewrite.
pub async fn rewrite_with_style(
    processor: Option<&TextProcessor>,
    style: &TakeStyle,
    selection: Option<&str>,
) -> RewriteOutcome {
    let Some(text) = selection.filter(|text| !text.trim().is_empty()) else {
        return RewriteOutcome::NothingSelected;
    };
    let Some(processor) = processor.filter(|processor| processor.is_llm_ready()) else {
        return RewriteOutcome::NoLlm;
    };
    match processor.rewrite(&style.prompt, text).await {
        Ok(result) if result.trim().is_empty() || result == text => RewriteOutcome::Unchanged,
        Ok(result) => RewriteOutcome::Replace(result),
        Err(e) => RewriteOutcome::Failed(format!("{:#}", e)),
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
            llm_provider: LlmProvider::None,
            polish: true,
            max_tokens: 200,
            temperature: 0.3,
        };

        let processor = TextProcessor::new(config, None).unwrap();

        let raw = "um so like I think um you know we should uh do this";
        let result = processor.process(raw).await.unwrap();

        // Should remove filler words
        assert!(!result.contains("um"));
        assert!(!result.contains("uh"));
        assert!(!result.contains("like"));
    }

    fn processor_with(provider: LlmProvider) -> TextProcessor {
        TextProcessor::new(
            ProcessingConfig {
                mode: EditingMode::Medium,
                llm_provider: provider,
                polish: true,
                max_tokens: 200,
                temperature: 0.3,
            },
            None,
        )
        .unwrap()
    }

    #[test]
    fn the_vocabulary_comes_from_the_given_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vocabulary.json");
        let config = ProcessingConfig {
            llm_provider: LlmProvider::None,
            ..ProcessingConfig::default()
        };
        let processor = TextProcessor::new(config, Some(path.clone())).unwrap();
        assert!(processor.user_terms().is_empty());

        let mut vocab = DomainVocabulary::new();
        vocab.add_technical_term("hushd".into(), "HushD".into());
        vocab.save_to_file(&path).unwrap();
        processor.reload_vocabulary();
        assert_eq!(processor.user_terms(), vec!["HushD".to_string()]);
    }

    #[test]
    fn no_vocabulary_file_means_no_user_terms() {
        assert!(processor_with(LlmProvider::None).user_terms().is_empty());
    }

    fn ollama_at(url: String) -> LlmProvider {
        LlmProvider::Ollama {
            url,
            model: "test".into(),
        }
    }

    async fn fake_ollama(reply: &'static str) -> String {
        fake_ollama_request(reply).await.0
    }

    /// The server's URL, and the request it gets, as text.
    async fn fake_ollama_request(reply: &'static str) -> (String, tokio::task::JoinHandle<String>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let request = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let read = socket.read(&mut chunk).await.unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
                let text = String::from_utf8_lossy(&request).to_lowercase();
                if let Some(end) = text.find("\r\n\r\n") {
                    let length = text[..end]
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length:"))
                        .and_then(|value| value.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let body = serde_json::json!({ "message": { "content": reply } }).to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            String::from_utf8_lossy(&request).into_owned()
        });
        (url, request)
    }

    #[tokio::test]
    async fn llm_error_types_the_unpolished_text() {
        let raw = "um we should not deploy to github on friday";
        let unpolished = processor_with(LlmProvider::None)
            .process(raw)
            .await
            .unwrap();
        let unreachable = processor_with(ollama_at("http://127.0.0.1:1".into()));
        assert_eq!(unreachable.process(raw).await.unwrap(), unpolished);
    }

    #[tokio::test]
    async fn polish_that_drops_a_negation_types_the_unpolished_text() {
        let raw = "we should not ship it";
        let unpolished = processor_with(LlmProvider::None)
            .process(raw)
            .await
            .unwrap();
        let url = fake_ollama("We should ship it.").await;
        let result = processor_with(ollama_at(url)).process(raw).await.unwrap();
        assert_eq!(result, unpolished);
    }

    #[tokio::test]
    async fn accepted_polish_is_typed() {
        let url = fake_ollama("We shouldn't ship it!").await;
        let result = processor_with(ollama_at(url))
            .process("we should not ship it")
            .await
            .unwrap();
        assert_eq!(result, "We shouldn't ship it!");
    }

    fn style(name: &str, prompt: &str, polish: bool) -> Option<TakeStyle> {
        Some(TakeStyle {
            name: StyleName::parse(name).unwrap(),
            prompt: prompt.to_string(),
            polish,
        })
    }

    #[tokio::test]
    async fn a_verbatim_take_skips_the_llm_but_not_the_rules() {
        let raw = "um we should ship it today";
        let unpolished = processor_with(LlmProvider::None)
            .process(raw)
            .await
            .unwrap();
        let url = fake_ollama("A different text.").await;
        let verbatim = ProcessingContext::default().with_style(styles::resolve(
            &std::collections::BTreeMap::new(),
            styles::VERBATIM,
        ));
        let result = processor_with(ollama_at(url))
            .process_for(raw, &verbatim)
            .await
            .unwrap();
        assert_eq!(result, unpolished);
        assert!(!result.contains("um"), "the filler removal still runs");
    }

    #[tokio::test]
    async fn a_style_prompt_reaches_the_polish_prompt() {
        let (url, request) = fake_ollama_request("We shall ship it today.").await;
        let context = ProcessingContext::default().with_style(style(
            "formal",
            "Formal English, no contractions.",
            true,
        ));
        let result = processor_with(ollama_at(url))
            .process_for("we should ship it today", &context)
            .await
            .unwrap();
        assert_eq!(result, "We shall ship it today.");
        let request = request.await.unwrap();
        let body = request.split("\r\n\r\n").nth(1).unwrap_or_default();
        let json: serde_json::Value = serde_json::from_str(body).unwrap();
        let system = json["messages"][0]["content"].as_str().unwrap();
        assert!(
            system.contains("\n- Style: Formal English, no contractions.\n"),
            "{system}"
        );
    }

    #[tokio::test]
    async fn the_style_decides_polish_and_the_guard_still_applies() {
        let mut processor = processor_with(ollama_at(fake_ollama("We must go now.").await));
        processor.config.polish = false;
        let formal = ProcessingContext::default().with_style(style("formal", "Formal.", true));
        assert_eq!(
            processor
                .process_for("we should go now", &formal)
                .await
                .unwrap(),
            "We must go now.",
            "a style with polish on polishes when llm.polish is off"
        );

        let raw = "we should not ship it";
        let unpolished = processor_with(LlmProvider::None)
            .process(raw)
            .await
            .unwrap();
        let url = fake_ollama("We should ship it.").await;
        let result = processor_with(ollama_at(url))
            .process_for(raw, &formal)
            .await
            .unwrap();
        assert_eq!(result, unpolished, "a lost negation discards the polish");
    }

    #[tokio::test]
    async fn a_style_rewrite_needs_a_selection_before_it_calls_the_llm() {
        let formal = style("formal", "Formal English.", true).unwrap();
        let unreachable = processor_with(ollama_at("http://127.0.0.1:1".into()));
        for selection in [None, Some(""), Some(" \n ")] {
            assert_eq!(
                rewrite_with_style(Some(&unreachable), &formal, selection).await,
                RewriteOutcome::NothingSelected,
                "{selection:?}: a failed call would say Failed"
            );
        }
        assert_eq!(
            rewrite_with_style(None, &formal, Some("hi")).await,
            RewriteOutcome::NoLlm
        );
        assert_eq!(
            rewrite_with_style(
                Some(&processor_with(LlmProvider::None)),
                &formal,
                Some("hi")
            )
            .await,
            RewriteOutcome::NoLlm
        );
    }

    #[tokio::test]
    async fn a_failed_style_rewrite_replaces_nothing() {
        let formal = style("formal", "Formal English.", true).unwrap();
        let unreachable = processor_with(ollama_at("http://127.0.0.1:1".into()));
        let outcome = rewrite_with_style(Some(&unreachable), &formal, Some("hey there")).await;
        assert!(
            matches!(&outcome, RewriteOutcome::Failed(reason) if reason.contains("127.0.0.1:1")),
            "{outcome:?}"
        );
    }

    #[tokio::test]
    async fn a_style_rewrite_sends_the_style_prompt_as_the_instruction() {
        let formal = style("formal", "Formal English.", true).unwrap();
        let (url, request) = fake_ollama_request("Good day.").await;
        let processor = processor_with(ollama_at(url));
        assert_eq!(
            rewrite_with_style(Some(&processor), &formal, Some("hey there")).await,
            RewriteOutcome::Replace("Good day.".to_string())
        );
        let request = request.await.unwrap();
        let body = request.split("\r\n\r\n").nth(1).unwrap_or_default();
        let json: serde_json::Value = serde_json::from_str(body).unwrap();
        let user = json["messages"][1]["content"].as_str().unwrap();
        assert!(
            user.starts_with("Instruction: Formal English.\n\nText:\nhey there"),
            "{user}"
        );

        let same = processor_with(ollama_at(fake_ollama("hey there").await));
        assert_eq!(
            rewrite_with_style(Some(&same), &formal, Some("hey there")).await,
            RewriteOutcome::Unchanged
        );
    }
}
