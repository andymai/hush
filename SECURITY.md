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

Hush reads keyboard events to detect the hotkey and injects keystrokes to insert text. On Linux both come from device nodes: `/dev/input/event*` for reading and `/dev/uinput` for writing. `hush setup permissions` installs one udev rule (`/etc/udev/rules.d/70-hush.rules`) that tags those nodes with `uaccess`. Then systemd-logind grants the physically seated user an ACL on them for the session, and removes it at logout. That single rule is the only thing setup runs as root. Any process running as the seated user can then read keystrokes and type; Hush never elevates beyond that.

The `.deb`, `.rpm`, and COPR packages install the same rule in `/usr/lib/udev/rules.d`. The Nix flake ships it in `lib/udev/rules.d` of its package, for `services.udev.packages`.

Transcription runs locally through transcribe.cpp by default, so audio stays on the machine. Cloud transcription through Groq or OpenAI is opt-in only (`transcription.backend`). It sends the audio of each dictation over HTTPS with a key from `GROQ_API_KEY` or `OPENAI_API_KEY`, and it never starts as a fallback. The `openai-compatible` backend is also opt-in only. It sends the audio to the URL in `transcription.server_url`, over HTTP or HTTPS as that URL says. It sends the key from the variable in `transcription.server_key_env` when you set that variable.

Model downloads come from `huggingface.co/ggerganov/whisper.cpp` and `huggingface.co/ggml-org/whisper-vad`. They also come from `huggingface.co/collectivecontext/hush-models`, but only for the language-ID model when you set `transcription.detect_languages`, and for the Parakeet model when you select it. Hush verifies each download against a pinned SHA-256 checksum.

LLM polish needs an LLM. Hush uses a local Ollama server when one answers, and Anthropic only when `ANTHROPIC_API_KEY` exists. It uses an OpenAI-compatible server only when `llm.provider` names it. With none of these, polish is rule-based and sends nothing. Polish and Command Mode send text, never audio, to the provider.

The update check is off by default. When you turn it on, an AppImage, tarball, or source install asks `api.github.com` for the latest release once a day. The request carries only the Hush version in its user agent. A package-manager install never runs it.

## Supply Chain

The build fails closed on the patterns the 2025-2026 npm and GitHub Actions supply-chain attacks exploited:

| Defense | Where | What it blocks |
|---|---|---|
| All GitHub Actions pinned to commit SHA | `.github/workflows/*.yml` | Tag-retag attacks. |
| OSV scan against `Cargo.lock` and `package-lock.json` (PRs report-only, main blocking, weekly) | `.github/workflows/osv-scan.yml` | Known-CVE versions in either ecosystem. |
| cargo-deny license and source policy, cargo-audit advisories | `deny.toml`, `.github/workflows/ci.yml` | Unvetted licenses, unknown registries, RustSec advisories. |
| Dependabot cooldown (7d default, 14d major) across cargo, npm, github-actions | `.github/dependabot.yml` | Fresh malicious uploads. |
| Workflow tokens default to no permissions; each job opts in | `.github/workflows/*.yml` | Credential reach from a compromised step. |
| Build provenance attestation for each release file, checked by `install.sh` when `gh` is present | `.github/workflows/release.yml`, `install.sh` | A release file that the release workflow did not build. |

From 0.17.0, each release file has a Sigstore-signed build provenance attestation. This includes the tarball, the `.deb`, the `.rpm`, the AppImage, and `SHA256SUMS`. To verify a file, run `gh attestation verify <file> --repo andymai/hush` with GitHub CLI 2.49 or later. [INSTALL.md](INSTALL.md#verify-a-download) has the full steps.
