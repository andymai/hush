# Integration Tests

## Running Tests

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_pipeline_with_mock_components
```

## Test Files

| File | Purpose |
|------|---------|
| `architecture_poc_test.rs` | Trait-based architecture tests using mocks |
| `archive/` | Legacy hardware-dependent tests (not run by default) |

## Key Features Tested

- Full pipeline: audio → transcription → text output
- Mock implementations for hardware-independent testing
- Error handling scenarios
- Component isolation

All tests run without requiring hardware (microphone, GPU, display).
