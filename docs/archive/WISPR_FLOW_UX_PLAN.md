# Wispr Flow-Style UX Implementation Plan

## 🎯 Goal
Transform Hush to have a Wispr Flow-like experience with keyboard shortcuts and an onscreen overlay button.

## 📋 Current State Analysis

### ✅ What We Have
1. **Hotkey System** (`src/hotkey/global.rs`)
   - Global hotkey registration working
   - Press/release event detection ✅
   - Configurable key combinations ✅

2. **Core Pipeline**
   - Audio capture working
   - Whisper transcription with GPU acceleration
   - UInput text insertion (universal compatibility)

3. **TUI Interface** (`src/tui/`)
   - Full-screen terminal UI
   - Not suitable for overlay usage

### ❌ What's Missing
1. **Floating Overlay Window**
   - Small, always-on-top window
   - Visual recording feedback
   - Minimalist design

2. **Push-to-Talk Mode**
   - Hold hotkey → start recording
   - Release hotkey → stop & transcribe & insert

3. **Background Daemon Mode**
   - Run in background
   - Show overlay when triggered
   - Minimal resource usage when idle

## 🎨 Wispr Flow UX Pattern

### User Experience Flow
1. **Idle State**
   - Small floating button visible (can be disabled)
   - Or completely hidden, waiting for hotkey

2. **Trigger Recording**
   - Press and HOLD hotkey (e.g., `Ctrl+Win+Space`)
   - Or click floating button

3. **Recording State**
   - Overlay shows "Recording..." with visual feedback
   - Waveform animation or pulsing indicator
   - Show duration timer

4. **Processing State**
   - Release hotkey to stop recording
   - Overlay shows "Transcribing..." with spinner
   - GPU-accelerated Whisper runs

5. **Completion**
   - Text inserted at cursor position
   - Overlay shows "✓ Inserted" briefly
   - Return to idle state

## 🏗️ Technical Implementation Plan

### Phase 1: Overlay Window Foundation
**Dependencies to Add:**
```toml
egui_overlay = "0.5"  # Floating overlay windows
egui = "0.29"          # UI framework
```

**New Module:** `src/overlay/`
- `mod.rs` - Public API
- `window.rs` - Overlay window management
- `ui.rs` - Visual components
- `state.rs` - Recording state machine

**Window Features:**
- Always on top
- Small, minimalist design
- Transparency support
- Position: Bottom-right by default (configurable)
- Size: ~150x60px when idle, ~300x100px when recording

### Phase 2: Visual States

#### Idle State UI
```
┌─────────────────┐
│  🎤  Hush      │
│  Click or      │
│  Ctrl+Win+V    │
└─────────────────┘
```

#### Recording State UI
```
┌─────────────────┐
│  🔴 Recording   │
│  ▓▓▓▒▒▒░░░  3s │
│  Release to     │
│  transcribe     │
└─────────────────┘
```

#### Processing State UI
```
┌─────────────────┐
│  ⟳ Transcribing │
│  Please wait... │
└─────────────────┘
```

#### Success State UI
```
┌─────────────────┐
│  ✓ Text Inserted│
│  "Hello world"  │
└─────────────────┘
```

### Phase 3: Push-to-Talk Integration

**State Machine:**
```
Idle → (Hotkey Press) → Recording → (Hotkey Release) → Processing → Transcribing → Inserting → Idle
                           ↑                                                          │
                           └──────────────────────────────────────────────────────────┘
```

**Implementation:**
1. Hook hotkey press → Start audio capture
2. Show overlay with recording UI
3. Capture audio while key held
4. On hotkey release → Stop capture
5. Show "Transcribing..." overlay
6. Run Whisper transcription
7. Insert text via UInput
8. Show success briefly
9. Return to idle

### Phase 4: Daemon Mode

**New Command:**
```bash
./hush overlay              # Start overlay daemon
./hush overlay --hide       # Hidden mode (hotkey only)
./hush overlay --position bottom-right
```

**Features:**
- Background process
- Minimal CPU/memory when idle
- Auto-start on login (optional)
- System tray icon (optional)

### Phase 5: Configuration

**New Config Options:** (`config/default.toml`)
```toml
[overlay]
enabled = true
show_button = true          # Show floating button when idle
position = "bottom-right"   # top-left, top-right, bottom-left, bottom-right, center
hotkey = "Ctrl+Win+V"
auto_hide_after_ms = 2000   # Hide success message after 2s
theme = "dark"              # dark, light, auto

[overlay.appearance]
opacity = 0.95
width = 150
height = 60
font_size = 14
```

## 🎯 Implementation Phases

### Phase 1: Basic Overlay (Week 1)
- [ ] Add egui_overlay dependency
- [ ] Create overlay module structure
- [ ] Implement basic floating window
- [ ] Add idle state UI
- [ ] Test window positioning and always-on-top

### Phase 2: Recording States (Week 1-2)
- [ ] Implement state machine
- [ ] Add recording state UI with timer
- [ ] Add processing state UI
- [ ] Add success/error states
- [ ] Test state transitions

### Phase 3: Hotkey Integration (Week 2)
- [ ] Integrate existing hotkey system
- [ ] Implement push-to-talk logic
- [ ] Connect to audio capture pipeline
- [ ] Test end-to-end flow

### Phase 4: Polish & Features (Week 2-3)
- [ ] Add waveform visualization
- [ ] Add configuration options
- [ ] Implement daemon mode
- [ ] Add auto-start support
- [ ] Test on different window managers (X11, Wayland)

### Phase 5: Testing & Documentation (Week 3)
- [ ] Comprehensive testing
- [ ] Update README
- [ ] Create user guide
- [ ] Performance optimization

## 🔧 Technical Considerations

### Linux Compatibility
- **X11**: Full support with `egui_overlay`
- **Wayland**: May need compositor-specific configuration
- **Tiling WMs** (i3, sway): May need window rules to keep floating

### Performance
- **Idle**: ~5MB RAM, 0% CPU
- **Recording**: ~20MB RAM, <5% CPU (audio capture)
- **Transcribing**: Depends on GPU (already optimized)

### Dependencies
- Requires compositor with transparency support
- For i3/sway: Add window rules to keep overlay floating

## 📝 Alternative Approaches Considered

### 1. GTK+ Based Overlay
**Pros:** Native Linux feel, good X11/Wayland support
**Cons:** More complex, larger dependencies
**Decision:** Rejected - egui is simpler and more Rust-native

### 2. System Tray Only (No Overlay)
**Pros:** Less intrusive, simpler
**Cons:** Misses visual feedback during recording
**Decision:** Could add as option, but overlay is primary UX

### 3. Pure X11 Drawing (No GUI Framework)
**Pros:** Minimal dependencies, maximum control
**Cons:** Too much low-level work, harder to maintain
**Decision:** Rejected - egui provides good balance

## 🎯 Success Criteria

1. **User can trigger recording with hotkey** (push-to-talk)
2. **Visual feedback during all states** (recording, processing, success)
3. **Works in all applications** (via UInput - already working)
4. **Minimal resource usage when idle** (<10MB RAM)
5. **Fast response time** (<500ms from release to transcription start)
6. **Configurable appearance and behavior**

## 🚀 Getting Started

After approval, we'll start with:
1. Add `egui_overlay` dependency to `Cargo.toml`
2. Create `src/overlay/` module structure
3. Implement basic floating window
4. Iterate on UI design

---

**Ready to proceed?** Should I start implementing Phase 1?
