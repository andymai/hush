# Hush

**Local voice-to-text for Linux developers**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

Hush runs OpenAI's Whisper models locally with GPU acceleration. Hold a hotkey, speak, release—your text appears at the cursor. Works everywhere: terminals, VMs, games, password fields.

All processing happens on your machine. Your voice data never leaves your computer.

## Quick Start

```bash
# Install dependencies (Ubuntu/Debian)
sudo apt install libasound2-dev pkg-config libx11-dev

# Build
git clone https://github.com/andymai/hush.git
cd hush
make release                      # GPU build (or `make release-cpu` for CPU-only)

# Setup
./hush setup init                 # Interactive configuration setup
./hush models download base       # Download Whisper model (~145MB)
./hush setup uinput --quick       # Enable text insertion (then log out/in)

# Run
./hush listen                     # Hold Ctrl+Shift+Space to dictate
```

For other distributions, see [INSTALL.md](INSTALL.md).

## Features

- **Local Whisper transcription** with optional CUDA acceleration
- **Hardware-level text insertion** via UInput—works in VMs, SSH, games, secure contexts
- **Push-to-talk hotkey** with visual overlay feedback
- **Filler word removal** ("um", "uh", "like")
- **Voice commands**: "undo", "new paragraph", "new line"
- **Optional LLM polishing** via Claude API

## Configuration

Initialize your configuration interactively:

```bash
./hush setup init           # Guided setup (recommended)
./hush setup init --defaults  # Use defaults without prompts
```

Or copy the example and edit manually:

```bash
cp config/default.toml.example config/default.toml
# Edit config/default.toml with your preferences
```

### Key Settings

| Setting | Description | Default |
|---------|-------------|---------|
| `transcription.model_size` | Whisper model (tiny/base/small/medium/large) | base |
| `transcription.use_cuda` | Enable GPU acceleration | true |
| `hotkey.combination` | Push-to-talk key | Ctrl+Shift+Space |

## Usage

### Listen Mode

```bash
./hush listen
```

Hold `Ctrl+Shift+Space`, speak, release. Text appears at your cursor.

Options:
```bash
./hush listen --editing-mode aggressive  # Heavier text cleanup
./hush listen --no-processing            # Raw transcription only
```

### Quick Recording

```bash
./hush record --duration 10              # Record for 10 seconds
./hush record --duration 5 --print-only  # Print without inserting
```

### Voice Commands

| Command | Effect |
|---------|--------|
| "new paragraph" | Insert double newline |
| "new line" | Insert single newline |
| "undo" / "undo that" | Remove last insertion |
| "cap that" | Capitalize preceding word |

## Models

| Model | Size | GPU Speed | CPU Speed |
|-------|------|-----------|-----------|
| tiny | 75 MB | ~0.3s | ~2-3s |
| **base** | **145 MB** | **~0.5s** | **~4-6s** |
| small | 466 MB | ~0.8s | ~8-12s |
| medium | 1.5 GB | ~1.5s | ~15-25s |
| large | 2.9 GB | ~2.5s | ~30-45s |

Base model is recommended for most use cases.

```bash
./hush models download base
./hush models list
```

## LLM Integration (Optional)

For text polishing with Claude:

```bash
echo "ANTHROPIC_API_KEY=your_key" > .env
./hush listen
```

## Requirements

- ALSA development libraries
- For GPU: NVIDIA GPU with CUDA 12.0+

## Troubleshooting

**Text not inserting:**
```bash
./hush setup diagnose-uinput
./hush setup uinput --auto-fix
```

**Audio issues:**
```bash
./hush status --devices
./hush test audio --duration 3
```

**GPU not detected:**
```bash
./hush status --full
nvidia-smi  # Check CUDA
```

## Building

```bash
make release      # GPU-accelerated (CUDA)
make release-cpu  # CPU-only
cargo test        # Run tests
```

## Contributing

1. Fork the repo
2. Create a feature branch
3. Run `cargo test && cargo clippy && cargo fmt`
4. Open a PR

## License

MIT. See [LICENSE](LICENSE).

## Acknowledgments

- [OpenAI Whisper](https://github.com/openai/whisper)
- [candle](https://github.com/huggingface/candle)
- [whisper-rs](https://github.com/tazz4843/whisper-rs)
