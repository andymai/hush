# Hush UI Expansion Roadmap

**Created:** 2025-11-19
**Based On:** `.ai/knowledge/rust-ui-frameworks-research.md`
**Strategy:** Expand existing egui implementation (not switch frameworks)

---

## Overview

This roadmap outlines a comprehensive plan to expand Hush's UI capabilities while maintaining its minimalist design philosophy. All enhancements use the existing egui framework.

**Goal:** Make Hush accessible to all users (not just CLI-comfortable developers) while preserving its lightweight, fast, and privacy-first design.

---

## Task Summary

| Task | Description | Priority | Complexity | Est. Days | Dependencies |
|------|-------------|----------|------------|-----------|--------------|
| **T-027** | Enhanced Settings Window | High | Medium | 1-2 | None |
| **T-028** | Model Management UI | High | Medium | 1-2 | T-027 |
| **T-029** | Model Download UI | High | Medium-High | 2-3 | T-027, T-028 |
| **T-030** | Audio Device Picker | High | Medium-High | 2-3 | T-027 |
| **T-031** | Hotkey Configurator | High | Medium | 2 | T-027 |
| **T-032** | LLM Configuration UI | Medium | Medium | 1-2 | T-027 |
| **T-033** | First-Run Setup Wizard | High | Medium-High | 2-3 | T-027-T-032 |
| **T-034** | System Tray Enhancements | Medium | Medium | 2 | T-027, T-028, T-030 |
| **T-035** | History Viewer | Low | Medium-High | 2-3 | T-027 |

**Total Estimated Time:** 16-23 days for all tasks

---

## Implementation Phases

### Phase 1: Foundation (Days 1-2)
**Goal:** Create core settings infrastructure

- **T-027: Enhanced Settings Window** ⭐ **START HERE**
  - Create settings module and window infrastructure
  - Implement tabbed interface (General, Models, Audio, Hotkeys, Advanced)
  - Add configuration persistence
  - Enable all other tasks

### Phase 2: Core Features (Days 3-10)
**Goal:** Implement essential UI features

**Week 1 (Days 3-5):**
- **T-028: Model Management UI**
  - Display models, show info, enable switching

- **T-030: Audio Device Picker**
  - List devices, live audio preview, test recording

**Week 2 (Days 6-10):**
- **T-029: Model Download UI**
  - Visual downloader with progress bars, queue management

- **T-031: Hotkey Configurator**
  - Custom hotkeys, conflict detection, validation

- **T-032: LLM Configuration UI**
  - API key management, editing modes, test connection

### Phase 3: Onboarding (Days 11-13)
**Goal:** Create guided first-run experience

- **T-033: First-Run Setup Wizard** ⭐ **HIGH IMPACT**
  - Multi-step wizard: model → UInput → audio → hotkey → LLM
  - Makes Hush accessible to new users
  - Reduces documentation burden

### Phase 4: Convenience Features (Days 14-16)
**Goal:** Enhance everyday usability

- **T-034: System Tray Enhancements**
  - Quick model/audio switching
  - Status display
  - Notifications

### Phase 5: Advanced Features (Days 17-19, Optional)
**Goal:** Add power-user features

- **T-035: History Viewer**
  - Transcription history with search
  - Statistics dashboard
  - Log viewer

---

## Minimum Viable Product (MVP)

For a functional UI expansion, complete these tasks:

### MVP Tasks (8-12 days):
1. ✅ **T-027: Enhanced Settings Window** (required foundation)
2. ✅ **T-028: Model Management UI** (critical for usability)
3. ✅ **T-029: Model Download UI** (replaces CLI download)
4. ✅ **T-030: Audio Device Picker** (makes audio config accessible)
5. ✅ **T-031: Hotkey Configurator** (removes hardcoded limitation)

### MVP+ (Add for best experience):
6. ✅ **T-033: First-Run Setup Wizard** (dramatically improves onboarding)

### Optional (Nice to have):
7. ⭕ **T-032: LLM Configuration UI** (for users wanting LLM)
8. ⭕ **T-034: System Tray Enhancements** (convenience)
9. ⭕ **T-035: History Viewer** (power users)

---

## Task Dependencies Graph

```
T-027 (Enhanced Settings Window)
  ├── T-028 (Model Management UI)
  │     └── T-029 (Model Download UI)
  ├── T-030 (Audio Device Picker)
  ├── T-031 (Hotkey Configurator)
  ├── T-032 (LLM Configuration UI)
  └── T-033 (First-Run Setup Wizard)
        ├── Requires: T-027, T-028, T-029, T-030, T-031
        └── Benefits from: T-032

T-027 (Enhanced Settings Window)
  ├── T-028 (Model Management UI)
  │     └── T-034 (System Tray - Model Menu)
  └── T-030 (Audio Device Picker)
        └── T-034 (System Tray - Audio Menu)

T-027 (Enhanced Settings Window)
  └── T-035 (History Viewer)
```

