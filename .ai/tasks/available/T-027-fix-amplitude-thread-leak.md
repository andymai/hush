# T-027: Fix Amplitude Monitoring Thread Leak

## Priority
**CRITICAL** - Resource leak

## Description
The amplitude monitoring thread in `listen.rs` is spawned without storing its `JoinHandle`, making it impossible to properly clean up on shutdown. This thread runs for the duration of each recording but is never joined, leading to potential resource cleanup issues.

## Location
`src/cli/commands/listen.rs:437`

## Current Code
```rust
std::thread::spawn(move || {
    let mut amplitude_buffer = Vec::with_capacity(10);
    let update_interval = std::time::Duration::from_millis(50);
    let mut last_update = std::time::Instant::now();

    loop {
        // ... amplitude monitoring logic ...
    }
});
```

## Problem
- JoinHandle is not stored, so thread cannot be joined during cleanup
- Thread will be forcibly terminated when process exits
- No graceful shutdown mechanism

## Solution
1. Store the JoinHandle in a collection
2. Join the thread when recording stops or on application shutdown
3. Implement proper termination signal (e.g., checking if state is still Recording)

## Implementation Steps
1. Search for where AudioCommand::StartRecording is handled
2. Store the spawned thread's JoinHandle (possibly in a Vec or dedicated field)
3. Join the handle when recording stops
4. Verify the thread exits cleanly when recording state changes to non-Recording

## Verification
- [ ] Thread is properly joined when recording stops
- [ ] No orphaned threads remain after recording session
- [ ] cargo test passes
- [ ] cargo clippy shows no warnings

## Related Code
- `src/cli/commands/listen.rs:422-525` - Audio handling loop
- `src/cli/commands/listen.rs:261-276` - Hotkey thread (similar pattern to review)
- `src/cli/commands/listen.rs:285-412` - Result thread (similar pattern to review)

## Estimated Complexity
Medium - Requires tracking thread lifecycle and implementing graceful shutdown
