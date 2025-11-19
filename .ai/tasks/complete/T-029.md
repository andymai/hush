# T-029: Add Bounds to Event Channels

## Priority
**CRITICAL** - Potential memory exhaustion

## Description
Two locations use `mpsc::unbounded_channel()` for event communication. Unbounded channels can grow without limit if producers outpace consumers, leading to potential memory exhaustion in high-frequency event scenarios.

## Locations
- `src/tray/adapters/ksni_adapter.rs:33` - Tray icon events
- `src/tray/mocks.rs:45` - Mock tray events

## Current Code
```rust
// ksni_adapter.rs:33
let (event_sender, event_receiver) = mpsc::unbounded_channel();

// mocks.rs:45
let (event_sender, event_receiver) = mpsc::unbounded_channel();
```

## Problem
- If tray events (clicks, menu actions) are generated faster than they're processed, the channel will grow unbounded
- In extreme cases (rapid clicking, menu spam), memory could grow uncontrollably
- No backpressure mechanism to slow down producers
- Silent failure mode - keeps allocating until OOM

## Solution
Replace with bounded channels with appropriate capacity:

```rust
// Bounded channel with reasonable capacity for UI events
let (event_sender, event_receiver) = mpsc::channel(100);
```

## Implementation Steps

1. **Determine appropriate channel capacity:**
   - UI events are typically low-frequency (< 10/second)
   - 100 capacity gives 10 seconds of buffering at peak rate
   - Consider 32-128 range based on expected event frequency

2. **Replace unbounded channels:**
   - `src/tray/adapters/ksni_adapter.rs:33`
   - `src/tray/mocks.rs:45`

3. **Handle backpressure:**
   - For `send()`: Consider using `try_send()` and logging dropped events
   - Or use blocking `send()` to apply backpressure to UI thread

4. **Update any code that assumes unbounded behavior:**
   - Check if sender is cloned multiple places
   - Verify error handling for `SendError`

## Recommended Capacity
```rust
// For UI events: 100 events buffer
const TRAY_EVENT_CAPACITY: usize = 100;
let (event_sender, event_receiver) = mpsc::channel(TRAY_EVENT_CAPACITY);
```

## Backpressure Strategy
For UI events, dropping old events is acceptable:
```rust
match event_sender.try_send(event) {
    Ok(_) => {},
    Err(mpsc::error::TrySendError::Full(_)) => {
        warn!("Tray event queue full, dropping event");
        // Optionally: drain and keep only latest event
    },
    Err(mpsc::error::TrySendError::Disconnected(_)) => {
        error!("Tray event receiver disconnected");
    },
}
```

## Verification
- [ ] Channels replaced with bounded versions
- [ ] Capacity constant defined with clear reasoning
- [ ] Error handling added for send failures
- [ ] cargo test passes
- [ ] cargo clippy shows no warnings
- [ ] Manual test: Rapid clicking on tray icon doesn't crash or grow memory

## Files to Modify
- `src/tray/adapters/ksni_adapter.rs` - Line 33
- `src/tray/mocks.rs` - Line 45

## Testing
```bash
# Stress test: Rapid tray icon interaction
# Should not grow memory or crash

# Monitor memory during rapid event generation
ps -o pid,vsz,rss,cmd -p $(pgrep hush)
```

## Alternative Considered
Keep unbounded but add monitoring:
- **Rejected**: Still allows unbounded growth
- Bounded channel is safer default

## Estimated Complexity
Low - Simple channel type change with error handling

## Related Issues
- None currently, but consider auditing all channel usage in codebase
- Bounded channels found in `listen.rs` (audio_cmd_tx, transcription_tx) - those are good examples
