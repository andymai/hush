# CLI Module Patterns for Hush

**Last Updated:** 2025-11-19
**Status:** Reference for AI agents working on CLI

This document outlines CLI command organization, patterns, and refactoring strategies for the Hush command-line interface.

---

## Overview

Hush CLI follows a **modular command pattern** where:
- Each subcommand has its own dedicated file
- Dispatcher routes commands to handler functions
- Command handlers are self-contained and testable
- Consistent structure across all commands

---

## CLI Architecture

### Directory Structure

```
src/cli/
├── mod.rs                 # CLI types and argument definitions (Clap)
├── dispatcher.rs          # Command routing and dispatching
└── commands/              # Individual command handlers
    ├── mod.rs            # Re-exports all command handlers
    ├── listen.rs         # handle_listen()
    ├── record.rs         # handle_record()
    ├── status.rs         # handle_status()
    ├── manual.rs         # handle_manual()
    ├── setup.rs          # handle_setup()
    └── test.rs           # handle_test()
```

### Data Flow

```
User Input (CLI args)
  → clap parsing (src/cli/mod.rs)
  → Commands enum
  → dispatcher.rs routes to handler
  → src/cli/commands/{command}.rs executes
  → Result back to user
```

---

## Command Definition Pattern

### Step 1: Define in `src/cli/mod.rs`

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "hush")]
#[command(about = "Fast, accurate voice-to-text for Linux developers")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start listening mode (hotkey-activated voice-to-text)
    Listen {
        #[arg(long, help = "Skip setup checks")]
        skip_setup: bool,
    },

    /// Record once without hotkey
    Record {
        #[arg(short, long, default_value = "5")]
        duration: u64,

        #[arg(long)]
        save: bool,

        #[arg(short, long, help = "Output file path")]
        output: Option<PathBuf>,
    },

    /// Show system status and configuration
    Status {
        #[arg(long, help = "Show detailed status")]
        full: bool,
    },

    // ... other commands
}
```

**Key patterns:**
- Use doc comments (`///`) for help text
- Use `#[arg]` attributes for argument configuration
- Group related arguments in the enum variant
- Provide sensible defaults with `default_value`

### Step 2: Create Handler in `src/cli/commands/{name}.rs`

```rust
// src/cli/commands/record.rs

use crate::core::error::HushError;
use anyhow::Result;
use std::path::PathBuf;

/// Handles the `record` subcommand
///
/// Records audio for a specified duration and optionally saves or transcribes it.
///
/// # Arguments
///
/// * `duration` - Recording duration in seconds
/// * `save` - Whether to save the audio to a file
/// * `output` - Optional output file path
///
/// # Returns
///
/// * `Ok(())` on successful recording
/// * `Err(HushError)` on recording failure
///
/// # Examples
///
/// ```no_run
/// // Record for 5 seconds and transcribe
/// handle_record(5, false, None)?;
///
/// // Record for 10 seconds and save to file
/// handle_record(10, true, Some(PathBuf::from("recording.wav")))?;
/// ```
pub fn handle_record(
    duration: u64,
    save: bool,
    output: Option<PathBuf>,
) -> Result<()> {
    // Implementation
    println!("Recording for {} seconds...", duration);

    // Use HushApp or other components
    // ...

    Ok(())
}
```

**Documentation requirements:**
- Module-level doc comment explaining the command
- Comprehensive doc comment on handler function
- Document all parameters
- Document return values (Ok and Err cases)
- Provide usage examples if helpful

### Step 3: Export from `src/cli/commands/mod.rs`

```rust
// src/cli/commands/mod.rs

pub mod listen;
pub mod record;
pub mod status;
pub mod manual;
pub mod setup;
pub mod test;

// Re-export handler functions
pub use listen::handle_listen;
pub use record::handle_record;
pub use status::handle_status;
pub use manual::handle_manual;
pub use setup::handle_setup;
pub use test::handle_test;
```

**Pattern:** Always re-export handler functions for easy access

### Step 4: Route in `src/cli/dispatcher.rs`

```rust
// src/cli/dispatcher.rs

use crate::cli::{Cli, Commands};
use crate::cli::commands;
use anyhow::Result;