**Critical Path:** T-027 → T-028 → T-029 → T-033

---

## Implementation Strategy

### 1. Start with Foundation
Begin with **T-027 (Enhanced Settings Window)** as it unblocks all other tasks. This creates:
- Settings module structure
- Window lifecycle management
- Configuration persistence
- Tab interface

### 2. Parallel Development (After T-027)
These can be developed independently:
- **T-028 + T-029** (Models) - One developer
- **T-030** (Audio) - One developer
- **T-031** (Hotkeys) - One developer
- **T-032** (LLM) - One developer

### 3. Integration Phase
- **T-033 (Setup Wizard)** - Integrates T-027 through T-032
- **T-034 (System Tray)** - Integrates T-027, T-028, T-030
- **T-035 (History)** - Independent feature

### 4. Testing Strategy
Each task includes:
- Unit tests for logic
- Integration tests for UI
- Manual testing checklist
- Cross-platform verification (X11, Wayland)

---

## Success Metrics

### Before UI Expansion:
- ❌ Settings require CLI commands
- ❌ New users struggle with setup
- ❌ Model management is cumbersome
- ❌ Hotkeys are hardcoded
- ❌ Audio device selection is CLI-only

### After UI Expansion:
- ✅ Settings accessible via GUI
- ✅ First-run wizard guides new users
- ✅ Model download/switching in UI
- ✅ Hotkeys customizable
- ✅ Audio devices selectable with preview
- ✅ Optional LLM configuration in UI
- ✅ System tray for quick access

### Key Performance Indicators:
- **Setup Time:** < 5 minutes (first-run wizard)
- **Binary Size:** < +5 MB (egui is lightweight)
- **UI Responsiveness:** < 16ms frame time
- **Configuration Time:** < 30 seconds (change settings)
- **New User Success Rate:** > 90% (complete setup without docs)

---

## Technical Considerations

### egui Best Practices:
- ✅ Use immediate mode efficiently (don't rebuild complex state every frame)
- ✅ Cache expensive computations
- ✅ Use `ui.ctx().request_repaint_after()` for appropriate refresh rates
- ✅ Separate window for settings (not overlay)
- ✅ Persist window positions and sizes

### Performance:
- Keep UI responsive (< 16ms per frame)
- Async operations for downloads, API calls
- Background threads for audio preview
- Lazy loading for history

### Error Handling:
- Clear, user-friendly error messages
- Recovery options (retry, skip, cancel)
- Never crash on UI errors
- Log errors for debugging

### Cross-Platform:
- Test on X11 and Wayland
- Handle platform-specific edge cases
- Graceful degradation (e.g., if ksni unavailable)

---

## Risk Assessment

### Low Risk:
- T-027 (settings window) - Established pattern
- T-028 (model management) - Simple UI
- T-031 (hotkey config) - Well-defined scope

### Medium Risk:
- T-029 (download UI) - Async complexity, network errors
- T-030 (audio picker) - Real-time audio, device handling
- T-032 (LLM config) - API key security, storage

### Higher Risk:
- T-033 (setup wizard) - Complex integration of all features
- T-034 (system tray) - Platform-specific behavior
- T-035 (history viewer) - Performance with large datasets

### Mitigation Strategies:
- Start with low-risk tasks to build momentum
- Prototype high-risk tasks early
- Extensive testing on target platforms
- Clear error handling and fallbacks
- Incremental delivery (ship features as completed)

---

## Future Enhancements (Beyond This Roadmap)

### Post-1.0 Features:
- Voice command configuration UI
- Themes and visual customization
- Cloud sync for settings (encrypted)
- Plugin system for extensions
- Multi-language support
- Accessibility features (screen reader, high contrast)
- Mobile companion app (view transcriptions)

### Advanced Features:
- Local LLM integration (llama.cpp)
- Custom text processing rules
- Macro/snippet system
- Integration with other apps (VS Code, Slack)
- Team/enterprise features

---

## Getting Started

### For Developers:

1. **Read the research:**
   - `.ai/knowledge/rust-ui-frameworks-research.md`

2. **Start with T-027:**
   - Create settings module structure
   - Implement basic window
   - Add configuration persistence

3. **Follow dependencies:**
   - Complete foundation before dependent tasks
   - Test thoroughly at each step

4. **Use the task templates:**
   - Each task has clear requirements
   - Success criteria defined
   - Testing checklists included

### For Project Managers:

- **Minimum viable:** T-027, T-028, T-029, T-030, T-031 (8-12 days)
- **Recommended:** Add T-033 for best user experience (11-15 days)
- **Full featured:** All tasks (16-23 days)

---

## Questions or Issues?

- Review `.ai/knowledge/rust-ui-frameworks-research.md` for framework research
- Check `.ai/knowledge/conventions.md` for coding standards
- Refer to individual task files for detailed requirements
- Open GitHub issue for questions

---

**Ready to start?** Begin with **T-027: Enhanced Settings Window** 🚀
