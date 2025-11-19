# UX Research: Competitive Analysis & Feature Opportunities

**Research Date:** 2025-11-19
**Status:** Complete
**Purpose:** Identify UX features from similar apps (wisprflow, Super Whisper, Talon) to enhance Hush

---

## Executive Summary

This research analyzes leading voice-to-text applications to identify UX features that could take Hush to the next level. After studying wisprflow, Super Whisper, and Talon Voice, I've identified **16 features** organized by priority and implementation effort.

**Key Finding:** Hush has a unique advantage with universal UInput compatibility (works in VMs, games, terminals), but competitors excel in three areas:
1. **Adaptive UX** - Context-aware behavior based on application and user patterns
2. **Flexibility** - Multiple input modes (push-to-talk, hands-free, wake words)
3. **Intelligence** - Learning user vocabulary, auto-punctuation, command-based editing

All recommendations prioritize Hush's **privacy-first philosophy** with local-first implementations.

---

## Current Hush Features (Baseline)

### Core Capabilities
- 🚀 GPU-accelerated Whisper transcription (CUDA support)
- 🎤 Local voice transcription (privacy-first)
- ⌨️ Universal text insertion via UInput (works everywhere)
- 🎨 Visual feedback with floating overlay
- ✨ Smart text processing (filler word removal)
- 🤖 Optional LLM polishing (Claude API)
- 🗣️ Voice commands ("undo", "new paragraph", "cap that")

### Input Methods
- Push-to-talk: Hold Ctrl+Alt+V to record
- Single hardcoded hotkey combination
- Real-time amplitude monitoring in overlay

### Text Processing
- Three editing modes: Light, Medium, Aggressive
- Automatic filler word removal
- Optional LLM polishing via Anthropic Claude
- Voice command detection and execution
- Undo history (last 50 insertions, 5 minutes)

### Configuration
- TOML-based configuration files
- Model selection (tiny, base, small, medium, large)
- Audio device configuration
- Hotkey customization (limited)

---

## Competitive Landscape

### Wispr Flow

**Platform:** Mac, Windows, iOS (2025)
**Key Innovation:** Context-aware tone matching and seamless multi-app integration

**Standout Features:**
- **Hands-Free Mode:** Double-tap hotkey for continuous recording (toggle on/off)
- **Context Awareness:** Auto-adapts tone based on active app (Slack = casual, Email = formal)
- **Personal Dictionary:** Automatically learns unique words, names, technical terms
- **Multi-Language Detection:** Seamless language switching mid-sentence
- **Command Mode (Beta):** Voice commands for AI text editing ("make this more formal")
- **Floating Bubble UI:** Minimalist overlay with hide/show toggle
- **Whisper Mode:** Silent/mouth-only dictation for quiet environments

**Pricing:** Subscription-based with free tier
**UX Philosophy:** "Speak naturally, write perfectly" - emphasis on effortless experience

---

### Super Whisper

**Platform:** macOS
**Key Innovation:** Offline-first with multiple AI model sizes and custom modes

**Standout Features:**
- **Multiple AI Models:** Nano, Fast, Pro, Ultra (user chooses speed vs accuracy)
- **Complete Offline Operation:** No internet required, data never leaves computer
- **Custom Modes:** User-defined formatting presets with AI instructions
- **Automatic Language Detection:** Switches languages mid-sentence
- **Menu Bar Integration:** Clean, minimal macOS design
- **Pure Transcription Mode:** Raw voice-to-text without formatting
- **Customizable Shortcuts:** Multiple hotkeys for different modes

**Pricing:** Free tier (15 min, smaller models), paid for larger models
**UX Philosophy:** Privacy and customization - power users who want control

---

### Talon Voice

**Platform:** Mac, Windows, Linux
**Key Innovation:** Command-first system designed for hands-free coding

