# T-030: Use Thread Pool for Audio Feedback

## Priority
**MEDIUM** - Code quality and resource management

## Description
Audio feedback functions (`play_start`, `play_stop`, `play_error`, `play_success`) spawn detached threads for each sound effect. While threads are short-lived (~100ms), rapid calls could accumulate threads. A thread pool or task queue would be more efficient.

## Locations
- `src/audio/feedback.rs:38` - `play_start()`
- `src/audio/feedback.rs:53` - `play_stop()`
- `src/audio/feedback.rs:68` - `play_error()`
- `src/audio/feedback.rs:83` - `play_success()`

## Current Pattern
```rust
pub fn play_start(&self) -> Result<()> {
    if !self.enabled {
        return Ok(());
    }

    std::thread::spawn(|| {
        if let Err(e) = play_tone(440.0, 100) {
            warn!("Failed to play start sound: {}", e);
        }
    });

    Ok(())
}
```

## Problem
- Each call spawns a new thread (overhead ~0.5-1ms per spawn)
- Threads are detached - no way to know when they complete
- If called in rapid succession (e.g., stress testing), could accumulate many threads
- Thread creation overhead may cause audio timing jitter

## Impact Assessment
- **Current**: Low risk - sounds are infrequent (user-triggered) and short-lived
- **Potential**: If used in automated/high-frequency scenarios, could be problematic
- **Best practice**: Use thread pool for all background tasks

## Solution Options

### Option 1: Use Rayon Thread Pool (Recommended)
```rust
use rayon::ThreadPool;
use std::sync::Arc;

pub struct AudioFeedback {
    _stream: Option<Arc<OutputStream>>,
    enabled: bool,
    thread_pool: Arc<rayon::ThreadPool>,
}

impl AudioFeedback {
    pub fn new() -> Result<Self> {
        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(2) // Only need 1-2 threads for audio playback
            .thread_name(|i| format!("audio-feedback-{}", i))
            .build()?;

        // ... existing code ...
        Ok(Self {
            _stream,
            enabled,
            thread_pool: Arc::new(thread_pool),
        })
    }

    pub fn play_start(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let pool = Arc::clone(&self.thread_pool);
        pool.spawn(|| {
            if let Err(e) = play_tone(440.0, 100) {
                warn!("Failed to play start sound: {}", e);
            }
        });

        Ok(())
    }
}
```

### Option 2: Use Tokio Runtime (if already available)
```rust
// If app already has tokio runtime
tokio::task::spawn_blocking(|| {
    if let Err(e) = play_tone(440.0, 100) {
        warn!("Failed to play start sound: {}", e);
    }
});
```

### Option 3: Single Dedicated Audio Thread + Queue
```rust
// More complex but gives precise control
use std::sync::mpsc;

enum AudioCommand {
    PlayStart,
    PlayStop,
    PlayError,
    PlaySuccess,
}

// Single thread processes audio commands
let (tx, rx) = mpsc::sync_channel(10);
std::thread::spawn(move || {
    while let Ok(cmd) = rx.recv() {
        match cmd {
            AudioCommand::PlayStart => play_tone(440.0, 100),
            // ... etc
        }
    }
});
```

## Recommended Approach
**Option 1 (Rayon)** is recommended because:
- Existing dependency (already in Cargo.toml)
- Simple API - minimal code changes
- Efficient thread reuse
- Bounded thread count (2 threads max)

## Implementation Steps

1. **Add rayon dependency** (if not already present):
   ```toml
   # Check Cargo.toml first
   [dependencies]
   rayon = "1.8"
   ```

2. **Modify `AudioFeedback` struct:**
   - Add `thread_pool: Arc<rayon::ThreadPool>` field
   - Initialize in `new()` with 2 threads
   - Add to `Default` impl

3. **Update all 4 play methods:**
   - Replace `std::thread::spawn` with `pool.spawn`
   - Keep existing error handling

4. **Test:**
   - Verify sounds still play correctly
   - Stress test: Call play methods 100 times rapidly
   - Check thread count stays bounded

## Verification
- [ ] Thread pool initialized in `AudioFeedback::new()`
- [ ] All 4 play methods use pool instead of spawning threads
- [ ] Stress test: 100 rapid calls doesn't create 100 threads
- [ ] Audio playback functionality unchanged
- [ ] cargo test passes
- [ ] cargo clippy shows no warnings

## Files to Modify
- `src/audio/feedback.rs` - Lines 9-100
- `Cargo.toml` - Verify rayon dependency

## Testing
```bash
# Stress test in a loop
for i in {1..100}; do
    # Trigger audio feedback rapidly
    # Check thread count
    ps -eLf | grep hush | wc -l
done

# Should see thread count stay constant, not grow
```

## Performance Impact
- **Before**: N thread spawns for N sounds = N × ~1ms overhead
- **After**: Thread pool reuse = ~0.01ms per task
- **Benefit**: 100× faster for rapid sequential sounds

## Estimated Complexity
Low-Medium - Straightforward refactoring with thread pool API

## Dependencies
```bash
# Verify rayon is available
rg "rayon" Cargo.toml
```

## Alternative: Do Nothing
Since audio feedback is user-triggered and infrequent, current approach is functionally acceptable. This is a code quality improvement, not a critical bug fix.

**Recommendation**: Implement as part of general code cleanup, not urgent.
