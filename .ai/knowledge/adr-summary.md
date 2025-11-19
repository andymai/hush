# Architecture Decision Records Summary

**Last Updated:** 2025-11-19
**Status:** Reference for AI agents

This document consolidates the key architecture decisions for the Hush project. These decisions guide implementation and ensure consistency across the codebase.

## Overview

Hush has adopted a **trait-based architecture** with:
- Component decoupling through traits
- Structured error handling for recovery
- Centralized state management for consistency

## ADR-001: Trait-Based Architecture

**Status:** Partially Implemented (UInput complete Oct 2025, other components in progress)
**Decision Date:** 2025-10-06

### Problem

The codebase suffered from tight coupling - all dependencies in `HushApp` were concrete types, making testing impossible without real hardware (microphone, X11, GPU).

**Impact:**
- Zero meaningful unit tests
- Cannot test without hardware
- Adding Wayland support requires rewriting
- Cannot swap implementations

### Solution

Adopt **trait-based architecture** using the **Adapter Pattern**:

1. Define traits for all component interfaces
2. Implement adapters for existing concrete types
3. Refactor HushApp to accept trait objects
4. Create mock implementations for testing
5. Isolate platform-specific code behind traits

### Core Traits

```rust
// Core component traits
pub trait AudioSource: Send + Sync { /* ... */ }
pub trait Transcriber: Send + Sync { /* ... */ }
pub trait TextOutput: Send + Sync { /* ... */ }
pub trait InputTrigger: Send + Sync { /* ... */ }

// Platform abstraction
pub trait DisplayServer: Send + Sync { /* ... */ }
pub trait HotkeyProvider: Send + Sync { /* ... */ }
```

### Key Files

- `src/core/traits.rs` - All trait definitions
- `src/adapters/` - Platform-specific implementations
- `src/core/mocks.rs` - Mock implementations for testing

### Benefits

✅ **Testability** - Can inject mock implementations
✅ **Extensibility** - Add new implementations without modifying core
✅ **Platform Independence** - Platform code isolated in adapters
✅ **Maintainability** - Clear contracts between components

### Trade-offs

❌ **More Code** - Trait definitions + adapters add ~1000-2000 LOC
❌ **Runtime Overhead** - Dynamic dispatch (<0.1% impact, negligible)
❌ **Learning Curve** - More abstraction layers to understand

### Implementation Notes

**For AI agents implementing traits:**

1. **Always verify trait exists first:**
   ```bash
   rg "pub trait AudioSource" src/core/traits.rs
   ```

2. **Check existing implementations:**
   ```bash
   rg "impl AudioSource for" --type rust
   ```

3. **All trait implementations must be Send + Sync** for async usage

4. **Use Box<dyn Trait> for trait objects:**
   ```rust
   pub struct HushApp {
       audio: Box<dyn AudioSource>,
       transcriber: Box<dyn Transcriber>,
   }
   ```

## ADR-002: Structured Error Handling

**Status:** Partially Implemented (Oct 2025)
**Decision Date:** 2025-10-06

### Problem

Error handling was inconsistent - used `anyhow::Result<T>` everywhere with no pattern matching, making error recovery impossible.

**Impact:**
- Cannot implement error-specific recovery logic
- All errors treated identically
- No user-friendly error messages
- Cannot distinguish transient vs permanent failures

### Solution

Adopt **structured error handling using `thiserror`** with layered approach:

1. **Domain Layer** - `thiserror` for structured errors (components)
2. **Application Layer** - `anyhow` for flexibility (CLI, main)
3. **Error Severity** - Transient, Recoverable, Fatal classification
4. **User Messages** - Actionable error messages with guidance

### Error Structure

```rust
// Top-level domain error
pub enum HushError {
    Audio(AudioError),
    Transcription(TranscriptionError),
    TextOutput(TextOutputError),
    InputTrigger(InputTriggerError),
    Config(ConfigError),
    State(StateError),
}

// Domain-specific errors with variants
pub enum AudioError {
    NoDeviceAvailable,
    DeviceNotFound(String),
    RecordingStartFailed(String),
    InvalidSampleRate { hz: u32, min: u32, max: u32 },
    // ... more variants
}

// Error severity for recovery
pub enum ErrorSeverity {
    Transient,    // Retry automatically
    Recoverable,  // User action needed
    Fatal,        // Must exit
}
```

### Key Files

- `src/core/error.rs` - All error type definitions
- `src/core/error_handler.rs` - Error handling logic

### Benefits

✅ **Pattern Matchable** - Implement specific error handling
✅ **User-Friendly** - Each error has actionable advice
✅ **Recovery Logic** - Retry transient errors automatically
✅ **Type Safety** - Compiler ensures errors are handled

### Implementation Notes

**For AI agents handling errors:**

1. **Always use domain-specific errors:**
   ```rust
   // Good
   return Err(AudioError::DeviceNotFound(name).into());

   // Bad
   return Err(anyhow!("Device not found"));
   ```

2. **Check error definitions first:**
   ```bash
   rg "pub enum.*Error" src/core/error.rs
   ```

3. **Provide actionable messages:**
   ```rust
   HushError::user_message() // Returns user-friendly guidance
   ```