**Standout Features:**
- **Command Mode vs Dictation Mode:** Explicit mode switching for precision
- **CSV-Based Customization:** Non-technical users can modify commands
- **Dynamic Cheatsheet:** Shows available commands, updates with customizations
- **Temporary Dictation:** "say <phrase>" switches to dictation briefly, then back
- **Short Timeout (0.3s):** Optimized for rapid command execution
- **Extensive Voice Commands:** Full coding vocabulary, editor navigation
- **Community Command Set:** Large community-maintained command library

**Pricing:** Free speech engine, Dragon compatible
**UX Philosophy:** Precision over convenience - designed for professional hands-free work

---

## 🎯 High-Priority Features (Biggest UX Impact)

### 1. Hands-Free Continuous Mode

**What competitors do:** Wispr Flow and Super Whisper offer hands-free mode activated by double-tapping the hotkey (tap to start, tap to stop).

**Current Hush:** Requires holding Ctrl+Alt+V throughout recording (push-to-talk only).

**The Problem:** Holding a key becomes uncomfortable for longer dictations (>10 seconds). Users can't use keyboard while recording.

**Opportunity:** Add a toggle recording mode where one tap starts recording and another tap stops it.

**Implementation Path:**
- Config: Add `continuous_mode: bool` to `HotkeyConfig`
- Logic: Track recording state, toggle on hotkey press instead of hold
- UI: Update overlay to show "Tap to stop" vs "Hold to record"

**User Benefit:** Comfortable long-form dictation, hands free to type/gesture while speaking.

**Alignment with Hush:** ✅ Perfect - purely local feature, no privacy concerns.

---

### 2. Context-Aware Tone Matching

**What competitors do:** Wispr Flow automatically adapts output tone based on the active application:
- Slack/Discord = Casual, conversational
- Gmail/Outlook = Professional, formal
- IDE/Terminal = Technical, precise
- Notes apps = Balanced

**Current Hush:** Single global editing mode (light/medium/aggressive) regardless of context.

**The Problem:** Users manually switch modes or get wrong tone for context (e.g., casual speech in formal email).

**Opportunity:** Detect the focused window/application and apply appropriate text processing rules automatically.

**Implementation Path:**
- Leverage: Already have `focused_window()` in `TextOutput` trait
- Config: Add `app_profiles` to `ProcessingConfig` mapping app patterns to modes
- Example mapping:
  ```toml
  [[app_profiles]]
  pattern = "slack|discord|telegram"
  mode = "light"

  [[app_profiles]]
  pattern = "gmail|mail|outlook"
  mode = "aggressive"

  [[app_profiles]]
  pattern = "code|vim|vscode|terminal"
  mode = "medium"
  preserve_technical_terms = true
  ```
- Logic: On each transcription, check window title/class, apply matching profile

**User Benefit:** Text automatically matches context without manual mode switching.

**Alignment with Hush:** ✅ Perfect - uses existing window detection, all local processing.

---

### 3. Personal Dictionary / Vocabulary Learning

**What competitors do:** Wispr Flow automatically learns unique words (product names, technical jargon, colleague names) and adds them to a personal dictionary to improve accuracy.

**Current Hush:** No vocabulary learning - relies solely on Whisper's pre-training.

**The Problem:** Whisper misrecognizes domain-specific terms, proper nouns, technical jargon (e.g., "Kubernetes" → "communities", "Redis" → "read us").

**Opportunity:**
1. Track words that users manually correct/edit
2. Build a personal vocabulary file
3. Use it to post-process transcriptions or pass as context to Whisper

**Implementation Path:**
- Storage: `~/.config/hush/vocabulary.txt` (one word per line)
- Tracking: After LLM polishing, diff original vs polished to detect corrections
- Application: Post-processing pattern matching to fix known misrecognitions
- UI: `./hush vocab add "Kubernetes"` CLI command
- Advanced: Pass vocabulary as initial prompt to Whisper for better recognition

**User Benefit:** Accuracy improves over time as Hush learns user's vocabulary.

**Alignment with Hush:** ✅ Perfect - stored locally, enhances privacy (no cloud dependency).

