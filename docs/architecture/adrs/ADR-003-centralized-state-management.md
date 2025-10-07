# ADR-003: Centralized State Management with Type-Safe State Machine

**Date**: 2025-10-06
**Status**: Proposed
**Deciders**: Architecture Review
**Related**: ADR-001, DEPENDENCY_ANALYSIS.md

## Context

The current codebase has **duplicated and fragmented state** across multiple components:

### Current State Distribution

| State Variable | Location | Type | Owner |
|---------------|----------|------|-------|
| `is_recording` | `HushApp` | `Arc<AtomicBool>` | HushApp |
| `is_recording` | `AudioCapture` | `Arc<Mutex<bool>>` | AudioCapture |
| `recording_start_time` | `HushApp` | `Option<Instant>` | HushApp |
| `stream` | `AudioCapture` | `Option<Stream>` | AudioCapture |
| `running` | `HotkeyManager` | `Arc<AtomicBool>` | HotkeyManager |

### Critical Issues

1. **Two `is_recording` flags** that can become inconsistent:
   ```rust
   // HushApp
   self.is_recording.store(true, Ordering::Relaxed);  // One flag

   // AudioCapture (internal)
   *self.is_recording.lock() = true;  // Another flag! ❌
   ```

2. **No single source of truth** for application state
3. **Race conditions possible** between state updates
4. **Difficult to debug** - state scattered across components
5. **Invalid states possible** - no validation of state transitions

### Example Problem

```rust
// What if this sequence happens?
hush_app.is_recording = true;
// << crash or error before AudioCapture sets its flag >>
audio_capture.is_recording = false; // Still false!
// Now states are inconsistent
```

## Decision

We will implement a **centralized state machine** with **type-safe state transitions** using the **Typestate Pattern** and **Observer Pattern** for state change notifications.

### Core Design

```
┌──────────────────────────────────────────────┐
│           Application State Machine          │
│                                              │
│   States: Idle → Recording → Transcribing   │
│           → Inserting → Idle                 │
│                                              │
│   Single source of truth                    │
│   Type-safe transitions                      │
│   Observable state changes                   │
└─────────────────┬────────────────────────────┘
                  │
                  │ Observers subscribe
                  │
        ┌─────────┴──────────┬──────────────┐
        │                    │              │
        ▼                    ▼              ▼
   ┌─────────┐          ┌─────────┐   ┌──────────┐
   │  Audio  │          │Feedback │   │ Metrics  │
   │Component│          │ System  │   │Collector │
   └─────────┘          └─────────┘   └──────────┘
```

### State Machine Implementation

