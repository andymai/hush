# macOS Cross-Platform Support - Task Overview

**Status:** Available
**Created:** 2025-11-19
**Total Tasks:** 9
**Estimated Effort:** 2-4 weeks

---

## Executive Summary

This task series adds full macOS support to Hush, including:
- ✅ Native text insertion via CGEvent and Accessibility API
- ✅ System tray integration via NSStatusBar
- ✅ Metal GPU acceleration for Apple Silicon (M1/M2/M3/M4)
- ✅ Hotkey support with proper threading model
- ✅ Comprehensive testing and documentation
- ✅ CI/CD pipeline for automated builds

---

## Task Dependency Graph

```
T-033: Platform-conditional dependencies (Foundation)
   ↓
T-034: Build system for macOS (Foundation)
   ↓
   ├─→ T-035: macOS text insertion adapter (Critical)
   │      ↓
   ├─→ T-036: Metal GPU acceleration (High Priority)
   │      ↓
   ├─→ T-037: macOS system tray adapter (Medium Priority)
   │      ↓
   └─→ T-038: Hotkey threading fix (High Priority)
          ↓
       T-039: macOS integration tests (Medium Priority)
          ↓
       T-040: macOS documentation (Medium Priority)
          ↓
       T-041: CI/CD pipeline (Low Priority)
```

---

## Task Breakdown

### Phase 1: Foundation (Week 1)

#### T-033: Add Platform-Conditional Dependencies
**Effort:** 2-3 hours
**Priority:** High
**Status:** Available

Move Linux-specific dependencies (x11rb, input-linux, ksni) to platform-conditional sections. Add macOS dependencies (cocoa, core-graphics, core-foundation).

**Deliverables:**
- Updated Cargo.toml with platform-specific dependencies
- Cross-platform dependencies verified
- Both platforms build successfully

**Success Criteria:**
- `cargo check` works on both Linux and macOS
- No Linux dependencies pulled on macOS builds

---

#### T-034: Update Build System for macOS
**Effort:** 2-3 hours
**Priority:** High
**Status:** Available

Make build.rs platform-aware. Add macOS framework detection (CoreAudio, CoreGraphics, AppKit). Detect Apple Silicon vs Intel for GPU recommendations.

**Deliverables:**
- Platform-conditional build checks
- macOS framework detection
- Apple Silicon detection
- Helpful error messages

**Success Criteria:**
- Build succeeds on macOS with appropriate checks
- Apple Silicon Macs see Metal recommendations
- Intel Macs see CPU recommendations

---

#### T-035: Implement macOS Text Insertion Adapter
**Effort:** 1-2 days (8-16 hours)
**Priority:** Critical
**Status:** Available

Create MacOSTextAdapter using CGEvent API for keyboard simulation and Accessibility API for window detection. Implement permission checks and fallbacks.

**Deliverables:**
- `src/adapters/text/macos_adapter.rs`
- `src/adapters/text/macos_accessibility.rs`
- Factory pattern for platform selection
- Accessibility permission flow

**Success Criteria:**
- Text insertion works via CGEvent
- Fallback to enigo when CGEvent fails
- Permission checks function correctly
- Window detection returns focused window info

---

### Phase 2: Feature Parity (Week 2)

#### T-036: Add Metal GPU Acceleration Support
**Effort:** 4-6 hours
**Priority:** High
**Status:** Available

Add `metal` feature flag alongside `cuda`. Make Metal default on macOS. Update Whisper adapter to use Metal backend on Apple Silicon.

**Deliverables:**
- Metal feature flag in Cargo.toml
- Device detection and selection
- `hush device-info` command
- Platform-specific defaults

**Success Criteria:**
- Metal GPU works on Apple Silicon (~120ms latency)
- CPU fallback works on Intel Macs
- Device detection correctly identifies GPU type
- 6-8x performance improvement on M1/M2

---

#### T-037: Implement macOS System Tray Adapter
**Effort:** 4-6 hours
**Priority:** Medium
**Status:** Available

Create MacOSTrayAdapter using `tray-icon` crate (or NSStatusBar directly). Implement menu bar icon, menu items, and state updates.