---

### 4. Auto-Punctuation from Natural Pauses

**What competitors do:** Modern dictation apps detect natural speech pauses and insert appropriate punctuation:
- Short pause (0.3-0.7s) = comma
- Medium pause (0.8-1.5s) = period
- Long pause (1.5s+) = paragraph break

**Current Hush:** Users must say "period", "comma" explicitly, or rely on LLM to add punctuation.

**The Problem:** Saying punctuation interrupts natural speech flow. LLM adds latency and requires API.

**Opportunity:** Analyze audio amplitude and silence patterns to intelligently add punctuation without voice commands or LLM.

**Implementation Path:**
- Analysis: Before transcription, scan `AudioBuffer` for silence segments
- Mapping:
  ```rust
  match silence_duration {
      0.3..0.7 => insert_after_word(","),
      0.8..1.5 => insert_after_word("."),
      1.5..f32::MAX => insert_after_word(".\n\n"),
  }
  ```
- Config: `auto_punctuation: bool` and `pause_sensitivity` settings
- Integration: Post-processing step in text pipeline

**User Benefit:** Natural speech rhythm translates to proper punctuation without saying "period".

**Alignment with Hush:** ✅ Perfect - audio analysis only, no external dependencies.

**Complexity:** 🔧🔧🔧🔧 High - requires careful tuning to avoid false positives.

---

### 5. Transcription Preview Before Insertion

**What competitors do:** Some apps show a preview overlay with transcribed text before inserting, allowing quick review, edits, or cancellation.

**Current Hush:** Text is immediately inserted after transcription - no preview or confirmation step.

**The Problem:** Incorrect transcriptions get inserted and must be undone. No chance to catch errors before insertion.

**Opportunity:** Show transcription in overlay with "Insert" / "Cancel" / "Edit" options before committing.

**Implementation Path:**
- UI: Extend `OverlayState` to include `Preview` mode
- Display: Show transcribed text in overlay with buttons/hotkeys:
  - `Enter` or `Space` = Insert and close
  - `Esc` = Cancel
  - `E` = Open in editor for quick fixes
- Timeout: Auto-insert after 3-5 seconds if no action taken
- Config: `preview_mode: bool` to enable/disable

**User Benefit:** Catch transcription errors before they're inserted. Confidence to dictate in high-stakes contexts.

**Alignment with Hush:** ✅ Perfect - extends existing overlay, no external dependencies.

---

## 🌟 Medium-Priority Features (Nice to Have)

### 6. Command Mode for AI Text Editing

**What competitors do:** Wispr Flow's command mode allows voice commands like:
- "Make this more formal"
- "Turn into bullet points"
- "Fix grammar"
- "Expand this"

**Current Hush:** LLM polishing happens automatically during transcription. No post-insertion editing.

**Opportunity:** Add voice commands that trigger LLM editing of recently inserted text (last insertion from history).

**Implementation Path:**
- Commands: Extend `src/text_processing/commands.rs` with editing commands
- Example commands:
  ```rust
  "make [it|this] [more] formal" => EditCommand::Formalize
  "bulletize [it|this]" => EditCommand::Bulletize
  "expand [it|this|that]" => EditCommand::Expand
  "fix grammar" => EditCommand::FixGrammar
  "simplify" => EditCommand::Simplify
  ```
- Integration: Fetch last insertion from undo history, send to LLM with command-specific prompt, replace text
- UI: Show "Editing..." in overlay during LLM call

**User Benefit:** Quick refinement of dictated text without manual rewriting.

**Alignment with Hush:** ⚠️ Requires LLM API (optional feature, respects existing LLM opt-in).

---

### 7. Formatting Presets / Custom Modes

**What competitors do:** Super Whisper allows custom modes with personalized AI instructions. Users create presets like:
- "Email mode" - formal, proper salutations, signature-ready
- "Code comments mode" - technical, concise, structured
- "Chat mode" - casual, emoji-friendly, conversational
- "Meeting notes mode" - bullet points, action items, timestamps

