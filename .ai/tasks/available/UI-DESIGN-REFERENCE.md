# UI Design Reference - Quick Start

**Purpose:** Quick reference for implementing Hush's polished UI
**Design Goal:** Match WisprFlow's level of polish

---

## 📚 Design Documentation

### 1. **UI Design Specification** (`.ai/knowledge/ui-design-specification.md`)
**Comprehensive design system covering:**
- Color palette (sage green #4CAF8C, cream backgrounds)
- Typography (Inter font, type scale, weights)
- Spacing system (4px base grid)
- Component specifications (buttons, inputs, toggles, sliders)
- Animation guidelines (timing, easing functions)
- Accessibility requirements (WCAG AA, keyboard nav, screen readers)
- Dark mode implementation
- Platform-specific considerations

**Use this for:** Exact specs, measurements, colors, and implementation details

### 2. **UI Mockups** (`.ai/knowledge/ui-mockups.md`)
**Visual reference with ASCII mockups:**
- Overlay states (idle, recording, processing, success, error)
- Settings window layouts (all 5 tabs)
- Model management UI
- Audio device picker
- Hotkey configurator
- First-run wizard (7 steps)
- System tray menu
- Component library examples

**Use this for:** Visual layout, component composition, user flows

### 3. **UI Expansion Roadmap** (`.ai/tasks/available/UI-EXPANSION-ROADMAP.md`)
**Implementation strategy:**
- Task breakdown and dependencies
- Phase-by-phase implementation plan
- MVP definition (8-12 days minimum)
- Success metrics

**Use this for:** Planning, task ordering, timeline estimates

---

## 🎨 Design Principles (from WisprFlow research)

### 1. **Intentional Invisibility**
- UI should "disappear into muscle memory"
- Present but not obtrusive
- Small, persistent, subtle

### 2. **Warmth & Clarity**
- Gentle curves, generous spacing, soft corners
- Warm colors (sage green, cream, lavender accents)
- Avoid cold, clinical AI aesthetic

### 3. **Speed & Responsiveness**
- Instant visual feedback
- 60 FPS animations minimum
- Smooth transitions (150-400ms)

### 4. **Privacy-First Visual Language**
- Emphasize local processing
- No cloud/upload indicators
- Reinforce on-device nature

---

## 🎯 Quick Reference

### Key Colors
```
Primary Accent:    #4CAF8C  (sage green)
Background Light:  #FAF8F5  (cream)
Background Dark:   #1C1C1E  (soft charcoal)
Success:           #4CAF8C  (same as primary)
Warning:           #F5A623  (amber)
Error:             #E74C3C  (coral red)
```

### Typography
```
Font Family:  Inter (sans-serif)
Heading:      16-32px, Semibold (600)
Body:         13-14px, Regular (400)
Small:        12px, Regular (400)
Overlay:      10px, Medium (500)
```

### Spacing (4px grid)
```
Tight:    4px
Compact:  8px
Default:  12px
Relaxed:  16px
Spacious: 24px
```

### Component Sizes
```
Buttons:     36px height, 8px radius
Inputs:      36px height, 6px radius
Toggles:     44×24px, 12px radius
Overlay:     60×4px idle, 80×24px recording
```

### Animations
```
Fast:    150ms (hover, click)
Medium:  250ms (modal, overlay)
Slow:    400ms (page transition)
Easing:  cubic-bezier(0.4, 0, 0.2, 1)
```

---

## 📐 Component Quick Reference

### Overlay States
```
Idle:       60×4px, subtle glow, bottom-center
Recording:  80×24px, green (#4CAF8C), waveform animation
Processing: 80×24px, gray, "processing..." text
Success:    80×24px, green, "done ✓" (1.5s)
Error:      80×24px, red, "error" + shake animation
```

### Settings Window
```
Size:       800×600px (resizable, min 600×400)
Tabs:       General, Models, Audio, Hotkeys, Advanced
Tab Height: 40px
Content:    600px max-width, 24px padding, centered
```

### Buttons
```
Primary:     #4CAF8C background, white text
Secondary:   Transparent, #4CAF8C border & text
Destructive: #E74C3C background, white text
All:         36px height, 8px radius, 0-20px padding
```

### Form Controls
```
Text Input:  36px height, 6px radius, 12px padding
Dropdown:    36px height, chevron right-aligned
Toggle:      44×24px, smooth 200ms transition
Slider:      4px track, 16px thumb
Radio:       20px diameter, 2px border, 10px dot
Checkbox:    20px square, 4px radius
```

---

## 🚀 Getting Started

### For Developers Implementing UI:

1. **Read the full specs:**
   - Start with `.ai/knowledge/ui-design-specification.md`
   - Review `.ai/knowledge/ui-mockups.md` for visual reference
   - Check task-specific requirements in task files

2. **Set up theme:**
   - Apply colors from specification
   - Configure spacing (4px grid)
   - Set up typography (Inter font)

3. **Build components:**
   - Follow component specs exactly
   - Use provided measurements
   - Test animations at 60 FPS

4. **Test thoroughly:**
   - Light and dark mode
   - All interaction states
   - Keyboard navigation
   - Screen reader compatibility

### For Designers Reviewing:

1. **Check against specs:**
   - Colors match palette exactly
   - Spacing follows 4px grid
   - Typography uses correct scale
   - Animations use correct timing

2. **Verify polish:**
   - All hover states present
   - Focus indicators visible
   - Transitions smooth
   - Micro-interactions delightful

3. **Test accessibility:**
   - WCAG AA contrast met
   - Keyboard nav works
   - Screen reader friendly

---

## 📋 Implementation Checklist

Before marking any UI task complete:

### Visual Polish
- [ ] Colors match specification exactly
- [ ] Typography is consistent (Inter font, correct sizes)
- [ ] Spacing follows 4px grid
- [ ] Border radius matches specs
- [ ] Shadows applied correctly

### Interaction
- [ ] Hover states work on all interactive elements
- [ ] Focus indicators visible and styled correctly
- [ ] Click/active states provide feedback
- [ ] Animations smooth at 60 FPS
- [ ] Transitions use correct timing (150-400ms)

### Accessibility
- [ ] WCAG AA contrast ratios met (4.5:1 minimum)
- [ ] Keyboard navigation works completely
- [ ] Focus order is logical
- [ ] Screen reader tested
- [ ] Reduced motion respected

### Responsiveness
- [ ] Works at minimum size (600×400px)
- [ ] Scales appropriately to larger sizes
- [ ] High DPI / Retina tested
- [ ] Dark mode works correctly
- [ ] No visual glitches or overlaps

### Performance
- [ ] No frame drops (60 FPS maintained)
- [ ] Fast startup / open time
- [ ] Smooth animations
- [ ] No memory leaks
- [ ] Efficient redraws

---

## 🎨 Figma / Design Tools

If creating Figma mockups (recommended for complex flows):

### Color Styles
```
Primary/Sage:        #4CAF8C
Secondary/Lavender:  #B4A5D9
Warning/Amber:       #F5A623
Error/Coral:         #E74C3C
BG Light/Cream:      #FAF8F5
BG Dark/Charcoal:    #1C1C1E
Text Primary:        #1A1A1A / #FAFAFA
Text Secondary:      #666666 / #B0B0B0
```

### Text Styles
```
Display Large:    Inter Semibold 32px / 40px
Display Medium:   Inter Semibold 24px / 32px
Heading Large:    Inter Semibold 20px / 28px
Heading Medium:   Inter Semibold 16px / 24px
Body Large:       Inter Regular 14px / 22px
Body Medium:      Inter Regular 13px / 20px
Body Small:       Inter Regular 12px / 18px
Tiny:             Inter Medium 10px / 16px
```

### Effect Styles
```
Shadow Small:   0px 2px 4px rgba(0,0,0,0.1)
Shadow Medium:  0px 4px 12px rgba(0,0,0,0.2)
Shadow Large:   0px 12px 48px rgba(0,0,0,0.3)
Glow Green:     0px 6px 20px rgba(76,175,140,0.4)
Focus Ring:     0px 0px 0px 3px rgba(76,175,140,0.2)
```

### Component Library
Create reusable Figma components for:
- Buttons (primary, secondary, destructive)
- Form controls (input, dropdown, toggle, slider)
- Cards (model card, device card)
- Modals (standard sizes)
- Tabs (active, inactive, hover states)

---

## 🔗 Related Documents

- **Framework Research:** `.ai/knowledge/rust-ui-frameworks-research.md`
- **Coding Conventions:** `.ai/knowledge/conventions.md`
- **Architecture:** `.ai/knowledge/architecture.md`
- **Task Roadmap:** `.ai/tasks/available/UI-EXPANSION-ROADMAP.md`
- **Individual Tasks:** `.ai/tasks/available/T-027-*.md` through `T-035-*.md`

---

## 💡 Tips

### For egui Implementation

**Apply theme once at startup:**
```rust
fn configure_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    // Colors
    style.visuals.window_fill = Color32::from_rgb(250, 248, 245);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(255, 255, 255);

    // Spacing
    style.spacing.item_spacing = egui::vec2(12.0, 12.0);
    style.spacing.button_padding = egui::vec2(20.0, 8.0);

    // Rounding
    style.visuals.widgets.inactive.rounding = egui::Rounding::same(8.0);

    ctx.set_style(style);
}
```

**Responsive to system theme:**
```rust
// Detect dark mode
if ctx.style().visuals.dark_mode {
    // Apply dark theme colors
} else {
    // Apply light theme colors
}
```

**Smooth animations:**
```rust
ui.ctx().request_repaint_after(Duration::from_millis(16)); // 60 FPS
```

### For Testing

**Visual regression testing:**
- Take screenshots of all states
- Compare against design specs
- Check at different sizes

**Animation testing:**
- Record at 60 FPS
- Verify timing matches specs
- Check for jank or stuttering

**Accessibility testing:**
- Use screen reader (Orca on Linux)
- Navigate with keyboard only
- Test with color blindness simulator

---

**Questions?** Refer to the full design specification or ask for clarification.

**Ready to implement?** Start with T-027 (Enhanced Settings Window) 🚀
