/// Centralized state management for Hush application
///
/// Provides a type-safe state machine with validated transitions
/// and observer pattern for component reactions.
use super::error::StateError;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tracing::info;

/// Application states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    /// Application is idle, waiting for input
    Idle,

    /// Recording audio from microphone
    Recording { started_at: Instant },

    /// Transcribing recorded audio
    Transcribing { audio_duration: std::time::Duration },

    /// Inserting transcribed text
    Inserting { text_length: usize },

    /// Error state (can transition back to Idle)
    Error { recoverable: bool },
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
#[derive(Clone)]
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
        *self.current.read().unwrap_or_else(|poisoned| {
            // If the lock is poisoned, recover by using the poisoned data
            // This is safe because AppState is Copy and doesn't hold any invariants
            *poisoned.into_inner()
        })
    }

    /// Attempt state transition (validates transition is legal)
    pub fn transition(&self, new_state: AppState) -> Result<(), StateError> {
        let mut current = self.current.write().map_err(|e| {
            StateError::LockPoisoned(format!(
                "Failed to acquire write lock on current state: {}",
                e
            ))
        })?;
        let old_state = *current;

        // Validate transition
        if !Self::is_valid_transition(&old_state, &new_state) {
            return Err(StateError::InvalidTransition {
                from: old_state.name().to_string(),
                to: new_state.name().to_string(),
            });
        }

        // Perform transition
        info!(
            "State transition: {} → {}",
            old_state.name(),
            new_state.name()
        );
        *current = new_state;

        // Record in history
        let transition = StateTransition {
            from: old_state,
            to: new_state,
            timestamp: Instant::now(),
        };
        self.history
            .write()
            .map_err(|e| {
                StateError::LockPoisoned(format!("Failed to acquire write lock on history: {}", e))
            })?
            .push(transition);

        // Notify observers (release lock first to avoid deadlock)
        drop(current);
        self.notify_observers(old_state, new_state);

        Ok(())
    }

    /// Validate state transition rules
    fn is_valid_transition(from: &AppState, to: &AppState) -> bool {
        use AppState::*;

        matches!(
            (from, to),
            // From Idle
            (Idle, Recording { .. })
            // From Recording
            | (Recording { .. }, Transcribing { .. })
            | (Recording { .. }, Idle)
            | (Recording { .. }, Error { .. })
            // From Transcribing
            | (Transcribing { .. }, Inserting { .. })
            | (Transcribing { .. }, Idle)
            | (Transcribing { .. }, Error { .. })
            // From Inserting
            | (Inserting { .. }, Idle)
            | (Inserting { .. }, Error { .. })
            // From Error
            | (Error { .. }, Idle)
        )
    }

    /// Add state observer
    pub fn add_observer(&self, observer: Box<dyn StateObserver>) {
        self.observers
            .write()
            .unwrap_or_else(|poisoned| {
                // Recover from poisoned lock by clearing the poison and continuing
                poisoned.into_inner()
            })
            .push(observer);
    }

    /// Notify all observers of state change
    fn notify_observers(&self, old_state: AppState, new_state: AppState) {
        let observers = self.observers.read().unwrap_or_else(|poisoned| {
            // Recover from poisoned lock - observers are still valid even if lock was poisoned
            poisoned.into_inner()
        });
        for observer in observers.iter() {
            observer.on_state_change(old_state, new_state);
        }
    }

    /// Get state history for debugging
    pub fn history(&self) -> Vec<StateTransition> {
        self.history
            .read()
            .unwrap_or_else(|poisoned| {
                // Recover from poisoned lock - history is still valid even if lock was poisoned
                poisoned.into_inner()
            })
            .clone()
    }
}

// Clone is automatically derived since all fields (Arc<T>) implement Clone

impl Default for StateMachine {
    fn default() -> Self {
        Self::new()
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
