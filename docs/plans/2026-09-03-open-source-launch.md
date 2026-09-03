# Open Source Launch Plan

Date: 2026-09-03
Status: Approved

Hush goes public as a 0.x beta once the repository is hardened (M0) and the launch blockers are fixed (M1). It reaches 1.0 by shipping the everyday-user experience (M2) in the open. Each milestone maps to a GitHub milestone; each bullet maps to an issue.

## Product decisions

| Area | Decision |
|---|---|
| Audience | Everyday Linux desktop users. The happy path never opens a terminal. |
| Display server | Native Wayland parity on KDE Plasma, GNOME, Hyprland, and Sway, plus X11. |
| GPU | Prebuilt binaries use the whisper.cpp Vulkan backend, which runs on NVIDIA, AMD, and Intel with the stock graphics driver. CUDA remains a source build behind `--features cuda`. |
| Hotkey | An evdev listener on `/dev/input` provides hold-to-talk and toggle on every session type. `hush toggle` over a Unix socket serves compositor keybinds. |
| Text insertion | uinput is primary. Clipboard-paste handles non-US layouts and apps that reject synthetic keys. Window detection is optional per compositor. |
| Permissions | One polkit prompt in the wizard installs a udev rule tagging `/dev/uinput` and `/dev/input/event*` with `uaccess`, so the seated user gets access without a logout. |
| App model | Background daemon with a StatusNotifierItem tray icon and autostart. A foreground mode serves terminals and systemd user units. |
| Feedback | Layer-shell overlay pill, tray icon state, opt-in sound cues. |
| Modes | Hold-to-talk and toggle, chosen in settings. |
| Languages | Multilingual transcription with automatic language detection. |
| Default model | `large-v3-turbo` q5_0 when a GPU is detected, `base` on CPU. The wizard shows the choice and allows an override. |
| Network | Transcription is local only. LLM polish is opt-in via Anthropic or Ollama. Model downloads are the only default network traffic. |
| Onboarding | GUI wizard from the app menu. The terminal wizard serves CLI launches and scripted installs. |
| Settings | A minimal settings window from the tray. The TOML file remains the full surface. |
| Toolkit | iced with iced_layershell for wizard, settings, and overlay. |
| Naming | Binary and packages are `hush`. App id `io.github.andymai.hush`. Tags `vX.Y.Z`. No crates.io publish. |
| Distribution | GitHub Releases (.deb, .rpm, AppImage, tar.gz, SHA256SUMS), a curl installer, AUR `hush-bin`, Fedora COPR. Flathub follows the portal backends in M3. |
| Launch | Public beta at 0.2.0 after M0 and M1. 1.0 when M2 ships. |

## Target setup flow

1. Install: download the package for the distro, run the one-line installer, or install from AUR or COPR.
2. Launch Hush from the app menu. The wizard opens.
3. Wizard: detect GPU, download the recommended model with progress, one polkit prompt for the udev rule, microphone check with a live level meter, choose hotkey and mode, dictate into a test box, enable autostart.
4. The wizard closes, the tray icon appears, and the hotkey works in any window.

Terminal path: `hush` with no config runs the same steps as text prompts. `hush setup --defaults` runs unattended.

## Milestones

### M0: Repo hardening

Mirrors the patterns in brepkit, brepjs, and elevator-core.

- CI workflow with SHA-pinned actions, `permissions: {}` by default, concurrency groups, and parallel jobs: fmt, clippy, nextest, doctests, MSRV, cargo-deny, cargo-audit, rustdoc with `-D warnings`, cargo-machete, taplo. A `CI Pass` job gates merges.
- OSV scan: report-only on PRs, blocking on main and weekly.
- release-please with the release-kun App token, `release-type: rust`, `bump-minor-pre-major`, auto-approve and auto-merge of release PRs, Cargo.lock refresh on the release PR, `workflow_dispatch` recovery path.
- Dependabot with 7-day cooldown (14 for majors), grouped minor and patch updates, github-actions group, dtolnay/rust-toolchain ignored. Auto-merge for patch and minor.
- PR-title commitlint on `pull_request_target`. Husky hooks: commit-msg commitlint, pre-commit fmt, clippy, taplo, machete.
- `rust-toolchain.toml` pin, `rust-version` in Cargo.toml, Cargo.lock committed, `deny.toml`, `.editorconfig`.
- Community files: CODE_OF_CONDUCT (Contributor Covenant 2.1), SECURITY.md with the supply-chain table, FUNDING.yml, issue forms with blank issues disabled, PR template, CONTRIBUTING rewritten for the new tooling.
- Repo settings: squash-only, delete branch on merge, Discussions on, branch protection on main requiring `CI Pass`, linear history, no force pushes.
- Makefile works on Fedora and Arch (no hardcoded Debian pkgconfig path). CUDA targets keep their preflight.

