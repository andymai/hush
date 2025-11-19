# Task: First-Run Setup Wizard

## Description

Create a guided setup wizard that runs on first launch to help new users configure Hush properly. The wizard should walk through essential setup steps: model download, UInput verification, audio device selection, and optional LLM configuration.

## Requirements

### Wizard Infrastructure
- [ ] Create `src/settings/wizard.rs` for wizard implementation
- [ ] Detect first run (no config file exists)
- [ ] Launch wizard automatically on first run
- [ ] Allow manual launch from CLI: `./hush setup wizard`
- [ ] Implement wizard state machine (intro → steps → complete)
- [ ] Add progress indicator showing current step (e.g., "Step 2 of 5")
- [ ] Support back/next navigation between steps
- [ ] Add skip wizard option (with warning)

### Wizard Steps

#### Step 1: Welcome Screen
- [ ] Show Hush logo and welcome message
- [ ] Brief explanation of what Hush does
- [ ] Estimated setup time: ~5 minutes
- [ ] Privacy notice: "All transcription is local"
- [ ] "Get Started" button

#### Step 2: Model Selection and Download
- [ ] Show model comparison table:
  - Model names with size
  - Speed estimates (CPU vs GPU)
  - Accuracy ratings
  - Recommended choice highlighted
- [ ] Explain: "Models run locally for privacy"
- [ ] Detect GPU availability and recommend accordingly:
  - GPU detected: Recommend "Base" or "Small"
  - No GPU: Recommend "Tiny" or "Base"
- [ ] Allow model selection (radio buttons)
- [ ] Start download automatically when "Next" clicked
- [ ] Show download progress (reuse T-029 downloader)
- [ ] Handle download errors with retry option
- [ ] Skip option if model already exists

#### Step 3: UInput Verification
- [ ] Check UInput status automatically
- [ ] Show status indicators:
  - ✓ uinput module loaded
  - ✓ /dev/uinput exists
  - ✓ /dev/uinput is readable/writable
  - ✗ Issues detected
- [ ] If issues detected:
  - Explain what UInput does
  - Show fix commands with copy button
  - Offer "Auto-fix" button (runs setup commands)
  - Require sudo password for auto-fix
- [ ] Test UInput by typing "test" in background
- [ ] If all checks pass, show success and continue
- [ ] Link to detailed guide: `./hush setup uinput`

#### Step 4: Audio Device Selection
- [ ] List available audio input devices
- [ ] Auto-select default device
- [ ] Show live audio level meter for selected device
- [ ] Add "Test Recording" button:
  - Record 3 seconds
  - Play back audio
  - Show waveform
- [ ] Verify audio is clear and loud enough
- [ ] Troubleshooting tips if no audio detected

#### Step 5: Hotkey Configuration
- [ ] Show default hotkey: Ctrl+Alt+V
- [ ] Explain push-to-talk workflow
- [ ] Offer to customize hotkey (optional)
- [ ] Test hotkey by pressing it
- [ ] Show visual confirmation when hotkey pressed
- [ ] Skip if user wants to use default

#### Step 6: Optional LLM Setup
- [ ] Explain LLM text polishing feature (optional)
- [ ] Show before/after examples
- [ ] Offer to configure API key
- [ ] Link to get API key: https://console.anthropic.com/
- [ ] Test API connection if key provided
- [ ] Skip if user doesn't want LLM
- [ ] Emphasize: "This is optional, Hush works without it"

#### Step 7: Completion Screen
- [ ] Show success message: "You're all set!"
- [ ] Summary of configuration:
  - Model: [selected model]
  - Audio device: [selected device]
  - Hotkey: [configured hotkey]
  - LLM: [enabled/disabled]
- [ ] Quick start guide:
  - Hold Ctrl+Alt+V (or custom hotkey)
  - Speak
  - Release
  - Text appears!
- [ ] Buttons:
  - "Start Using Hush" (launch main app)
  - "Open Settings" (open settings window)
  - "Exit" (quit)

### Configuration Persistence
- [ ] Save configuration after completion
- [ ] Create config file: `~/.config/hush/settings.toml`
- [ ] Set first_run = false in config
- [ ] Store all selected settings
- [ ] Create log entry for setup completion

### Error Handling
- [ ] Handle network errors (model download)
- [ ] Handle permission errors (UInput setup)
- [ ] Handle missing audio devices
- [ ] Handle invalid API keys
- [ ] Provide clear error messages and solutions
- [ ] Allow retry or skip on errors

## Success Criteria

- `cargo check` passes
- `cargo test` passes
- `cargo clippy -- -D warnings` passes
- `cargo fmt` applied
- Wizard launches on first run
- All steps complete successfully
- Configuration is saved correctly
- Hush works immediately after wizard completion
- Wizard can be manually launched
- Errors are handled gracefully

## Context

This is Phase 3 of the UI expansion project. It builds on all previous settings tasks and provides a guided onboarding experience for new users.

**Current state:**
- Manual setup: README.md and INSTALL.md
- CLI commands for each step
- No guided onboarding
- New users must read documentation

**User experience goal:**
- Zero documentation required
- 5-minute setup
- Clear, visual, guided
- Works immediately after setup

## Files to Create

- `src/settings/wizard.rs` - Main wizard implementation
- `src/settings/wizard_steps.rs` - Individual step implementations
- `src/settings/wizard_state.rs` - Wizard state management

## Files to Check/Modify

- `src/main.rs` - Detect first run and launch wizard
- `src/settings/window.rs` - Integration point
- All previous settings tasks (T-027 through T-032)
- `src/cli/commands/setup.rs` - Reuse setup logic

## Reference Documentation

- Setup guide: INSTALL.md, SETUP.md
- Model specs: README.md (lines 341-349)
- UInput setup: `.ai/knowledge/uinput-guide.md`
- egui wizard patterns: Look for multi-step form examples

## Estimated Complexity

**Medium-High** - Multi-step wizard, state management, integration with all previous tasks, error handling. Estimated 2-3 days.

## Dependencies

- **Requires:** T-027 (Enhanced Settings Window) completed
- **Requires:** T-028 (Model Management UI) completed
- **Requires:** T-029 (Model Download UI) completed
- **Requires:** T-030 (Audio Device Picker) completed
- **Requires:** T-031 (Hotkey Configurator) completed
- **Strongly Benefits From:** T-032 (LLM Configuration UI) completed

## Testing Checklist

- [ ] Test wizard on fresh install (no config file)
- [ ] Test each step individually
- [ ] Test back button navigation
- [ ] Test skip wizard option
- [ ] Test with GPU detected
- [ ] Test without GPU detected
- [ ] Test model download step
- [ ] Test UInput verification (both passing and failing)
- [ ] Test audio device selection
- [ ] Test hotkey configuration
- [ ] Test LLM setup (with and without API key)
- [ ] Test completion screen
- [ ] Test manual wizard launch: `./hush setup wizard`
- [ ] Test wizard with network errors
- [ ] Test wizard with permission errors

## Future Enhancements

- Video tutorials embedded in wizard
- Interactive demo mode (try transcription in wizard)
- Preset configurations (developer, writer, casual user)
- Skip individual steps if already configured
- Wizard progress saved (resume if interrupted)
- Accessibility mode setup
- Language selection for non-English users
