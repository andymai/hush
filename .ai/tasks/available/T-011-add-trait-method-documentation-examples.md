# Task: Add Documentation Examples to Core Trait Methods

## Description

Add usage examples to all public trait methods in `src/core/traits.rs`. This is a Priority 3 (Quality of Life) task from the architecture analysis to improve developer experience.

## Requirements

- [ ] Add doc examples to all methods in `AudioSource` trait
- [ ] Add doc examples to all methods in `Transcriber` trait
- [ ] Add doc examples to all methods in `TextOutput` trait
- [ ] Add doc examples to all methods in `InputTrigger` trait
- [ ] Add doc examples to all methods in `SystemTray` trait
- [ ] Add doc examples to all methods in `HistoryStore` trait
- [ ] Add doc examples to all methods in `NotificationProvider` trait
- [ ] Examples should use mock implementations
- [ ] Examples should be runnable with `cargo test --doc`

## Success Criteria

- All public trait methods have doc examples
- Examples compile and run with `cargo test --doc`
- Examples demonstrate typical usage patterns
- Examples use mock implementations (no hardware required)
- `cargo doc --open` shows all examples clearly
- Code follows Rust documentation best practices

## Context

Architecture analysis noted: "⚠️ **Some traits lack examples** - Not all public APIs have usage examples in doc comments"

The codebase has excellent trait definitions but could improve developer experience with concrete usage examples.

**Current state:**
- Traits are well-documented with descriptions
- Some traits have examples, others don't
- Mock implementations exist in `src/core/mocks.rs` for use in examples

## Example Documentation Pattern

```rust
/// Start capturing audio from the source
///
/// # Example
///
/// ```rust
/// use hush::core::traits::AudioSource;
/// use hush::core::mocks::MockAudioSource;
///
/// let mut audio = MockAudioSource::new();
/// audio.start_recording()?;
///
/// // Record for some time...
/// std::thread::sleep(std::time::Duration::from_secs(3));
///
/// let buffer = audio.stop_recording()?;
/// assert!(!buffer.is_empty());
/// # Ok::<(), anyhow::Error>(())
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - Already recording
/// - Audio device unavailable
/// - Device configuration invalid
fn start_recording(&mut self) -> Result<()>;
```

## Files to Check

- `src/core/traits.rs` - All trait definitions
- `src/core/mocks.rs` - Mock implementations to use in examples
- Existing well-documented traits as patterns
- Rust API Guidelines on documentation

## Commands to Run

```bash
# Generate docs and check
cargo doc --open

# Test doc examples
cargo test --doc

# Check for missing examples
cargo doc 2>&1 | grep "missing documentation"
```

## Estimated Complexity

**Low-Medium** - Straightforward work but needs careful attention to:
- Writing clear, helpful examples
- Ensuring examples compile
- Covering common use cases
- Using appropriate error handling in examples

## Dependencies

- **Requires:** T-007 completed (core modules need clean error handling for examples)
- **Benefits from:** T-008 completed (business logic examples)

## Related Tasks

- Part of Priority 3 (Quality of Life) recommendations
- Improves developer onboarding
- Makes the codebase more accessible for contributors
- Demonstrates the excellent trait-based architecture
