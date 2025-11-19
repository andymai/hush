# Hush Model Directory

---
**Last Updated**: 2025-11-19
**Status**: Active
**Purpose**: Information about Whisper models for voice-to-text transcription
**Related Documents**: [Main README](../README.md) | [Documentation Index](../DOCUMENTATION_INDEX.md)
---

This directory contains Whisper models for offline voice-to-text transcription.

## Available Models

| Model    | Size   | Use Case                          |
|----------|--------|-----------------------------------|
| `tiny`   | 39MB   | Fastest, lowest quality           |
| `base`   | 74MB   | Good balance for testing          |
| `small`  | 244MB  | Good quality, reasonable speed    |
| `medium` | 769MB  | Higher quality, slower            |
| `large`  | 1.55GB | Best quality, slowest             |
| `large-v3` | 1.55GB | Latest model with improvements  |

## Quick Start

### Download a model using the script:
```bash
./scripts/download-models.sh tiny
```

### Or use the simple model manager directly:
```bash
cargo run --bin simple-model-manager download tiny
```

### List available and cached models:
```bash
./scripts/download-models.sh --list
```

### Download all models (3.2GB total):
```bash
./scripts/download-models.sh --all
```

## Advanced Usage

### Check cache information:
```bash
./scripts/download-models.sh --info
# OR
cargo run --bin simple-model-manager info
```

### Clear model cache:
```bash
./scripts/download-models.sh --clear
# OR  
cargo run --bin simple-model-manager clear
```

### Use custom cache directory:
```bash
./scripts/download-models.sh --models-dir /path/to/models tiny
# OR
cargo run --bin simple-model-manager --cache-dir /path/to/models download tiny
```

## Model Storage

Models are downloaded from HuggingFace Hub and stored as:
- `model.safetensors` - Main model weights
- `config.json` - Model configuration (if available)
- `tokenizer.json` - Tokenizer configuration (if available)

The models are verified by file size during download and caching.

## Integration

The Hush application will automatically detect and use models in this directory. If no model is found, it will run in simulation mode for development and testing.

To use real transcription:
1. Download at least one model using the methods above
2. The application will automatically use the available model
3. For best results, use `small` or larger models

## Troubleshooting

**Model download fails:**
- Check internet connection
- Verify HuggingFace Hub access
- Try a different model size

**Models not detected:**
- Check file permissions in the models directory
- Verify model files are complete (check with `--list`)
- Ensure models directory is accessible to the application

**Performance issues:**
- Use smaller models (tiny/base) for faster inference
- Enable CUDA acceleration if available
- Consider available RAM when choosing model size