# Hush UI Mockups & Visual Reference

**Last Updated:** 2025-11-19
**Purpose:** Visual reference for implementing Hush's polished UI
**See Also:** `ui-design-specification.md` for detailed design specs

---

## Overlay States (Main UI)

### Idle State
```
Screen Position: Bottom-center, 32px from bottom

┌─────────────────────────────────────┐
│                                     │
│                                     │
│                                     │
│                ___                  │  <- Tiny pill (60×4px)
│                                     │     Almost invisible, subtle presence
│                                     │
└─────────────────────────────────────┘
```

**Visual Properties:**
- 60px wide × 4px tall
- Dark glass (80% opacity black) or light glass (85% opacity white)
- Soft glow (4px shadow)
- Subtle pulsing animation (1s cycle, 10% opacity change)

### Recording State
```
Screen Position: Bottom-center, 32px from bottom (expands from idle)

┌─────────────────────────────────────┐
│                                     │
│                                     │
│          ╔════════════╗             │
│          ║ ▂▅▇▆▄▃▅▇▆▄ ║             │  <- Waveform (80×24px)
│          ╚════════════╝             │     Sage green glow
│                                     │
└─────────────────────────────────────┘
```

**Visual Properties:**
- 80px wide × 24px tall (expanded)
- Sage green (#4CAF8C) with 95% opacity
- 12 animated waveform bars (white)
- Bars respond to audio amplitude
- Green glow shadow (20px spread)

**Waveform Detail:**
```
▂ ▄ ▇ ▅ ▃ ▂ ▄ ▆ ▇ ▅ ▃ ▂   <- 12 bars, sine wave animation
```

### Processing State
```
┌─────────────────────────────────────┐
│                                     │
│                                     │
│          ╔════════════╗             │
│          ║processing...║             │  <- Processing indicator
│          ╚════════════╝             │     Neutral gray
│                                     │
└─────────────────────────────────────┘
```

**Visual Properties:**
- Text: "processing..." (10px, centered)
- Background: Light gray or dark gray (theme-dependent)
- Subtle pulsing dots animation

### Success State (Brief, 1.5s)
```
┌─────────────────────────────────────┐
│                                     │
│                                     │
│          ╔════════════╗             │
│          ║  done ✓    ║             │  <- Success flash
│          ╚════════════╝             │     Green
│                                     │
└─────────────────────────────────────┘
```

**Visual Properties:**
- Green background (#4CAF8C)
- Checkmark icon (white)
- Brief appearance (1.5s), then fade to idle

---

## Settings Window

### Full Window Layout
```
╔══════════════════════════════════════════════════════════════╗
║  Hush Settings                                          ✕    ║  <- Title bar (48px)
╠══════════════════════════════════════════════════════════════╣
║  General  Models  Audio  Hotkeys  Advanced                   ║  <- Tab bar (40px)
╠══════════════════════════════════════════════════════════════╣
║                                                              ║
║    Display Settings                                          ║  <- Section header
║    ─────────────────────────────────────────────────────     ║
║                                                              ║
║    Theme:                          [Dark  ▼]                 ║  <- Form row
║    Overlay Position:               [Bottom Center  ▼]        ║
║    Overlay Opacity:                ●──────────────  85%      ║  <- Slider
║    Show button when idle:          [Toggle ON ]              ║  <- Toggle
║                                                              ║
║    Audio Feedback                                            ║
║    ─────────────────────────────────────────────────────     ║
║                                                              ║
║    Play sound on complete:         [Toggle OFF]              ║
║    Feedback volume:                ●───────────────  50%     ║
║                                                              ║
║                                                              ║
║                                                              ║
╠══════════════════════════════════════════════════════════════╣
║                                    [Cancel]  [Apply]         ║  <- Button bar
╚══════════════════════════════════════════════════════════════╝

Window: 800×600px (resizable, min 600×400px)
```

### Models Tab Layout
```
╔══════════════════════════════════════════════════════════════╗
║  Hush Settings                                          ✕    ║
╠══════════════════════════════════════════════════════════════╣
║  General  [Models]  Audio  Hotkeys  Advanced                 ║  <- Models tab active
╠══════════════════════════════════════════════════════════════╣
║                                                              ║
║    Available Models                                          ║
║    ─────────────────────────────────────────────────────     ║
║                                                              ║
║    ┌──────────────────────────────────────────────────┐     ║
║    │ ● Base Model              ✓ Installed     [Set]  │     ║  <- Active model
║    │ 145 MB • CPU: 4-6s • GPU: 0.5s • Good            │     │
║    └──────────────────────────────────────────────────┘     ║
║                                                              ║
║    ┌──────────────────────────────────────────────────┐     ║
║    │ ○ Tiny                    ⌛ Not installed[Download]│   ║  <- Available model
║    │ 75 MB • CPU: 2-3s • GPU: 0.3s • Basic            │     ║
║    └──────────────────────────────────────────────────┘     ║
║                                                              ║
║    ┌──────────────────────────────────────────────────┐     ║
║    │ ○ Small                   ⌛ Downloading...       │     ║  <- Downloading
║    │ 466 MB • CPU: 8-12s • GPU: 0.8s • Better         │     ║
║    │ ████████████░░░░░░░░░░░░░░ 45% • 2.4 MB/s        │     ║  <- Progress bar
║    └──────────────────────────────────────────────────┘     ║
║                                                              ║
║    Storage: Using 612 MB of 2.9 GB models                   ║
║                                                              ║
╠══════════════════════════════════════════════════════════════╣
║                                              [Close]         ║
╚══════════════════════════════════════════════════════════════╝
```

**Model Card Anatomy:**
```
┌──────────────────────────────────────────────────┐
│ ● Base Model              ✓ Installed     [Set]  │  <- Radio, Name, Status, Action
│ 145 MB • CPU: 4-6s • GPU: 0.5s • Good            │  <- Specs (size, speed, quality)
│ ████████████████████████████████ 100%            │  <- Progress (if downloading)
└──────────────────────────────────────────────────┘

States:
- Installed + Active: Green border, filled radio button, "Set" disabled
- Installed + Inactive: No border, empty radio, "Set" enabled
- Not Installed: Gray text, "Download" button
- Downloading: Progress bar, percentage, speed, "Cancel" button
```

### Audio Tab Layout
```
╔══════════════════════════════════════════════════════════════╗
║  Hush Settings                                          ✕    ║
╠══════════════════════════════════════════════════════════════╣
║  General  Models  [Audio]  Hotkeys  Advanced                 ║
╠══════════════════════════════════════════════════════════════╣
║                                                              ║
║    Input Device                                              ║
║    ─────────────────────────────────────────────────────     ║
║                                                              ║
║    ┌──────────────────────────────────────────────────┐     ║
║    │ ● USB Microphone (Yeti)           [Test] ▂▅█▅▂  │     ║  <- Active device
║    └──────────────────────────────────────────────────┘     ║
║                                                              ║
║    ┌──────────────────────────────────────────────────┐     ║
║    │ ○ Built-in Audio Analog Stereo    [Test] ▂▂▂__  │     ║  <- Other device
║    └──────────────────────────────────────────────────┘     ║
║                                                              ║
║    Live Audio Level                                          ║
║    ─────────────────────────────────────────────────────     ║
║                                                              ║
║    ████████████████░░░░░░░░░░░░░░░░░░░░░░░░ -12dB           ║  <- Level meter
║                                              ▲               ║  <- Peak indicator
║                                                              ║
║    [🎤 Record 3s Test]                                       ║  <- Test button
║                                                              ║
║    Advanced Settings                                         ║
║    ─────────────────────────────────────────────────────     ║
║                                                              ║
║    Sample Rate:                    [16000 Hz  ▼]             ║
║    Auto-gain Control:              [Toggle ON ]              ║
║                                                              ║
╠══════════════════════════════════════════════════════════════╣
║                                              [Close]         ║
╚══════════════════════════════════════════════════════════════╝
```

**Audio Level Meter Detail:**
```
Good level (Green):
████████████████████░░░░░░░░░░░░░░░░ -18dB

Loud (Amber):
████████████████████████████░░░░░░░░ -6dB

Clipping (Red):
█████████████████████████████████░░░ -1dB
                                 ▲    <- Peak hold indicator
```

### Hotkeys Tab Layout
```
╔══════════════════════════════════════════════════════════════╗
║  Hush Settings                                          ✕    ║
╠══════════════════════════════════════════════════════════════╣
║  General  Models  Audio  [Hotkeys]  Advanced                 ║
╠══════════════════════════════════════════════════════════════╣
║                                                              ║
║    Keyboard Shortcuts                                        ║
║    ─────────────────────────────────────────────────────     ║
║                                                              ║
║    Push-to-Talk:        ┌───────────────────┐  [Change]     ║
║                         │ Ctrl + Alt + V    │               ║
║                         └───────────────────┘               ║
║                                                              ║
║    Open Settings:       ┌───────────────────┐  [Change]     ║
║                         │ Ctrl + Alt + S    │               ║
║                         └───────────────────┘               ║
║                                                              ║
║    Toggle Overlay:      ┌───────────────────┐  [Change]     ║
║                         │ Ctrl + Alt + H    │               ║
║                         └───────────────────┘               ║
║                                                              ║
║    Quick Undo:          ┌───────────────────┐  [Change]     ║
║                         │ Not set           │               ║
║                         └───────────────────┘               ║
║                                                              ║
║                                     [Reset to Defaults]      ║
║                                                              ║
║    ⓘ Hotkeys require at least 2 modifiers + 1 key           ║
║                                                              ║
╠══════════════════════════════════════════════════════════════╣
║                                              [Close]         ║
╚══════════════════════════════════════════════════════════════╝
```

**Hotkey Recorder Modal:**
```
┌──────────────────────────────────────┐
│                                      │
│        Press keys to record          │  <- Prompt
│                                      │
│        Ctrl + Alt + V                │  <- Live display
│                                      │
│                                      │
│                                      │
│    ⓘ Requires 2+ modifiers + 1 key   │  <- Help text
│                                      │
│          [Cancel]  [Apply]           │  <- Actions
│                                      │
└──────────────────────────────────────┘

States:
1. Waiting: "Press keys..." (gray)
2. Recording: Live update as keys pressed
3. Valid: Green check, Apply enabled
4. Invalid: Red X, error message, Apply disabled
```

---

## First-Run Setup Wizard

### Wizard Navigation
```
╔══════════════════════════════════════════════════════════════╗
║                                                              ║
║                         Step 2 of 7                          ║  <- Progress text
║  ████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  ║  <- Progress bar
║                                                              ║
╠══════════════════════════════════════════════════════════════╣
║                                                              ║
║                                                              ║
║                   [Step Content Here]                        ║  <- Step area
║                                                              ║
║                                                              ║
╠══════════════════════════════════════════════════════════════╣
║  [< Back]              [Skip]                 [Next >]       ║  <- Navigation
╚══════════════════════════════════════════════════════════════╝

Window: 640×560px (fixed size, centered)
```

### Step 1: Welcome
```
╔══════════════════════════════════════════════════════════════╗
║                                                              ║
║                         Step 1 of 7                          ║
║  ██████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  ║
║                                                              ║
╠══════════════════════════════════════════════════════════════╣
║                                                              ║
║                        🤫                                    ║  <- Hush logo (80px)
║                                                              ║
║                   Welcome to Hush                            ║  <- Title (32px)
║                                                              ║
║          Fast, private voice-to-text for Linux               ║  <- Subtitle (16px)
║                                                              ║
║    Hush uses OpenAI's Whisper models locally for fast,       ║
║    accurate transcription. Your voice never leaves your      ║  <- Description
║    computer. Setup takes about 5 minutes.                    ║
║                                                              ║
║                  🔒 All transcription is local                ║  <- Privacy badge
║                                                              ║
║                                                              ║
║                    [Get Started]                             ║  <- Large CTA (44px)
║                                                              ║
╠══════════════════════════════════════════════════════════════╣
║                                              [Exit]          ║
╚══════════════════════════════════════════════════════════════╝
```

### Step 2: Model Selection
```
╔══════════════════════════════════════════════════════════════╗
║                                                              ║
║                         Step 2 of 7                          ║
║  ████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  ║
║                                                              ║
╠══════════════════════════════════════════════════════════════╣
║                                                              ║
║                   Choose a Model                             ║
║                                                              ║
║    Select which Whisper model to download. Models run        ║
║    locally on your computer for complete privacy.            ║
║                                                              ║
║    ✅ GPU Detected - Faster models recommended                ║
║                                                              ║
║    ┌──────────────────────────────────────────────────┐     ║
║    │ ○ Tiny          75 MB   • Fast    • Basic        │     ║
║    └──────────────────────────────────────────────────┘     ║
║    ┌──────────────────────────────────────────────────┐     ║
║    │ ● Base         145 MB   • Good    • Recommended  │  ⭐ ║
║    └──────────────────────────────────────────────────┘     ║
║    ┌──────────────────────────────────────────────────┐     ║
║    │ ○ Small        466 MB   • Better  • High Quality │     ║
║    └──────────────────────────────────────────────────┘     ║
║                                                              ║
║    Model will download when you click Next (2-3 minutes)     ║
║                                                              ║
╠══════════════════════════════════════════════════════════════╣
║  [< Back]                                        [Next >]    ║
╚══════════════════════════════════════════════════════════════╝
```

### Step 2b: Model Downloading
```
╔══════════════════════════════════════════════════════════════╗
║                                                              ║
║                         Step 2 of 7                          ║
║  ████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  ║
║                                                              ║
╠══════════════════════════════════════════════════════════════╣
║                                                              ║
║                                                              ║
║               Downloading Base Model...                      ║
║                                                              ║
║    ████████████████████████░░░░░░░░░░░░░░░░░░░              ║  <- Progress bar
║                                                              ║
║         72 MB / 145 MB • 2.4 MB/s • 30s remaining            ║
║                                                              ║
║                                                              ║
║                                                              ║
║                      [Cancel Download]                       ║
║                                                              ║
║                                                              ║
╠══════════════════════════════════════════════════════════════╣
║  [< Back]                                                    ║
╚══════════════════════════════════════════════════════════════╝
```

### Step 7: Completion
```
╔══════════════════════════════════════════════════════════════╗
║                                                              ║
║                         Step 7 of 7                          ║
║  ████████████████████████████████████████████████████████░░  ║
║                                                              ║
╠══════════════════════════════════════════════════════════════╣
║                                                              ║
║                        ✓                                     ║  <- Success (80px)
║                                                              ║
║                   You're all set!                            ║
║                                                              ║
║    ✅ Model: Base (145 MB)                                    ║
║    ✅ Audio: USB Microphone                                   ║  <- Summary
║    ✅ Hotkey: Ctrl+Alt+V                                      ║
║    ✅ Text Processing: Medium                                 ║
║                                                              ║
║    Quick Start:                                              ║
║    1. Hold Ctrl+Alt+V                                        ║
║    2. Speak naturally                                        ║  <- Instructions
║    3. Release when done                                      ║
║    4. Your text appears!                                     ║
║                                                              ║
║                  [Start Using Hush]                          ║  <- Large CTA
║                                                              ║
╠══════════════════════════════════════════════════════════════╣
║                        [⚙️  Settings]  [Exit]                 ║
╚══════════════════════════════════════════════════════════════╝
```

---

## System Tray

### Tray Icon States
```
Idle:       🎤   (microphone outline)
Recording:  🎤   (filled, pulsing)
Processing: 🎤⟳  (with spinner)
Error:      🎤•  (with red dot)
```

### Context Menu
```
┌────────────────────────────────┐
│ 🎤 Start Recording             │  <- Quick action
│ ───────────────────────────────│
│ Model ▸                        │  <- Submenu
│ Audio Device ▸                 │  <- Submenu
│ ───────────────────────────────│
│ ✨ LLM Polishing: Enabled      │  <- Toggle
│ 🎯 Editing Mode: Medium        │  <- Status
│ ───────────────────────────────│
│ ⚙️  Settings...                 │
│ 📊 Status                       │
│ 📋 History                      │
│ ───────────────────────────────│
│ About Hush                     │
│ Help & Documentation           │
│ Quit                           │
└────────────────────────────────┘
```

### Model Submenu (Flyout)
```
┌────────────────────────────────┐   ┌────────────────────────┐
│ 🎤 Start Recording             │   │ ● Base (current)       │
│ ───────────────────────────────│   │ ○ Tiny                 │
│ Model ▸                        │───│ ○ Small                │
│ Audio Device ▸                 │   │ ───────────────────────│
│ ───────────────────────────────│   │ Manage Models...       │
│ ✨ LLM Polishing: Enabled      │   └────────────────────────┘
│ 🎯 Editing Mode: Medium        │
```

---

## Component Library

### Buttons

**Primary Button:**
```
┌─────────────────┐
│   Apply         │  <- 36px height, sage green, white text
└─────────────────┘

Hover:
┌─────────────────┐
│   Apply         │  <- Darker green, elevated shadow
└─────────────────┘
```

**Secondary Button:**
```
┌─────────────────┐
│   Cancel        │  <- 36px height, transparent, green border & text
└─────────────────┘

Hover:
┌─────────────────┐
│   Cancel        │  <- Light green background (10% opacity)
└─────────────────┘
```

**Destructive Button:**
```
┌─────────────────┐
│   Delete        │  <- 36px height, coral red, white text
└─────────────────┘

Hover:
┌─────────────────┐
│   Delete        │  <- Darker red, elevated shadow
└─────────────────┘
```

### Form Controls

**Toggle Switch:**
```
OFF:  ⚪─────  (gray track, white knob left)
ON:   ─────⚪  (green track, white knob right)
```

**Slider:**
```
●────────────────  50%   (green fill left, gray track right, white thumb)
```

**Text Input:**
```
┌───────────────────────────────┐
│ Enter API key...              │  <- 36px height, placeholder gray
└───────────────────────────────┘

Focus:
┌───────────────────────────────┐
│ sk-ant-api...█                │  <- Green border, green glow
└───────────────────────────────┘

Error:
┌───────────────────────────────┐
│ Invalid format                │  <- Red border, red glow
└───────────────────────────────┘
```

**Dropdown:**
```
┌───────────────────────────▼───┐
│ Base Model                    │  <- 36px height, chevron right
└───────────────────────────────┘

Expanded:
┌───────────────────────────▲───┐
│ Base Model                    │  <- Green border
├───────────────────────────────┤
│ ● Base Model                  │  <- Selected (radio filled)
│ ○ Tiny                        │
│ ○ Small                       │
│ ○ Medium                      │
└───────────────────────────────┘
```

**Radio Button:**
```
○ Option 1    (unselected)
● Option 2    (selected - filled dot, green)
```

**Checkbox:**
```
☐ Option 1    (unchecked)
☑ Option 2    (checked - checkmark, green background)
```

### Progress Indicators

**Progress Bar (Determinate):**
```
████████████████░░░░░░░░░░░░░░░░░░░░  45%
```

**Progress Bar (Indeterminate):**
```
░░░░████████████░░░░░░░░░░░░░░░░░░░░  (sliding shimmer animation)
```

**Spinner:**
```
   ⟳    (rotating partial circle, 20px, green)
```

---

## Typography Examples

### Headings
```
Display Large (32px):    Welcome to Hush
Display Medium (24px):   Choose a Model
Heading Large (20px):    Display Settings
Heading Medium (16px):   Advanced Options
```

### Body Text
```
Body Large (14px):    This is the main content text used throughout the application.
Body Medium (13px):   This is secondary content and most UI text (labels, buttons).
Body Small (12px):    This is for captions, metadata, and fine print.
Tiny (10px):          Overlay states, timestamps
```

### Weights
```
Regular (400):   Standard body text for readability
Medium (500):    Emphasis in buttons and labels
Semibold (600):  Headings and active states
Bold (700):      Strong emphasis (rarely used)
```

---

## Color Swatches

### Light Mode
```
Background:     ░░░░░  #FAF8F5  (cream)
Panel:          ████  #FFFFFF  (white)
Text Primary:   ████  #1A1A1A  (near-black)
Text Secondary: ▓▓▓▓  #666666  (medium gray)
Accent:         ████  #4CAF8C  (sage green)
```

### Dark Mode
```
Background:     ████  #1C1C1E  (soft charcoal)
Panel:          ▓▓▓▓  #2C2C2E  (dark panel)
Text Primary:   ░░░░  #FAFAFA  (off-white)
Text Secondary: ▓▓▓▓  #B0B0B0  (light gray)
Accent:         ████  #4CAF8C  (sage green)
```

### Accent Colors
```
Success/Primary:  ████  #4CAF8C  (sage green)
Secondary:        ████  #B4A5D9  (lavender)
Warning:          ████  #F5A623  (amber)
Error:            ████  #E74C3C  (coral red)
```

---

## Animation Timing

### Durations
```
Fast (hover, click):     150ms
Medium (modal, overlay): 250ms
Slow (page transition):  400ms
Bounce (success):        600ms
```

### Easing
```
Standard:  cubic-bezier(0.4, 0, 0.2, 1)  ─────╮
                                              ╱
                                            ╱
                                          ╱
Bounce:    cubic-bezier(0.34, 1.56, 0.64, 1)  ╱╲
                                              ╱  ╲
                                            ╱    ╲
```

---

## Spacing Scale

```
Tiny (4px):     ▪ Text to icon
Small (8px):    ▪▪ Button padding
Medium (12px):  ▪▪▪ Element spacing (default)
Large (16px):   ▪▪▪▪ Section spacing
XLarge (24px):  ▪▪▪▪▪▪ Major sections
XXLarge (32px): ▪▪▪▪▪▪▪▪ Window margins
Huge (48px):    ▪▪▪▪▪▪▪▪▪▪▪▪ Dramatic spacing
```

---

## Grid & Layout

### 4px Base Grid
```
All spacing should align to 4px grid:
✓ 4px, 8px, 12px, 16px, 20px, 24px...
✗ 5px, 7px, 13px, 15px...
```

### Golden Ratio (Optional, for visual hierarchy)
```
1.618 ratio for sizing related elements:
- If heading is 24px, body could be 15px (24 / 1.618)
- If icon is 40px, related icon could be 25px
```

---

## Accessibility

### Focus Indicators
```
┌───────────────────────────────┐
│ [Focus]                       │  <- 2px solid green outline, 2px offset
└───────────────────────────────┘
  ▲─────────────────────────────▲
```

### Color Contrast Examples
```
✓ PASS: #1A1A1A text on #FAF8F5 background (12:1)
✓ PASS: #4CAF8C accent on #FFFFFF background (4.5:1)
✗ FAIL: #CCCCCC text on #FFFFFF background (1.7:1)
```

---

## Implementation Notes

### egui Styling
```rust
// Apply custom theme
let mut style = (*ctx.style()).clone();
style.visuals.window_fill = Color32::from_rgb(250, 248, 245);
style.spacing.item_spacing = egui::vec2(12.0, 12.0);
ctx.set_style(style);
```

### Asset Paths
```
assets/icons/app-icon.svg
assets/icons/tray-recording.svg
assets/fonts/Inter-Medium.ttf
```

---

**Next Steps:**
1. Use these mockups as reference when implementing
2. Iterate based on actual implementation
3. Update mockups if design evolves
4. Create Figma designs if needed for more complex flows

**See Also:**
- `ui-design-specification.md` - Detailed design specs
- `rust-ui-frameworks-research.md` - Framework choice rationale
- Individual task files (T-027 through T-035) - Implementation requirements
