# Core Trait Abstractions - Hush Voice-to-Text

**Date**: 2025-10-07
**Status**: Partially Implemented - UInput Integration Complete  
**Related**: DEPENDENCY_ANALYSIS.md, ADR-001, /DESIGN_PATTERNS_ANALYSIS.md

## Overview

This document defines the core trait abstractions that will decouple components and enable testability, extensibility, and platform independence.

## Design Principles

1. **Interface Segregation**: Small, focused traits
2. **Dependency Inversion**: Depend on abstractions, not concretions
3. **Open/Closed**: Open for extension, closed for modification
4. **Testability**: All traits must be mockable
5. **Platform Agnostic**: No platform-specific types in trait definitions

## Core Trait Hierarchy

```
                    ┌─────────────┐
                    │  Pipeline   │  ← High-level orchestration
                    └──────┬──────┘
                           │
         ┌─────────────────┼─────────────────┐
         │                 │                 │
         ▼                 ▼                 ▼
    ┌─────────┐      ┌──────────┐     ┌──────────┐
    │  Input  │      │Processing│     │  Output  │
    │ Trigger │      │ Pipeline │     │          │
    └─────────┘      └──────────┘     └──────────┘
         │                 │                 │
         │           ┌─────┴─────┐          │
         │           │           │          │
         ▼           ▼           ▼          ▼
    ┌─────────┐ ┌─────────┐ ┌──────────┐ ┌──────┐
    │ Hotkey  │ │  Audio  │ │Transcribe│ │ Text │
    │ Trigger │ │ Source  │ │          │ │Output│
    └─────────┘ └─────────┘ └──────────┘ └──────┘
```

## 1. Audio Abstraction

### AudioSource Trait

```rust
/// Core trait for audio capture systems
///
/// Implementations: CpalAudioSource, SimulatedAudioSource, PulseAudioSource
#[async_trait::async_trait]
pub trait AudioSource: Send + Sync {
    /// Start capturing audio from the source
    fn start_recording(&mut self) -> Result<()>;

    /// Stop capturing and return all recorded audio samples
    /// Returns empty vec if no audio was captured
    fn stop_recording(&mut self) -> Result<AudioBuffer>;

    /// Check if currently recording
    fn is_recording(&self) -> bool;

    /// Get human-readable device name
    fn device_name(&self) -> &str;

    /// Get audio configuration (sample rate, channels)
    fn config(&self) -> AudioConfig;

    /// List available audio devices (optional, for device selection)
    fn list_devices() -> Result<Vec<AudioDeviceInfo>> where Self: Sized {
        Ok(vec![])
    }
}

/// Audio buffer with metadata
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration: std::time::Duration,
}

impl AudioBuffer {
    pub fn new(samples: Vec<f32>, sample_rate: u32, channels: u16) -> Self {
        let duration = std::time::Duration::from_secs_f32(
            samples.len() as f32 / (sample_rate as f32 * channels as f32)
        );
        Self { samples, sample_rate, channels, duration }
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }
}

/// Audio device information for selection
#[derive(Debug, Clone)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// Audio configuration
#[derive(Debug, Clone, Copy)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_size: usize,
}
```

### AudioProcessor Trait (Middleware)

```rust
/// Audio preprocessing middleware
///
/// Examples: NoiseReduction, VolumeNormalization, SilenceRemoval
pub trait AudioProcessor: Send + Sync {
    /// Process audio buffer in-place or return new buffer
    fn process(&self, audio: AudioBuffer) -> Result<AudioBuffer>;

    /// Get processor name for logging/debugging
    fn name(&self) -> &str;
}

/// Chain multiple audio processors
pub struct AudioProcessorChain {
    processors: Vec<Box<dyn AudioProcessor>>,
}

impl AudioProcessorChain {
    pub fn new() -> Self {
        Self { processors: Vec::new() }
    }

    pub fn add(mut self, processor: Box<dyn AudioProcessor>) -> Self {
        self.processors.push(processor);
        self
    }

    pub fn process(&self, mut audio: AudioBuffer) -> Result<AudioBuffer> {
        for processor in &self.processors {
            audio = processor.process(audio)?;
        }
        Ok(audio)
    }
}
```

