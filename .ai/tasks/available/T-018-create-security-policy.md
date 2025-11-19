# Task: Create Security Policy

## Description

Create a `SECURITY.md` file in `.github/` to document how to report security vulnerabilities and which versions are supported.

## Requirements

- [ ] Create `.github/SECURITY.md`
- [ ] Document supported versions (currently 0.1.x)
- [ ] Provide clear instructions for reporting vulnerabilities
- [ ] Include contact method or GitHub security advisory process
- [ ] Follow GitHub's security policy best practices

## Success Criteria

- File created at `.github/SECURITY.md`
- Clear reporting instructions
- Supported versions documented
- Professional and actionable content
- Follows markdown formatting standards

## Implementation

Create `.github/SECURITY.md`:

```markdown
# Security Policy

## Supported Versions

We currently support the following versions with security updates:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

**Please do NOT report security vulnerabilities through public GitHub issues.**

To report a security vulnerability, please use one of the following methods:

1. **GitHub Security Advisories** (Preferred):
   - Go to the [Security tab](../../security/advisories/new)
   - Click "Report a vulnerability"
   - Provide detailed information about the vulnerability

2. **Email** (Alternative):
   - Send details to [maintainer email - update this]
   - Include "SECURITY" in the subject line

## What to Include

When reporting a vulnerability, please include:

- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Affected versions (if known)
- Suggested fix (if you have one)

## Response Timeline

- **Initial response:** Within 48 hours
- **Status update:** Within 7 days
- **Fix timeline:** Depends on severity (critical issues prioritized)

## Disclosure Policy

- We follow responsible disclosure practices
- We will credit reporters (unless they prefer to remain anonymous)
- Security fixes will be released ASAP once validated

Thank you for helping keep Hush and its users safe!
```

## Context

**Current state:** No security policy exists
**Impact:** Unclear how to report security issues, potential public disclosure of vulnerabilities
**Priority:** High - security best practice

## Files to Create

- `.github/SECURITY.md`

## Verification Steps

1. Create the file with the template above
2. Update maintainer email address
3. Verify markdown renders correctly on GitHub
4. Run `cargo fmt` (won't affect this file, but good practice)
5. Commit with clear message

## Estimated Complexity

**Low** - Simple documentation file, 5 minutes

## Dependencies

None - can be done in parallel with other tasks

## Notes

- Update the maintainer email before committing
- This file will automatically appear in GitHub's Security tab
- Consider adding PGP key if email reporting is used
