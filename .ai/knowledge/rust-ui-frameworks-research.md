# Rust UI Framework Research for Hush

**Date:** 2025-11-19
**Researcher:** Claude AI Agent
**Context:** Research into whether Hush would benefit from a UI framework, and evaluation of Rust GUI ecosystem

---

## Executive Summary

**Should Hush adopt a UI framework?**

**Recommendation:** Hush should **expand its existing egui implementation** rather than switch frameworks.

**Key Finding:** The Rust GUI ecosystem is still maturing - 94.4% of surveyed libraries aren't production-ready (2025 survey). However, a few frameworks stand out: **egui** (already used by Hush), **iced**, **Slint**, and **Tauri**.

---

## Current State of Hush UI

### What Hush Has:
- ✅ egui 0.23 + egui_overlay 0.5
- ✅ Minimalist pill-shaped overlay (idle, recording, processing states)
- ✅ Waveform animation with audio amplitude feedback
- ✅ Basic settings panel (theme toggle only)
- ✅ Draggable, mouse-passthrough overlay
- ✅ Optional system tray support (ksni)

### What's Missing:
- ⚠️ Comprehensive settings UI (model selection, audio devices, hotkeys, LLM config)
- ⚠️ Model management interface (currently CLI: `./hush models download`)
- ⚠️ Hotkey customization (hardcoded to Ctrl+Alt+V)
- ⚠️ Audio device picker (currently CLI: `./hush status --devices`)
- ⚠️ History/logs viewer
- ⚠️ Visual onboarding/setup wizard

---

## Assessment: Does Hush Need a New Framework?

### Arguments AGAINST Switching:
1. **Target Audience** - Linux developers are comfortable with CLI
2. **Design Philosophy** - Minimalist, stays out of the way
3. **egui Works Well** - Perfect for immediate mode overlay
4. **One-Time Setup** - Most configuration is set-once-and-forget
5. **Complexity Cost** - New framework = larger binary, more dependencies, learning curve

### Arguments FOR UI Enhancement:
1. **User Experience** - Settings should be more accessible
2. **Onboarding** - Visual setup would help new users
3. **Model Management** - CLI is cumbersome for model downloads
4. **Accessibility** - GUI makes Hush approachable for non-CLI users

### Final Recommendation:
Expand egui implementation with:
- **Enhanced settings window** (not just overlay panel)
- **Model management UI** (download, switch, view info)
- **Audio device picker** with live preview
- **Hotkey configurator** with conflict detection
- **First-run setup wizard**

**Do NOT** switch frameworks. egui is lightweight, well-documented, and perfect for Hush's use case.

---

## Rust UI Frameworks: Comprehensive Comparison

### 1. egui ⭐ (Currently Used by Hush)

**Type:** Immediate Mode GUI
**Maturity:** Production-ready, active development
**License:** MIT/Apache-2.0
**Hush Fit:** ⭐⭐⭐⭐⭐ **Perfect fit**

#### Pros:
- ✅ **Excellent for Hush's use case** - overlays, tools, inspectors
- ✅ **Tiny binary size** - smallest among major frameworks
- ✅ **Fast development** - no complex state management
- ✅ **Pure Rust** - no DSL, macros, or special syntax
- ✅ **Good documentation** - docs.rs, tutorials, examples
- ✅ **Cross-platform** - web (WASM), desktop, embedded
- ✅ **egui_overlay** - perfect for transparent, always-on-top windows
- ✅ **Beginner-friendly** - describe UI each frame

#### Cons:
- ⚠️ **IMGUI limitations** - rebuilds UI every frame
- ⚠️ **Breaking changes** - API still evolving
- ⚠️ **Custom widgets harder** - not designed for complex widget libraries

#### Documentation Quality: ⭐⭐⭐⭐ (4/5)
- Official docs: https://docs.rs/egui
- Web demo: https://egui.rs
- GitHub: https://github.com/emilk/egui
- Tutorials: https://hackmd.io/@Hamze/Sys9nvF6Jl

