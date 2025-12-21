# Contributing

## Setup

```bash
git clone https://github.com/andymai/hush.git
cd hush
sudo apt install libasound2-dev pkg-config libx11-dev  # Ubuntu/Debian
make build
cargo test
```

## Before Submitting

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```

A pre-commit hook runs these checks automatically.

## Project Structure

```
src/
├── core/           # Traits, errors, state machine
├── adapters/       # Audio, transcription, text implementations
├── application/    # App orchestration
├── cli/            # Commands
├── text_processing/# Filler removal, voice commands
├── transcription/  # Whisper integration
└── overlay/        # UI
```

## Code Style

- Run `cargo fmt` before committing
- No `.unwrap()` in production code—use proper error handling
- Document public APIs with `///` comments

## Pull Requests

1. One logical change per PR
2. Include tests for new functionality
3. Reference related issues with "Fixes #123"

## License

Contributions are licensed under MIT.