**Deliverables:**
- `src/tray/adapters/macos_tray_adapter.rs`
- Icon assets (idle, recording, processing, error)
- Menu creation and event handling
- Factory pattern for platform selection

**Success Criteria:**
- Tray icon appears in macOS menu bar
- Menu items display and respond to clicks
- Icon updates based on application state
- Keyboard shortcuts work (Cmd+Shift+V)

---

#### T-038: Fix Hotkey Threading for macOS
**Effort:** 3-4 hours
**Priority:** High
**Status:** Available

Ensure GlobalHotKeyManager is created on main thread (macOS requirement). Update event loop and threading model to respect platform constraints.

**Deliverables:**
- Main thread check for HotkeyAdapter
- Platform-specific initialization order
- Updated main.rs for macOS threading

**Success Criteria:**
- Hotkey registration succeeds on macOS
- Events trigger correctly
- No panics or hangs
- Linux functionality unchanged

---

### Phase 3: Quality & Documentation (Week 3-4)

#### T-039: Add macOS Integration Tests
**Effort:** 4-6 hours
**Priority:** Medium
**Status:** Available

Create macOS-specific test suite covering text insertion, tray, Metal GPU, hotkeys, and permissions. Add manual test script and CI support.

**Deliverables:**
- `tests/macos_integration.rs`
- `scripts/test-macos.sh`
- `.github/workflows/macos-tests.yml`
- `hush check-permissions` command

**Success Criteria:**
- Tests compile and run on macOS
- Permission checks work
- CI workflow runs on GitHub Actions
- Manual test script validates all features

---

#### T-040: Create macOS Documentation
**Effort:** 3-4 hours
**Priority:** Medium
**Status:** Available

Write comprehensive macOS documentation including installation, permissions, troubleshooting, and performance tuning.

**Deliverables:**
- `docs/macos/INSTALL.md`
- `docs/macos/PERMISSIONS.md`
- `docs/macos/TROUBLESHOOTING.md`
- Updated README.md

**Success Criteria:**
- Installation guide is clear and complete
- Permission setup is well-documented
- Common issues have solutions
- macOS section added to main README

---

#### T-041: Set Up macOS CI/CD Pipeline
**Effort:** 4-6 hours
**Priority:** Low-Medium
**Status:** Available

Create GitHub Actions workflow for automated macOS builds, tests, and releases. Support both Apple Silicon (macos-14) and Intel (macos-13) runners.

**Deliverables:**
- `.github/workflows/macos-ci.yml`
- `entitlements.plist` (for code signing)
- `docs/RELEASING.md`
- Release automation

**Success Criteria:**
- Tests run on both architectures in CI
- Release binaries built automatically
- arm64, x64, and universal binaries created
- GitHub Releases automated

---

## Effort Estimation

| Phase | Tasks | Hours | Days | Priority |
|-------|-------|-------|------|----------|
| **Phase 1: Foundation** | T-033 to T-035 | 12-21 | 1.5-3 | Critical |
| **Phase 2: Feature Parity** | T-036 to T-038 | 11-16 | 1.5-2 | High |
| **Phase 3: Quality** | T-039 to T-041 | 11-16 | 1.5-2 | Medium |
| **Total** | 9 tasks | **34-53 hours** | **4-7 days** | - |

**With buffer:** 2-4 weeks for production-ready macOS support

---

## Critical Path

**Must complete in order:**
1. T-033 (dependencies) → Foundation for all other work
2. T-034 (build system) → Enables platform detection
3. T-035 (text adapter) → Core functionality, highest priority
4. T-038 (hotkey threading) → Required for usability
5. T-036 (Metal GPU) → Performance differentiator

**Can parallelize:**
- T-037 (tray) - Independent of text/hotkey
- T-039 (tests) - Can start after T-035/T-036/T-037 complete
- T-040 (docs) - Can start after implementation complete
- T-041 (CI/CD) - Infrastructure task, can be done anytime

---

## Success Metrics

### Technical Metrics
- ✅ All tasks complete with passing tests
- ✅ macOS builds succeed in CI
- ✅ Metal GPU achieves ~120ms latency on Apple Silicon
- ✅ Text insertion works in 95%+ of applications
- ✅ Zero regressions on Linux