pub fn dispatch(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Listen { skip_setup } => {
            commands::handle_listen(skip_setup)
        }

        Commands::Record { duration, save, output } => {
            commands::handle_record(duration, save, output)
        }

        Commands::Status { full } => {
            commands::handle_status(full)
        }

        Commands::Manual { text } => {
            commands::handle_manual(&text)
        }

        Commands::Setup { command } => {
            commands::handle_setup(command)
        }

        Commands::Test { command } => {
            commands::handle_test(command)
        }
    }
}
```

**Key patterns:**
- Match on `Commands` enum
- Extract arguments from enum variants
- Call corresponding handler function
- Propagate Result with `?`

---

## Command Handler Patterns

### Simple Command (No Dependencies)

```rust
// src/cli/commands/status.rs

use anyhow::Result;

/// Handles the `status` subcommand
///
/// Displays system status including:
/// - Audio device availability
/// - CUDA/GPU status
/// - Model availability
/// - Configuration
pub fn handle_status(full: bool) -> Result<()> {
    println!("Hush Status");
    println!("===========");

    // Check audio
    println!("Audio: {}", check_audio_status());

    // Check GPU
    println!("GPU: {}", check_gpu_status());

    if full {
        // Show detailed info
        println!("\nDetailed Status:");
        show_detailed_status();
    }

    Ok(())
}

fn check_audio_status() -> &'static str {
    // Implementation
    "Available"
}

fn check_gpu_status() -> &'static str {
    // Implementation
    "CUDA Available"
}

fn show_detailed_status() {
    // Implementation
}
```

**Pattern:** Helper functions keep handler clean

### Command with Dependencies (Builder Pattern)

```rust
// src/cli/commands/listen.rs

use crate::application::HushAppBuilder;
use crate::core::error::HushError;
use anyhow::Result;

/// Handles the `listen` subcommand
///
/// Starts the application in listening mode with hotkey activation.
pub fn handle_listen(skip_setup: bool) -> Result<()> {
    if !skip_setup {
        // Run setup checks
        verify_uinput_available()?;
        verify_audio_device()?;
    }

    // Build application
    let app = HushAppBuilder::new()
        .with_default_audio()?
        .with_default_transcriber()?
        .with_default_text_output()?
        .with_default_trigger()?
        .daemon_mode()
        .build()?;

    // Run application
    println!("Listening... Press Ctrl+Alt+V to record");
    app.run().await?;

    Ok(())
}

fn verify_uinput_available() -> Result<()> {
    // Check /dev/uinput access
    if !std::path::Path::new("/dev/uinput").exists() {
        return Err(HushError::Setup("UInput not available".into()).into());
    }
    Ok(())
}

fn verify_audio_device() -> Result<()> {
    // Check audio device
    Ok(())
}
```

**Pattern:** Use builder to construct dependencies

### Command with Subcommands

```rust
// src/cli/mod.rs - Define subcommand enum

#[derive(Subcommand)]
pub enum Commands {
    Setup {
        #[command(subcommand)]
        command: SetupCommands,
    },
    // ...
}

#[derive(Subcommand)]
pub enum SetupCommands {
    /// Setup UInput for text insertion
    Uinput {
        #[arg(long, help = "Quick setup without prompts")]
        quick: bool,
    },

    /// Diagnose UInput issues
    DiagnoseUinput,

    /// Setup CUDA for GPU acceleration
    Cuda,
}

// src/cli/commands/setup.rs - Handle subcommands

use crate::cli::SetupCommands;
use anyhow::Result;

/// Handles the `setup` subcommand and its subcommands
pub fn handle_setup(command: SetupCommands) -> Result<()> {
    match command {
        SetupCommands::Uinput { quick } => {
            setup_uinput(quick)
        }

        SetupCommands::DiagnoseUinput => {
            diagnose_uinput()
        }

        SetupCommands::Cuda => {
            setup_cuda()
        }
    }
}

fn setup_uinput(quick: bool) -> Result<()> {
    if quick {
        println!("Quick UInput setup...");
        // Run automated setup
    } else {
        println!("Interactive UInput setup...");
        // Interactive prompts
    }
    Ok(())
}

fn diagnose_uinput() -> Result<()> {
    println!("Diagnosing UInput...");
    // Check /dev/uinput, permissions, etc.
    Ok(())
}

