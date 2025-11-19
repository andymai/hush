# Task: Document Unsafe Code Safety Invariants

## Description

Add comprehensive safety documentation for all `unsafe` code blocks in the codebase, particularly the `unsafe impl Send` and `unsafe impl Sync` in the audio adapter. This is a Priority 2 task from the architecture analysis.

## Requirements

- [ ] Audit all `unsafe` code in the codebase
- [ ] Add detailed `SAFETY:` comments explaining invariants
- [ ] Document why `unsafe` is necessary and alternatives considered
- [ ] Add runtime assertions where possible to verify safety
- [ ] Consider refactoring to avoid `unsafe` if practical
- [ ] Add tests that stress the safety assumptions
- [ ] Document in architecture docs if pattern is used elsewhere

## Success Criteria

- All `unsafe` blocks have comprehensive `SAFETY:` documentation
- Safety invariants are clearly explained
- Tests verify the safety assumptions hold
- `cargo check` and `cargo test` pass
- Architecture docs mention unsafe patterns and rationale

## Context

Architecture analysis found **2 uses of `unsafe impl Send/Sync`** in `src/adapters/audio/cpal_adapter.rs`. While documented, they represent technical debt and need comprehensive safety analysis.

**Current unsafe code:**
```rust
// src/adapters/audio/cpal_adapter.rs:17-18
unsafe impl Send for ThreadSafeAudioCapture {}
unsafe impl Sync for ThreadSafeAudioCapture {}
```

**Current documentation:**
```rust
// SAFETY: We use Arc<Mutex<>> to ensure only one thread can access at a time
```

**Needs:**
- More detailed explanation of why cpal::Stream isn't Send/Sync
- Proof that Arc<Mutex<>> actually prevents concurrent access
- Discussion of what could go wrong
- Whether callbacks hold references that violate assumptions
- Testing strategy to verify safety

## Tasks

1. **Audit Phase:**
   ```bash
   rg "unsafe" --type rust src/
   rg "unsafe impl" --type rust src/
   ```

2. **Documentation Phase:**
   - Add extended SAFETY comments
   - Document the cpal::Stream threading model
   - Explain why wrapping in Arc<Mutex<>> is sufficient
   - Note any runtime checks

3. **Testing Phase:**
   - Add multi-threaded tests
   - Test concurrent access patterns
   - Use tools like `loom` for concurrency testing if appropriate

4. **Refactoring Phase (if possible):**
   - Consider alternatives that avoid unsafe
   - Could we use a different audio library?
   - Could we restrict to single-threaded usage?

## Files to Check

- `src/adapters/audio/cpal_adapter.rs` - Primary unsafe code
- `src/audio/` - Audio capture implementation
- CPAL documentation on threading
- `.ai/knowledge/conventions.md` - Add unsafe code guidelines

## Safety Documentation Template

```rust
/// SAFETY: This unsafe impl is required because [reason].
///
/// # Why unsafe is necessary
/// [Explanation of why the wrapped type isn't Send/Sync]
///
/// # Safety invariants
/// 1. [First invariant and how it's maintained]
/// 2. [Second invariant and how it's maintained]
///
/// # What could go wrong
/// [Scenarios that would violate safety]
///
/// # Why this is safe
/// [Proof that invariants are maintained]
///
/// # Alternatives considered
/// [Other approaches and why they weren't used]
///
/// # Testing
/// [How safety is tested]
unsafe impl Send for Type {}
```

## Estimated Complexity

**Medium** - Requires deep understanding of:
- Rust's Send/Sync auto-traits
- CPAL threading model
- Audio stream lifecycle
- Concurrency patterns

Not much code to write, but significant analysis required.

## Related Tasks

- Part of Priority 2 recommendations from architecture analysis
- Improves code maintainability and safety verification
- Sets pattern for any future unsafe code
