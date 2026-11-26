# Privacy and security

Everything runs locally by default. The network carries the one-time model download and the text for your LLM, never audio. It also carries the update check if you turn it on: one request a day to GitHub for the newest release number. Hush uses a local Ollama server when one answers, and Anthropic only when `ANTHROPIC_API_KEY` exists. It uses an OpenAI-compatible server only when `llm.provider` names it.

With `transcription.backend` set to `groq` or `openai`, Hush sends the audio of each dictation to that service. It adds the title of the focused window when `transcription.context_prompt` is on. With `openai-compatible`, Hush sends the same data to the URL in `transcription.server_url`. `hush doctor` and `hush status` show whether that address is a local server. When that service fails, Hush reports the error and does not try another service.

Hush writes audio to disk only when you set `history.keep_audio_days`, while a hands-free recording transcribes in parts, and when a transcription fails. Hush keeps the parts only until it types the text, and it keeps the audio of a failed take for 7 days, for `hush recover`. When the hardware tune asks for it, Hush also keeps the next take in its runtime folder, until the next take, a daemon restart, or 10 minutes. Only you can read those files.

`hush report` writes a file for a bug report on your computer and uploads nothing. It holds the version, the checks, your settings, and the last 500 log lines at INFO and above, which hold no dictated text. It replaces your home paths, host name, user name, and API keys, and it removes style prompts, hook commands, and server addresses. Read the file before you attach it to an issue.

Hush reads keyboard events and types through `/dev/input` and `/dev/uinput`. One udev rule grants the logged-in user access to those for the length of the session. Details in [SECURITY.md](../SECURITY.md).
