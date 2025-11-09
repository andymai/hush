/// Configuration for text processing
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Processing mode determining aggressiveness of editing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditingMode {
    /// Minimal editing - only remove obvious filler words
    Light,

    /// Balanced editing - remove fillers, fix basic grammar
    Medium,

    /// Aggressive editing - heavy rewriting for professional output
    Aggressive,
}

impl Default for EditingMode {
    fn default() -> Self {
        Self::Medium
    }
}

/// Configuration for text processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingConfig {
    /// Editing aggressiveness level
    pub mode: EditingMode,

    /// Whether to use LLM for polishing (slower but higher quality)
    pub use_llm: bool,

    /// Path to LLM model file (GGUF format)
    pub llm_model_path: PathBuf,
}

impl Default for ProcessingConfig {
    fn default() -> Self {
        Self {
            mode: EditingMode::Medium,
            use_llm: true,
            llm_model_path: dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".hush/models/llm-model.gguf"),
        }
    }
}

impl ProcessingConfig {
    /// Create a light processing config (fast, minimal edits)
    pub fn light() -> Self {
        Self {
            mode: EditingMode::Light,
            use_llm: false,
            llm_model_path: Default::default(),
        }
    }

    /// Create a medium processing config (balanced)
    pub fn medium() -> Self {
        Self {
            mode: EditingMode::Medium,
            use_llm: true,
            ..Default::default()
        }
    }

    /// Create an aggressive processing config (maximum quality)
    pub fn aggressive() -> Self {
        Self {
            mode: EditingMode::Aggressive,
            use_llm: true,
            ..Default::default()
        }
    }
}
