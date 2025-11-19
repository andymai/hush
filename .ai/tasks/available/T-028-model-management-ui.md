# Task: Model Management UI - Selection and Info Display

## Description

Add a "Models" tab to the settings window that displays available Whisper models, shows model information (size, speed, accuracy), allows switching between installed models, and displays model status. This does NOT include the download UI (that's T-029).

## Requirements

### Models Tab UI
- [ ] Create "Models" tab in settings window
- [ ] Display list of available Whisper models (tiny, base, small, medium, large)
- [ ] Show model information cards:
  - Model name and size (e.g., "Base - 145 MB")
  - Speed estimate (CPU/GPU)
  - Accuracy rating
  - Download status (✓ Installed, ⌛ Not installed)
  - Current model indicator (radio button or checkmark)
- [ ] Implement model selection (radio buttons)
- [ ] Add "Set as Active" button
- [ ] Show storage location and total space used

### Model Info Integration
- [ ] Create `src/settings/model_info.rs` with model metadata
- [ ] Define model specs: size, CPU speed, GPU speed, accuracy level
- [ ] Add model descriptions and use case recommendations
- [ ] Implement model detection (check if model files exist)

### Model Switching
- [ ] Wire model selection to transcription engine
- [ ] Add confirmation dialog for model switch
- [ ] Update config file when model changes
- [ ] Show loading state during model switch
- [ ] Handle errors gracefully (model not found, load failure)

### Status Display
- [ ] Show currently active model prominently
- [ ] Display model load status (loaded, not loaded, loading)
- [ ] Show storage usage: "Using 612 MB of 2.9 GB available models"
- [ ] Add tooltips with detailed information on hover

## Success Criteria

- `cargo check` passes
- `cargo test` passes
- `cargo clippy -- -D warnings` passes
- `cargo fmt` applied
- Models tab displays correctly in settings window
- Model information is accurate and helpful
- Model switching works without errors
- Current model is clearly indicated
- Storage usage is calculated correctly
- Settings persist after restart

## Context

This is Phase 2, Step 1 of the UI expansion project. It builds on T-027 (Enhanced Settings Window) and focuses on displaying and selecting models, but NOT downloading them.

**Current state:**
- Model management is CLI-only: `./hush models download <model>`
- Models stored in: `models/` directory
- Model loading: `src/transcription/whisper.rs`

**Model specifications (from research):**

| Model  | Size    | CPU Speed | GPU Speed | Accuracy |
|--------|---------|-----------|-----------|----------|
| Tiny   | 75 MB   | ~2-3s     | ~0.3s     | Basic    |
| Base   | 145 MB  | ~4-6s     | ~0.5s     | Good     |
| Small  | 466 MB  | ~8-12s    | ~0.8s     | Better   |
| Medium | 1.5 GB  | ~15-25s   | ~1.5s     | High     |
| Large  | 2.9 GB  | ~30-45s   | ~2.5s     | Highest  |

## Files to Create

- `src/settings/model_info.rs` - Model metadata and specifications
- `src/settings/models_tab.rs` - Models tab UI implementation

## Files to Check/Modify

- `src/settings/window.rs` - Add models tab
- `src/transcription/whisper.rs` - Model loading integration
- `src/settings/config.rs` - Persist model selection
- `.ai/knowledge/rust-ui-frameworks-research.md` - UI recommendations

## Reference Documentation

- Model specs: README.md (lines 341-349)
- Model manager: `src/bin/model-manager.rs`
- Whisper integration: `src/transcription/whisper.rs`
- `.ai/knowledge/conventions.md` - Coding standards

## Existing Patterns to Follow

Search the codebase for patterns before implementing:
```bash
# Find config patterns
rg "pub struct.*Config" src/config/ --type rust

# Find state patterns
rg "pub enum.*State" src/ --type rust

# Find error handling patterns
rg "pub enum.*Error" src/core/error.rs --type rust
```

**Follow existing Hush patterns:**
- State-based UI (see `src/overlay/ui.rs`)
- Structured error types (see `src/core/error.rs`)
- Config with serde (see `src/config/settings.rs`)

## Estimated Complexity

**Medium** - Model metadata management, UI layout, integration with existing model system. Estimated 1-2 days.

## Dependencies

- **Requires:** T-027 (Enhanced Settings Window) completed
- **Blocks:** T-029 (Model Download UI)

## Future Enhancements

- T-029 will add download functionality to this tab
- Could add model benchmarking (actual speed test)
- Could add model auto-selection based on GPU availability