4. **Never use unwrap() or panic()** - Always handle errors properly

5. **Use severity classification:**
   ```rust
   match error.severity() {
       ErrorSeverity::Transient => retry(),
       ErrorSeverity::Recoverable => show_guidance(),
       ErrorSeverity::Fatal => exit(),
   }
   ```

## ADR-003: Centralized State Management

**Status:** Partially Implemented (Oct 2025, Overlay state added Nov 2025)
**Decision Date:** 2025-10-06

### Problem

The codebase had **duplicated and fragmented state** - two `is_recording` flags that could become inconsistent, leading to race conditions and bugs.

**Impact:**
- No single source of truth
- Race conditions possible
- Difficult to debug
- Invalid states possible

### Solution

Implement a **centralized state machine** with:

1. Type-safe state transitions
2. Observer pattern for state change notifications
3. State history for debugging
4. Validated transitions

### State Machine Design

```rust
// Application states
pub enum AppState {
    Idle,
    Recording { started_at: Instant },
    Transcribing { audio_duration: Duration },
    Inserting { text_length: usize },
    Error { recoverable: bool },
}

// State machine with validated transitions
pub struct StateMachine {
    current: Arc<RwLock<AppState>>,
    observers: Arc<RwLock<Vec<Box<dyn StateObserver>>>>,
    history: Arc<RwLock<Vec<StateTransition>>>,
}

// Observer trait
pub trait StateObserver: Send + Sync {
    fn on_state_change(&self, old_state: AppState, new_state: AppState);
}
```

### Valid Transitions

```
Idle → Recording
Recording → Transcribing | Idle | Error
Transcribing → Inserting | Idle | Error
Inserting → Idle | Error
Error → Idle
```

### Key Files

- `src/core/state.rs` - State machine implementation

### Benefits

✅ **Single Source of Truth** - One state machine, no duplication
✅ **Type-Safe** - Compiler enforces valid transitions
✅ **Impossible States Prevented** - Cannot have inconsistent state
✅ **Observable** - Components react to state changes
✅ **Debuggable** - State history available
✅ **Thread-Safe** - RwLock ensures safe concurrent access

### Implementation Notes

**For AI agents working with state:**

1. **Check state definitions:**
   ```bash
   rg "pub enum AppState" src/core/state.rs
   ```

2. **Always use state machine for transitions:**
   ```rust
   // Good
   state_machine.transition(AppState::Recording {
       started_at: Instant::now()
   })?;

   // Bad
   self.is_recording = true; // Direct state modification
   ```

3. **Check if transition is valid:**
   ```rust
   // State machine validates automatically
   // Invalid transitions return Err(StateError::InvalidTransition)
   ```

4. **Use observers for component coordination:**
   ```rust
   impl StateObserver for AudioComponent {
       fn on_state_change(&self, old: AppState, new: AppState) {
           match (old, new) {
               (Idle, Recording { .. }) => start_recording(),
               (Recording { .. }, Transcribing { .. }) => stop_recording(),
               _ => {}
           }
       }
   }
   ```

5. **Never duplicate state** - Always query the state machine

## Summary of Architectural Principles

These three ADRs establish the following principles for Hush:

### 1. Abstraction Over Concreteness
- Use traits for all component interfaces
- Depend on abstractions, not implementations
- Enable testability and extensibility

### 2. Structured Over Generic
- Use domain-specific error types
- Pattern match for recovery logic
- Provide actionable user messages

### 3. Centralized Over Distributed
- Single state machine for application state
- No duplicated state variables
- Observable state changes for coordination

### 4. Type Safety Over Runtime Checks
- Compiler-enforced valid state transitions
- Trait bounds ensure thread safety (Send + Sync)
- Structured errors prevent missing error cases

## Quick Reference for AI Agents

When working on Hush code:

### Before Implementing
```bash
# Check trait definitions
rg "pub trait" src/core/traits.rs

# Check error types
rg "pub enum.*Error" src/core/error.rs

# Check state machine
rg "pub enum AppState" src/core/state.rs
```

### Common Patterns
```rust
// 1. Component with trait objects
pub struct Component {
    audio: Box<dyn AudioSource>,
    state: StateMachine,
}

// 2. Error handling with pattern matching
match result {
    Err(HushError::Audio(AudioError::NoDeviceAvailable)) => {
        // Handle specific error
    }
    Err(e) => return Err(e),
}

// 3. State transitions
state_machine.transition(AppState::Recording {
    started_at: Instant::now()
})?;
```

### Testing
```rust
// Use mocks for all components
let audio = Box::new(MockAudioSource::new());
let transcriber = Box::new(MockTranscriber::new());

// Test state transitions
let state = StateMachine::new();
state.transition(AppState::Recording { started_at: Instant::now() })?;
assert!(state.current().is_recording());
```

## References

For complete details, see:
- `architecture.md` - Full architecture overview
- `conventions.md` - Coding conventions and patterns
- `error-handling.md` - Complete error handling guide
- Trait definitions in `src/core/traits.rs`
- Error types in `src/core/error.rs`
- State machine in `src/core/state.rs`
