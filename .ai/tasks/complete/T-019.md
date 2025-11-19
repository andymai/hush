# Task: Create Pull Request Template

## Description

Create a PR template to ensure consistent PR descriptions and help contributors provide necessary information for reviews.

## Requirements

- [ ] Create `.github/PULL_REQUEST_TEMPLATE.md`
- [ ] Include sections for description, type of change, testing, and checklist
- [ ] Make it specific to Hush (include hardware testing considerations)
- [ ] Keep it concise but comprehensive
- [ ] Use markdown checkboxes for interactive elements

## Success Criteria

- File created at `.github/PULL_REQUEST_TEMPLATE.md`
- Template is helpful without being burdensome
- Covers code quality, testing, and hardware verification
- Renders correctly on GitHub
- Follows project conventions

## Implementation

Create `.github/PULL_REQUEST_TEMPLATE.md`:

```markdown
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
- [ ] Tested hotkey capture (Ctrl+Alt+V or configured keys)
- [ ] Tested with CUDA/GPU acceleration (if available)
- [ ] Tested on target Linux distribution

## Code Quality Checklist
- [ ] Code follows project conventions (see `.ai/knowledge/conventions.md`)
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
```

## Context

**Current state:** No PR template exists
**Impact:** Inconsistent PR descriptions, missing test information
**Priority:** Medium - improves PR quality and review process

## Files to Create

- `.github/PULL_REQUEST_TEMPLATE.md`

## Verification Steps

1. Create the file with the template above
2. Verify markdown formatting
3. Create a test PR to see how it renders
4. Ensure checkboxes are interactive
5. Commit with clear message

## Estimated Complexity

**Low** - Documentation file with standard template, 10 minutes

## Dependencies

None - can be done in parallel with other tasks

## Notes

- This template will automatically appear when creating PRs
- Contributors can delete sections that don't apply
- Template balances thoroughness with usability
- Hardware testing section is specific to Hush's needs
