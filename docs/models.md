# Models

| Model | Download |
|---|---|
| tiny | 78 MB |
| **base** (default) | **147 MB** |
| small | 488 MB |
| medium | 1.5 GB |
| large, large-v2, large-v3 | 3.1 GB each |
| large-v3-turbo | 874 MB |
| parakeet-v3 (opt-in) | 740 MB |

Larger models are more accurate and slower. `large-v3-turbo` is large-v3 with a smaller decoder: it is close to large-v3 in accuracy and much faster, and Hush downloads its 8-bit file. Manage models with `hush models list`, `hush models download <size>`, and `hush models set <size>`. Hush downloads the Whisper models from the whisper.cpp catalogue and checks each download against a SHA-256 checksum.

`hush tune` times base on your GPU and your CPU, each in a process of its own. It recommends the most accurate model whose text comes within about one second after a 10-second recording. It times on the take that the setup kept, else on a public-domain clip, and it frees the daemon's model meanwhile. The speeds of the larger models are estimates until `hush tune --apply` times the pick. The Tune button on the Speech page runs `hush tune` and shows its result.

Without a tune, the Speech page names medium for a GPU with 16 GB of RAM or more, and small for less. On a CPU, it names base with 16 GB or more, and tiny with less. A tune applies to one GPU and driver, and Hush tells you at start to tune again when they change.

`parakeet-v3` is NVIDIA's Parakeet TDT 0.6B v3, an opt-in model for 25 European languages. The languages are Bulgarian, Croatian, Czech, Danish, Dutch, English, Estonian, Finnish, French, German, Greek, and Hungarian. They also include Italian, Latvian, Lithuanian, Maltese, Polish, Portuguese, Romanian, Russian, Slovak, Slovenian, Spanish, Swedish, and Ukrainian. Turn it on with `hush models set parakeet-v3`. It tells these languages apart on its own, so `transcription.language` and `detect_languages` do not apply to it. `hush doctor` warns when `language` names a language it lacks.

It reads no prompt. The window title, your learned terms, and the text before each part of a hands-free recording do not prime it. Your vocabulary replacements, styles, Command Mode, and the silence guards still apply to its text. It hears a take of more than 30 seconds in pieces, cut in the pauses between speech. On a GPU, its decoder still runs on up to 8 CPU threads.

Hush also downloads two small helper models. The voice activity model comes with `hush models download`. The language-ID model comes only when you set `transcription.detect_languages`.

| Model | Source | License |
|---|---|---|
| Whisper (OpenAI) | ggml files from [ggerganov/whisper.cpp](https://huggingface.co/ggerganov/whisper.cpp) | MIT |
| Silero VAD v6.2 | [ggml-org/whisper-vad](https://huggingface.co/ggml-org/whisper-vad), from [snakers4/silero-vad](https://github.com/snakers4/silero-vad) | MIT |
| Parakeet TDT 0.6B v3 (NVIDIA, opt-in) | [collectivecontext/hush-models](https://huggingface.co/collectivecontext/hush-models), converted to GGUF and quantized to Q8_0 with transcribe.cpp from [nvidia/parakeet-tdt-0.6b-v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3) | CC BY 4.0 |
| VoxLingua107 ECAPA-TDNN language ID | [collectivecontext/hush-models](https://huggingface.co/collectivecontext/hush-models), converted with transcribe.cpp from [speechbrain/lang-id-voxlingua107-ecapa](https://huggingface.co/speechbrain/lang-id-voxlingua107-ecapa) | Apache-2.0 |
