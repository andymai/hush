/// Configuration for text processing
use crate::config::settings::LlmConfig;
use serde::{Deserialize, Serialize};

/// LLM provider for text polishing
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LlmProvider {
    /// Anthropic Claude API
    Anthropic {
        /// API key (read from env var if None)
        api_key: Option<String>,
        /// Model to use, from `llm.anthropic_model`
        model: String,
    },
    /// A local Ollama server
    Ollama {
        /// Base URL, such as http://localhost:11434
        url: String,
        /// Model name, such as llama3.2
        model: String,
    },
    /// A server that speaks the OpenAI chat completions API
    OpenAiCompatible {
        /// The full chat completions endpoint
        url: String,
        model: String,
        /// Sent as a bearer token when set
        api_key: Option<String>,
    },
    /// No LLM - rule-based only
    None,
}

impl Default for LlmProvider {
    fn default() -> Self {
        Self::Anthropic {
            api_key: None,
            model: "claude-haiku-4-5".to_string(),
        }
    }
}

/// Processing mode determining aggressiveness of editing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EditingMode {
    /// Minimal editing - only remove obvious filler words
    Light,

    /// Balanced editing - remove fillers, fix basic grammar
    #[default]
    Medium,

    /// Aggressive editing - heavy rewriting for professional output
    Aggressive,
}

/// Configuration for text processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingConfig {
    /// Editing aggressiveness level
    pub mode: EditingMode,

    /// LLM provider for text polishing
    pub llm_provider: LlmProvider,

    /// Run the LLM polish stage (Command Mode uses the LLM either way)
    pub polish: bool,

    /// Maximum tokens for LLM response
    pub max_tokens: u32,

    /// Temperature for LLM generation (0.0-1.0, lower = more conservative)
    pub temperature: f32,
}

impl Default for ProcessingConfig {
    fn default() -> Self {
        Self {
            mode: EditingMode::Medium,
            llm_provider: LlmProvider::default(),
            polish: true,
            max_tokens: 200,
            temperature: 0.3,
        }
    }
}

impl ProcessingConfig {
    /// The text processing of dictation: `mode`, the LLM in `provider`, and
    /// the polish, token, and temperature settings of `[llm]`.
    pub fn from_llm(mode: EditingMode, provider: LlmProvider, llm: &LlmConfig) -> Self {
        Self {
            mode,
            llm_provider: provider,
            polish: llm.polish,
            max_tokens: llm.max_tokens,
            temperature: llm.temperature,
        }
    }

    /// Create a light processing config (fast, minimal edits)
    pub fn light() -> Self {
        Self {
            mode: EditingMode::Light,
            llm_provider: LlmProvider::None,
            polish: true,
            ..Default::default()
        }
    }

    /// Create a medium processing config (balanced)
    pub fn medium() -> Self {
        Self {
            mode: EditingMode::Medium,
            ..Default::default()
        }
    }

    /// Create an aggressive processing config (maximum quality)
    pub fn aggressive() -> Self {
        Self {
            mode: EditingMode::Aggressive,
            ..Default::default()
        }
    }

    /// Create config with specific LLM provider
    pub fn with_llm(mut self, provider: LlmProvider) -> Self {
        self.llm_provider = provider;
        self
    }

    /// Disable LLM processing
    pub fn without_llm(mut self) -> Self {
        self.llm_provider = LlmProvider::None;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictation_settings_come_from_the_llm_section() {
        let llm = LlmConfig {
            polish: false,
            max_tokens: 123,
            temperature: 0.7,
            ..LlmConfig::defaults()
        };
        let config = ProcessingConfig::from_llm(EditingMode::Light, LlmProvider::None, &llm);
        assert_eq!(config.mode, EditingMode::Light);
        assert_eq!(config.llm_provider, LlmProvider::None);
        assert!(!config.polish);
        assert_eq!(config.max_tokens, 123);
        assert_eq!(config.temperature, 0.7);
    }
}
