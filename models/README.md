# Whisper Models

This directory contains Whisper models for offline voice-to-text transcription.

## Available Models

| Model    | Size   | Use Case                          |
|----------|--------|-----------------------------------|
| `tiny`   | 39MB   | Fastest, lowest quality           |
| `base`   | 74MB   | Good balance (recommended)        |
| `small`  | 244MB  | Good quality, reasonable speed    |
| `medium` | 769MB  | Higher quality, slower            |
| `large`  | 1.55GB | Best quality, slowest             |

## Quick Start

Download a model:
```bash
./hush models download base
```

List available models:
```bash
./hush models list
```

## Troubleshooting

**Model download fails:**
- Check internet connection
- Try a different model size

**Models not detected:**
- Check file permissions in the models directory
- Verify model files are complete with `./hush models list`

**Performance issues:**
- Use smaller models (tiny/base) for faster inference
- Enable CUDA acceleration if available