## 2. Transcription Abstraction

### Transcriber Trait

```rust
/// Speech-to-text transcription engine
///
/// Implementations: WhisperTranscriber, OpenAITranscriber, MockTranscriber
#[async_trait::async_trait]
pub trait Transcriber: Send + Sync {
    /// Transcribe audio to text asynchronously
    async fn transcribe(&self, audio: &AudioBuffer) -> Result<TranscriptionResult>;

    /// Get transcriber capabilities/info
    fn info(&self) -> TranscriberInfo;

    /// Check if transcriber is ready (model loaded, etc.)
    async fn is_ready(&self) -> bool {
        true
    }
}

/// Transcription result with metadata
#[derive(Debug, Clone)]
pub struct TranscriptionResult {
    pub text: String,
    pub confidence: f32,
    pub language: Option<String>,
    pub processing_time: std::time::Duration,
    pub segments: Vec<TranscriptionSegment>,
}

impl TranscriptionResult {
    pub fn simple(text: String, confidence: f32, duration: std::time::Duration) -> Self {
        Self {
            text,
            confidence,
            language: None,
            processing_time: duration,
            segments: vec![],
        }
    }
}

/// Individual transcription segment (for longer audio)
#[derive(Debug, Clone)]
pub struct TranscriptionSegment {
    pub text: String,
    pub start_time: f32,
    pub end_time: f32,
    pub confidence: f32,
}

/// Transcriber capabilities and info
#[derive(Debug, Clone)]
pub struct TranscriberInfo {
    pub name: String,
    pub version: String,
    pub supports_languages: Vec<String>,
    pub max_audio_duration: Option<std::time::Duration>,
    pub requires_network: bool,
    pub hardware_accelerated: bool,
}
```

### TranscriptionPostProcessor Trait

```rust
/// Post-process transcribed text
///
/// Examples: Punctuation, Formatting, CodeSnippetDetection
pub trait TranscriptionPostProcessor: Send + Sync {
    /// Process transcribed text
    fn process(&self, result: TranscriptionResult) -> Result<TranscriptionResult>;

    fn name(&self) -> &str;
}
```

## 3. Text Insertion Abstraction

### TextOutput Trait

```rust
/// Text insertion and output
///
/// Implementations: X11TextInserter, WaylandTextInserter, ClipboardOutput, StdoutOutput
#[async_trait::async_trait]
pub trait TextOutput: Send + Sync {
    /// Insert text at current cursor position
    async fn insert_text(&mut self, text: &str) -> Result<()>;

    /// Get currently focused window (if available)
    async fn focused_window(&self) -> Result<Option<WindowInfo>>;

    /// Check if text output is available/ready
    fn is_available(&self) -> bool;

    /// Get output method name
    fn output_method(&self) -> &str;
}

/// Window information (platform-agnostic)
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub title: String,
    pub class: String,
    pub app_name: String,
}

/// Text insertion strategy selector
pub trait InsertionStrategy: Send + Sync {
    /// Determine best insertion method for given text and window
    fn choose_method(&self, text: &str, window: Option<&WindowInfo>) -> InsertionMethod;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertionMethod {
    /// Direct keyboard simulation
    DirectTyping,
    /// Via system clipboard + paste
    Clipboard,
    /// Application-specific method
    AppSpecific,
}
```

### DisplayServer Trait

```rust
/// Platform-specific display server interface
///
/// Implementations: X11DisplayServer, WaylandDisplayServer
pub trait DisplayServer: Send + Sync {
    /// Get active/focused window
    fn get_focused_window(&self) -> Result<WindowInfo>;

    /// Simulate keyboard input
    fn simulate_keystrokes(&mut self, text: &str) -> Result<()>;

    /// Get/set clipboard content
    fn clipboard_get(&self) -> Result<String>;
    fn clipboard_set(&mut self, text: &str) -> Result<()>;

    /// Paste operation (Ctrl+V or equivalent)
    fn paste(&mut self) -> Result<()>;

    /// Display server name
    fn server_type(&self) -> &str;
}
```

## 4. Input Trigger Abstraction

### InputTrigger Trait

