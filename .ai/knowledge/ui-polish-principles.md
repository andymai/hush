# UI Polish Principles - The "Feel" Guide

**Purpose:** Bridge the gap between specs and "feels polished"
**When to use:** After basic implementation, before marking task complete
**Complements:** `ui-design-specification.md` (the WHAT) with the WHY and HOW

---

## The 80/20 Rule

**Technical specs get you 80% there.**
**This guide covers the final 20% - the subtle details.**

---

## 1. Visual Hierarchy (The Squint Test)

**Test:** Blur your eyes or squint at the UI.

**Good hierarchy (clear even when blurred):**
- Important elements stand out (darker, larger, more contrast)
- Clear focal points (eye knows where to look)
- Balanced visual weight (not all heavy on one side)

**Example - Settings Window:**
```
Blurred view should show:
━━━━━━━━━━━━━ (Tab bar - distinct band)
                (White space)
████ (Section header - dark block)
░░░░░░░ (Content - lighter)
                (White space)
▓▓▓▓ (Button - solid shape)
```

**Bad hierarchy:**
- Everything same visual weight (all medium gray)
- No clear sections
- Buttons blend into background

---

## 2. Spacing That Breathes

**Beyond the 4px grid, understand relationships:**

```
Related items:        8-12px apart  (visually grouped)
Unrelated items:      24-32px apart (clearly separate)
Major sections:       48px+ apart   (obvious division)
```

**Example:**
```
✓ GOOD:
Theme:           [Dark  ▼]
     ↕ 16px
Audio Device:    [USB Mic  ▼]

✗ BAD (too tight):
Theme:           [Dark  ▼]
     ↕ 4px
Audio Device:    [USB Mic  ▼]

✗ BAD (too loose):
Theme:           [Dark  ▼]
     ↕ 48px
Audio Device:    [USB Mic  ▼]
```

**Key insight:** White space is a feature that makes content easier to scan.

---

## 3. Optical vs Grid Alignment

**Grid alignment:** Mathematical (everything aligns to 4px grid)
**Optical alignment:** Perceptual (looks aligned even if not exact)

**When to break the grid:**
- Icons next to text: Adjust 1-2px for visual centering
- Rounded shapes: May need offset for perceived center
- Different font weights: Heavier text may need less space

**Example:**
```
✗ Grid aligned (feels off):
🎤  Push to Talk
 ↑ Icon feels too high

✓ Optically aligned:
🎤  Push to Talk
 ↑ Shifted down 2px, feels centered
```

---

## 4. Color & Contrast Subtlety

### Perceived Brightness
Same color looks different on different backgrounds:
- #4CAF8C on dark bg → Brighter
- #4CAF8C on light bg → Less bright
- Adjust opacity, not just color

### Shadow Creates Depth
```
Light elevation:   0px 2px 4px rgba(0,0,0,0.1)
Medium elevation:  0px 8px 24px rgba(0,0,0,0.15)
High elevation:    0px 20px 60px rgba(0,0,0,0.3)

Rule: Higher = darker & more spread
```

**Common mistakes:**
- Shadow too dark (looks harsh)
- Shadow too light (no depth)
- All shadows same (no hierarchy)

---

## 5. Animation Nuance

### When to Animate
✅ **Do animate:**
- Modal open/close (draws attention)
- State transitions (overlay idle → recording)
- Success feedback (checkmark bounce)
- Button press (subtle scale)

❌ **Don't animate:**
- Text color changes (distracting)
- Every hover state (overwhelming)
- Static labels (unnecessary)

### Easing Functions

**Standard (most animations):**
```
cubic-bezier(0.4, 0, 0.2, 1)
Start slow, end slow - feels natural
```

**Bounce (success states):**
```
cubic-bezier(0.34, 1.56, 0.64, 1)
Overshoots then settles - feels playful
```

### Timing
- Fast actions: 100-150ms (button press)
- Medium: 200-250ms (overlay transition)
- Slow: 300-400ms (page transition)
- Too fast: Jarring
- Too slow: Sluggish

---

