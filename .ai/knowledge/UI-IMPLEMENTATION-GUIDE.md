# UI Implementation Guide - Quick Reference

**Purpose:** Focused guide for implementing Hush's UI expansion
**For:** AI agents and developers implementing tasks T-027 through T-035

---

## 📚 Essential Documentation

### 1. **Design System** → `ui-design-specification.md`
**Use for:** Exact colors, typography, spacing, component specs

**Key sections:**
- Color palette (sage green #4CAF8C, cream backgrounds)
- Typography (Inter font, 10-32px scale)
- Spacing (4px base grid)
- Component specifications (50+ components)
- Animation timing (150-400ms transitions)
- Accessibility (WCAG AA)

### 2. **Visual Layouts** → `ui-mockups.md`
**Use for:** Screen layouts, component composition, user flows

**Key sections:**
- Overlay states (idle, recording, processing)
- Settings window (all 5 tabs)
- First-run wizard (7 steps)
- Component library examples

### 3. **Implementation Plan** → `UI-EXPANSION-ROADMAP.md`
**Use for:** Task order, dependencies, timeline

**Key sections:**
- Phase breakdown (Foundation → Core → Onboarding → Polish)
- Task dependencies
- MVP definition (8-12 days minimum)
- Success metrics

---

## 🎯 Core Design Principles

### From WisprFlow Research:

1. **Intentional Invisibility** - UI disappears into muscle memory
2. **Warmth & Clarity** - Gentle curves, sage green, cream backgrounds
3. **Speed & Responsiveness** - 60 FPS, instant feedback
4. **Privacy-First** - Emphasize local processing visually

---

## 🎨 Quick Design Reference

### Colors
```
Primary:    #4CAF8C  (sage green)
Background: #FAF8F5  (cream light) / #1C1C1E (dark)
Success:    #4CAF8C  (same as primary)
Warning:    #F5A623  (amber)
Error:      #E74C3C  (coral red)
Text:       #1A1A1A  (light) / #FAFAFA (dark)
Secondary:  #666666  (light) / #B0B0B0 (dark)
```

### Typography
```
Font:      Inter
Heading:   16-32px, Semibold (600)
Body:      13-14px, Regular (400)
Small:     12px, Regular (400)
Overlay:   10px, Medium (500)
```

### Spacing (4px grid)
```
4px  = Tiny (icon padding)
8px  = Compact (button padding)
12px = Default (element spacing)
16px = Relaxed (section spacing)
24px = Spacious (major sections)
32px = Large (window margins)
```

### Components
```
Buttons:      36px height, 8px radius
Inputs:       36px height, 6px radius
Toggles:      44×24px, 12px radius
Overlay idle: 60×4px
Overlay rec:  80×24px
```

### Animations
```
Fast:    150ms (hover, click)
Medium:  250ms (modal, overlay)
Slow:    400ms (transition)
Easing:  cubic-bezier(0.4, 0, 0.2, 1)
```

---

## 🔑 Key Implementation Tips

### 1. **Polish vs Perfection**

**Ship at 85% polish:**
- ✅ All functionality works
- ✅ No visual glitches
- ✅ Passes accessibility tests
- ✅ Consistent with design system
- ⚠️ Minor spacing tweaks can come later

**Don't over-polish:**
- Perfect pixel alignment (nice-to-have)
- Custom animations everywhere (unnecessary)
- Every edge case handled (iterate)

### 2. **Visual Hierarchy (Squint Test)**

Blur your eyes - important elements should stand out:
- Tab bar = distinct horizontal band
- Section headers = darker blocks
- Buttons = solid shapes at bottom
- Content = lighter in middle

If everything looks the same when blurred → needs more contrast.

### 3. **Spacing That Breathes**

Beyond the 4px grid:
- **Related items**: 8-12px (tight grouping)
- **Unrelated items**: 24-32px (breathing room)
- **Major sections**: 48px+ (clear separation)

White space is a feature, not wasted space.

### 4. **Animation Feel**

**When to animate:**
- ✅ Modal open/close (draws attention)
- ✅ State transitions (overlay idle → recording)
- ✅ Success feedback (checkmark)
- ✅ Button press (scale 98%)

**When NOT to animate:**
- ❌ Text color changes (distracting)
- ❌ Every hover state (overwhelming)
- ❌ Static labels (unnecessary)

### 5. **egui-Specific**

**Cache expensive operations:**
```rust
// ✗ BAD (recomputes every frame)
ui.label(format_text(&data));

// ✓ GOOD (cache result)
let cached = use_memo(|| format_text(&data));
ui.label(cached);
```

**Request repaints appropriately:**
```rust
// Animating (waveform)
ctx.request_repaint_after(Duration::from_millis(50)); // 20 FPS

// Static (settings window)
ctx.request_repaint_after(Duration::from_secs(1)); // 1 FPS
```

---

## 📋 Implementation Checklist

Before marking any task complete:

### Must Have:
- [ ] Functionality works correctly
- [ ] Colors match specification
- [ ] Typography is consistent
- [ ] Spacing follows 4px grid
- [ ] Animations are smooth (60 FPS)
- [ ] Keyboard navigation works
- [ ] No visual glitches
- [ ] cargo check passes
- [ ] cargo clippy passes
- [ ] cargo test passes

### Should Have (85% polish):
- [ ] Hover states on interactive elements
- [ ] Focus indicators visible
- [ ] Loading states implemented
- [ ] Error states clear
- [ ] Empty states helpful
- [ ] Responsive to resizing

### Nice to Have (final 15%):
- [ ] Perfect pixel alignment
- [ ] Custom micro-animations
- [ ] Advanced accessibility
- [ ] All edge cases handled

---

## 🎯 Task-Specific Guidance

### T-027 (Enhanced Settings Window)
**Focus:** Foundation, structure, basic theming
**Polish level:** 80% is fine - this enables other tasks
**Key:** Tabbed interface, config persistence

### T-028 (Model Management UI)
**Focus:** Clear information display, status indicators
**Polish level:** 85% - users interact with this frequently
**Key:** Model cards, status badges, selection clarity

### T-029 (Model Download UI)
**Focus:** Progress feedback, clear states
**Polish level:** 90% - users wait and watch this
**Key:** Progress bars, speed display, error handling

### T-030 (Audio Device Picker)
**Focus:** Real-time feedback, live preview
**Polish level:** 90% - critical for setup success
**Key:** Level meter, device selection, test recording

### T-031 (Hotkey Configurator)
**Focus:** Clear recording, conflict detection
**Polish level:** 85% - used once during setup
**Key:** Visual feedback, validation, error messages

### T-032 (LLM Configuration)
**Focus:** Security (API key), clear states
**Polish level:** 80% - optional feature
**Key:** Password field, test connection, privacy notice

### T-033 (First-Run Wizard)
**Focus:** User confidence, clear progress
**Polish level:** 95% - first impression matters!
**Key:** Step progression, success states, helpful copy

### T-034 (System Tray)
**Focus:** Quick access, clear status
**Polish level:** 80% - utility over beauty
**Key:** Menu structure, status indicators

### T-035 (History Viewer)
**Focus:** Information clarity, search/filter
**Polish level:** 75% - optional feature
**Key:** Table layout, search, export

---

## 🚫 Common Pitfalls

### 1. Over-Engineering
❌ Don't: Build complex animation system
✅ Do: Use simple transitions (150-400ms)

### 2. Ignoring egui Limitations
❌ Don't: Try to replicate web effects exactly
✅ Do: Work within egui's strengths

### 3. Inconsistent Spacing
❌ Don't: Use random values (7px, 15px, 23px)
✅ Do: Follow 4px grid (8px, 16px, 24px)

### 4. Poor Contrast
❌ Don't: Use #CCCCCC text on #FFFFFF background
✅ Do: Meet WCAG AA (4.5:1 minimum)

### 5. Forgetting States
❌ Don't: Only design/implement idle state
✅ Do: Consider hover, active, disabled, loading, error

---

## 🎓 Learning from Loveable

**Why Loveable achieves 95% polish:**
1. React + Tailwind (huge AI training data)
2. Component libraries (MUI, Ant Design = pre-built polish)
3. Instant preview (iterate in seconds)

**Why Hush is harder but achievable:**
1. egui (limited AI training data)
2. No component library (build from scratch)
3. Compile time (iterate in minutes)

**Solution:**
- Comprehensive specs (our "component library")
- Detailed mockups (visual reference)
- Iterative refinement (multiple passes)
- **Target: 85% AI polish → 15% human refinement**

**This is normal and professional!**

---

## 🚀 Getting Started

### For First Implementation (T-027):

1. **Read design spec** - Familiarize with colors, typography, spacing
2. **Review mockup** - Understand layout structure
3. **Start with structure** - Get tabs, layout working
4. **Add styling** - Apply colors, spacing, typography
5. **Test thoroughly** - All states, keyboard nav, resize
6. **Iterate** - Refine based on testing

### For AI Agents:

**When implementing:**
1. Reference `ui-design-specification.md` for exact specs
2. Reference `ui-mockups.md` for layout structure
3. Follow the checklist before marking complete
4. Don't aim for perfection - 85% is the target

**When stuck:**
1. Check if spec covers it (usually does)
2. Look at similar components
3. Follow established patterns
4. Ask for clarification if truly ambiguous

---

## 📖 Documentation Map

```
.ai/knowledge/
├── ui-design-specification.md    ← Design system (colors, components)
├── ui-mockups.md                  ← Visual layouts
├── ui-polish-guide.md             ← Polish details (when needed)
└── rust-ui-frameworks-research.md ← Why egui (reference)

.ai/tasks/available/
├── UI-EXPANSION-ROADMAP.md        ← Implementation strategy
├── T-027-enhanced-settings-window.md
├── T-028-model-management-ui.md
├── T-029-model-download-ui.md
├── T-030-audio-device-picker.md
├── T-031-hotkey-configurator.md
├── T-032-llm-configuration-ui.md
├── T-033-first-run-setup-wizard.md
├── T-034-system-tray-enhancements.md
└── T-035-history-viewer.md
```

**Start here:** `UI-EXPANSION-ROADMAP.md`
**Then:** Task files (T-027 first)
**Reference:** Design spec + mockups as needed

---

## ✅ Success Criteria

**You'll know the UI expansion is successful when:**

1. **Functional** - All features work correctly
2. **Consistent** - Design system applied throughout
3. **Accessible** - Keyboard nav, screen reader friendly
4. **Polished** - 85%+ "feels good" test
5. **Performant** - 60 FPS, no lag
6. **Tested** - All states, edge cases covered

**Not required:**
- Perfect pixel alignment
- Custom animations everywhere
- Every edge case handled
- 100% polish (impossible)

---

**Ready to implement? Start with T-027 (Enhanced Settings Window)** 🚀

**Questions?** Check design spec first, then ask for clarification.
