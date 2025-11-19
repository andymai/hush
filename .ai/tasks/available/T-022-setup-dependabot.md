# Task: Set Up Dependabot for Automated Dependency Updates

## Description

Create a Dependabot configuration to automatically monitor and update Rust dependencies and GitHub Actions, improving security and reducing maintenance burden.

## Requirements

- [ ] Create `.github/dependabot.yml` configuration
- [ ] Configure Cargo ecosystem monitoring
- [ ] Configure GitHub Actions ecosystem monitoring
- [ ] Set weekly update schedule
- [ ] Group related dependencies (candle-*, egui*, tracing*)
- [ ] Verify configuration syntax

## Success Criteria

- File created at `.github/dependabot.yml`
- Dependabot recognizes the configuration
- Weekly PRs are created for dependency updates
- Grouped dependencies update together (e.g., all candle crates)
- Configuration follows best practices

## Implementation

Create `.github/dependabot.yml`:

```yaml
version: 2
updates:
  # Rust dependencies
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "weekly"
      day: "monday"
      time: "09:00"
    # Group related dependencies to reduce PR noise
    groups:
      candle:
        patterns:
          - "candle-*"
      egui:
        patterns:
          - "egui*"
      tracing:
        patterns:
          - "tracing*"
      tokio:
        patterns:
          - "tokio*"
    # Keep existing version constraints
    open-pull-requests-limit: 10

  # GitHub Actions
  - package-ecosystem: "github-actions"
    directory: "/"
    schedule:
      interval: "weekly"
      day: "monday"
      time: "09:00"
    open-pull-requests-limit: 5
```

## Context

**Current state:** No automated dependency updates
**Impact:** Security vulnerabilities, outdated dependencies, manual update burden
**Priority:** High - security and maintenance automation

## Files to Create

- `.github/dependabot.yml`

## Verification Steps

1. Create the configuration file
2. Validate YAML syntax
3. Check GitHub's Dependabot config validator
4. Commit and push
5. Verify Dependabot recognizes config (check Insights > Dependency graph > Dependabot)
6. Wait for first scheduled run or trigger manually
7. Review first batch of PRs

## Estimated Complexity

**Low** - Configuration file, 10 minutes

## Dependencies

None - can be done in parallel with other tasks

## Notes

- Dependabot PRs will trigger CI automatically
- Review and merge PRs promptly to stay up to date
- Groups reduce noise by combining related updates
- Monday 9am schedule avoids weekend surprises
- Can adjust `open-pull-requests-limit` if too many PRs
- Consider adding `.github/dependabot.yml` to `.github/CODEOWNERS` if using that feature
- Dependabot respects version constraints in Cargo.toml (e.g., `= 0.15` won't be updated)