---

### 2. iced ⭐

**Type:** Elm Architecture (Retained Mode)
**Maturity:** Pre-1.0 (0.13.1), experimental
**License:** MIT
**Hush Fit:** ⭐⭐ **Not ideal** - too complex

#### Pros:
- ✅ **Structured architecture** - Model-View-Update pattern
- ✅ **Scalable** - great for complex desktop apps
- ✅ **Multi-window support** - added in 0.12
- ✅ **WebGPU backend** - modern rendering
- ✅ **Good documentation** - book.iced.rs

#### Cons:
- ⚠️ **Experimental** - still pre-1.0
- ⚠️ **Steeper learning curve** - requires Elm architecture knowledge
- ⚠️ **Larger binary** - more complex than egui
- ⚠️ **Overkill for simple UIs**

#### Documentation Quality: ⭐⭐⭐⭐ (4/5)
- Official book: https://book.iced.rs
- GitHub: https://github.com/iced-rs/iced
- API docs: https://docs.rs/iced

---

### 3. Slint ⭐

**Type:** DSL-based (declarative markup)
**Maturity:** 1.x (API stable)
**License:** GPL v3 / Commercial
**Hush Fit:** ⭐⭐ **Not ideal** - GPL license

#### Pros:
- ✅ **API stability** - reached 1.x
- ✅ **Excellent tooling** - VS Code extension, LSP, live preview
- ✅ **Comprehensive docs** - tutorials, API ref, examples
- ✅ **Embedded-friendly** - microcontrollers
- ✅ **Native look** - platform-specific rendering

#### Cons:
- ⚠️ **DSL required** - must learn .slint markup
- ⚠️ **GPL license** - requires commercial license for proprietary apps
- ⚠️ **Build complexity** - requires build.rs integration

#### Documentation Quality: ⭐⭐⭐⭐⭐ (5/5)
- Excellent: https://slint.dev
- GitHub: https://github.com/slint-ui/slint
- Docs: https://slint.rs/docs/rust/slint/

---

### 4. Tauri ⭐

**Type:** Web-based (HTML/CSS/JS frontend)
**Maturity:** 2.0 stable (Oct 2024)
**License:** MIT/Apache-2.0
**Hush Fit:** ⭐ **Poor fit** - web-based overkill

#### Pros:
- ✅ **Web tech** - use React, Vue, Svelte
- ✅ **Small binaries** - uses system WebView
- ✅ **Cross-platform** - desktop + mobile
- ✅ **Familiar** - web developers can jump in

#### Cons:
- ⚠️ **WebView dependency** - requires system browser
- ⚠️ **Not native** - HTML/CSS, not native widgets
- ⚠️ **Startup overhead** - slower than native GUI
- ⚠️ **Wrong paradigm** - overkill for minimal overlay

#### Documentation Quality: ⭐⭐⭐⭐⭐ (5/5)
- Excellent: https://v2.tauri.app
- GitHub: https://github.com/tauri-apps/tauri

---

### 5. Dioxus ⭐ (Emerging)

**Type:** React-like (declarative, reactive)
**Maturity:** 0.5, rapidly maturing
**License:** MIT/Apache-2.0
**Hush Fit:** ⭐⭐ **Not ideal** - WebView-based

#### Pros:
- ✅ **React-like** - familiar to web developers
- ✅ **Cross-platform** - web, desktop, mobile, SSR
- ✅ **Hot reload** - state preserved
- ✅ **Narrator support** - good accessibility

#### Cons:
- ⚠️ **Pre-1.0** - still evolving
- ⚠️ **WebView-based desktop** - uses WebView2/WebKitGTK
- ⚠️ **Not production-ready** - per 2025 survey

#### Documentation Quality: ⭐⭐⭐⭐ (4/5)
- Good: https://dioxuslabs.com

---

### 6. Xilem ⭐ (Experimental)

**Type:** Reactive UI with custom rendering
**Maturity:** Very early stage
**License:** Apache-2.0
**Hush Fit:** ⭐ **Not ready**

