# Task: LLM Configuration UI

## Description

Add an "Advanced" tab to the settings window that provides configuration for the optional LLM text polishing feature. Include API key management, editing mode selection, and model settings.

## Requirements

### Advanced Tab UI
- [ ] Create "Advanced" tab in settings window
- [ ] Add "LLM Text Polishing" section with:
  - Enable/disable toggle
  - API provider selector (currently: Anthropic Claude)
  - API key input field (password-style, hidden)
  - "Test Connection" button
  - Connection status indicator
- [ ] Show current LLM settings status (enabled/disabled, configured/not configured)

### Editing Mode Configuration
- [ ] Add editing mode selector:
  - Light: Minimal editing, preserves natural speech
  - Medium: Removes filler words, light polishing (default)
  - Aggressive: Heavy editing for professional text
- [ ] Show description for each mode
- [ ] Display example before/after for each mode
- [ ] Add custom mode option (advanced users)

### API Key Management
- [ ] Implement secure API key input:
  - Password field (hidden characters)
  - Show/hide button
  - Validate key format (basic check)
  - Store securely in config file
- [ ] Add "Test Connection" functionality:
  - Send test request to API
  - Show success/failure status
  - Display error messages if failed
  - Show API rate limit info if available
- [ ] Option to load from `.env` file
- [ ] Option to use environment variable

### LLM Model Settings
- [ ] Add model selection dropdown:
  - Claude 3 Opus (highest quality)
  - Claude 3.5 Sonnet (balanced, recommended)
  - Claude 3 Haiku (fastest)
- [ ] Add temperature slider (0.0 - 1.0):
  - 0.0: Deterministic, consistent
  - 0.5: Balanced (default)
  - 1.0: Creative, varied
- [ ] Add max tokens setting (optional)
- [ ] Show cost estimate per request

### Text Processing Options
- [ ] Add filler word removal toggle (even without LLM)
- [ ] Configure which filler words to remove:
  - um, uh, like, you know, sort of, kind of, etc.
  - Custom filler words (user-defined list)
- [ ] Add capitalization options:
  - Auto-capitalize sentences
  - Preserve original capitalization
- [ ] Add punctuation options:
  - Auto-punctuate
  - Preserve original punctuation

### Privacy and Security
- [ ] Show privacy notice:
  - When LLM is enabled, text is sent to API
  - Link to Anthropic privacy policy
  - Remind that transcription is still local
- [ ] Add option to disable LLM for sensitive content
- [ ] Option to log/not log LLM requests
- [ ] Clear API key button (with confirmation)

### Integration
- [ ] Wire to existing text processing in `src/text_processing/`
- [ ] Update config file with LLM settings
- [ ] Handle API errors gracefully (network, auth, rate limit)
- [ ] Show LLM status in overlay (optional: "✨" icon when enabled)

## Success Criteria

- `cargo check` passes
- `cargo test` passes
- `cargo clippy -- -D warnings` passes
- `cargo fmt` applied
- Advanced tab displays correctly in settings window
- API key input is secure (not visible in plaintext)
- Test connection works and shows clear results
- Editing modes apply correctly to transcription
- LLM settings persist across restarts
- Errors are handled gracefully with user-friendly messages
- Privacy implications are clear to users

## Context

This is Phase 2, Step 5 of the UI expansion project. It builds on T-027 (Enhanced Settings Window) and makes the optional LLM feature more accessible and configurable.

**Current state:**
- LLM config: `.env` file with `ANTHROPIC_API_KEY`
- Text processing: `src/text_processing/`
- Editing modes: CLI flag `--editing-mode <light|medium|aggressive>`
- No GUI configuration available

**LLM integration details:**
- Optional feature (works without API key)
- Uses Anthropic Claude API
- Editing modes: light, medium (default), aggressive
- Filler word removal works independently of LLM

## Files to Create

- `src/settings/advanced_tab.rs` - Advanced tab UI implementation
- `src/settings/llm_config.rs` - LLM configuration management
- `src/settings/api_key_storage.rs` - Secure API key storage

## Files to Check/Modify

- `src/settings/window.rs` - Add advanced tab
- `src/text_processing/mod.rs` - LLM integration
- `src/text_processing/context.rs` - Editing mode settings
- `src/settings/config.rs` - Persist LLM configuration
- `Cargo.toml` - Ensure dotenvy is available

## Reference Documentation

- Text processing: `src/text_processing/mod.rs`
- Editing modes: `.ai/knowledge/conventions.md`
- Anthropic API: https://docs.anthropic.com/
- egui password fields: https://docs.rs/egui (TextEdit::password)

## Estimated Complexity

**Medium** - API key management, secure storage, API testing, UI integration. Estimated 1-2 days.

## Dependencies

- **Requires:** T-027 (Enhanced Settings Window) completed

## Testing Checklist

- [ ] Test API key input and storage
- [ ] Test connection with valid key
- [ ] Test connection with invalid key
- [ ] Test each editing mode (light, medium, aggressive)
- [ ] Test with LLM disabled
- [ ] Test filler word removal without LLM
- [ ] Verify API key is not visible in config file (plaintext)
- [ ] Test loading key from .env file
- [ ] Test loading key from environment variable
- [ ] Test error handling (network error, auth error, rate limit)

## Security Considerations

- **API Key Storage:** Consider using system keyring (keyring-rs crate) instead of plaintext config file
- **Config File Permissions:** Ensure config file is readable only by user (0600)
- **Memory Security:** Clear API key from memory when not in use
- **Logging:** Never log API keys, even in debug mode

## Future Enhancements

- Support multiple LLM providers (OpenAI, local models)
- LLM response caching to reduce API costs
- Batch processing for multiple transcriptions
- Custom prompt templates
- Usage statistics and cost tracking
- Offline mode with local LLM (llama.cpp, ggml)