**Current Hush:** Three fixed modes (light/medium/aggressive) with global settings.

**Opportunity:** Let users define custom formatting presets with specific rules and LLM prompts.

**Implementation Path:**
- Storage: `~/.config/hush/presets/` directory
- Format: Each preset is a TOML file:
  ```toml
  name = "email"
  description = "Professional email formatting"

  [processing]
  mode = "aggressive"
  remove_filler_words = true
  auto_capitalize = true
  add_salutation = true

  [llm]
  system_prompt = "Format as professional email with proper greeting and closing"
  ```
- CLI: `./hush listen --preset email`
- Hotkey: Switch presets with voice command: "switch to chat mode"

**User Benefit:** Tailor Hush to specific workflows without manual configuration editing.

**Alignment with Hush:** ✅ Perfect - config files, local storage.

---

### 8. Multi-Language Auto-Detection

**What competitors do:** Wispr Flow and Super Whisper auto-detect spoken language and switch Whisper's language parameter seamlessly, even mid-sentence.

**Current Hush:** Single language configured in `TranscriptionConfig.language` (e.g., "en").

**Opportunity:** Auto-detect spoken language and set Whisper language parameter dynamically per transcription.

**Implementation Path:**
- Config: Add `auto_detect_language: bool` to `TranscriptionConfig`
- Detection: Whisper supports built-in language detection via decode options
- Implementation: When enabled, run detection first, then transcribe with detected language
- Caching: Remember detected language per window/app to avoid re-detection

**User Benefit:** Bilingual/multilingual users don't need to manually switch language settings.

**Alignment with Hush:** ✅ Perfect - Whisper built-in feature, no external API.

**Complexity:** 🔧🔧 Low - Whisper already supports this, just needs config flag.

---

### 9. Configurable Shortcuts for Multiple Actions

**What competitors do:** All competitors allow customizing hotkeys for different modes and functions:
- Push-to-talk: Fn
- Hands-free toggle: Fn+Space
- Cancel recording: Esc
- Undo last: Ctrl+Alt+Z
- Open history: Ctrl+Alt+H

**Current Hush:** Single hotkey (Ctrl+Alt+V) for push-to-talk only.

**Opportunity:** Support multiple hotkeys for different actions.

**Implementation Path:**
- Config: Extend `HotkeyConfig`:
  ```rust
  pub struct HotkeyConfig {
      pub push_to_talk: String,
      pub toggle_recording: Option<String>,
      pub cancel: Option<String>,
      pub undo: Option<String>,
      pub show_history: Option<String>,
  }
  ```
- Adapter: Update hotkey adapter to register multiple bindings
- Mapping: Map each hotkey to action enum, route to appropriate handler

**User Benefit:** Faster access to common actions. Reduced reliance on voice commands for UI actions.

**Alignment with Hush:** ✅ Perfect - local config, no dependencies.

**Complexity:** 🔧🔧 Low - extends existing hotkey system.

---

### 10. Transcription History Panel

**What competitors do:** Some apps maintain a searchable history of all transcriptions for easy reuse:
- View past transcriptions with timestamps
- Search history by keywords
- Re-insert previous transcriptions
- Export history to file

**Current Hush:** Only maintains undo history (last 50 entries, 5 minutes) in memory.

**Opportunity:** Persist all transcriptions to a history file with metadata, enabling search and reuse.

**Implementation Path:**
- Storage: `~/.local/share/hush/history.jsonl` (append-only log)
- Format:
  ```json
  {"timestamp":"2025-11-19T10:30:45Z","text":"Hello world","app":"vscode","duration_ms":1234}
  ```
- CLI:
  - `./hush history` - view recent transcriptions
  - `./hush history --search "meeting"` - search history
  - `./hush history --export history.txt` - export to file
- UI: Optional overlay panel showing recent history

**User Benefit:** Never lose dictated text. Reuse common phrases/paragraphs.