## 6. State Management

### Every Action Needs Feedback

**Immediate (< 100ms):**
- Button press animation
- Visual state change
- Cursor change

**Loading (> 500ms):**
- Spinner or progress indicator
- Button disabled
- Status text

**Complete:**
- Success state shown
- Button re-enabled
- Result displayed

**Error:**
- Clear error message
- Actionable suggestion
- Retry option

### Empty States

Don't show blank space - provide:
- Clear message why it's empty
- Illustration or icon (optional)
- Primary action to fix it
- Help link if needed

```
✗ BAD:
┌─────────────────┐
│                 │  ← Confusing
└─────────────────┘

✓ GOOD:
┌─────────────────┐
│ No models yet   │
│ [Download Base] │
└─────────────────┘
```

---

## 7. egui-Specific Tips

### Cache Computations
```rust
// ✗ BAD (recomputes every frame)
ui.label(format_expensive(&data));

// ✓ GOOD (cache result)
let cached = use_memo(|| format_expensive(&data));
ui.label(cached);
```

### Request Repaints Appropriately
```rust
// Animating
ctx.request_repaint_after(Duration::from_millis(50)); // 20 FPS

// Static
ctx.request_repaint_after(Duration::from_secs(1)); // 1 FPS
```

### Custom Rendering for Precision
```rust
// Use painter for pixel-perfect control
let painter = ui.painter();
painter.rect_filled(rect, Rounding::same(8.0), color);
```

---

## 8. Testing for Polish

### The "Feels Good" Test
Use the UI yourself:
- Does clicking feel responsive?
- Do animations feel smooth?
- Is text readable for 5+ minutes?
- Do you notice glitches?
- Does it feel fast?

### Comparison Test
Side-by-side with WisprFlow or similar:
- Spacing feels similar?
- Colors have similar warmth?
- Animations similar smoothness?
- Overall vibe matches?

### Performance Test
```rust
// Log slow frames
if frame_time > Duration::from_millis(16) {
    warn!("Slow frame: {}ms", frame_time.as_millis());
}
```

Target: 60 FPS (16.67ms per frame)

---

## 9. Common Pitfalls

### 1. Spec Rigidity
❌ "Spec says 12px, so I used 12px even though it looks wrong"
✅ Use judgment - try 16px if 12px feels cramped

### 2. Animation Overload
❌ Animating too many things
✅ Animate what matters, make rest instant

### 3. Forgetting Edge Cases
❌ Looks great with perfect data
✅ Test with long text, empty state, error state

### 4. Pixel-Pushing
❌ Spending hours on 1px shifts
✅ Fix functionality first, then polish

---

## 10. When to Ship

### Ship at 85% If:
- ✅ All functionality works
- ✅ No visual glitches
- ✅ Passes accessibility tests
- ✅ Users can complete tasks

### Iterate If:
- ❌ Visual glitches or bugs
- ❌ Animations are janky
- ❌ Users get confused
- ❌ Performance problems

### Polish Later If:
- ⚠️ Minor spacing inconsistencies
- ⚠️ Could use better animations
- ⚠️ Some states could be prettier

**Perfect is the enemy of good.**
Ship at 85%, iterate to 95%, never reach 100%.

---

## Quick Checklist

Before marking task complete:

**Must Have (85% polish):**
- [ ] Functionality works
- [ ] Colors match spec
- [ ] Spacing follows 4px grid
- [ ] Animations smooth (60 FPS)
- [ ] No visual glitches
- [ ] Keyboard nav works
- [ ] States implemented (hover, disabled, error)

**Nice to Have (final 15%):**
- [ ] Perfect pixel alignment
- [ ] Custom micro-animations
- [ ] Every edge case handled

---

**Remember:** This guide covers the judgment calls and subtle details that specs can't capture. Use it when basic implementation is done and you're refining the feel.

**See also:**
- `ui-design-specification.md` - WHAT to build (exact specs)
- `ui-mockups.md` - WHERE things go (layouts)
- `UI-IMPLEMENTATION-GUIDE.md` - Quick reference
