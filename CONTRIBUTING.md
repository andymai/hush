# Contributing

## Setup

1. Install Rust via [rustup](https://rustup.rs/). `rust-toolchain.toml` pins the version and pulls in rustfmt and clippy.
2. Install the system libraries:

   ```bash
   # Ubuntu / Debian
   sudo apt install cmake pkg-config libasound2-dev libx11-dev libxi-dev libxtst-dev libxcursor-dev libxrandr-dev libxinerama-dev libgl1-mesa-dev
   # Fedora
   sudo dnf install cmake pkgconf-pkg-config alsa-lib-devel libX11-devel libXi-devel libXtst-devel libXcursor-devel libXrandr-devel libXinerama-devel mesa-libGL-devel
   # Arch
   sudo pacman -S cmake pkgconf alsa-lib libx11 libxi libxtst libxcursor libxrandr libxinerama mesa
   ```

3. Install Node.js 20+ and run `npm install` to enable the commit hooks.
4. Build and test:

   ```bash
   cargo build
   cargo nextest run --lib --bins   # cargo install cargo-nextest
   ```

## Workflow

1. Branch from `main` as `<type>/<kebab-description>`, for example `feat/evdev-hotkey`.
2. Make the change with tests alongside the code.
3. Commit with a [Conventional Commits](https://www.conventionalcommits.org/) message. The commit-msg hook runs commitlint; the pre-commit hook runs `cargo fmt`, `cargo clippy`, `taplo fmt`, and `cargo machete`.
4. Open a pull request. The title must also be a conventional commit subject, because squash merges use it and release-please reads it.

CI gates every PR on format, clippy, tests, MSRV, rustdoc, cargo-deny, cargo-audit, unused dependencies, TOML format, and an OSV scan. The `CI Pass` check must be green to merge.

## Commit Messages

```
feat: add evdev hotkey backend
fix: restore clipboard after paste insertion
perf: reuse the whisper state across dictations
docs: describe the udev rule
refactor: split the listen loop into a session type
test: cover the hotkey combination parser
chore: bump whisper-rs
```

`feat` and `fix` appear in the changelog and drive the version bump. Breaking changes carry a `!` after the type or a `BREAKING CHANGE:` footer.

## Code Style

- `rustfmt` defaults, enforced by CI.
- No `unwrap()` or `expect()` in non-test code. Propagate with `?` and add context with `anyhow::Context`.
- Async-first with tokio; trait methods use `async_trait`.
- Comments explain a non-obvious why, never what the code already says.
- Public APIs get `///` doc comments; examples must compile.

## Testing

Unit tests live in `#[cfg(test)] mod tests` next to the code, using the mocks in `src/core/mocks.rs`. Audio capture, hotkeys, and text insertion depend on hardware that CI does not have, so describe the manual test you ran in the PR. `hush test all` exercises the full pipeline on a real machine.

## Releases

release-please opens a release PR from the merged conventional commits and updates `CHANGELOG.md` and the version in `Cargo.toml`. Do not edit those by hand.

## License

Contributions are licensed under MIT.