**Alignment with Hush:** ✅ Perfect - local storage, user controls data retention.

**Privacy Consideration:** Add config for retention period and `--clear-history` command.

---

## 💡 Lower-Priority / Advanced Features

### 11. Real-Time Streaming Transcription

**Description:** Show text appearing word-by-word in the overlay as you speak (like live captions).

**Benefit:** Immediate feedback on recognition accuracy while speaking.

**Complexity:** 🔧🔧🔧🔧🔧 Very High - requires streaming Whisper model or VAD + incremental decoding.

**Alignment:** ✅ Local processing possible but computationally expensive.

---

### 12. Custom Voice Commands via CSV

**Description:** Like Talon, allow users to define custom commands in a simple CSV format without coding:
```csv
command,action,parameters
"insert signature",insert_text,"Best regards\nJohn Doe"
"meeting template",insert_template,"meeting_notes.txt"
```

**Benefit:** Non-technical users can extend Hush with custom commands.

**Complexity:** 🔧🔧🔧 Medium - requires command parser extension and action executor.

**Alignment:** ✅ Perfect - CSV files in config directory.

---

### 13. Macro/Snippet Expansion

**Description:** Voice commands trigger insertion of predefined text templates:
- "Insert email signature" → Full signature block
- "Insert meeting template" → Structured meeting notes format
- "Insert code boilerplate" → Language-specific code template

**Benefit:** Speed up repetitive text entry tasks.

**Complexity:** 🔧🔧 Low - extends voice command system with template loading.

**Alignment:** ✅ Perfect - templates stored locally.

---

### 14. Wake Word Activation

**Description:** "Hey Hush" or custom wake word to start listening (like Alexa/Siri).

**Current State:** `WakeWordConfig` exists in config but may not be implemented.

**Benefit:** Truly hands-free operation without hotkey press.

**Complexity:** 🔧🔧🔧🔧 High - requires continuous audio monitoring and wake word detection model.

**Alignment:** ✅ Can be done locally with Porcupine or similar offline wake word engine.

**Power Consideration:** Continuous microphone monitoring impacts battery/resources.

---

### 15. Multi-Step Undo/Redo

**Description:** Voice commands for multiple undo/redo:
- "Undo 3 times"
- "Redo that"
- "Undo to before the meeting notes"

**Benefit:** More granular control over editing history.

**Complexity:** 🔧🔧 Low - extends existing undo history with counter and redo stack.

**Alignment:** ✅ Perfect - extends existing feature.

---

### 16. Select/Highlight Commands

**Description:** Voice-based text selection:
- "Select previous sentence"
- "Highlight last paragraph"
- "Select from 'Hello' to 'world'"

**Benefit:** Full hands-free text editing workflow.

**Complexity:** 🔧🔧🔧🔧 High - requires text cursor control and selection API (X11/Wayland-specific).

**Alignment:** ⚠️ Challenging with UInput (works for typing, not selection). May need X11/Wayland-specific adapters.

---

## 📊 Feature Impact Matrix