```rust
/// User input trigger for recording
///
/// Implementations: HotkeyTrigger, CLITrigger, DBusTrigger, WakeWordTrigger
#[async_trait::async_trait]
pub trait InputTrigger: Send + Sync {
    /// Start listening for trigger events
    async fn start_listening(&mut self) -> Result<()>;

    /// Stop listening
    async fn stop_listening(&mut self) -> Result<()>;

    /// Get stream of trigger events
    fn event_stream(&self) -> mpsc::Receiver<TriggerEvent>;

    /// Get trigger description (e.g., "Ctrl+Shift+Space")
    fn description(&self) -> String;
}

/// Trigger events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerEvent {
    /// Start recording (hotkey pressed, wake word detected, etc.)
    StartRecording,
    /// Stop recording (hotkey released, timeout, etc.)
    StopRecording,
    /// Cancel recording
    Cancel,
}
```

### HotkeyProvider Trait

```rust
/// Platform-specific hotkey registration
pub trait HotkeyProvider: Send + Sync {
    /// Register a hotkey combination
    fn register(&mut self, combination: &str) -> Result<HotkeyId>;

    /// Unregister a hotkey
    fn unregister(&mut self, id: HotkeyId) -> Result<()>;

    /// Get hotkey events
    fn event_receiver(&self) -> mpsc::Receiver<HotkeyProviderEvent>;
}

pub type HotkeyId = u32;

#[derive(Debug, Clone)]
pub struct HotkeyProviderEvent {
    pub id: HotkeyId,
    pub state: HotkeyState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyState {
    Pressed,
    Released,
}
```

## 5. Configuration Abstraction

### ConfigProvider Trait

```rust
/// Configuration access
pub trait ConfigProvider: Send + Sync {
    fn audio_config(&self) -> &AudioConfig;
    fn transcription_config(&self) -> &TranscriptionConfig;
    fn hotkey_config(&self) -> &HotkeyConfig;
    fn text_output_config(&self) -> &TextOutputConfig;
    fn feedback_config(&self) -> &FeedbackConfig;
}

/// Configuration watching for hot-reload
#[async_trait::async_trait]
pub trait ConfigWatcher: Send + Sync {
    /// Start watching for config changes
    async fn watch(&mut self) -> Result<()>;

    /// Get stream of config change events
    fn change_stream(&self) -> mpsc::Receiver<ConfigChange>;

    /// Stop watching
    async fn stop(&mut self) -> Result<()>;
}

#[derive(Debug, Clone)]
pub enum ConfigChange {
    AudioConfig(AudioConfig),
    TranscriptionConfig(TranscriptionConfig),
    HotkeyConfig(HotkeyConfig),
    FullReload,
}
```

## 6. State Management Abstraction

### ApplicationState

```rust
/// Centralized application state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Idle,
    Recording { started_at: std::time::Instant },
    Transcribing,
    Inserting,
    Error,
}

/// State machine with type-safe transitions
pub struct StateMachine {
    current: AppState,
    observers: Vec<Box<dyn StateObserver>>,
}

impl StateMachine {
    pub fn new() -> Self {
        Self {
            current: AppState::Idle,
            observers: Vec::new(),
        }
    }

    pub fn current(&self) -> AppState {
        self.current
    }

    /// Transition to new state (validates transition is legal)
    pub fn transition(&mut self, new_state: AppState) -> Result<()> {
        if !self.is_valid_transition(&self.current, &new_state) {
            return Err(anyhow::anyhow!(
                "Invalid state transition: {:?} -> {:?}",
                self.current, new_state
            ));
        }

        let old_state = self.current;
        self.current = new_state;

        // Notify observers
        for observer in &self.observers {
            observer.on_state_change(old_state, new_state);
        }

        Ok(())
    }

    pub fn add_observer(&mut self, observer: Box<dyn StateObserver>) {
        self.observers.push(observer);
    }

    fn is_valid_transition(&self, from: &AppState, to: &AppState) -> bool {
        use AppState::*;
        matches!(
            (from, to),
            (Idle, Recording { .. })
            | (Recording { .. }, Transcribing)
            | (Recording { .. }, Idle)
            | (Transcribing, Inserting)
            | (Transcribing, Idle)
            | (Inserting, Idle)
            | (_, Error)
            | (Error, Idle)
        )
    }
}

/// Observer for state changes
pub trait StateObserver: Send + Sync {
    fn on_state_change(&self, old_state: AppState, new_state: AppState);
}
```

