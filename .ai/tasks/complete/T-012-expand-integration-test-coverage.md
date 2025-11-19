# Task: Expand Integration Test Coverage

## Description

Add comprehensive integration tests for the full voice-to-text pipeline using mock implementations. This is a Priority 3 task from the architecture analysis to improve test coverage and reliability.

## Requirements

- [ ] Create integration test for full pipeline (audio → transcription → text output)
- [ ] Test error scenarios with mocks
- [ ] Test state machine transitions in realistic scenarios
- [ ] Test builder pattern with various configurations
- [ ] Add tests for concurrent operations
- [ ] Test all application modes (Daemon, OneShot, Manual)
- [ ] Measure and document test coverage
- [ ] All tests should run without hardware dependencies

## Success Criteria

- Integration tests cover main user workflows
- Tests run in CI without requiring microphone/GPU/X11
- Test coverage increases measurably
- All tests pass with `cargo test`
- Tests are well-documented and serve as examples
- Tests verify error handling paths

## Context

Architecture analysis noted the need for expanded test coverage:
- Existing: 13 test files, unit tests in modules, some mocks
- Needed: Comprehensive integration tests, property-based tests, coverage measurement

**Current test infrastructure:**
- Excellent mock suite in `src/core/mocks.rs`
- Builder pattern enables easy test setup
- Some integration test binaries exist (`src/bin/test-*.rs`)

## Test Scenarios to Add

### 1. Full Pipeline Integration Test
```rust
#[tokio::test]
async fn test_complete_voice_to_text_pipeline() {
    // Setup mocks
    let audio = Box::new(MockAudioSource::new());
    let transcriber = Box::new(MockTranscriber::with_responses(vec![
        "Hello world".to_string()
    ]));
    let output = Box::new(MockTextOutput::new());

    // Build app
    let app = HushAppBuilder::new()
        .with_audio(audio)
        .with_transcriber(transcriber)
        .with_text_output(output)
        .oneshot_mode(3, false)
        .build()?;

    // Run pipeline
    let result = app.run_once().await?;

    // Verify
    assert_eq!(result.text, "Hello world");
}
```

### 2. Error Scenario Tests
- Audio device unavailable
- Transcription failure with retry
- Text insertion failure
- Invalid state transitions

### 3. Concurrent Operation Tests
- Multiple recordings in sequence
- State machine under concurrent access
- Observer notifications

### 4. Configuration Tests
- Different audio configurations
- Different transcriber configurations
- Mode switching

## Files to Create/Modify

- `tests/integration_full_pipeline.rs` - Main integration tests
- `tests/integration_error_scenarios.rs` - Error handling tests
- `tests/integration_state_machine.rs` - State transition tests
- `tests/integration_concurrent.rs` - Concurrency tests
- Update `src/lib.rs` to expose necessary internals for testing

## Coverage Measurement

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Measure coverage
cargo tarpaulin --out Html --output-dir coverage/

# Set coverage goals
# Current: Unknown
# Target: >70% line coverage in core modules
```

## Property-Based Testing

Consider adding `proptest` for property-based tests:

```toml
[dev-dependencies]
proptest = "1.0"
```

Example properties:
- Audio buffer size matches duration × sample_rate
- State transitions are always valid
- Error severity classification is consistent

## Estimated Complexity

**Medium-High** - Requires:
- Understanding the full application flow
- Designing comprehensive test scenarios
- Setting up test infrastructure
- Writing clear, maintainable tests
- Measuring and documenting coverage

Significant value but requires time investment.

## Dependencies

- **Benefits from:** T-007, T-008, T-009 completed (clean error handling makes tests easier)
- **Benefits from:** T-011 completed (documentation examples show patterns)

## Related Tasks

- Part of Priority 3 recommendations
- Complements existing unit tests
- Provides regression protection
- Serves as living documentation of system behavior