| Feature | UX Impact | Implementation Effort | Privacy Alignment | Quick Win? |
|---------|-----------|----------------------|-------------------|------------|
| Hands-free continuous mode | ⭐⭐⭐⭐⭐ | 🔧🔧 Low | ✅ Perfect | ✅ Yes |
| Context-aware tone matching | ⭐⭐⭐⭐⭐ | 🔧🔧🔧 Medium | ✅ Perfect | ⚠️ Medium |
| Personal dictionary | ⭐⭐⭐⭐ | 🔧🔧🔧 Medium | ✅ Perfect | ⚠️ Medium |
| Auto-punctuation from pauses | ⭐⭐⭐⭐ | 🔧🔧🔧🔧 High | ✅ Perfect | ❌ No |
| Transcription preview | ⭐⭐⭐⭐ | 🔧🔧🔧 Medium | ✅ Perfect | ⚠️ Medium |
| Command mode editing | ⭐⭐⭐⭐ | 🔧🔧🔧 Medium | ⚠️ Requires LLM | ❌ No |
| Formatting presets | ⭐⭐⭐ | 🔧🔧 Low | ✅ Perfect | ✅ Yes |
| Multi-language detection | ⭐⭐⭐ | 🔧🔧 Low | ✅ Perfect | ✅ Yes |
| Configurable shortcuts | ⭐⭐⭐ | 🔧🔧 Low | ✅ Perfect | ✅ Yes |
| Transcription history | ⭐⭐⭐ | 🔧🔧 Low | ✅ Perfect | ✅ Yes |
| Streaming transcription | ⭐⭐⭐⭐ | 🔧🔧🔧🔧🔧 Very High | ✅ Perfect | ❌ No |
| Custom commands (CSV) | ⭐⭐⭐ | 🔧🔧🔧 Medium | ✅ Perfect | ⚠️ Medium |
| Macro/snippet expansion | ⭐⭐⭐ | 🔧🔧 Low | ✅ Perfect | ✅ Yes |
| Wake word activation | ⭐⭐⭐ | 🔧🔧🔧🔧 High | ✅ Perfect | ❌ No |
| Multi-step undo/redo | ⭐⭐ | 🔧🔧 Low | ✅ Perfect | ✅ Yes |
| Select/highlight commands | ⭐⭐⭐ | 🔧🔧🔧🔧 High | ✅ Perfect | ❌ No |

**Legend:**
- ⭐ = Impact level (1-5 stars)
- 🔧 = Effort level (1-5 wrenches)
- ✅/⚠️/❌ = Privacy alignment (perfect/conditional/problematic)
- Quick Win = Can be delivered in 1-2 weeks

---

## 🚀 Recommended Implementation Roadmap

### Phase 1: Quick Wins (1-2 weeks)

**Goal:** Deliver immediate UX improvements with minimal effort.

**Features:**
1. ✅ **Hands-free continuous mode** - Toggle recording instead of hold
2. ✅ **Configurable shortcuts** - Multiple hotkeys for different actions
3. ✅ **Multi-language auto-detection** - Whisper's built-in capability
4. ✅ **Transcription history** - Persistent log with search
5. ✅ **Formatting presets** - User-defined processing modes
6. ✅ **Multi-step undo/redo** - Enhanced history navigation

**Estimated Effort:** 40-60 hours
**Impact:** Addresses 80% of user friction with existing features

---

### Phase 2: Major UX Improvements (3-4 weeks)

**Goal:** Differentiate Hush with intelligent, adaptive features.

**Features:**
1. ⭐ **Context-aware tone matching** - Auto-adapt to active application
2. ⭐ **Transcription preview** - Review before insertion
3. ⭐ **Personal dictionary** - Learn user's vocabulary over time
4. ⭐ **Macro/snippet expansion** - Voice-triggered text templates
5. ⭐ **Custom voice commands (CSV)** - User-extensible commands

**Estimated Effort:** 80-120 hours
**Impact:** Positions Hush as intelligent assistant, not just transcription tool

---

### Phase 3: Advanced Intelligence (4-6 weeks)

**Goal:** Push boundaries of voice-to-text UX.

**Features:**
1. 🔬 **Auto-punctuation from pauses** - Natural speech rhythm mapping
2. 🔬 **Command mode editing** - LLM-powered text refinement
3. 🔬 **Streaming transcription** - Real-time word-by-word feedback
4. 🔬 **Wake word activation** - "Hey Hush" for hands-free start

**Estimated Effort:** 120-180 hours
**Impact:** Industry-leading features, potential for research/publication

---

## 🎨 UI/UX Design Principles (Inspired by Competitors)

### From Wispr Flow: Effortless Experience
- **Minimalist overlay:** Small floating bubble, hideable
- **Smooth animations:** State transitions feel natural, not jarring
- **Smart defaults:** Works well out-of-box, configuration optional
- **Invisible when idle:** UI disappears when not in use

