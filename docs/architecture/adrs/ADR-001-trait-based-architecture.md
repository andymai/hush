# ADR-001: Adopt Trait-Based Architecture for Component Decoupling

---
**Last Updated**: 2025-11-19
**Original Date**: 2025-10-06
**Status**: Partially Implemented - UInput complete (Oct 2025), other components in progress
**Deciders**: Architecture Review
**Purpose**: Documents decision to adopt trait-based architecture for component decoupling
**Related Documents**: [Architecture](../../ARCHITECTURE.md) | [Design Patterns](../DESIGN_PATTERNS.md) | [Documentation Index](../../../DOCUMENTATION_INDEX.md)
---

**Implementation Status**: UInput text insertion has been successfully refactored to use trait abstractions. Other components (audio, transcription, hotkeys) are planned for future refactoring.

## Context

The current Hush codebase suffers from tight coupling between components. All dependencies in `HushApp` are concrete types, making testing nearly impossible without real hardware (microphone, X11 server, CUDA GPU). This severely limits:

1. **Testability**: Cannot unit test components in isolation
2. **Extensibility**: Adding new implementations requires modifying core code
3. **Platform Independence**: Platform-specific code (X11, cpal) embedded throughout
4. **Maintainability**: Changes ripple across multiple modules

### Current Problems

```rust
// HushApp owns concrete types - CANNOT mock or substitute
pub struct HushApp {
    audio_capture: AudioCapture,          // Concrete ❌
    transcriber: WhisperTranscriber,      // Concrete ❌
    text_inserter: TextInserter,          // Concrete ❌
    hotkey_manager: Option<HotkeyManager>,// Concrete ❌
}
```

**Impact**:
- Zero meaningful unit tests (tests/integration_tests.rs is nearly empty)
- Cannot test without hardware dependencies
- Adding Wayland support requires rewriting TextInserter
- Cannot swap Whisper for alternative transcription engines
- Simulation modes are workarounds, not proper abstractions

## Decision

We will **adopt a trait-based architecture** using the **Adapter Pattern** to decouple components and enable dependency injection.

### Core Principles

1. **Define traits for all component interfaces** (`AudioSource`, `Transcriber`, `TextOutput`, `InputTrigger`)
2. **Implement adapters** for existing concrete types (e.g., `CpalAudioSource` implements `AudioSource`)
3. **Refactor HushApp** to accept trait objects instead of concrete types
4. **Create mock implementations** for all traits to enable unit testing
5. **Isolate platform-specific code** behind platform abstraction traits

### Key Traits

```rust
// Core component traits
pub trait AudioSource: Send + Sync { /* ... */ }
pub trait Transcriber: Send + Sync { /* ... */ }
pub trait TextOutput: Send + Sync { /* ... */ }
pub trait InputTrigger: Send + Sync { /* ... */ }

// Platform abstraction
pub trait DisplayServer: Send + Sync { /* ... */ }
pub trait HotkeyProvider: Send + Sync { /* ... */ }

// Middleware/extensibility
pub trait AudioProcessor: Send + Sync { /* ... */ }
pub trait TranscriptionPostProcessor: Send + Sync { /* ... */ }
```

### Refactored HushApp

```rust
pub struct HushApp {
    audio: Box<dyn AudioSource>,
    transcriber: Box<dyn Transcriber>,
    text_output: Box<dyn TextOutput>,
    input_trigger: Box<dyn InputTrigger>,
    state: StateMachine,
}

impl HushApp {
    pub fn new(
        audio: Box<dyn AudioSource>,
        transcriber: Box<dyn Transcriber>,
        text_output: Box<dyn TextOutput>,
        input_trigger: Box<dyn InputTrigger>,
    ) -> Self {
        // Dependency injection ✅
    }
}
```

## Consequences

### Positive

1. **Testability**: Can inject mock implementations for all components
   ```rust
   let app = HushApp::new(
       Box::new(MockAudioSource::new()),
       Box::new(MockTranscriber::new()),
       Box::new(MockTextOutput::new()),
       Box::new(MockInputTrigger::new()),
   );
   ```

2. **Extensibility**: Add new implementations without modifying core logic
   - Example: Add `WaylandTextOutput` alongside `X11TextOutput`
   - Example: Add `OpenAITranscriber` alongside `WhisperTranscriber`

3. **Platform Independence**: Platform-specific code isolated in adapters
   - Core logic is platform-agnostic
   - Easy to add Windows/macOS support later

