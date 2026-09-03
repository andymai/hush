# Security Policy

## Reporting a Vulnerability

Use [GitHub Security Advisories](https://github.com/andymai/hush/security/advisories/new) for private disclosure.

Include:

- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

Expect an initial response within 48 hours.

## What Hush Can Do on Your Machine

Hush reads keyboard events to detect the hotkey and injects keystrokes to insert text. On Linux both come from device nodes: `/dev/input/event*` for reading and `/dev/uinput` for writing. `hush setup permissions` installs one udev rule (`/etc/udev/rules.d/70-hush.rules`) that tags those nodes with `uaccess`, so systemd-logind grants the physically seated user an ACL on them for the session and removes it at logout. That single rule is the only thing setup runs as root. Any process running as the seated user can then read keystrokes and type; Hush never elevates beyond that.

Transcription runs locally through whisper.cpp, so audio never leaves the machine. Model downloads come from `huggingface.co/ggerganov/whisper.cpp` and are verified against pinned SHA256 checksums. LLM polish is off by default; when enabled it sends transcribed text, never audio, to the configured provider.

## Supply Chain

The build fails closed on the patterns the 2025-2026 npm and GitHub Actions supply-chain attacks exploited:

| Defense | Where | What it blocks |
|---|---|---|
| All GitHub Actions pinned to commit SHA | `.github/workflows/*.yml` | Tag-retag attacks. |
| OSV scan against `Cargo.lock` and `package-lock.json` (PRs report-only, main blocking, weekly) | `.github/workflows/osv-scan.yml` | Known-CVE versions in either ecosystem. |
| cargo-deny license and source policy, cargo-audit advisories | `deny.toml`, `.github/workflows/ci.yml` | Unvetted licenses, unknown registries, RustSec advisories. |
| Dependabot cooldown (7d default, 14d major) across cargo, npm, github-actions | `.github/dependabot.yml` | Fresh malicious uploads. |
| Workflow tokens default to no permissions; each job opts in | `.github/workflows/*.yml` | Credential reach from a compromised step. |
