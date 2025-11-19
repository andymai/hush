# Task: Add SHA256 Checksum Validation for Whisper Models

## Description

Add SHA256 checksum validation when downloading/loading Whisper models to ensure model integrity and detect corruption.

## Requirements

- [ ] Add SHA256 hash constants for each model (tiny, base, small, medium, large)
- [ ] Implement checksum validation in `src/transcription/models.rs`
- [ ] Add validation after model download
- [ ] Add validation when loading existing models
- [ ] Provide clear error messages if validation fails
- [ ] Add option to skip validation if needed (for custom models)

## Success Criteria

- `cargo check` passes
- `cargo test` passes
- `cargo clippy -- -D warnings` passes
- `cargo fmt` applied
- Checksum validation works for all official Whisper models
- Clear error messages when checksum fails
- Tests added for validation logic

## Context

Currently there's a TODO in `src/transcription/models.rs:246` to add SHA256 checksum validation. This will help ensure:
- Models aren't corrupted during download
- Models haven't been tampered with
- Users are running the expected model versions

## Files to Check

- `src/transcription/models.rs:246` - TODO location
- Official Whisper model hashes (need to find from openai/whisper repo)
- `.ai/knowledge/conventions.md` - Error handling patterns
- `src/core/error.rs` - Add checksum error variant if needed

## Dependencies to Verify

```bash
# Check if SHA256 library is available
grep "sha2\|sha256" Cargo.toml
```

May need to add `sha2` crate dependency.

## Estimated Complexity

**Medium** - Requires finding official checksums, adding crypto dependency, implementing validation logic, and proper error handling