### User Experience Metrics
- ✅ Installation takes <5 minutes
- ✅ Permission setup is clear and straightforward
- ✅ Hotkey registration is reliable
- ✅ Transcription accuracy matches Linux version
- ✅ Documentation covers common questions

### Quality Metrics
- ✅ All integration tests pass
- ✅ No clippy warnings
- ✅ Code coverage >70% for new adapters
- ✅ Manual testing checklist complete
- ✅ User feedback incorporated

---

## Risk Mitigation

### High-Risk Areas

**1. Accessibility Permissions**
- **Risk:** Users struggle with permission setup
- **Mitigation:** Clear documentation, `check-permissions` command, helpful error messages

**2. Hotkey Threading**
- **Risk:** Main thread requirement breaks architecture
- **Mitigation:** Conditional initialization, fallback strategies, thorough testing

**3. Metal GPU Compatibility**
- **Risk:** Metal backend has issues on some Macs
- **Mitigation:** Robust CPU fallback, feature flags, device detection

### Medium-Risk Areas

**4. Text Insertion Reliability**
- **Risk:** CGEvent doesn't work in all apps
- **Mitigation:** Multiple fallback methods (enigo, clipboard)

**5. CI/CD Costs**
- **Risk:** macOS CI minutes expensive
- **Mitigation:** Use public repo, optimize build caching, limit to tags

---

## Testing Strategy

### Unit Tests
- Platform-specific adapter logic
- Permission checking functions
- Device detection
- Character-to-keycode mapping

### Integration Tests
- End-to-end text insertion
- Hotkey registration and triggering
- Tray icon updates
- Metal GPU transcription

### Manual Testing
- Real macOS machines (Apple Silicon + Intel)
- Various applications (TextEdit, browser, terminal, IDE)
- Permission flows (grant, deny, revoke)
- Performance benchmarking

### CI Testing
- GitHub Actions on macos-14 (Apple Silicon)
- GitHub Actions on macos-13 (Intel)
- Automated release builds
- Smoke tests (no permissions required)

---

## Post-MVP Enhancements

### Additional Features (Not in Scope)
- Universal2 binary (arm64 + x86_64)
- Code signing and notarization
- Homebrew formula
- macOS-specific UI improvements
- Wayland support on Linux (separate effort)
- Windows support (separate major effort)

### Performance Optimizations
- Streaming transcription (word-by-word)
- Model quantization for smaller size
- Audio preprocessing optimizations
- Lazy model loading

### UX Improvements
- In-app permission management
- Better error messaging
- Onboarding wizard
- Keyboard shortcut customization UI

---

## Resources

### Documentation
- [macOS Accessibility API](https://developer.apple.com/documentation/applicationservices/accessibility)
- [CoreGraphics CGEvent](https://developer.apple.com/documentation/coregraphics/cgevent)
- [Metal Performance](https://developer.apple.com/metal/)
- [Candle Framework](https://github.com/huggingface/candle)

### Crates Used
- `cocoa` - macOS Cocoa/AppKit bindings
- `core-graphics` - CoreGraphics framework bindings
- `core-foundation` - CoreFoundation bindings
- `tray-icon` - Cross-platform system tray
- `global-hotkey` - Cross-platform hotkeys
- `cpal` - Cross-platform audio (CoreAudio on macOS)
- `candle-*` - ML framework with Metal support

### Tools
- GitHub Actions (macOS runners)
- cargo, clippy, rustfmt
- Xcode Command Line Tools
- lipo (universal binary creation)
- codesign (optional, for signing)

---

## Notes

- All tasks follow the agent protocol in CLAUDE.md
- Each task is atomic and can be completed independently
- Linux functionality must remain unchanged
- All code must pass clippy and fmt checks
- Documentation must be tested on real macOS machines
- CI/CD optional for MVP, recommended for production

---

## Claiming a Task

```bash
# Pick a task
TASK="T-033-add-platform-conditional-dependencies.md"

# Claim it
mv ".ai/tasks/available/${TASK}" ".ai/tasks/claimed/${TASK%.md}-${AGENT_ID}.md"

# Create branch
git checkout -b "agent/${AGENT_ID}/T-033"

# Start working!
```

---

**Last Updated:** 2025-11-19
**Created By:** AI Assessment
**Review Status:** Pending human review
