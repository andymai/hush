/// Text insertion history for undo functionality
///
/// Tracks text insertions so they can be undone via voice commands

use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tracing::{debug, info};

/// A single text insertion entry
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// The text that was inserted
    pub text: String,

    /// When this text was inserted
    pub timestamp: Instant,

    /// Number of characters inserted
    pub char_count: usize,
}

impl HistoryEntry {
    /// Create a new history entry
    pub fn new(text: String) -> Self {
        let char_count = text.chars().count();
        Self {
            text,
            timestamp: Instant::now(),
            char_count,
        }
    }

    /// Check if this entry is still recent enough to undo
    pub fn is_recent(&self, max_age: Duration) -> bool {
        self.timestamp.elapsed() < max_age
    }
}

/// Text insertion history tracker
pub struct InsertionHistory {
    /// Maximum number of entries to keep
    max_entries: usize,

    /// Maximum age for undo operations
    max_age: Duration,

    /// History of insertions (most recent last)
    entries: VecDeque<HistoryEntry>,
}

impl InsertionHistory {
    /// Create a new insertion history
    pub fn new() -> Self {
        Self {
            max_entries: 50,
            max_age: Duration::from_secs(300), // 5 minutes
            entries: VecDeque::new(),
        }
    }

    /// Create with custom limits
    pub fn with_limits(max_entries: usize, max_age: Duration) -> Self {
        Self {
            max_entries,
            max_age,
            entries: VecDeque::new(),
        }
    }

    /// Record a text insertion
    pub fn record(&mut self, text: String) {
        if text.is_empty() {
            return;
        }

        let entry = HistoryEntry::new(text.clone());
        debug!("Recording insertion: '{}' ({} chars)",
               if text.len() > 50 { &text[..50] } else { &text },
               entry.char_count);

        self.entries.push_back(entry);

        // Trim old entries if we exceed max_entries
        while self.entries.len() > self.max_entries {
            let removed = self.entries.pop_front();
            if let Some(entry) = removed {
                debug!("Removed old history entry: {} chars", entry.char_count);
            }
        }

        info!("Insertion recorded (history size: {})", self.entries.len());
    }

    /// Get the last insertion for undo
    /// Returns None if history is empty or last insertion is too old
    pub fn get_last(&self) -> Option<&HistoryEntry> {
        self.entries.back().filter(|entry| entry.is_recent(self.max_age))
    }

    /// Remove and return the last insertion
    pub fn pop_last(&mut self) -> Option<HistoryEntry> {
        if let Some(entry) = self.entries.back() {
            if !entry.is_recent(self.max_age) {
                info!("Last insertion too old to undo");
                return None;
            }
        }

        let entry = self.entries.pop_back();
        if let Some(ref e) = entry {
            info!("Popped last insertion: '{}' ({} chars)",
                  if e.text.len() > 50 { &e.text[..50] } else { &e.text },
                  e.char_count);
        }
        entry
    }

    /// Clear all history
    pub fn clear(&mut self) {
        let count = self.entries.len();
        self.entries.clear();
        debug!("Cleared {} history entries", count);
    }

    /// Get the number of entries in history
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if history is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Remove entries older than max_age
    pub fn prune_old(&mut self) {
        let original_len = self.entries.len();
        self.entries.retain(|entry| entry.is_recent(self.max_age));
        let removed = original_len - self.entries.len();
        if removed > 0 {
            debug!("Pruned {} old entries from history", removed);
        }
    }
}

impl Default for InsertionHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_record_and_get() {
        let mut history = InsertionHistory::new();

        history.record("first insertion".to_string());
        history.record("second insertion".to_string());

        assert_eq!(history.len(), 2);

        let last = history.get_last().unwrap();
        assert_eq!(last.text, "second insertion");
    }

    #[test]
    fn test_pop_last() {
        let mut history = InsertionHistory::new();

        history.record("first".to_string());
        history.record("second".to_string());

        let last = history.pop_last().unwrap();
        assert_eq!(last.text, "second");
        assert_eq!(history.len(), 1);

        let last = history.pop_last().unwrap();
        assert_eq!(last.text, "first");
        assert_eq!(history.len(), 0);

        assert!(history.pop_last().is_none());
    }

    #[test]
    fn test_max_entries() {
        let mut history = InsertionHistory::with_limits(3, Duration::from_secs(60));

        history.record("one".to_string());
        history.record("two".to_string());
        history.record("three".to_string());
        history.record("four".to_string());

        // Should only keep last 3
        assert_eq!(history.len(), 3);

        let last = history.get_last().unwrap();
        assert_eq!(last.text, "four");
    }

    #[test]
    fn test_age_limit() {
        let mut history = InsertionHistory::with_limits(10, Duration::from_millis(50));

        history.record("old text".to_string());

        // Wait for entry to age
        thread::sleep(Duration::from_millis(100));

        // Should not return old entry
        assert!(history.get_last().is_none());
        assert!(history.pop_last().is_none());
    }

    #[test]
    fn test_empty_text_ignored() {
        let mut history = InsertionHistory::new();

        history.record("".to_string());
        assert_eq!(history.len(), 0);

        history.record("not empty".to_string());
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn test_clear() {
        let mut history = InsertionHistory::new();

        history.record("one".to_string());
        history.record("two".to_string());

        assert_eq!(history.len(), 2);

        history.clear();
        assert_eq!(history.len(), 0);
        assert!(history.is_empty());
    }

    #[test]
    fn test_prune_old() {
        let mut history = InsertionHistory::with_limits(10, Duration::from_millis(50));

        history.record("old1".to_string());
        history.record("old2".to_string());

        thread::sleep(Duration::from_millis(100));

        history.record("recent".to_string());

        history.prune_old();

        assert_eq!(history.len(), 1);
        assert_eq!(history.get_last().unwrap().text, "recent");
    }
}