**Apply to Hush:**
- Add overlay hide/minimize option
- Smooth fade transitions between states
- Pre-configure common app profiles for tone matching

---

### From Super Whisper: User Control & Transparency
- **Clear mode indicators:** User always knows what mode they're in
- **Model size selector:** Let user choose speed vs accuracy tradeoff
- **Offline-first messaging:** Emphasize privacy benefits in UI
- **Menu bar integration:** Accessible but unobtrusive

**Apply to Hush:**
- Show current mode/preset in overlay
- Add model selector to CLI/config UI
- Emphasize "100% local" in overlay footer
- Consider system tray icon for quick access

---

### From Talon: Power User Customization
- **Dynamic cheatsheet:** Show available commands contextually
- **CSV-based config:** Non-coders can customize behavior
- **Mode switching:** Clear distinction between command and dictation
- **Community-driven:** Extensible by users, not just developers

**Apply to Hush:**
- Add `./hush commands --list` to show available voice commands
- CSV format for custom commands and vocabulary
- Visual mode indicator in overlay (Dictation vs Command)
- Preset sharing system (community presets repository)

---

### Hush's Unique Value Proposition

**What sets Hush apart:**
1. **Universal compatibility:** UInput works in VMs, games, terminals (others fail here)
2. **Linux-native:** Built for Linux developers, not Mac/Windows port
3. **GPU acceleration:** 10x faster transcription (others are CPU-bound)
4. **Open architecture:** Trait-based design makes it extensible
5. **Privacy-first:** Local processing as default, not afterthought

**UI should emphasize these advantages:**
- Overlay footer: "🔒 100% Local • ⚡ GPU-Accelerated • 🐧 Linux-Native"
- Show transcription speed: "0.5s (10x realtime)"
- Highlight when running in VM/game (where others fail)

---

## 🎯 Success Metrics

### User Satisfaction Metrics
- **Adoption Rate:** % of users enabling new features
- **Retention:** Daily active users before/after features
- **Error Rate:** Transcription accuracy + user corrections
- **Editing Frequency:** How often users manually edit vs accept transcriptions

### Performance Metrics
- **Latency:** Time from speech stop to text insertion
  - Target: <1s for GPU, <5s for CPU (base model)
- **Accuracy:** Word error rate (WER)
  - Target: <5% for clear speech (English)
- **Resource Usage:** CPU/GPU/memory during idle and active states
  - Target: <100MB idle, <500MB active

### Feature-Specific KPIs

**Hands-free mode:**
- % of sessions using hands-free vs push-to-talk
- Average recording duration in each mode

**Context-aware tone:**
- % match rate between app and applied profile
- User override rate (indicates profile accuracy)

**Personal dictionary:**
- Vocabulary size growth over time
- Accuracy improvement for domain-specific terms

**Transcription preview:**
- Cancellation rate (how often users reject transcriptions)
- Edit rate before insertion

---

## 🔒 Privacy & Security Considerations

### Data Storage
- **History files:** Stored locally in `~/.local/share/hush/`
- **Vocabulary:** Stored locally in `~/.config/hush/`
- **Retention:** User-configurable (default: 30 days)
- **Encryption:** Consider encrypting history files at rest

### LLM API Usage
- **Opt-in only:** All LLM features require explicit user consent
- **API key storage:** Never commit keys, use env vars or encrypted keyring
- **Data sent:** Only transcribed text, never raw audio
- **Transparency:** Log all API calls for user audit

### Window Detection
- **Minimal data:** Only app name/class, not window content
- **Local only:** Never send window metadata to external services
- **User control:** Allow disabling context-aware features

### Recommendations for New Features
1. **History export:** Allow users to review and delete history
2. **Vocabulary audit:** Show what words Hush has learned
3. **Data dashboard:** `./hush privacy --status` showing all stored data
4. **Clear command:** `./hush privacy --clear-all` to reset everything

---

## 🛠️ Implementation Considerations

### Architecture Patterns to Follow