```rust
// src/core/state.rs

use std::sync::{Arc, RwLock};
use std::time::Instant;
use tracing::{info, warn};

/// Application states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    /// Application is idle, waiting for input
    Idle,

    /// Recording audio from microphone
    Recording {
        started_at: Instant,
    },

    /// Transcribing recorded audio
    Transcribing {
        audio_duration: std::time::Duration,
    },

    /// Inserting transcribed text
    Inserting {
        text_length: usize,
    },

    /// Error state (can transition back to Idle)
    Error {
        recoverable: bool,
    },
}

impl AppState {
    pub fn name(&self) -> &'static str {
        match self {
            AppState::Idle => "Idle",
            AppState::Recording { .. } => "Recording",
            AppState::Transcribing { .. } => "Transcribing",
            AppState::Inserting { .. } => "Inserting",
            AppState::Error { .. } => "Error",
        }
    }

    pub fn is_idle(&self) -> bool {
        matches!(self, AppState::Idle)
    }

    pub fn is_recording(&self) -> bool {
        matches!(self, AppState::Recording { .. })
    }

    pub fn recording_duration(&self) -> Option<std::time::Duration> {
        match self {
            AppState::Recording { started_at } => Some(started_at.elapsed()),
            _ => None,
        }
    }
}

/// State machine with validated transitions
pub struct StateMachine {
    current: Arc<RwLock<AppState>>,
    observers: Arc<RwLock<Vec<Box<dyn StateObserver>>>>,
    history: Arc<RwLock<Vec<StateTransition>>>,
}

impl StateMachine {
    pub fn new() -> Self {
        Self {
            current: Arc::new(RwLock::new(AppState::Idle)),
            observers: Arc::new(RwLock::new(Vec::new())),
            history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get current state (cheap read-only access)
    pub fn current(&self) -> AppState {
        *self.current.read().unwrap()
    }

    /// Attempt state transition (validates transition is legal)
    pub fn transition(&self, new_state: AppState) -> Result<(), StateError> {
        let mut current = self.current.write().unwrap();
        let old_state = *current;

        // Validate transition
        if !Self::is_valid_transition(&old_state, &new_state) {
            return Err(StateError::InvalidTransition {
                from: old_state.name().to_string(),
                to: new_state.name().to_string(),
            });
        }

        // Perform transition
        info!("State transition: {} → {}", old_state.name(), new_state.name());
        *current = new_state;

        // Record in history
        let transition = StateTransition {
            from: old_state,
            to: new_state,
            timestamp: Instant::now(),
        };
        self.history.write().unwrap().push(transition);

        // Notify observers (release lock first to avoid deadlock)
        drop(current);
        self.notify_observers(old_state, new_state);

        Ok(())
    }

    /// Validate state transition rules
    fn is_valid_transition(from: &AppState, to: &AppState) -> bool {
        use AppState::*;

        match (from, to) {
            // From Idle
            (Idle, Recording { .. }) => true,

            // From Recording
            (Recording { .. }, Transcribing { .. }) => true,
            (Recording { .. }, Idle) => true, // Cancel recording
            (Recording { .. }, Error { .. }) => true,

            // From Transcribing
            (Transcribing { .. }, Inserting { .. }) => true,
            (Transcribing { .. }, Idle) => true, // Empty transcription
            (Transcribing { .. }, Error { .. }) => true,

            // From Inserting
            (Inserting { .. }, Idle) => true,
            (Inserting { .. }, Error { .. }) => true,

            // From Error
            (Error { .. }, Idle) => true,

            // All other transitions are invalid
            _ => false,
        }
    }

    /// Add state observer
    pub fn add_observer(&self, observer: Box<dyn StateObserver>) {
        self.observers.write().unwrap().push(observer);
    }

    /// Notify all observers of state change
    fn notify_observers(&self, old_state: AppState, new_state: AppState) {
        let observers = self.observers.read().unwrap();
        for observer in observers.iter() {
            observer.on_state_change(old_state, new_state);
        }
    }

    /// Get state history for debugging
    pub fn history(&self) -> Vec<StateTransition> {
        self.history.read().unwrap().clone()
    }

    /// Clear history (for testing or memory management)
    pub fn clear_history(&self) {
        self.history.write().unwrap().clear();
    }
}

// Clone for Arc sharing
impl Clone for StateMachine {
    fn clone(&self) -> Self {
        Self {
            current: Arc::clone(&self.current),
            observers: Arc::clone(&self.observers),
            history: Arc::clone(&self.history),
        }
    }
}

/// Observer trait for state changes
pub trait StateObserver: Send + Sync {
    fn on_state_change(&self, old_state: AppState, new_state: AppState);
}

/// State transition record for history/debugging
#[derive(Debug, Clone, Copy)]
pub struct StateTransition {
    pub from: AppState,
    pub to: AppState,
    pub timestamp: Instant,
}

/// State-related errors
#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("Invalid state transition: {from} → {to}")]
    InvalidTransition { from: String, to: String },

    #[error("Operation not allowed in state: {state}")]
    OperationNotAllowed { state: String },
}
```

### Integration with Components

Components observe state changes instead of managing their own:

```rust
// Example: AudioComponent as state observer
pub struct AudioComponent {
    source: Box<dyn AudioSource>,
    state_machine: StateMachine,
}

impl StateObserver for AudioComponent {
    fn on_state_change(&self, old_state: AppState, new_state: AppState) {
        use AppState::*;

        match (old_state, new_state) {
            (Idle, Recording { .. }) => {
                // Start recording when entering Recording state
                if let Err(e) = self.source.start_recording() {
                    error!("Failed to start recording: {:?}", e);
                    // Transition to error state
                    self.state_machine.transition(Error { recoverable: true }).ok();
                }
            }
            (Recording { .. }, Transcribing { .. }) => {
                // Stop recording when entering Transcribing state
                if let Err(e) = self.source.stop_recording() {
                    error!("Failed to stop recording: {:?}", e);
                }
            }
            _ => {}
        }
    }
}
```

### Refactored HushApp

