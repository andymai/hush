use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
/// Domain-specific vocabulary management
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Domain-specific vocabulary configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainVocabulary {
    /// Abbreviations and their expansions
    #[serde(default)]
    pub abbreviations: HashMap<String, String>,

    /// Words that should be preserved exactly (case-sensitive)
    #[serde(default)]
    pub preserve_exact: Vec<String>,

    /// Technical terms with proper capitalization
    #[serde(default)]
    pub technical_terms: HashMap<String, String>,

    /// Custom entities (lowercase -> proper case)
    #[serde(default)]
    pub entities: HashMap<String, String>,

    /// Common phrases/idioms to preserve
    #[serde(default)]
    pub phrases: HashMap<String, String>,
}

impl Default for DomainVocabulary {
    fn default() -> Self {
        Self {
            abbreviations: HashMap::new(),
            preserve_exact: Vec::new(),
            technical_terms: HashMap::new(),
            entities: HashMap::new(),
            phrases: HashMap::new(),
        }
    }
}

impl DomainVocabulary {
    /// Create a new empty vocabulary
    pub fn new() -> Self {
        Self::default()
    }

    /// Load vocabulary from JSON file
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        info!("Loading domain vocabulary from: {}", path.display());

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read vocabulary file: {}", path.display()))?;

        let vocab: DomainVocabulary = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse vocabulary file: {}", path.display()))?;

        info!("Loaded vocabulary: {} terms", vocab.total_entries());
        Ok(vocab)
    }

    /// Save vocabulary to JSON file
    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        info!("Saving domain vocabulary to: {}", path.display());

        let json = serde_json::to_string_pretty(self).context("Failed to serialize vocabulary")?;

        std::fs::write(path, json)
            .with_context(|| format!("Failed to write vocabulary file: {}", path.display()))?;

        Ok(())
    }

    /// Get default vocabulary file path
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("hush")
            .join("vocabulary.json")
    }

    /// Apply vocabulary transformations to text
    pub fn apply(&self, text: &str) -> String {
        let mut result = text.to_string();

        // Apply abbreviation expansions
        for (abbrev, expansion) in &self.abbreviations {
            let pattern = format!(r"\b{}\b", regex::escape(abbrev));
            if let Ok(re) = regex::Regex::new(&pattern) {
                result = re.replace_all(&result, expansion.as_str()).to_string();
            }
        }

        // Apply phrase replacements
        for (phrase, replacement) in &self.phrases {
            result = result.replace(phrase, replacement);
        }

        // Apply technical term capitalization
        for (lowercase, proper) in &self.technical_terms {
            let pattern = format!(r"(?i)\b{}\b", regex::escape(lowercase));
            if let Ok(re) = regex::Regex::new(&pattern) {
                result = re.replace_all(&result, proper.as_str()).to_string();
            }
        }

        result
    }

    /// Check if a word should be preserved exactly
    pub fn should_preserve(&self, word: &str) -> bool {
        self.preserve_exact.contains(&word.to_string())
    }

    /// Get expansion for an abbreviation
    pub fn expand(&self, abbrev: &str) -> Option<&String> {
        self.abbreviations.get(abbrev)
    }

    /// Add a new abbreviation
    pub fn add_abbreviation(&mut self, abbrev: String, expansion: String) {
        self.abbreviations.insert(abbrev, expansion);
    }

    /// Add a technical term
    pub fn add_technical_term(&mut self, lowercase: String, proper: String) {
        self.technical_terms.insert(lowercase, proper);
    }

    /// Add a word to preserve exactly
    pub fn add_preserve_exact(&mut self, word: String) {
        if !self.preserve_exact.contains(&word) {
            self.preserve_exact.push(word);
        }
    }

    /// Merge another vocabulary into this one
    pub fn merge(&mut self, other: &DomainVocabulary) {
        self.abbreviations.extend(other.abbreviations.clone());
        self.technical_terms.extend(other.technical_terms.clone());
        self.entities.extend(other.entities.clone());
        self.phrases.extend(other.phrases.clone());

        for word in &other.preserve_exact {
            if !self.preserve_exact.contains(word) {
                self.preserve_exact.push(word.clone());
            }
        }
    }

    /// Get total number of entries
    pub fn total_entries(&self) -> usize {
        self.abbreviations.len()
            + self.preserve_exact.len()
            + self.technical_terms.len()
            + self.entities.len()
            + self.phrases.len()
    }
}

/// Predefined domain vocabularies
pub struct DomainVocabularies;

impl DomainVocabularies {
    /// Get web development vocabulary
    pub fn web_dev() -> DomainVocabulary {
        let mut vocab = DomainVocabulary::new();

        // Abbreviations
        vocab.add_abbreviation("js".to_string(), "JavaScript".to_string());
        vocab.add_abbreviation("ts".to_string(), "TypeScript".to_string());
        vocab.add_abbreviation("css".to_string(), "CSS".to_string());
        vocab.add_abbreviation("html".to_string(), "HTML".to_string());

        // Technical terms
        vocab.add_technical_term("react".to_string(), "React".to_string());
        vocab.add_technical_term("vue".to_string(), "Vue".to_string());
        vocab.add_technical_term("angular".to_string(), "Angular".to_string());
        vocab.add_technical_term("nextjs".to_string(), "Next.js".to_string());
        vocab.add_technical_term("nodejs".to_string(), "Node.js".to_string());

        vocab
    }