4. **Composability**: Mix and match implementations at runtime
   - Switch transcription engines based on config
   - Use different text insertion strategies per application

5. **Maintainability**: Clear contracts between components
   - Changes to internals don't affect interfaces
   - Easier to reason about component responsibilities

### Negative

1. **More Code**: Trait definitions + adapters add ~1000-2000 LOC
   - **Mitigation**: Code is well-organized and reusable

2. **Runtime Overhead**: Dynamic dispatch (vtable lookups)
   - **Impact**: Negligible (<1μs per call, audio processing is milliseconds)
   - **Mitigation**: Can use generics in hot paths if needed

3. **Learning Curve**: More abstraction layers to understand
   - **Mitigation**: Comprehensive documentation and examples

4. **Refactoring Effort**: Significant work to migrate existing code
   - **Mitigation**: Incremental migration, maintain backward compatibility

### Neutral

- Trait objects require `Box<dyn Trait>` heap allocation
  - Not a concern for long-lived components (1 allocation at startup)

## Implementation Plan

### Phase 1: Foundation (Week 1)
1. Create `src/core/traits.rs` with all trait definitions
2. Define common types (`AudioBuffer`, `TranscriptionResult`, etc.)
3. Document traits with usage examples

### Phase 2: Adapters (Week 2)
4. Create `src/adapters/` directory structure
5. Implement `CpalAudioSource` (adapter for existing AudioCapture)
6. Verify adapter works with existing code

### Phase 3: Proof of Concept (Week 2)
7. Create `MockAudioSource` implementation
8. Write unit test using mock audio
9. Validate approach before proceeding

### Phase 4: Full Migration (Week 3-4)
10. Implement adapters for all components
11. Refactor `HushApp` to use trait objects
12. Update all binaries and tests
13. Add comprehensive unit tests (target 80% coverage)

### Phase 5: Platform Abstraction (Week 5)
14. Extract X11-specific code into `X11DisplayServer`
15. Design `WaylandDisplayServer` interface
16. Implement platform detection layer

## Alternatives Considered

### Alternative 1: Keep Concrete Types, Add Test Mocks Manually
**Rejected**: Requires maintaining parallel implementations, doesn't solve extensibility

### Alternative 2: Use Generics Instead of Trait Objects
```rust
pub struct HushApp<A: AudioSource, T: Transcriber, O: TextOutput> {
    audio: A,
    transcriber: T,
    output: O,
}
```

**Rejected**:
- Type explosion (every combination is a different type)
- Cannot dynamically swap implementations at runtime
- More complex API for users
- Still achieves decoupling, but less flexible

### Alternative 3: Plugin System with Dynamic Loading
**Rejected for now**:
- Too complex for current needs
- Can be added later on top of trait system
- Security and stability concerns

## Risks and Mitigations

### Risk 1: Performance Regression
**Likelihood**: Low
**Impact**: Low
**Mitigation**:
- Benchmark before and after
- Use `#[inline]` on hot paths
- Consider monomorphization for critical loops

### Risk 2: Refactoring Breaks Existing Code
**Likelihood**: Medium
**Impact**: High
**Mitigation**:
- Incremental migration (old code works alongside new)
- Comprehensive integration tests
- Keep old implementations as adapters initially

### Risk 3: Trait Design Issues Discovered Late
**Likelihood**: Medium
**Impact**: Medium
**Mitigation**:
- Proof of concept with one component first
- Review trait APIs with team before full implementation
- Be willing to iterate on design

## Success Metrics

1. **Testability**: Unit tests can run without hardware (microphone, X11, GPU)
2. **Test Coverage**: Achieve >80% code coverage with unit tests
3. **Extensibility**: Add mock implementation for each trait within 1 hour
4. **Performance**: No regression in end-to-end latency (<5% acceptable)
5. **Code Quality**: Pass `cargo clippy` with no warnings

## References

- [Dependency Analysis](../DEPENDENCY_ANALYSIS.md)
- [Trait Design Document](../TRAIT_DESIGN.md)
- Rust Book: [Trait Objects](https://doc.rust-lang.org/book/ch17-02-trait-objects.html)
- [Adapter Pattern](https://refactoring.guru/design-patterns/adapter)

## Review and Approval

- [ ] Architecture review completed
- [ ] Team consensus achieved
- [ ] Proof of concept successful
- [ ] Decision approved

## Changelog

- 2025-10-06: Initial proposal
