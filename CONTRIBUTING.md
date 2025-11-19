# Contributing to Hush

Thank you for your interest in contributing to Hush! This guide will help you get started with development.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Development Setup](#development-setup)
- [Project Structure](#project-structure)
- [Coding Standards](#coding-standards)
- [Testing](#testing)
- [Pull Request Process](#pull-request-process)
- [Documentation](#documentation)

## Code of Conduct

Be respectful, constructive, and collaborative. We want this to be a welcoming project for all contributors.

## Development Setup

### Prerequisites

- **Rust 1.70+** - Install from [rustup.rs](https://rustup.rs/)
- **CUDA 12.0+** (optional) - For GPU acceleration
- **System dependencies:**
  ```bash
  sudo apt install libasound2-dev pkg-config libx11-dev
  ```

### Getting Started

1. **Clone the repository:**
   ```bash
   git clone https://github.com/yourusername/hush.git
   cd hush
   ```

2. **Build the project:**
   ```bash
   make build
   # Or for release build
   make release
   ```

3. **Run tests:**
   ```bash
   cargo test
   ```

4. **Check code quality:**
   ```bash
   cargo clippy
   cargo fmt -- --check
   ```

## Project Structure

```
hush/
├── src/
│   ├── main.rs                    # Entry point
│   ├── lib.rs                     # Library exports
│   ├── core/                      # New trait-based architecture
│   │   ├── traits.rs              # Core trait definitions
│   │   ├── error.rs               # Structured error system
│   │   ├── state.rs               # State machine
│   │   └── mocks.rs               # Test mocks
│   ├── adapters/                  # Trait implementations
│   │   ├── audio/                 # Audio capture adapters
│   │   ├── transcription/         # Whisper adapters
│   │   └── text/                  # Text insertion adapters
│   ├── application/               # Application logic
│   ├── audio/                     # Audio capture (legacy)
│   ├── transcription/             # Whisper integration (legacy)
│   ├── text/                      # Text insertion (legacy)
│   ├── text_processing/           # Intelligent text processing
│   ├── overlay/                   # Visual overlay UI
│   ├── hotkey/                    # Global hotkey management
│   ├── config/                    # Configuration management
│   ├── logging/                   # Structured logging
│   └── cli/                       # Command-line interface
│       ├── dispatcher.rs          # Command routing
│       └── commands/              # Modular command handlers
│           ├── status.rs
│           ├── record.rs
│           └── utils.rs
├── tests/                         # Integration tests
├── .ai/                           # AI agent documentation
│   └── knowledge/                 # Knowledge base for AI agents
│       ├── architecture.md        # System architecture
│       ├── conventions.md         # Coding standards & patterns
│       ├── error-handling.md      # Error handling guide
│       └── adr-summary.md         # Architecture decisions
├── models/                        # Whisper models directory
└── config/                        # Configuration files
```

## Coding Standards

### Rust Style

We follow the [Rust Style Guide](https://doc.rust-lang.org/nightly/style-guide/). Key points:

- **Use `cargo fmt`** before committing
- **Max line length:** 100 characters
- **Imports:** Group by `std`, external crates, then local modules
- **Naming:**
  - Types: `PascalCase`
  - Functions/variables: `snake_case`
  - Constants: `SCREAMING_SNAKE_CASE`

### Error Handling

**Always use proper error handling. Never use `.unwrap()` or `.expect()` in production code.**

```rust
// ❌ BAD
let device = get_device().unwrap();

// ✅ GOOD
let device = get_device()
    .ok_or_else(|| AudioError::NoDeviceAvailable)?;

// ✅ ALSO GOOD
use crate::core::error::{HushError, AudioError};

fn capture_audio() -> Result<Vec<f32>, HushError> {
    let device = get_device()
        .ok_or_else(|| AudioError::NoDeviceAvailable)?;
    Ok(device.record())
}
```

See [.ai/knowledge/error-handling.md](.ai/knowledge/error-handling.md) for comprehensive guidelines.

### Documentation

All public items must have documentation:

```rust
/// Records audio from the default microphone
///
/// # Arguments
///
/// * `duration` - Recording duration in seconds
///
/// # Examples
///
/// ```no_run
/// let samples = record_audio(5)?;
/// println!("Recorded {} samples", samples.len());
/// ```
///
/// # Errors
///
/// Returns `AudioError::NoDeviceAvailable` if no microphone is found.
pub fn record_audio(duration: u64) -> Result<Vec<f32>, AudioError> {
    // Implementation
}
```

### Module Documentation

Every `mod.rs` should have module-level documentation:

```rust
/// Audio capture and feedback system for Hush voice-to-text
///
/// This module provides real-time audio recording with automatic device selection,
/// pre-allocated buffers for performance, and RMS amplitude calculation.
///
/// # Examples
///
/// ```no_run
/// use hush::audio::AudioCapture;
/// let capture = AudioCapture::new(None)?;
/// ```

pub mod capture;
pub mod feedback;
```

## Testing

### Running Tests

```bash
# All tests
cargo test

# Specific test
cargo test test_name

# With output
cargo test -- --nocapture

# Integration tests only
cargo test --test integration_tests
```

### Writing Tests

#### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sample_rate_validation() {
        let result = validate_sample_rate(16000);
        assert!(result.is_ok());

        let result = validate_sample_rate(99999);
        assert!(matches!(
            result,
            Err(AudioError::InvalidSampleRate { .. })
        ));
    }

    #[tokio::test]
    async fn test_async_function() {
        let result = async_operation().await;
        assert!(result.is_ok());
    }
}
```

#### Integration Tests

Create files in `tests/` directory:

```rust
// tests/command_tests.rs
use hush::cli::commands::handle_status;

#[tokio::test]
async fn test_status_command() {
    let result = handle_status(false, false, false).await;
    assert!(result.is_ok());
}
```

### Test Coverage

Aim for:
- **70%+** for core modules (audio, transcription, text)
- **90%+** for text_processing
- **50%+** for adapters

Check coverage:
```bash
cargo install cargo-tarpaulin
cargo tarpaulin --out Html
```

## Pull Request Process

### Before Submitting

1. **Run all checks:**
   ```bash
   cargo fmt
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test
   cargo build --release
   ```

2. **Update documentation** if you changed public APIs

3. **Add tests** for new functionality

4. **Update CHANGELOG.md** with your changes

### PR Guidelines

1. **One logical change per PR** - Don't mix unrelated changes

2. **Clear title and description:**
   ```
   Title: Add retry logic for transient transcription errors

   Description:
   - Implemented automatic retry for ErrorSeverity::Transient
   - Added exponential backoff (100ms, 200ms, 400ms)
   - Added tests for retry behavior
   - Updated ERROR_HANDLING.md documentation

   Fixes #123
   ```

3. **Small, focused commits:**
   ```bash
   git commit -m "Add retry logic for transient errors"
   git commit -m "Add tests for retry behavior"
   git commit -m "Update error handling documentation"
   ```

4. **Reference issues:** Use "Fixes #123" or "Relates to #456"

### CI Checks

All PRs must pass:
- ✅ Code formatting (`cargo fmt`)
- ✅ Linting (`cargo clippy`)
- ✅ Tests (`cargo test`)
- ✅ Build (`cargo build`)
- ✅ Security audit (`cargo audit`)

## Common Tasks

### Adding a New Command

Follow the modular command pattern documented in `.ai/knowledge/conventions.md`.

1. Create `src/cli/commands/mycommand.rs`
2. Implement `pub async fn handle_mycommand(...) -> Result<()>`
3. Add to `src/cli/commands/mod.rs`
4. Update dispatcher routing
5. Add tests
6. Update documentation

### Adding a New Error Type

1. Add variant to appropriate enum in `src/core/error.rs`:
   ```rust
   pub enum AudioError {
       // Existing errors...

       #[error("New error: {0}")]
       NewError(String),
   }
   ```

2. Update `severity()` method if needed

3. Update `user_message()` for user-friendly message

4. Add tests

### Fixing `.unwrap()` Usage

```bash
# Find all unwrap calls
grep -r "\.unwrap()" src/ --include="*.rs"

# Replace with proper error handling
# Before:
let value = some_function().unwrap();

# After:
let value = some_function()
    .context("Failed to do something")?;
```

## Documentation

### Types of Documentation

1. **Code documentation** - Inline doc comments (`///`)
2. **Module documentation** - Module-level docs in `mod.rs`
3. **User guides** - Markdown files in project root
4. **Architecture docs** - `.ai/knowledge/` directory for AI agents

### Documentation Standards

- **User guides:** Written for end users, focus on "how to"
- **Architecture docs:** Written for developers, focus on "why" and "how it works"
- **Code docs:** Written for API consumers, focus on usage and examples
- **Comments:** Used sparingly, only for complex logic

### Updating Documentation

When you make changes:

1. **Public API changes** → Update inline docs and potentially `README.md`
2. **Architecture changes** → Update `.ai/knowledge/architecture.md` or `.ai/knowledge/adr-summary.md`
3. **New features** → Update relevant user guides
4. **Coding patterns** → Update `.ai/knowledge/conventions.md`

## Getting Help

- **Questions:** Open a GitHub Discussion
- **Bugs:** Open a GitHub Issue
- **Security:** Email security@example.com (do not open public issues)

## License

By contributing, you agree that your contributions will be licensed under the same license as the project (TBD).

## Recognition

Contributors will be recognized in:
- GitHub contributors page
- CHANGELOG.md for significant contributions
- README.md for major features

Thank you for contributing to Hush! 🤫