    /// Get backend development vocabulary
    pub fn backend() -> DomainVocabulary {
        let mut vocab = DomainVocabulary::new();

        // Abbreviations
        vocab.add_abbreviation("api".to_string(), "API".to_string());
        vocab.add_abbreviation("rest".to_string(), "REST".to_string());
        vocab.add_abbreviation("crud".to_string(), "CRUD".to_string());
        vocab.add_abbreviation("db".to_string(), "database".to_string());

        // Technical terms
        vocab.add_technical_term("postgresql".to_string(), "PostgreSQL".to_string());
        vocab.add_technical_term("postgres".to_string(), "PostgreSQL".to_string());
        vocab.add_technical_term("mysql".to_string(), "MySQL".to_string());
        vocab.add_technical_term("mongodb".to_string(), "MongoDB".to_string());
        vocab.add_technical_term("redis".to_string(), "Redis".to_string());

        vocab
    }

    /// Get DevOps vocabulary
    pub fn devops() -> DomainVocabulary {
        let mut vocab = DomainVocabulary::new();

        // Abbreviations
        vocab.add_abbreviation("k8s".to_string(), "Kubernetes".to_string());
        vocab.add_abbreviation("ci".to_string(), "CI".to_string());
        vocab.add_abbreviation("cd".to_string(), "CD".to_string());

        // Technical terms
        vocab.add_technical_term("docker".to_string(), "Docker".to_string());
        vocab.add_technical_term("kubernetes".to_string(), "Kubernetes".to_string());
        vocab.add_technical_term("aws".to_string(), "AWS".to_string());
        vocab.add_technical_term("gcp".to_string(), "GCP".to_string());

        vocab
    }

    /// Get machine learning vocabulary
    pub fn machine_learning() -> DomainVocabulary {
        let mut vocab = DomainVocabulary::new();

        // Abbreviations
        vocab.add_abbreviation("ml".to_string(), "machine learning".to_string());
        vocab.add_abbreviation("ai".to_string(), "AI".to_string());
        vocab.add_abbreviation("llm".to_string(), "LLM".to_string());
        vocab.add_abbreviation("nlp".to_string(), "NLP".to_string());

        // Technical terms
        vocab.add_technical_term("pytorch".to_string(), "PyTorch".to_string());
        vocab.add_technical_term("tensorflow".to_string(), "TensorFlow".to_string());
        vocab.add_technical_term("numpy".to_string(), "NumPy".to_string());

        vocab
    }

    /// Get combined vocabulary for general programming
    pub fn general_programming() -> DomainVocabulary {
        let mut vocab = DomainVocabulary::new();

        // Merge multiple domains
        vocab.merge(&Self::web_dev());
        vocab.merge(&Self::backend());
        vocab.merge(&Self::devops());

        vocab
    }
}

/// Vocabulary manager for loading and managing multiple vocabularies
pub struct VocabularyManager {
    /// Active vocabularies
    vocabularies: Vec<DomainVocabulary>,
}

impl VocabularyManager {
    /// Create a new vocabulary manager
    pub fn new() -> Self {
        Self {
            vocabularies: Vec::new(),
        }
    }

    /// Add a vocabulary
    pub fn add(&mut self, vocab: DomainVocabulary) {
        self.vocabularies.push(vocab);
    }

    /// Load vocabulary from file if it exists
    pub fn load_if_exists(&mut self, path: impl AsRef<Path>) -> Result<bool> {
        let path = path.as_ref();
        if path.exists() {
            let vocab = DomainVocabulary::load_from_file(path)?;
            self.add(vocab);
            Ok(true)
        } else {
            debug!("Vocabulary file not found: {}", path.display());
            Ok(false)
        }
    }

    /// Apply all vocabularies to text
    pub fn apply(&self, text: &str) -> String {
        let mut result = text.to_string();
        for vocab in &self.vocabularies {
            result = vocab.apply(&result);
        }
        result
    }

    /// Get combined count of all entries
    pub fn total_entries(&self) -> usize {
        self.vocabularies.iter().map(|v| v.total_entries()).sum()
    }
}

impl Default for VocabularyManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abbreviation_expansion() {
        let mut vocab = DomainVocabulary::new();
        vocab.add_abbreviation("api".to_string(), "API".to_string());
        vocab.add_abbreviation("db".to_string(), "database".to_string());

        let result = vocab.apply("the api connects to the db");
        assert_eq!(result, "the API connects to the database");
    }

    #[test]
    fn test_technical_term_capitalization() {
        let mut vocab = DomainVocabulary::new();
        vocab.add_technical_term("postgresql".to_string(), "PostgreSQL".to_string());
        vocab.add_technical_term("nodejs".to_string(), "Node.js".to_string());

        let result = vocab.apply("using postgresql with nodejs");
        assert_eq!(result, "using PostgreSQL with Node.js");
    }

    #[test]
    fn test_predefined_web_dev() {
        let vocab = DomainVocabularies::web_dev();

        let result = vocab.apply("building with react and nextjs");
        assert_eq!(result, "building with React and Next.js");
    }

    #[test]
    fn test_predefined_backend() {
        let vocab = DomainVocabularies::backend();

        let result = vocab.apply("rest api using postgresql");
        assert_eq!(result, "REST API using PostgreSQL");
    }

    #[test]
    fn test_vocabulary_merge() {
        let mut vocab1 = DomainVocabulary::new();
        vocab1.add_abbreviation("api".to_string(), "API".to_string());

        let mut vocab2 = DomainVocabulary::new();
        vocab2.add_abbreviation("db".to_string(), "database".to_string());

        vocab1.merge(&vocab2);

        let result = vocab1.apply("api connects to db");
        assert_eq!(result, "API connects to database");
    }

    #[test]
    fn test_vocabulary_manager() {
        let mut manager = VocabularyManager::new();
        manager.add(DomainVocabularies::web_dev());
        manager.add(DomainVocabularies::backend());

        let result = manager.apply("building rest api with react and postgresql");
        assert_eq!(result, "building REST API with React and PostgreSQL");
    }
}
