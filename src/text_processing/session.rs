/// Session memory for multi-turn context awareness
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tracing::debug;

/// Maximum number of recent insertions to remember
const MAX_HISTORY_SIZE: usize = 10;

/// Session timeout - clear memory after this period of inactivity
const SESSION_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

/// A single entry in session memory
#[derive(Debug, Clone)]
pub struct SessionEntry {
    /// The text that was inserted
    pub text: String,
    /// When it was inserted
    pub timestamp: Instant,
    /// Application context at time of insertion
    pub context: Option<String>,
}

/// Session memory for tracking recent insertions
pub struct SessionMemory {
    /// Recent insertions (newest first)
    entries: VecDeque<SessionEntry>,
    /// Session start time
    session_start: Instant,
    /// Last activity time
    last_activity: Instant,
}

impl SessionMemory {
    /// Create a new session memory
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            entries: VecDeque::with_capacity(MAX_HISTORY_SIZE),
            session_start: now,
            last_activity: now,
        }
    }

    /// Add a new entry to session memory
    pub fn add(&mut self, text: String, context: Option<String>) {
        let now = Instant::now();

        // Check if session has timed out
        if now.duration_since(self.last_activity) > SESSION_TIMEOUT {
            debug!("Session timeout - clearing memory");
            self.clear();
            self.session_start = now;
        }

        self.last_activity = now;

        // Add new entry
        self.entries.push_front(SessionEntry {
            text,
            timestamp: now,
            context,
        });

        // Trim to max size
        if self.entries.len() > MAX_HISTORY_SIZE {
            self.entries.pop_back();
        }

        debug!("Session memory: {} entries", self.entries.len());
    }

    /// Get recent entries (up to N most recent)
    pub fn recent(&self, count: usize) -> Vec<&SessionEntry> {
        self.entries.iter().take(count).collect()
    }

    /// Get all entries in session
    pub fn all(&self) -> Vec<&SessionEntry> {
        self.entries.iter().collect()
    }

    /// Get the most recent entry
    pub fn last(&self) -> Option<&SessionEntry> {
        self.entries.front()
    }

    /// Get combined text from recent entries
    pub fn recent_text(&self, count: usize) -> String {
        self.recent(count)
            .iter()
            .rev() // Oldest first for context
            .map(|entry| entry.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Build context string for LLM prompting
    pub fn build_context(&self, count: usize) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }

        let entries = self.recent(count);
        if entries.is_empty() {
            return None;
        }

        let mut context = String::from("Recent conversation:\n");
        for (i, entry) in entries.iter().rev().enumerate() {
            let time_ago = entry.timestamp.elapsed().as_secs();
            context.push_str(&format!(
                "{}. ({} seconds ago) {}\n",
                i + 1,
                time_ago,
                entry.text
            ));
        }

        Some(context)
    }

    /// Check if session is active (has recent activity)
    pub fn is_active(&self) -> bool {
        self.last_activity.elapsed() < SESSION_TIMEOUT
    }

    /// Get session duration
    pub fn duration(&self) -> Duration {
        self.session_start.elapsed()
    }

    /// Get time since last activity
    pub fn time_since_last_activity(&self) -> Duration {
        self.last_activity.elapsed()
    }

    /// Clear all session memory
    pub fn clear(&mut self) {
        self.entries.clear();
        debug!("Session memory cleared");
    }

    /// Get number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if session memory is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Extract potential topics/entities from recent entries
    pub fn extract_topics(&self) -> Vec<String> {
        let mut topics = Vec::new();

        // Simple topic extraction - look for capitalized words
        for entry in self.entries.iter().take(5) {
            for word in entry.text.split_whitespace() {
                // Look for words starting with capital letter (potential proper nouns)
                if word.len() > 3 && word.chars().next().is_some_and(|c| c.is_uppercase()) {
                    let clean_word = word.trim_matches(|c: char| !c.is_alphanumeric());
                    if !clean_word.is_empty() && !topics.contains(&clean_word.to_string()) {
                        topics.push(clean_word.to_string());
                    }
                }
            }
        }

        topics.truncate(10); // Limit to 10 topics
        topics
    }
}

impl Default for SessionMemory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_add_and_retrieve() {
        let mut memory = SessionMemory::new();

        memory.add("Hello world".to_string(), None);
        memory.add("This is a test".to_string(), None);

        assert_eq!(memory.len(), 2);
        assert_eq!(memory.last().unwrap().text, "This is a test");
    }

    #[test]
    fn test_recent_entries() {
        let mut memory = SessionMemory::new();

        for i in 0..5 {
            memory.add(format!("Entry {}", i), None);
        }

        let recent = memory.recent(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].text, "Entry 4"); // Most recent
        assert_eq!(recent[2].text, "Entry 2"); // Third most recent
    }

    #[test]
    fn test_max_size() {
        let mut memory = SessionMemory::new();

        // Add more than MAX_HISTORY_SIZE entries
        for i in 0..15 {
            memory.add(format!("Entry {}", i), None);
        }

        assert_eq!(memory.len(), MAX_HISTORY_SIZE);
        // Should have most recent entries
        assert_eq!(memory.last().unwrap().text, "Entry 14");
    }

    #[test]
    fn test_recent_text() {
        let mut memory = SessionMemory::new();

        memory.add("First".to_string(), None);
        memory.add("Second".to_string(), None);
        memory.add("Third".to_string(), None);

        let text = memory.recent_text(2);
        assert_eq!(text, "Second Third"); // Oldest to newest
    }

    #[test]
    fn test_clear() {
        let mut memory = SessionMemory::new();

        memory.add("Test".to_string(), None);
        assert_eq!(memory.len(), 1);

        memory.clear();
        assert_eq!(memory.len(), 0);
        assert!(memory.is_empty());
    }

    #[test]
    fn test_extract_topics() {
        let mut memory = SessionMemory::new();

        memory.add("Working on GitHub integration".to_string(), None);
        memory.add("Need to update PostgreSQL schema".to_string(), None);

        let topics = memory.extract_topics();
        assert!(topics.contains(&"GitHub".to_string()));
        assert!(topics.contains(&"PostgreSQL".to_string()));
    }

    #[test]
    fn test_build_context() {
        let mut memory = SessionMemory::new();

        memory.add("First message".to_string(), None);
        sleep(Duration::from_millis(100));
        memory.add("Second message".to_string(), None);

        let context = memory.build_context(2).unwrap();
        assert!(context.contains("Recent conversation"));
        assert!(context.contains("First message"));
        assert!(context.contains("Second message"));
    }
}