### M1: Launch blockers

- Config resolves from `$XDG_CONFIG_HOME/hush/config.toml` with `--config` and `HUSH_CONFIG` overrides. Built-in defaults compile in from `config/default.toml`. `hush setup init` writes to the XDG path.
- Remove the candle stack (candle-core, candle-nn, candle-transformers, hf-hub, tokenizers, safetensors) and the experimental safetensors path. Model downloads use reqwest against `huggingface.co/ggerganov/whisper.cpp` with SHA256 verification.
- `vulkan` Cargo feature via `whisper-rs/vulkan`. GPU detection reports the Vulkan device name. Falls back to CPU when no device is found.
- evdev hotkey backend implementing `InputTrigger`, with hold and toggle modes, device hotplug, and a combination parser shared with the config format. The X11 backend remains a fallback when `/dev/input` is not readable.
- Daemon and IPC: `hush daemon start|stop|restart`, `hush toggle`, `hush cancel`, `hush status`, JSON over a Unix socket in `$XDG_RUNTIME_DIR/hush`. `hush listen` becomes an alias for `hush daemon start --foreground`.
- Text insertion: uinput then clipboard-paste. Remove enigo and the hard X11 connection. Window detection becomes an optional provider (X11 when `DISPLAY` is set, `hyprctl` and `swaymsg` on wlroots, none on GNOME).
- Permission setup: `hush setup permissions` writes `/etc/udev/rules.d/70-hush.rules` via `pkexec`, reloads udev, and verifies access. Replaces the `input` group instructions.
- Remove every non-test `unwrap`, fix the drifted doctests, and run doctests in CI.
- Retire `install.sh` and `scripts/` in favor of the wizard and the release installer. Correct stale docs (hotkey name, config paths, MSRV).
- Release pipeline: on a release tag, build `hush` with `--features vulkan`, produce `.deb` (cargo-deb), `.rpm` (cargo-generate-rpm), AppImage, `tar.gz`, and `SHA256SUMS`, upload to the GitHub Release, publish the AUR `hush-bin` PKGBUILD, and trigger the COPR build.
- `install.sh` served from the repo detects the distro, downloads the matching asset, verifies the checksum, and installs it.
- Packaged desktop entry, icon, and systemd user unit under `io.github.andymai.hush`.
- README rewritten for the everyday-user path with a demo GIF, install matrix, and privacy statement.

Exit: repository set to public, release 0.2.0 tagged.

### M2: Everyday-user experience (1.0)

- iced GUI wizard covering the target setup flow.
- Tray icon via ksni with idle, recording, and processing states and a menu: pause, open settings, view log, quit.
- Overlay pill on iced_layershell: recording indicator with level meter, processing spinner, error toast. Never takes focus.
- Settings window: hotkey capture, mode, model picker with inline download, language, microphone with level meter, insertion method, autostart, sound cues, LLM polish.
- Multilingual transcription with `language = "auto"`, and the clipboard-paste insertion path selected automatically for non-US layouts.
- Ollama as an LLM polish provider next to Anthropic.
- Hardware-based default model selection in the wizard.
- `hush doctor` with actionable fixes replacing `status --full`.
- Test matrix pass on KDE Wayland, GNOME Wayland, Hyprland, and X11 with NVIDIA, AMD, and Intel GPUs, collected through community beta reports.

Exit: release 1.0.0.

### M3: After 1.0

- Portal backends (GlobalShortcuts, RemoteDesktop with libei) and a Flathub listing.
- Full preferences: vocabulary editor, transcription history, per-app insertion rules.
- Voice-activity auto-stop for toggle mode.
- Cloud transcription backends (Groq, OpenAI) as explicit opt-in.
- Nix flake, Ubuntu PPA, update notifications for non-repo installs.

## Architecture changes

### Configuration

`Config::load` resolves, in order: `--config`, `HUSH_CONFIG`, `$XDG_CONFIG_HOME/hush/config.toml`. Missing files fall back to compiled defaults. Models live in `$XDG_DATA_HOME/hush/models`; the cache location keeps working through a one-time migration. Runtime files (socket, PID) live in `$XDG_RUNTIME_DIR/hush`.

### Transcription