## 7. Feedback Abstraction

### FeedbackProvider Trait

```rust
/// User feedback system
#[async_trait::async_trait]
pub trait FeedbackProvider: Send + Sync {
    /// Provide feedback for recording start
    async fn on_recording_start(&self) -> Result<()>;

    /// Provide feedback for recording stop
    async fn on_recording_stop(&self) -> Result<()>;

    /// Provide feedback for transcription complete
    async fn on_transcription_complete(&self, result: &TranscriptionResult) -> Result<()>;

    /// Provide feedback for error
    async fn on_error(&self, error: &str) -> Result<()>;
}

/// Composite feedback (audio + visual + haptic)
pub struct CompositeFeedback {
    providers: Vec<Box<dyn FeedbackProvider>>,
}

impl CompositeFeedback {
    pub fn new() -> Self {
        Self { providers: Vec::new() }
    }

    pub fn add(mut self, provider: Box<dyn FeedbackProvider>) -> Self {
        self.providers.push(provider);
        self
    }
}

#[async_trait::async_trait]
impl FeedbackProvider for CompositeFeedback {
    async fn on_recording_start(&self) -> Result<()> {
        for provider in &self.providers {
            provider.on_recording_start().await?;
        }
        Ok(())
    }

    async fn on_recording_stop(&self) -> Result<()> {
        for provider in &self.providers {
            provider.on_recording_stop().await?;
        }
        Ok(())
    }

    async fn on_transcription_complete(&self, result: &TranscriptionResult) -> Result<()> {
        for provider in &self.providers {
            provider.on_transcription_complete(result).await?;
        }
        Ok(())
    }

    async fn on_error(&self, error: &str) -> Result<()> {
        for provider in &self.providers {
            provider.on_error(error).await?;
        }
        Ok(())
    }
}
```

## 8. High-Level Pipeline Trait

### Pipeline Trait

```rust
/// Complete voice-to-text pipeline
#[async_trait::async_trait]
pub trait Pipeline: Send + Sync {
    /// Start the pipeline (daemon mode)
    async fn start(&mut self) -> Result<()>;

    /// Stop the pipeline
    async fn stop(&mut self) -> Result<()>;

    /// Process a single recording (one-shot mode)
    async fn process_once(&mut self) -> Result<TranscriptionResult>;

    /// Get current pipeline state
    fn state(&self) -> AppState;
}
```

## Implementation Strategy

### Phase 1: Define Traits (Week 1)
- Create `src/core/traits.rs` with all trait definitions
- Define common types (`AudioBuffer`, `TranscriptionResult`, etc.)
- Document each trait with examples

### Phase 2: Adapter Pattern (Week 2)
- Create `src/adapters/` directory structure
- Implement adapters for existing concrete types
- Example: `CpalAudioSource` implements `AudioSource`

### Phase 3: Refactor HushApp (Week 3)
- Change `HushApp` to accept trait objects
- Use dependency injection pattern
- Maintain backward compatibility

### Phase 4: Mock Implementations (Week 4)
- Create mock implementations for all traits
- Add unit tests using mocks
- Verify 100% mockable

## Example: AudioSource Adapter

```rust
// src/adapters/audio/cpal_adapter.rs

use crate::core::traits::{AudioSource, AudioBuffer, AudioConfig};
use crate::Result;

pub struct CpalAudioSource {
    device: cpal::Device,
    config: cpal::StreamConfig,
    stream: Option<cpal::Stream>,
    buffer: Arc<Mutex<Vec<f32>>>,
    is_recording: Arc<Mutex<bool>>,
}

impl CpalAudioSource {
    pub fn new(device_name: Option<&str>) -> Result<Self> {
        // Existing AudioCapture::new logic
        todo!()
    }
}

#[async_trait::async_trait]
impl AudioSource for CpalAudioSource {
    fn start_recording(&mut self) -> Result<()> {
        // Existing AudioCapture::start_recording logic
        todo!()
    }

    fn stop_recording(&mut self) -> Result<AudioBuffer> {
        let samples = /* existing logic */;
        Ok(AudioBuffer::new(
            samples,
            self.config.sample_rate.0,
            self.config.channels,
        ))
    }

    fn is_recording(&self) -> bool {
        *self.is_recording.lock()
    }

    fn device_name(&self) -> &str {
        // existing logic
        todo!()
    }

    fn config(&self) -> AudioConfig {
        AudioConfig {
            sample_rate: self.config.sample_rate.0,
            channels: self.config.channels,
            buffer_size: 1024,
        }
    }
}
```

