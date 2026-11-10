# Speech models

Hush downloads models to `$XDG_DATA_HOME/hush/models` (usually
`~/.local/share/hush/models`), not to this directory. `hush settings` offers
them with a download button, and `hush models download <name>` does the same
from a terminal.

| Name | Download | Notes |
|------|----------|-------|
| `tiny` | 78 MB | Fastest, least accurate. Suggested on a CPU with less than 16 GB of memory |
| `base` | 147 MB | The default. Suggested on a CPU with 16 GB of memory or more |
| `small` | 488 MB | Noticeably better, still quick on a GPU. Suggested with a GPU and less than 16 GB of memory |
| `medium` | 1.5 GB | Suggested with a GPU and 16 GB of memory or more |
| `large` | 3.1 GB | The original large model |
| `large-v2` | 3.1 GB | |
| `large-v3` | 3.1 GB | Most accurate |
| `parakeet-v3` | 740 MB | Opt-in NVIDIA Parakeet TDT 0.6B v3 (CC BY 4.0, converted to GGUF and quantized to Q8_0): 25 European languages, no prompt |

The Speech page of `hush settings` names the one that suits your machine.
Larger models are more accurate and slower; a GPU changes the trade-off more
than anything else.

## Commands

```bash
hush models list              # what the catalogue offers and what is installed
hush models download base     # fetch one
hush doctor                   # which model is configured and whether it is there
```

`hush models download` also fetches the Silero voice activity model. When you
set `transcription.detect_languages`, it fetches the VoxLingua107 language-ID
model (27 MB) too. The main README lists the source and license of each model.

Hush checks each download against a SHA-256 checksum before it uses the file.
Set `model_path` under `[transcription]` to point at a file elsewhere.
