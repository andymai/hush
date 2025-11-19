# Task: Model Download UI - Visual Downloader with Progress

## Description

Add model download functionality to the Models tab in the settings window. This includes a visual downloader with progress bars, download queue management, and integration with the existing model download system.

## Requirements

### Download UI
- [ ] Add "Download" button next to each uninstalled model
- [ ] Implement download progress bar with:
  - Download progress percentage
  - Downloaded size / Total size (e.g., "72 MB / 145 MB")
  - Download speed (e.g., "2.4 MB/s")
  - Estimated time remaining
  - Cancel button
- [ ] Show download queue if multiple models downloading
- [ ] Add "Download All" button (optional, with confirmation)

### Download Management
- [ ] Create `src/settings/model_downloader.rs` for download logic
- [ ] Implement async download with progress updates
- [ ] Support download cancellation
- [ ] Handle download errors gracefully:
  - Network errors (retry option)
  - Disk space errors (clear message)
  - Checksum validation failures (re-download)
- [ ] Implement download queue management

### Integration with Existing System
- [ ] Wire to existing model download in `src/bin/model-manager.rs`
- [ ] Use existing HuggingFace Hub integration
- [ ] Verify checksums after download (SHA256)
- [ ] Update model list after successful download
- [ ] Show success notification on completion

### Storage Management
- [ ] Check available disk space before download
- [ ] Warn if insufficient space
- [ ] Show storage usage updating during download
- [ ] Add "Delete Model" button for installed models (with confirmation)
- [ ] Update storage display after delete

### User Experience
- [ ] Disable model switching while download in progress
- [ ] Show download status in system tray (if enabled)
- [ ] Add background download support (window can close)
- [ ] Persist download state across app restarts
- [ ] Show download history/log (optional)

## Success Criteria

- `cargo check` passes
- `cargo test` passes
- `cargo clippy -- -D warnings` passes
- `cargo fmt` applied
- Model download works with visual progress
- Download can be cancelled cleanly
- Multiple models can queue for download
- Errors are handled gracefully with clear messages
- Downloaded models are immediately available
- Storage calculations are accurate
- No memory leaks during long downloads

## Context

This is Phase 2, Step 2 of the UI expansion project. It builds on T-028 (Model Management UI) and replaces the CLI model download command with a visual interface.

**Current state:**
- CLI download: `./hush models download <model>`
- Model manager: `src/bin/model-manager.rs`
- Uses `hf-hub` crate for HuggingFace downloads
- Models stored in: `models/` directory

**Download sources:**
- HuggingFace: `openai/whisper-<model>` repositories
- Requires network connection
- Files are large (75 MB - 2.9 GB)

## Files to Create

- `src/settings/model_downloader.rs` - Download logic with progress
- `src/settings/download_progress.rs` - Progress tracking state

## Files to Check/Modify

- `src/settings/models_tab.rs` - Add download UI
- `src/bin/model-manager.rs` - Extract reusable download logic
- `Cargo.toml` - Ensure reqwest has "stream" feature
- `src/settings/config.rs` - Persist download state

## Reference Documentation

- Model manager implementation: `src/bin/model-manager.rs`
- HuggingFace Hub docs: https://docs.rs/hf-hub
- reqwest streaming: https://docs.rs/reqwest/latest/reqwest/
- egui progress bars: https://docs.rs/egui (ProgressBar widget)

## Estimated Complexity

**Medium-High** - Async download with progress, error handling, UI integration, storage management. Estimated 2-3 days.

## Dependencies

- **Requires:** T-027 (Enhanced Settings Window) completed
- **Requires:** T-028 (Model Management UI) completed

## Testing Checklist

- [ ] Download tiny model (fastest, for testing)
- [ ] Download large model (tests long-running download)
- [ ] Cancel download mid-progress
- [ ] Download with network interruption
- [ ] Download with insufficient disk space
- [ ] Download multiple models sequentially
- [ ] Delete downloaded model
- [ ] Verify checksum validation

## Future Enhancements

- Pause/resume download support
- Bandwidth throttling option
- Mirror/alternative download sources
- Model verification and integrity checks
- Automatic model updates