## Benefits of This Design

1. **Testability**: Every trait can be mocked
2. **Extensibility**: New implementations without changing core
3. **Platform Independence**: Platform-specific code isolated
4. **Composability**: Mix and match implementations
5. **Maintainability**: Clear contracts and boundaries

## Trade-offs

- **More Code**: Trait definitions and adapters add lines of code
- **Runtime Overhead**: Dynamic dispatch (trait objects) has small performance cost
- **Complexity**: More abstraction layers to understand

**Verdict**: Trade-offs are worthwhile for long-term maintainability and quality.

## Current Implementation Status (2025-10-07)

### ✅ **Completed Components**

#### Text Insertion System
- **Location**: `src/text/insertion.rs`, `src/text/uinput_keyboard.rs`
- **Status**: ✅ **Functionally Complete** but needs trait refactoring
- **Features**:
  - Linux UInput integration for universal text insertion
  - Multi-method fallback (UInput → X11/enigo → clipboard)
  - Application-aware insertion strategy
  - Comprehensive error handling and user guidance
  - CLI tools for setup and diagnostics

#### Error Handling & User Guidance  
- **Location**: `src/text/insertion.rs` - guidance functions
- **Status**: ✅ **Excellent** - exceeds design expectations
- **Features**:
  - User-friendly setup guidance
  - Intelligent diagnostics with specific solutions
  - CLI integration (`hush setup-uinput`, `hush diagnose-uinput`)

### ⚠️ **Needs Refactoring**

#### TextOutput Trait Implementation
- **Current**: Concrete `TextInserter` struct
- **Needs**: `TextOutput` trait abstraction
- **Priority**: High - affects testability

#### DisplayServer Abstraction
- **Current**: X11 calls mixed with business logic
- **Needs**: `DisplayServer` trait with `X11DisplayServer` impl
- **Priority**: High - affects platform independence

#### Mock-ability
- **Current**: No mocking support
- **Needs**: Mock implementations for all components
- **Priority**: High - needed for comprehensive testing

### 📋 **Refactoring Plan**

See `/DESIGN_PATTERNS_ANALYSIS.md` for detailed refactoring plan:

1. **Phase 1** (Week 1): Define core traits (`TextOutput`, `DisplayServer`, etc.)
2. **Phase 2** (Week 2): Implement adapters while maintaining compatibility  
3. **Phase 3** (Week 3): Add mock implementations and tests
4. **Phase 4** (Week 4): Full dependency injection support

### 🎯 **Assessment**

| Aspect | Current Status | Design Pattern Alignment |
|--------|---------------|-------------------------|
| **Functionality** | ✅ Excellent | ✅ Exceeds expectations |
| **Error Handling** | ✅ Excellent | ✅ Fully aligned |
| **User Experience** | ✅ Excellent | ✅ Fully aligned |
| **Trait Architecture** | ❌ Missing | ❌ Needs refactoring |
| **Testability** | ⚠️ Limited | ❌ Needs mocks |
| **Platform Independence** | ⚠️ Partial | ❌ X11-coupled |

**Overall**: The implementation is functionally excellent but needs architectural refactoring to align with trait-based design patterns.

---

## Next Steps

1. ✅ ~~Review trait designs with team~~
2. ✅ ~~Implement functional text insertion system~~  
3. 🔄 **IN PROGRESS**: Refactor to trait-based architecture
4. ⏳ **NEXT**: Add comprehensive testing with mocks
5. ⏳ **FUTURE**: Extend patterns to other components (audio, transcription)
