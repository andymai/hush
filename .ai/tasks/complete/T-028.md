# T-028: Consolidate Tokio Runtime Creation

## Priority
**CRITICAL** - Performance issue

## Description
Multiple locations create new `tokio::Runtime` instances, which is expensive as each spawns a full thread pool. This causes performance degradation and excessive thread creation. The codebase should reuse a single runtime or use `Handle::current()` where possible.

## Locations
- `src/transcription/whisper.rs:146` - In `transcribe()` method
- `src/cli/commands/listen.rs:286` - In result handler thread
- `src/bin/test-intelligent-editing.rs:83, 216` - Test binaries (less critical)
- `src/bin/test-full-integration.rs:81` - Test binaries (less critical)

## Current Pattern (Anti-pattern)
```rust
// whisper.rs:146
let rt = tokio::runtime::Runtime::new()?;
let result = rt.block_on(self.transcribe_with_sample_rate(audio_data, 16000))?;

// listen.rs:286
let result_runtime = match tokio::runtime::Runtime::new() {
    Ok(runtime) => runtime,
    Err(e) => {
        error!("Failed to create tokio runtime: {}", e);
        return;
    },
};
```

## Problem
- Creating a Runtime spawns ~N CPU worker threads (where N = number of cores)
- Multiple runtimes = multiple thread pools = resource waste
- Runtime creation overhead on every transcription call
- Can lead to thread exhaustion in high-frequency scenarios

## Solution Options

### Option 1: Reuse Existing Runtime (Preferred)
```rust
// Check if we're already in a runtime
match tokio::runtime::Handle::try_current() {
    Ok(handle) => handle.block_on(async_operation()),
    Err(_) => {
        // Only create if absolutely necessary
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async_operation())
    }
}
```

### Option 2: Application-Level Runtime
- Create one runtime at application startup
- Pass `Handle` to components that need it
- Store in application state

### Option 3: Thread-Local Runtime
- Use `thread_local!` for runtime storage
- Initialize once per thread

## Implementation Steps

1. **For `whisper.rs:146`:**
   - Check for existing runtime with `Handle::try_current()`
   - Only create new runtime if none exists
   - Consider making `transcribe()` method async to avoid blocking entirely

2. **For `listen.rs:286` (result handler thread):**
   - Create runtime once when thread starts, not on every message
   - Reuse the same runtime instance for all transcription results
   - Store runtime outside the message loop

3. **For test binaries:**
   - Less critical but follow same pattern
   - Consider using `#[tokio::test]` macro instead

4. **Verify no performance regression:**
   - Benchmark transcription throughput before/after
   - Check thread count during operation

## Verification
- [ ] Only 1-2 tokio runtimes exist during normal operation (check with profiler)
- [ ] Thread count reduced significantly
- [ ] Transcription performance maintained or improved
- [ ] cargo test passes
- [ ] cargo clippy shows no warnings

## Files to Modify
- `src/transcription/whisper.rs` - Lines 138-151
- `src/cli/commands/listen.rs` - Lines 286-295
- `src/bin/test-intelligent-editing.rs` - Lines 83, 216 (optional)
- `src/bin/test-full-integration.rs` - Line 81 (optional)

## Testing
```bash
# Before fix - count threads during operation
ps -eLf | grep hush | wc -l

# After fix - should be significantly fewer
ps -eLf | grep hush | wc -l

# Functional test
cargo test test_concurrent_transcription
```

## Estimated Complexity
Medium - Requires understanding of tokio runtime lifecycle and refactoring multiple callsites

## References
- Tokio best practices: https://tokio.rs/tokio/topics/bridging
- Runtime creation overhead: ~1ms + N threads per core