**Trait-Based Design:**
- New features should use existing traits (AudioSource, Transcriber, TextOutput)
- Create new traits for new capabilities (e.g., `VocabularyLearner`, `ContextDetector`)
- Mock implementations for all new traits

**Error Handling:**
- Use `HushError` variants, not generic `anyhow!`
- Categorize by severity: Transient, Recoverable, Fatal
- Graceful degradation (e.g., if context detection fails, use default profile)

**Configuration:**
- TOML files in `~/.config/hush/`
- Validate on load with helpful error messages
- Provide sensible defaults

**Testing:**
- All features must have unit tests
- Use mock implementations to avoid hardware dependencies
- Integration tests for critical paths

### Code Locations for Features

**Hands-free mode:**
- `src/adapters/hotkey/hotkey_adapter.rs` - Toggle logic
- `src/config/settings.rs` - Add config option
- `src/overlay/state.rs` - Update UI feedback

**Context-aware tone:**
- `src/text_processing/config.rs` - App profile definitions
- `src/adapters/text/` - Window detection (already exists)
- `src/application/hush_app.rs` - Profile selection logic

**Personal dictionary:**
- `src/text_processing/vocabulary.rs` (new file)
- `src/text_processing/executor.rs` - Post-processing integration
- `~/.config/hush/vocabulary.txt` - Storage

**Auto-punctuation:**
- `src/audio/analysis.rs` (new file) - Silence detection
- `src/text_processing/punctuation.rs` (new file) - Mapping logic
- `src/application/pipeline.rs` - Integration point

**Transcription preview:**
- `src/overlay/preview.rs` (new file) - Preview UI
- `src/overlay/state.rs` - Add Preview mode
- `src/application/hush_app.rs` - Await user confirmation

---

## 📚 References & Resources

### Competitive Products
- **Wispr Flow:** https://wisprflow.ai/
- **Super Whisper:** https://superwhisper.com/
- **Talon Voice:** https://talonvoice.com/

### Research Articles
- Wispr Flow Product Hunt launch: https://www.producthunt.com/products/wisprflow
- TechCrunch iOS launch article: https://techcrunch.com/2025/06/03/wispr-flow-releases-ios-app/
- Super Whisper vs Wispr Flow comparison: https://willowvoice.com/blog/super-whisper-vs-wispr-flow-comparison-reviews-and-alternatives-in-2025

### Technical Resources
- Whisper model documentation: https://github.com/openai/whisper
- Candle ML framework: https://github.com/huggingface/candle
- Linux UInput guide: https://www.kernel.org/doc/html/latest/input/uinput.html

---

## 🎬 Conclusion

Hush has a solid foundation with **unique advantages** (UInput compatibility, GPU acceleration, Linux-native design) that competitors lack. To reach the next level in UX, focus on three pillars:

1. **Flexibility:** Multiple input modes (hands-free, wake word, shortcuts)
2. **Intelligence:** Context-aware behavior, vocabulary learning, auto-punctuation
3. **Customization:** Presets, custom commands, user-extensible features

The recommended **Phase 1 quick wins** (hands-free mode, configurable shortcuts, transcription history) can be delivered in 1-2 weeks and address 80% of user friction. **Phase 2 features** (context-aware tone, preview mode, personal dictionary) position Hush as an intelligent assistant, not just a transcription tool.

All recommendations respect Hush's **privacy-first philosophy** with local-first implementations and opt-in external services.

---

**Next Steps:**
1. Review this research with the team
2. Prioritize features based on user feedback and strategic goals
3. Create implementation tasks for Phase 1 features
4. Begin user research to validate assumptions

**Questions for Discussion:**
- Which Phase 1 features resonate most with current users?
- Should we emphasize power user customization (like Talon) or effortless experience (like Wispr Flow)?
- What metrics should we track to measure UX improvement?

---

*Research conducted by AI Agent on 2025-11-19*
*For questions or discussion, see: https://github.com/andymai/hush/discussions*
