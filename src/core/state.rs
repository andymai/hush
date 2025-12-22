/// Centralized state management for Hush application
///
/// Provides a type-safe state machine with validated transitions
/// and observer pattern for component reactions.
use super::error::StateError;
use parking_lot::RwLock;
use std::sync::Arc;
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
        *self.current.read()
    }

    /// Attempt state transition (validates transition is legal)
    pub fn transition(&self, new_state: AppState) -> Result<(), StateError> {
        let mut current = self.current.write();
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
        self.history.write().push(transition);

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
        self.observers.write().push(observer);
    }

    /// Notify all observers of state change
    fn notify_observers(&self, old_state: AppState, new_state: AppState) {
        let observers = self.observers.read();
        for observer in observers.iter() {
            observer.on_state_change(old_state, new_state);
        }
    }

    /// Get state history for debugging
    pub fn history(&self) -> Vec<StateTransition> {
        self.history.read().clone()
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    // Shared state for mock observer
    #[derive(Clone)]
    struct MockObserverState {
        call_count: Arc<AtomicUsize>,
        last_transition: Arc<RwLock<Option<(AppState, AppState)>>>,
    }

    impl MockObserverState {
        fn new() -> Self {
            Self {
                call_count: Arc::new(AtomicUsize::new(0)),
                last_transition: Arc::new(RwLock::new(None)),
            }
        }

        fn calls(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }

        fn last_transition(&self) -> Option<(AppState, AppState)> {
            *self.last_transition.read()
        }
    }

    // Mock observer for testing notifications
    struct MockObserver {
        state: MockObserverState,
    }

    impl MockObserver {
        fn new(state: MockObserverState) -> Self {
            Self { state }
        }
    }

    impl StateObserver for MockObserver {
        fn on_state_change(&self, old_state: AppState, new_state: AppState) {
            self.state.call_count.fetch_add(1, Ordering::SeqCst);
            *self.state.last_transition.write() = Some((old_state, new_state));
        }
    }

    // ========================================================================
    // Valid State Transitions
    // ========================================================================

    #[test]
    fn test_idle_to_recording() {
        let sm = StateMachine::new();
        assert_eq!(sm.current(), AppState::Idle);

        let result = sm.transition(AppState::Recording {
            started_at: Instant::now(),
        });
        assert!(result.is_ok());
        assert!(sm.current().is_recording());
    }

    #[test]
    fn test_recording_to_transcribing() {
        let sm = StateMachine::new();
        sm.transition(AppState::Recording {
            started_at: Instant::now(),
        })
        .unwrap();

        let result = sm.transition(AppState::Transcribing {
            audio_duration: Duration::from_secs(3),
        });
        assert!(result.is_ok());
        assert_eq!(sm.current().name(), "Transcribing");
    }

    #[test]
    fn test_transcribing_to_inserting() {
        let sm = StateMachine::new();
        sm.transition(AppState::Recording {
            started_at: Instant::now(),
        })
        .unwrap();
        sm.transition(AppState::Transcribing {
            audio_duration: Duration::from_secs(3),
        })
        .unwrap();

        let result = sm.transition(AppState::Inserting { text_length: 42 });
        assert!(result.is_ok());
        assert_eq!(sm.current().name(), "Inserting");
    }

    #[test]
    fn test_inserting_to_idle() {
        let sm = StateMachine::new();
        sm.transition(AppState::Recording {
            started_at: Instant::now(),
        })
        .unwrap();
        sm.transition(AppState::Transcribing {
            audio_duration: Duration::from_secs(3),
        })
        .unwrap();
        sm.transition(AppState::Inserting { text_length: 42 })
            .unwrap();

        let result = sm.transition(AppState::Idle);
        assert!(result.is_ok());
        assert!(sm.current().is_idle());
    }

    #[test]
    fn test_error_recovery() {
        let sm = StateMachine::new();

        // Any state can transition to Error
        sm.transition(AppState::Recording {
            started_at: Instant::now(),
        })
        .unwrap();
        let result = sm.transition(AppState::Error { recoverable: true });
        assert!(result.is_ok());
        assert_eq!(sm.current().name(), "Error");

        // Error can transition back to Idle
        let result = sm.transition(AppState::Idle);
        assert!(result.is_ok());
        assert!(sm.current().is_idle());
    }

    #[test]
    fn test_recording_to_idle_shortcut() {
        let sm = StateMachine::new();
        sm.transition(AppState::Recording {
            started_at: Instant::now(),
        })
        .unwrap();

        // Recording can go directly back to Idle (user released hotkey early)
        let result = sm.transition(AppState::Idle);
        assert!(result.is_ok());
        assert!(sm.current().is_idle());
    }

    #[test]
    fn test_transcribing_to_idle_shortcut() {
        let sm = StateMachine::new();
        sm.transition(AppState::Recording {
            started_at: Instant::now(),
        })
        .unwrap();
        sm.transition(AppState::Transcribing {
            audio_duration: Duration::from_secs(3),
        })
        .unwrap();

        // Transcribing can go directly to Idle (no text to insert)
        let result = sm.transition(AppState::Idle);
        assert!(result.is_ok());
        assert!(sm.current().is_idle());
    }

    // ========================================================================
    // Invalid Transitions (should be rejected)
    // ========================================================================

    #[test]
    fn test_idle_to_inserting_rejected() {
        let sm = StateMachine::new();
        assert_eq!(sm.current(), AppState::Idle);

        let result = sm.transition(AppState::Inserting { text_length: 42 });
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StateError::InvalidTransition { .. }
        ));
        // State should remain unchanged
        assert!(sm.current().is_idle());
    }

    #[test]
    fn test_recording_to_inserting_rejected() {
        let sm = StateMachine::new();
        sm.transition(AppState::Recording {
            started_at: Instant::now(),
        })
        .unwrap();

        let result = sm.transition(AppState::Inserting { text_length: 42 });
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StateError::InvalidTransition { .. }
        ));
        // State should remain unchanged
        assert!(sm.current().is_recording());
    }

    #[test]
    fn test_idle_to_transcribing_rejected() {
        let sm = StateMachine::new();
        assert_eq!(sm.current(), AppState::Idle);

        let result = sm.transition(AppState::Transcribing {
            audio_duration: Duration::from_secs(3),
        });
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StateError::InvalidTransition { .. }
        ));
        // State should remain unchanged
        assert!(sm.current().is_idle());
    }

    #[test]
    fn test_inserting_to_recording_rejected() {
        let sm = StateMachine::new();
        sm.transition(AppState::Recording {
            started_at: Instant::now(),
        })
        .unwrap();
        sm.transition(AppState::Transcribing {
            audio_duration: Duration::from_secs(3),
        })
        .unwrap();
        sm.transition(AppState::Inserting { text_length: 42 })
            .unwrap();

        let result = sm.transition(AppState::Recording {
            started_at: Instant::now(),
        });
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StateError::InvalidTransition { .. }
        ));
    }

    // ========================================================================
    // Observer Notifications
    // ========================================================================

    #[test]
    fn test_observer_notified_on_transition() {
        let sm = StateMachine::new();
        let observer_state = MockObserverState::new();
        let observer = MockObserver::new(observer_state.clone());

        sm.add_observer(Box::new(observer));

        // Perform a transition
        sm.transition(AppState::Recording {
            started_at: Instant::now(),
        })
        .unwrap();

        // Observer should have been called once
        assert_eq!(observer_state.calls(), 1);

        // Check the transition details
        let (old, new) = observer_state.last_transition().unwrap();
        assert!(old.is_idle());
        assert!(new.is_recording());
    }

    #[test]
    fn test_observer_not_notified_on_failed_transition() {
        let sm = StateMachine::new();
        let observer_state = MockObserverState::new();
        let observer = MockObserver::new(observer_state.clone());

        sm.add_observer(Box::new(observer));

        // Attempt an invalid transition
        let _ = sm.transition(AppState::Inserting { text_length: 42 });

        // Observer should not have been called
        assert_eq!(observer_state.calls(), 0);
    }

    #[test]
    fn test_multiple_observers() {
        let sm = StateMachine::new();
        let observer1_state = MockObserverState::new();
        let observer1 = MockObserver::new(observer1_state.clone());
        let observer2_state = MockObserverState::new();
        let observer2 = MockObserver::new(observer2_state.clone());

        sm.add_observer(Box::new(observer1));
        sm.add_observer(Box::new(observer2));

        sm.transition(AppState::Recording {
            started_at: Instant::now(),
        })
        .unwrap();

        assert_eq!(observer1_state.calls(), 1);
        assert_eq!(observer2_state.calls(), 1);
    }

    // ========================================================================
    // History Recording
    // ========================================================================

    #[test]
    fn test_history_records_transitions() {
        let sm = StateMachine::new();

        // Initial state - no history
        assert_eq!(sm.history().len(), 0);

        // First transition
        sm.transition(AppState::Recording {
            started_at: Instant::now(),
        })
        .unwrap();
        assert_eq!(sm.history().len(), 1);

        // Second transition
        sm.transition(AppState::Transcribing {
            audio_duration: Duration::from_secs(3),
        })
        .unwrap();
        assert_eq!(sm.history().len(), 2);

        // Verify history order
        let history = sm.history();
        assert!(history[0].from.is_idle());
        assert!(history[0].to.is_recording());
        assert_eq!(history[1].from.name(), "Recording");
        assert_eq!(history[1].to.name(), "Transcribing");
    }

    #[test]
    fn test_history_not_recorded_on_failed_transition() {
        let sm = StateMachine::new();

        // Attempt an invalid transition
        let _ = sm.transition(AppState::Inserting { text_length: 42 });

        // History should remain empty
        assert_eq!(sm.history().len(), 0);
    }

    // ========================================================================
    // Recording Duration
    // ========================================================================

    #[test]
    fn test_recording_duration() {
        let sm = StateMachine::new();

        // No duration when idle
        assert_eq!(sm.current().recording_duration(), None);

        // Start recording
        sm.transition(AppState::Recording {
            started_at: Instant::now(),
        })
        .unwrap();

        // Should have a duration (might be very small)
        let duration = sm.current().recording_duration();
        assert!(duration.is_some());
        assert!(duration.unwrap() >= Duration::from_secs(0));

        // No duration after transitioning away
        sm.transition(AppState::Transcribing {
            audio_duration: Duration::from_secs(3),
        })
        .unwrap();
        assert_eq!(sm.current().recording_duration(), None);
    }

    // ========================================================================
    // State Query Methods
    // ========================================================================

    #[test]
    fn test_is_idle() {
        let sm = StateMachine::new();
        assert!(sm.current().is_idle());

        sm.transition(AppState::Recording {
            started_at: Instant::now(),
        })
        .unwrap();
        assert!(!sm.current().is_idle());

        sm.transition(AppState::Idle).unwrap();
        assert!(sm.current().is_idle());
    }

    #[test]
    fn test_is_recording() {
        let sm = StateMachine::new();
        assert!(!sm.current().is_recording());

        sm.transition(AppState::Recording {
            started_at: Instant::now(),
        })
        .unwrap();
        assert!(sm.current().is_recording());

        sm.transition(AppState::Transcribing {
            audio_duration: Duration::from_secs(3),
        })
        .unwrap();
        assert!(!sm.current().is_recording());
    }

    #[test]
    fn test_current_state() {
        let sm = StateMachine::new();

        // Test each state
        assert_eq!(sm.current().name(), "Idle");

        sm.transition(AppState::Recording {
            started_at: Instant::now(),
        })
        .unwrap();
        assert_eq!(sm.current().name(), "Recording");

        sm.transition(AppState::Transcribing {
            audio_duration: Duration::from_secs(3),
        })
        .unwrap();
        assert_eq!(sm.current().name(), "Transcribing");

        sm.transition(AppState::Inserting { text_length: 42 })
            .unwrap();
        assert_eq!(sm.current().name(), "Inserting");

        sm.transition(AppState::Error { recoverable: true })
            .unwrap();
        assert_eq!(sm.current().name(), "Error");
    }

    #[test]
    fn test_state_machine_clone() {
        let sm1 = StateMachine::new();
        sm1.transition(AppState::Recording {
            started_at: Instant::now(),
        })
        .unwrap();

        // Clone shares state (Arc-based)
        let sm2 = sm1.clone();
        assert!(sm2.current().is_recording());

        // Transitions in one affect the other
        sm2.transition(AppState::Idle).unwrap();
        assert!(sm1.current().is_idle());
    }

    #[test]
    fn test_state_machine_default() {
        let sm = StateMachine::default();
        assert!(sm.current().is_idle());
    }
}