whisper-rs is the only engine. Cargo features: `vulkan` (release builds), `cuda` (source builds), neither (CPU). Device detection queries whisper.cpp's backend registry and reports the active device for the wizard and `hush doctor`. Model catalogue adds `large-v3-turbo` and its q5_0 variant with verified checksums.

### Hotkey

`EvdevTrigger` opens every keyboard-capable device under `/dev/input`, tracks modifier state, and emits press and release for the configured combination. Hold mode maps press to start and release to stop. Toggle mode flips on press. A udev monitor handles hotplug. `hush toggle` reaches the same state machine over IPC.

### Text insertion

`TextInserter` picks a strategy per dictation: uinput for US layouts and ordinary apps, clipboard-paste for other layouts, very long text, and apps on the paste-preferred list. Clipboard-paste saves the current clipboard, sets the text, sends Ctrl+V through uinput, and restores the clipboard. Wayland clipboards use arboard's Wayland data-control backend.

### Daemon and IPC

The daemon owns the hotkey listener, audio capture, transcription, insertion, tray, and overlay. Commands and state updates are newline-delimited JSON on a Unix socket. A PID file enforces a single instance. Autostart uses a `.desktop` entry in `~/.config/autostart`, with an optional systemd user unit.

### UI

Wizard, settings, and overlay are iced views hosted by the daemon process. The overlay uses layer-shell on Wayland and a floating undecorated window on X11. The tray uses ksni (StatusNotifierItem), which KDE, GNOME with AppIndicator, and waybar display.

### Packaging

One release workflow builds the Vulkan binary on ubuntu-22.04 for glibc compatibility, then packages it four ways. `.deb` and `.rpm` depend on `libvulkan1`, `libasound2`, and `libxkbcommon0`. The AppImage bundles everything except libvulkan and the graphics driver. The udev rule ships in every package and the wizard installs it for tarball and AppImage users.

## Security model

Hush reads keyboard events and injects keystrokes. Both capabilities come from `/dev/input` and `/dev/uinput`. The udev rule grants them to the physically seated user only, through systemd's `uaccess` ACL, and removes them at logout. SECURITY.md states this plainly: a process running as the user can already do both on X11, and on Wayland this is the price of a hotkey that works in every app. Transcription runs locally. LLM polish sends text (never audio) only when the user enables it. Model downloads are checksum-verified.

## Release pipeline

release-please opens a release PR from conventional commits, CI validates it, the release-kun App merges it, and the tag triggers the release workflow. Every workflow pins actions by SHA and runs with the minimum token permissions. A `workflow_dispatch` input re-runs packaging for an existing tag.

## Test matrix

| Environment | Owner | Covers |
|---|---|---|
| Fedora Atomic host, KDE Wayland, RTX 4080 SUPER | maintainer | evdev, uinput, udev on ostree, Vulkan and CUDA, tray on Plasma, layer-shell on KWin |
| Ubuntu GNOME, Fedora GNOME | community beta | .deb and .rpm, AppIndicator tray, GNOME without window detection |
| Arch with Hyprland or Sway | community beta | AUR, wlroots layer-shell, compositor keybind via `hush toggle` |
| AMD and Intel GPUs | community beta | Vulkan backend on non-NVIDIA drivers |
| Non-US layouts | community beta | clipboard-paste insertion |

## Owner actions

- Install the release-kun GitHub App on `andymai/hush` and add `RELEASE_KUN_APP_ID` and `RELEASE_KUN_APP_PRIVATE_KEY` as repository secrets.
- Create the COPR project `andymai/hush` and add `COPR_API_LOGIN` and `COPR_API_TOKEN` secrets.
- Register `hush-bin` on AUR and add `AUR_SSH_PRIVATE_KEY`.
- Set the repository to public when M1 exits.

## Risks

| Risk | Mitigation |
|---|---|
| Vulkan shader compilation at build time needs `glslc` on the runner | Install `glslc` and `libvulkan-dev` in CI; the release job caches the ggml build. |
| `uaccess` does not apply on sessions without logind (some Sway setups) | `hush doctor` detects the missing ACL and offers the `input` group path with the trade-off explained. |
| iced_layershell tracks iced closely and can lag a release | Pin both crates; the overlay is small enough to port. |
| evdev sees keys while another app has a grab | Modifier tracking is local, so a grab in another app never blocks the hotkey; the release edge is always observed. |
| AppImage on systems without FUSE | The installer prefers `.deb` and `.rpm`; the AppImage documents `--appimage-extract-and-run`. |