fn setup_cuda() -> Result<()> {
    println!("CUDA setup...");
    // Check CUDA installation
    Ok(())
}
```

**Pattern:** Nested match for subcommands

---

## Error Handling in CLI

### Converting Domain Errors to User Messages

```rust
use crate::core::error::HushError;
use anyhow::{Result, Context};

pub fn handle_record(duration: u64) -> Result<()> {
    // Domain code returns HushError
    let result = record_audio(duration)
        .context("Failed to record audio")?;

    // Convert to user-friendly output
    match result {
        Ok(audio) => {
            println!("Recorded {} samples", audio.len());
            Ok(())
        }
        Err(HushError::Audio(audio_err)) => {
            eprintln!("Audio error: {}", audio_err.user_message());
            Err(audio_err.into())
        }
        Err(e) => Err(e.into()),
    }
}
```

**Pattern:** Use `anyhow` at CLI boundary, `HushError` in domain

### Graceful Error Display

```rust
pub fn handle_command() -> Result<()> {
    match risky_operation() {
        Ok(result) => {
            println!("✅ Success: {}", result);
            Ok(())
        }
        Err(e) => {
            eprintln!("❌ Error: {}", e);
            eprintln!("\nTry:");
            eprintln!("  - Check configuration");
            eprintln!("  - Run with --verbose for details");
            Err(e)
        }
    }
}
```

**Pattern:** Provide actionable error messages

---

## Refactoring CLI Commands

### When to Extract a Command

**Extract when:**
- ✅ Function is >20 lines in dispatcher
- ✅ Command has multiple responsibilities
- ✅ Command needs unit testing
- ✅ Command logic is duplicated
- ✅ Following established pattern (consistency)

**Don't extract when:**
- ❌ Function is <10 lines
- ❌ Command is trivial (just prints help)
- ❌ Would be only command not extracted

### Safe Refactoring Process

**Step 1: Verify current implementation**

```bash
# Find the command in dispatcher
rg "Commands::Record" src/cli/dispatcher.rs --context=10

# Check if it already has a handler
rg "handle_record" src/cli/

# Run tests to ensure they pass
cargo test
```

**Step 2: Create handler file**

```bash
# Create file
touch src/cli/commands/record.rs

# Add to mod.rs
# pub mod record;
# pub use record::handle_record;
```

**Step 3: Move implementation**

```rust
// Copy implementation from dispatcher.rs to record.rs
// Add proper documentation
// Add error handling
```

**Step 4: Update imports and dispatcher**

```rust
// In dispatcher.rs, replace:
Commands::Record { ... } => {
    // Old implementation here
}

// With:
Commands::Record { duration, save, output } => {
    commands::handle_record(duration, save, output)
}
```

**Step 5: Verify**

```bash
# Compile
cargo check

# Run tests
cargo test

# Run clippy
cargo clippy -- -D warnings

# Format
cargo fmt

# Test command manually
cargo run -- record --duration 5
```

**Step 6: Commit**

```bash
git add src/cli/commands/record.rs
git add src/cli/commands/mod.rs
git add src/cli/dispatcher.rs
git commit -m "Extract handle_record command

