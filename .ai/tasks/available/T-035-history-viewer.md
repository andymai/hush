# Task: History Viewer - Transcription History and Logs

## Description

Create a history viewer that displays recent transcriptions, allows searching/filtering, provides options to copy or re-insert text, and shows application logs. This is an optional but valuable feature for reviewing past transcriptions.

## Requirements

### History Tab in Settings Window
- [ ] Create "History" tab in settings window
- [ ] Display list of recent transcriptions:
  - Timestamp (e.g., "2 minutes ago", "Today 3:45 PM")
  - Transcribed text (truncated if long)
  - Duration (recording length)
  - Model used
  - Editing mode (if LLM was used)
  - Status (success, error, cancelled)
- [ ] Limit to most recent 100 entries (configurable)
- [ ] Sort by timestamp (newest first)

### Transcription Details View
- [ ] Click on entry to show full details:
  - Complete transcribed text (full, not truncated)
  - Raw transcription (before text processing)
  - Polished text (after LLM, if used)
  - Recording duration
  - Transcription duration
  - Model used
  - Audio file path (if saved)
  - Timestamp (full date/time)
- [ ] Add actions for selected entry:
  - Copy text to clipboard
  - Re-insert text at cursor
  - Export as text file
  - Delete from history
  - Play audio (if available)

### Search and Filter
- [ ] Add search bar:
  - Search transcription text
  - Real-time filtering as user types
  - Highlight matching text
- [ ] Add filter options:
  - Date range (today, this week, this month, custom)
  - Model (filter by model used)
  - Status (success, error, cancelled)
  - Editing mode (light, medium, aggressive, none)
- [ ] Add sort options:
  - Newest first (default)
  - Oldest first
  - Longest duration
  - Shortest duration

### History Management
- [ ] Create `src/settings/history.rs` for history storage
- [ ] Implement history data structure:
  ```rust
  struct HistoryEntry {
      id: String,
      timestamp: DateTime<Local>,
      raw_text: String,
      polished_text: Option<String>,
      duration: Duration,
      transcription_time: Duration,
      model: String,
      editing_mode: Option<EditingMode>,
      status: TranscriptionStatus,
      audio_path: Option<PathBuf>,
  }
  ```
- [ ] Store history in: `~/.local/share/hush/history.json`
- [ ] Implement history rotation (keep last 100, delete older)
- [ ] Add option to clear all history (with confirmation)
- [ ] Export history to JSON/CSV

### Statistics Dashboard
- [ ] Add "Statistics" section showing:
  - Total transcriptions
  - Total recording time
  - Most used model
  - Average transcription length
  - Success rate
  - LLM usage percentage
- [ ] Add charts/graphs (optional):
  - Transcriptions per day (bar chart)
  - Model usage (pie chart)
  - Recording duration histogram

### Log Viewer
- [ ] Add "Logs" section in History tab:
  - Show recent application logs
  - Color-coded by level (ERROR=red, WARN=yellow, INFO=white)
  - Collapsible/expandable entries
  - Show timestamp for each log entry
- [ ] Add log filtering:
  - Filter by level (ERROR, WARN, INFO, DEBUG)
  - Search logs by keyword
- [ ] Add "Export Logs" button:
  - Export to text file
  - Include system info for bug reports

### Privacy and Data Management
- [ ] Add privacy options:
  - Disable history recording (toggle)
  - Auto-delete after X days
  - Clear history on exit
- [ ] Add data export:
  - Export all history as JSON
  - Export selected entries
  - Export statistics
- [ ] Show storage usage: "History using 2.3 MB"

### Integration
- [ ] Hook into transcription pipeline:
  - Record every transcription automatically
  - Store in history database
  - Update history UI in real-time
- [ ] Link from overlay:
  - Show "View History" button/link
  - Open history to most recent entry
- [ ] Link from system tray:
  - "Recent Transcriptions" submenu (last 5)
  - "View All History" opens history tab

## Success Criteria

- `cargo check` passes
- `cargo test` passes
- `cargo clippy -- -D warnings` passes
- `cargo fmt` applied
- History tab displays correctly in settings window
- Transcriptions are recorded automatically
- Search and filtering work correctly
- History persists across restarts
- Performance is good with 100+ entries
- Export functions work correctly
- Privacy options are respected

## Context

This is Phase 5 of the UI expansion project (optional). It builds on all previous settings tasks and provides a way to review, search, and manage transcription history.

**Current state:**
- No transcription history
- Logs are written to files but not viewable in UI
- No way to review past transcriptions

**Use cases:**
- "What did I transcribe 5 minutes ago?"
- "I need to copy that transcription from earlier"
- "How much time have I saved with Hush this week?"
- "Why did my last transcription fail?" (check logs)

## Files to Create

- `src/settings/history.rs` - History storage and management
- `src/settings/history_tab.rs` - History tab UI implementation
- `src/settings/history_entry.rs` - History entry data structure
- `src/settings/statistics.rs` - Statistics calculation and display

## Files to Check/Modify

- `src/settings/window.rs` - Add history tab
- `src/application/hush_app.rs` - Hook transcription recording
- `src/transcription/whisper.rs` - Record transcription metadata
- `src/text_processing/mod.rs` - Record processing results

## Reference Documentation

- egui table widget: https://docs.rs/egui (Table, Grid)
- chrono for timestamps: https://docs.rs/chrono
- serde for JSON storage: https://docs.rs/serde_json
- Log viewer inspiration: VS Code, IntelliJ IDEA

## Estimated Complexity

**Medium-High** - Data storage, search/filter, UI table, statistics. Estimated 2-3 days.

## Dependencies

- **Requires:** T-027 (Enhanced Settings Window) completed
- **Benefits From:** All previous settings tasks (better metadata to record)

## Testing Checklist

- [ ] Test history recording during transcription
- [ ] Test with empty history
- [ ] Test with 100+ entries (performance)
- [ ] Test search functionality
- [ ] Test filtering by date, model, status
- [ ] Test copy to clipboard
- [ ] Test re-insert text
- [ ] Test export to file
- [ ] Test delete entry
- [ ] Test clear all history
- [ ] Test statistics calculations
- [ ] Test log viewer
- [ ] Test privacy options (disable recording)
- [ ] Test history rotation (old entries deleted)

## Privacy Considerations

- **Transcription Text:** Users may transcribe sensitive information
- **Storage:** History stored locally, not sent anywhere
- **Deletion:** Provide easy way to clear history
- **Auto-Expire:** Option to auto-delete old entries
- **Opt-Out:** Option to disable history recording entirely

## Future Enhancements

- Cloud sync (encrypted, optional)
- Tags/labels for entries
- Favorites/starred entries
- Full-text search across all history
- Export to various formats (Markdown, PDF)
- Audio playback if recordings are saved
- Voice command: "Show me my last transcription"
- Integration with note-taking apps
- Backup and restore history
