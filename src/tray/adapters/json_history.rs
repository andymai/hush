/// JSON-based persistent transcription history storage
/// 
/// Implements HistoryStore trait with circular buffer and atomic file operations.

use crate::Result;
use crate::core::traits::{HistoryStore, TranscriptionEntry};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use std::time::SystemTime;

/// Serializable wrapper for history data
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HistoryData {
    entries: VecDeque<TranscriptionEntry>,
    max_entries: usize,
}

/// JSON-based transcription history store with circular buffer
pub struct JsonHistoryStore {
    file_path: PathBuf,
    entries: VecDeque<TranscriptionEntry>,
    max_entries: usize,
}

impl JsonHistoryStore {
    /// Create new JSON history store
    /// 
    /// # Arguments
    /// * `config_dir` - Directory to store history file (e.g., "~/.config/hush")
    /// * `max_entries` - Maximum number of entries to keep (default: 100)
    pub async fn new<P: AsRef<Path>>(config_dir: P, max_entries: Option<usize>) -> Result<Self> {
        let max_entries = max_entries.unwrap_or(100);
        let config_path = config_dir.as_ref();
        
        // Ensure config directory exists
        if !config_path.exists() {
            fs::create_dir_all(&config_path).await?;
        }
        
        let file_path = config_path.join("transcription_history.json");
        
        // Load existing entries if file exists
        let entries = if file_path.exists() {
            Self::load_from_file(&file_path, max_entries).await
                .unwrap_or_else(|_| VecDeque::new())
        } else {
            VecDeque::new()
        };
        
        Ok(Self {
            file_path,
            entries,
            max_entries,
        })
    }
    
    /// Create history store with default config directory
    pub async fn with_default_config(max_entries: Option<usize>) -> Result<Self> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?
            .join("hush");
            
        Self::new(config_dir, max_entries).await
    }
    
    /// Load entries from file
    async fn load_from_file(file_path: &Path, max_entries: usize) -> Result<VecDeque<TranscriptionEntry>> {
        let content = fs::read_to_string(file_path).await?;
        let history_data: HistoryData = serde_json::from_str(&content)?;
        
        // Ensure we don't exceed max_entries when loading
        let mut entries = history_data.entries;
        while entries.len() > max_entries {
            entries.pop_front();
        }
        
        Ok(entries)
    }
    
    /// Persist entries to file atomically
    async fn persist(&self) -> Result<()> {
        let history_data = HistoryData {
            entries: self.entries.clone(),
            max_entries: self.max_entries,
        };
        
        let json_content = serde_json::to_string_pretty(&history_data)?;
        
        // Atomic write: write to temp file, then rename
        let temp_file = self.file_path.with_extension("tmp");
        
        let mut file = fs::File::create(&temp_file).await?;
        file.write_all(json_content.as_bytes()).await?;
        file.sync_all().await?;
        drop(file);
        
        // Atomic rename
        fs::rename(temp_file, &self.file_path).await?;
        
        Ok(())
    }
    
    /// Generate unique ID for transcription entry
    fn generate_id() -> String {
        format!("tx_{}", SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos())
    }
    
    /// Truncate text for preview (internal utility)
    pub fn create_preview(text: &str, max_length: usize) -> String {
        if text.len() <= max_length {
            text.to_string()
        } else {
            format!("{}...", &text[..max_length.saturating_sub(3)])
        }
    }
}

#[async_trait]
impl HistoryStore for JsonHistoryStore {
    async fn add_entry(&mut self, mut entry: TranscriptionEntry) -> Result<()> {
        // Ensure entry has an ID
        if entry.id.is_empty() {
            entry.id = Self::generate_id();
        }
        
        // Add to end of circular buffer
        self.entries.push_back(entry);
        
        // Remove oldest entry if we exceed max
        while self.entries.len() > self.max_entries {
            self.entries.pop_front();
        }
        
        // Persist to disk
        self.persist().await?;
        
        Ok(())
    }
    
    async fn get_recent(&self, limit: usize) -> Result<Vec<TranscriptionEntry>> {
        // Return newest entries first
        Ok(self.entries
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect())
    }
    
    async fn clear(&mut self) -> Result<()> {
        self.entries.clear();
        self.persist().await?;
        Ok(())
    }
    
    fn len(&self) -> usize {
        self.entries.len()
    }
}