- ⚠️ **Very experimental** - not production-ready
- ⚠️ **Limited documentation**
- ⚠️ **Incomplete features**

---

## Ecosystem State (2025)

### Production Readiness
- **2025 Survey Results:** 94.4% of 43 surveyed Rust GUI libraries are **NOT production-ready**
- **Common Issues:** Build failures, missing features, poor documentation, unmaintained

### The "Big 4" Production-Ready:
1. **egui** - Tools, overlays, immediate UIs ⭐ **Best for Hush**
2. **Slint** - Native apps, embedded systems
3. **iced** - Complex desktop apps (approaching 1.0)
4. **Tauri** - Web-based cross-platform apps

### Accessibility
- **Good support:** Dioxus, Slint, egui, WinSafe
- **Partial support:** Freya, Vizia, Xilem
- **Most frameworks:** Incomplete accessibility

---

## Recommendations for Hush

### ✅ Keep egui, Expand Functionality

**Why:**
- Already integrated and working well
- Perfect for overlay use case
- Lightweight, fast, no DSL
- Good documentation
- No code rewrite needed

### 📋 Suggested Enhancements:

#### 1. Enhanced Settings Window (High Priority)
Create a proper settings window (not just overlay panel):
- Model management (download, switch, view info)
- Audio device picker with live preview
- Hotkey configuration with conflict detection
- Text processing mode selector
- LLM settings (API key, model, temperature)
- Display settings (theme, position, opacity)

#### 2. First-Run Setup Wizard (Medium Priority)
Guide new users through:
1. Model download (with size/speed comparison)
2. UInput setup verification
3. Audio device selection + test
4. Hotkey customization
5. Optional LLM configuration

#### 3. Model Management UI (Medium Priority)
- Visual model downloader with progress bar
- Model info cards (size, speed, accuracy)
- One-click model switching
- Storage usage display

#### 4. System Tray Enhancement (Low Priority)
Expand existing ksni integration:
- Quick model switching
- Audio device switching
- Pause/resume
- Settings shortcut

#### 5. History Viewer (Optional)
- Recent transcriptions
- Clipboard with search
- Export to file

---

## Performance Considerations

### Binary Size Comparison:
- **egui**: ~5-10 MB (smallest)
- **iced**: ~15-25 MB
- **Slint**: ~10-20 MB
- **Tauri**: ~30-50 MB

**Hush Current:** ~30-40 MB (mostly Whisper + CUDA)
**Impact:** egui expansion would add ~1-2 MB (negligible)

---

## Conclusion

**Hush should stick with egui** and expand its capabilities rather than switching frameworks.

**Key Reasons:**
1. ✅ egui is proven, lightweight, and production-ready
2. ✅ Perfect fit for overlay + settings UI
3. ✅ No need to rewrite existing code
4. ✅ Rust GUI ecosystem is still immature (94.4% not production-ready)
5. ✅ Other frameworks are overkill for Hush's needs

**Alternative Considered:** If Hush ever needs native-looking widgets or more complex UI, **iced** would be the best alternative once it reaches 1.0.

**Do NOT Consider:** Tauri (web-based overkill), Dioxus (WebView-based), Xilem (too experimental), Slint (GPL license issues)

---

## References

### Official Documentation
- egui: https://docs.rs/egui, https://egui.rs
- iced: https://book.iced.rs
- Slint: https://slint.dev
- Tauri: https://v2.tauri.app
- Dioxus: https://dioxuslabs.com

### Research Sources
- Are We GUI Yet: https://areweguiyet.com
- 2025 Survey: https://www.boringcactus.com/2025/04/13/2025-survey-of-rust-gui-libraries.html
- LogRocket: https://blog.logrocket.com/state-rust-gui-libraries/
- Performance Comparison: http://lukaskalbertodt.github.io/2023/02/03/tauri-iced-egui-performance-comparison.html

---

**Last Updated:** 2025-11-19
**Next Review:** When considering major UI overhaul or new framework adoption
