## Description
<!-- Brief description of what this PR does -->

## Type of Change
- [ ] Bug fix (non-breaking change that fixes an issue)
- [ ] New feature (non-breaking change that adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to change)
- [ ] Performance improvement
- [ ] Refactoring (no functional changes)
- [ ] Documentation update
- [ ] CI/CD or tooling change

## Testing Performed
<!-- Describe the tests you ran and their results -->

- [ ] Unit tests pass (`cargo test`)
- [ ] Integration tests pass
- [ ] Manual testing completed

### Hardware Testing (if applicable)
- [ ] Tested with actual audio input (microphone)
- [ ] Tested text insertion via UInput/X11
- [ ] Tested hotkey capture (Ctrl+Shift+Space or configured keys)
- [ ] Tested with CUDA/GPU acceleration (if available)
- [ ] Tested on target Linux distribution

## Code Quality Checklist
- [ ] Code follows project conventions
- [ ] No clippy warnings (`cargo clippy --all-targets --all-features -- -D warnings`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] Documentation updated (README, doc comments, etc.)
- [ ] No `unwrap()` or `expect()` in production code paths
- [ ] Error handling uses `HushError` types appropriately
- [ ] Added/updated tests for new functionality
- [ ] Changelog updated (if applicable)

## Additional Context
<!-- Any additional information, screenshots, or context -->

## Related Issues
<!-- Link to related issues: Fixes #123, Relates to #456 -->