Moves record command implementation from dispatcher to dedicated
module following the pattern established by other commands."
```

---

## Testing CLI Commands

### Unit Testing Command Handlers

```rust
// src/cli/commands/record.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_record_valid_duration() {
        let result = handle_record(5, false, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_record_with_save() {
        let output = Some(PathBuf::from("/tmp/test.wav"));
        let result = handle_record(5, true, output);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_record_zero_duration() {
        let result = handle_record(0, false, None);
        assert!(result.is_err());
    }
}
```

### Integration Testing CLI

```rust
// tests/cli_integration.rs

use assert_cmd::Command;

#[test]
fn test_record_command() {
    let mut cmd = Command::cargo_bin("hush").unwrap();
    cmd.arg("record")
        .arg("--duration")
        .arg("5");

    cmd.assert().success();
}

#[test]
fn test_status_command() {
    let mut cmd = Command::cargo_bin("hush").unwrap();
    cmd.arg("status");

    cmd.assert()
        .success()
        .stdout(predicates::str::contains("Hush Status"));
}

#[test]
fn test_invalid_command() {
    let mut cmd = Command::cargo_bin("hush").unwrap();
    cmd.arg("invalid");

    cmd.assert()
        .failure()
        .stderr(predicates::str::contains("unrecognized subcommand"));
}
```

---

## Best Practices

### ✅ DO

**Use consistent naming:**
```rust
// Command: record
// Handler: handle_record()
// File: src/cli/commands/record.rs
```

**Document all public functions:**
```rust
/// Handles the `record` subcommand
///
/// [Description]
///
/// # Arguments
/// # Returns
/// # Examples
pub fn handle_record(...) -> Result<()> { }
```

**Keep handlers focused:**
```rust
// Good - single responsibility
pub fn handle_record(...) -> Result<()> {
    let audio = record_audio()?;
    save_or_transcribe(audio)?;
    Ok(())
}

// Bad - doing too much
pub fn handle_record(...) -> Result<()> {
    // 100 lines of mixed concerns
}
```

**Provide helpful output:**
```rust
println!("✅ Recording saved to {}", path.display());
eprintln!("❌ Failed to access microphone");
eprintln!("💡 Tip: Check 'hush status' for diagnostics");
```

**Use clap attributes for validation:**
```rust
#[arg(short, long, value_parser = clap::value_parser!(u64).range(1..=300))]
duration: u64,  // Validates 1-300 seconds
```

### ❌ DON'T

**Don't mix UI and logic:**
```rust
// Bad
pub fn handle_record(...) -> Result<()> {
    println!("Starting...");
    let audio = complicated_audio_logic();  // Mixed
    println!("Done");
    Ok(())
}

// Good
pub fn handle_record(...) -> Result<()> {
    println!("Starting...");
    let audio = audio::record()?;  // Separate logic
    println!("Done");
    Ok(())
}
```

**Don't duplicate code across commands:**
```rust
// Bad - duplicated in multiple commands
fn check_uinput() { ... }  // In listen.rs
fn check_uinput() { ... }  // In record.rs

// Good - shared utility
// src/cli/utils.rs
pub fn verify_uinput() -> Result<()> { ... }
```

**Don't ignore errors:**
```rust
// Bad
let _ = save_audio();  // Error ignored

// Good
save_audio()
    .context("Failed to save audio to file")?;
```

**Don't use unwrap in CLI code:**
```rust
// Bad
let config = load_config().unwrap();

// Good
let config = load_config()
    .context("Failed to load config from ~/.config/hush/config.toml")?;
```

---

## Quick Reference

### Adding a New CLI Command

```bash
# 1. Define command in src/cli/mod.rs (Commands enum)
# 2. Create src/cli/commands/{name}.rs
# 3. Implement handle_{name}() function
# 4. Add to src/cli/commands/mod.rs (pub mod and pub use)
# 5. Route in src/cli/dispatcher.rs
# 6. Test: cargo run -- {name} --help
# 7. Add tests in src/cli/commands/{name}.rs
# 8. Run cargo check && cargo test && cargo clippy
# 9. Commit
```

### Finding CLI Code

```bash
# Find command definitions
rg "pub enum Commands" src/cli/mod.rs

# Find command handlers
rg "pub fn handle_" src/cli/commands/

# Find dispatcher routing
rg "Commands::" src/cli/dispatcher.rs

# Find command usage
rg "cargo run -- record" --type md
```

### Command Template

```rust
// src/cli/commands/new_command.rs

use anyhow::Result;

/// Handles the `new-command` subcommand
///
/// [Description of what this command does]
///
/// # Arguments
///
/// * `arg1` - Description
/// * `arg2` - Description
///
/// # Returns
///
/// * `Ok(())` on success
/// * `Err(...)` on failure with descriptive message
pub fn handle_new_command(arg1: Type1, arg2: Type2) -> Result<()> {
    // Implementation

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_new_command_success() {
        let result = handle_new_command(valid_arg1, valid_arg2);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_new_command_error() {
        let result = handle_new_command(invalid_arg1, valid_arg2);
        assert!(result.is_err());
    }
}
```

---

## Related Documentation

- `src/cli/mod.rs` - Command definitions with clap
- `src/cli/dispatcher.rs` - Command routing
- `src/cli/commands/` - Individual command handlers
- `.ai/knowledge/conventions.md` - General coding conventions
- `.ai/knowledge/error-handling.md` - Error handling patterns

---

**Remember:** CLI is the user's first interaction with Hush. Make it intuitive, provide helpful messages, and handle errors gracefully. Consistency across commands creates a polished user experience.