```rust
pub struct HushApp {
    state: StateMachine,
    audio: AudioComponent,
    transcriber: Box<dyn Transcriber>,
    text_output: Box<dyn TextOutput>,
    input_trigger: Box<dyn InputTrigger>,
}

impl HushApp {
    pub async fn new(/* ... */) -> Result<Self> {
        let state = StateMachine::new();

        // Create components with shared state machine
        let audio = AudioComponent::new(audio_source, state.clone());

        // Register audio component as observer
        state.add_observer(Box::new(audio.clone()));

        // ... other components

        Ok(Self {
            state,
            audio,
            transcriber,
            text_output,
            input_trigger,
        })
    }

    pub async fn handle_recording_start(&mut self) -> Result<()> {
        // Single state transition triggers all necessary actions
        self.state.transition(AppState::Recording {
            started_at: Instant::now(),
        })?;

        // Audio component starts recording automatically via observer

        Ok(())
    }

    pub async fn handle_recording_stop(&mut self) -> Result<()> {
        let duration = self.state.current().recording_duration()
            .ok_or_else(|| anyhow::anyhow!("Not recording"))?;

        // Transition to transcribing
        self.state.transition(AppState::Transcribing {
            audio_duration: duration,
        })?;

        // Audio component stops recording automatically via observer
        // Get audio buffer and transcribe
        let audio = self.audio.get_buffer()?;
        let result = self.transcriber.transcribe(&audio).await?;

        // Transition to inserting
        self.state.transition(AppState::Inserting {
            text_length: result.text.len(),
        })?;

        // Insert text
        self.text_output.insert_text(&result.text).await?;

        // Back to idle
        self.state.transition(AppState::Idle)?;

        Ok(())
    }

    pub fn is_recording(&self) -> bool {
        self.state.current().is_recording()
    }
}
```

## Consequences

### Positive

1. **Single Source of Truth**: One state machine, no duplication
2. **Type-Safe Transitions**: Compiler enforces valid state transitions
3. **Impossible States Prevented**: Cannot have inconsistent state
4. **Observable**: Components react to state changes automatically
5. **Debuggable**: State history available for debugging
6. **Testable**: Easy to test state transitions in isolation
7. **Thread-Safe**: `RwLock` ensures safe concurrent access

### Negative

1. **Slight Performance Overhead**: RwLock adds ~100ns per access
   - **Mitigation**: Negligible compared to audio/ML processing (milliseconds)

2. **More Boilerplate**: Observer registration code
   - **Mitigation**: Centralized state is worth it

3. **Lock Contention Possible**: Multiple threads accessing state
   - **Mitigation**: Readers don't block each other (RwLock), writes are rare

## Alternatives Considered

### Alternative 1: Keep Fragmented State
**Rejected**: Current situation is error-prone and hard to debug

### Alternative 2: Message Passing (Actor Model)
```rust
// Each component is an actor with message queue
enum Message {
    StartRecording,
    StopRecording,
    // ...
}
```

**Rejected for now**:
- More complex implementation
- Adds latency (message queue overhead)
- Can revisit if we need true concurrent actors

### Alternative 3: Typestate Pattern (Compile-Time State)
```rust
struct HushApp<S: State> {
    state: PhantomData<S>,
    // ...
}

// Different methods available in each state
impl HushApp<Idle> {
    fn start_recording(self) -> HushApp<Recording> { /* ... */ }
}

impl HushApp<Recording> {
    fn stop_recording(self) -> HushApp<Transcribing> { /* ... */ }
}
```

**Rejected**:
- HushApp must be long-lived (daemon mode)
- Cannot transition without consuming self
- Too restrictive for runtime state management

## Implementation Plan

1. **Week 1**: Implement StateMachine and StateObserver trait
2. **Week 2**: Refactor AudioComponent to use state machine
3. **Week 2**: Remove duplicated `is_recording` flags
4. **Week 3**: Add state observers for all components
5. **Week 3**: Add state history and debugging tools
6. **Week 4**: Write comprehensive state machine tests

## Success Metrics

1. **Zero Duplicated State**: Only one `is_recording` source
2. **Invalid Transitions Impossible**: All transitions validated
3. **Test Coverage**: 100% coverage of state transitions
4. **No State-Related Bugs**: Zero issues from inconsistent state
5. **Debug Time Reduced**: State history helps diagnose issues

## References

- [Typestate Pattern](http://cliffle.com/blog/rust-typestate/)
- [Observer Pattern](https://refactoring.guru/design-patterns/observer)
- [State Machine Design](https://hoverbear.org/blog/rust-state-machine-pattern/)

## Changelog

- 2025-10-06: Initial proposal
