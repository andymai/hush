# Whisper models

Hush downloads models to `$XDG_DATA_HOME/hush/models` (usually
`~/.local/share/hush/models`), not to this directory. `hush settings` offers
them with a download button, and `hush models download <name>` does the same
from a terminal.

| Name | Download | Notes |
|------|----------|-------|
| `tiny` | 78 MB | Fastest, least accurate |
| `base` | 147 MB | A good start on a CPU |
| `small` | 488 MB | Noticeably better, still quick on a GPU |
| `medium` | 1.5 GB | Suggested with a GPU and 16 GB of memory |
| `large` | 3.1 GB | The original large model |
| `large-v2` | 3.1 GB | |
| `large-v3` | 3.1 GB | Most accurate |

`hush doctor` names the one that suits your machine, and the setup pane marks
it. Larger models are more accurate and slower; a GPU changes the trade-off
more than anything else.

## Commands

```bash
hush models list              # what the catalogue offers and what is installed
hush models download base     # fetch one
hush doctor                   # which model is configured and whether it is there
```

Each download is checked against a SHA256 checksum before it is used. Set
`model_path` under `[transcription]` to point at a file elsewhere.
