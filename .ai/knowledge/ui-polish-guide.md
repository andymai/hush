# UI Polish Guide - Beyond the Specs

**Purpose:** Bridge the gap between technical specs and "feels polished"
**Audience:** AI agents and developers implementing Hush's UI
**Goal:** Achieve WisprFlow-level polish, not just functional UI

---

## The 20% That Makes It Feel Polished

Technical specs get you 80% there. This guide covers the final 20% - the subtle details that make UI feel premium vs just functional.

---

## 1. Visual Hierarchy & Balance

### The "Squint Test"
Blur your eyes (or apply Gaussian blur in design tool) and look at the UI:
- **Important elements should stand out** even when blurred
- **Visual weight should be balanced** - not all on one side
- **Clear focal points** - eye knows where to look first

**Example - Settings Window:**
```
Blurred view should show:
- Tab bar as distinct horizontal band
- Section headers as darker blocks
- Buttons as solid rectangles at bottom
- Content as lighter gray in middle
```

**Anti-pattern:**
- Everything same visual weight (all medium gray)
- No clear sections when blurred
- Buttons blend into background

### Spacing That "Breathes"
Beyond the 4px grid, some rules of thumb:

**Grouping Related Items:**
- Related items: 8-12px apart (tight grouping)
- Unrelated items: 24-32px apart (breathing room)
- Major sections: 48px+ apart (clear separation)

**Example - Form Row:**
```
✓ GOOD:
Label              [Input           ]
     ↑ 16px gap ↑
Label              [Input           ]

✗ BAD (too tight):
Label              [Input           ]
     ↑ 4px gap ↑
Label              [Input           ]

✗ BAD (too loose):
Label              [Input           ]
     ↑ 32px gap ↑
Label              [Input           ]
```

**White Space is a Feature:**
- Don't fill every pixel
- Generous margins make content easier to read
- Cramped UI feels cheap, spacious UI feels premium

### Optical Alignment vs Grid Alignment

**Grid alignment** is mathematical - everything aligns to the 4px grid.
**Optical alignment** is perceptual - things look aligned even if they're not exactly.

**When to break the grid:**
- Icons next to text: Adjust 1-2px for visual centering
- Rounded corners: Inner content may need offset
- Triangular shapes (play buttons): Shift right slightly for perceived center

**Example - Icon + Text:**
```
✗ Grid aligned (looks off):
[▶]  Play Recording
 ↑ Icon feels too high

✓ Optically aligned:
[▶]  Play Recording
 ↑ Icon shifted down 1-2px, feels centered
```

---

## 2. Color & Contrast Subtlety

### Beyond WCAG Ratios

Meeting 4.5:1 contrast is baseline. Polish comes from:

**1. Perceived Brightness**
- #4CAF8C (sage green) looks brighter on dark bg than light bg
- Adjust opacity, not just color, for different contexts
- Test in actual UI, not just color picker

**2. Temperature Harmony**
- Warm colors together (cream + amber + coral)
- Cool colors together (charcoal + lavender + sage)
- Mix thoughtfully - don't clash

**3. Elevation Through Contrast**
```
Background → Panel → Component → Interactive Element
#FAF8F5  →  #FFFFFF → #F5F5F5  → #4CAF8C

Each layer slightly higher contrast
```

### Shadow & Glow Mastery

**Shadows create depth:**
```
Light elevation (card):     0px 2px 4px rgba(0,0,0,0.1)
Medium elevation (modal):   0px 8px 24px rgba(0,0,0,0.15)
High elevation (overlay):   0px 20px 60px rgba(0,0,0,0.3)

Rule: Higher = darker & more spread
```

**Glows create emphasis:**
```
Hover glow (subtle):  0px 0px 12px rgba(76,175,140,0.2)
Active glow (strong): 0px 0px 24px rgba(76,175,140,0.4)
Recording glow:       0px 6px 20px rgba(76,175,140,0.4)

Rule: Animate glow changes for smooth feel
```

**Common mistakes:**
- Shadow too dark (looks harsh)
- Shadow too light (no depth)
- No shadow variation (everything same elevation)
- Glow too strong (looks radioactive)

---

## 3. Typography Refinement

### Beyond Font Size

**Letter Spacing (Tracking):**
```
Headlines:      -0.02em (slightly tighter, feels premium)
Body text:      0em (normal)
Small caps:     +0.05em (needs breathing room)
Buttons:        +0.01em (slightly wider, more readable)
```

**Line Height (Leading):**
```
Headlines:      1.2-1.3 (tighter, more impact)
Body text:      1.5-1.6 (comfortable reading)
UI labels:      1.4 (compact but readable)
Captions:       1.5 (small text needs more space)
```

**Optical Sizing:**
egui might not support this, but be aware:
- Small text (< 12px): Slightly heavier weight, more spacing
- Large text (> 20px): Can be lighter weight

**Text Color Subtlety:**
```
✗ Pure black on white: #000000 on #FFFFFF (too harsh)
✓ Near-black on cream:  #1A1A1A on #FAF8F5 (softer)

✗ 50% gray: #808080 (looks dull)
✓ Warm gray: #666666 (better for warm theme)
```

### Text Truncation & Overflow

**When text is too long:**
1. **Truncate with ellipsis**: "This is a very long mode..." (most common)
2. **Word wrap**: Multi-line (cards, descriptions)
3. **Scroll**: In defined containers (logs, history)
4. **Scale font down**: Last resort, hurts readability

**Never:**
- Cut off mid-letter
- Overlap with other elements
- Make container overflow

---

## 4. Animation Nuance

### Easing Functions Explained

**cubic-bezier(0.4, 0, 0.2, 1) - "Standard Easing"**
```
     ╱────
   ╱
 ╱
Start slow, end slow - feels natural
Use for: Most animations
```

**cubic-bezier(0, 0, 0.2, 1) - "Deceleration"**
```
────╱
  ╱
╱
Start fast, end slow - feels like settling
Use for: Elements entering screen
```

**cubic-bezier(0.4, 0, 1, 1) - "Acceleration"**
```
     ──────
   ╱
 ╱
Start slow, end fast - feels like launching
Use for: Elements leaving screen
```

**cubic-bezier(0.34, 1.56, 0.64, 1) - "Bounce"**
```
        ╱╲
      ╱    ╲
    ╱        ╲
  ╱            ─
Overshoots then settles - feels playful
Use for: Success states, confirmations
```

### Animation Choreography

**When multiple things animate:**

1. **Stagger delays** - Don't animate everything at once
```
✓ GOOD:
Item 1: 0ms delay
Item 2: 50ms delay
Item 3: 100ms delay
(feels fluid, cascading)

✗ BAD:
All items: 0ms delay
(feels chaotic, jarring)
```

2. **Coordinate direction** - Elements should move cohesively
```
✓ GOOD:
Modal: Fades in + scales up (same direction)

✗ BAD:
Modal: Fades in + slides left (confusing)
```

3. **Respect timing hierarchy** - Fast actions finish before slow ones start
```
✓ GOOD:
Button click (100ms) → Modal open (250ms)

✗ BAD:
Modal open starts before button click finishes
```

### Micro-interactions

**Button press:**
```
1. Hover:   Background color transition (150ms)
2. Press:   Scale down to 98% (100ms, ease-in)
3. Release: Scale up to 100% (150ms, ease-out)
4. Success: Brief green flash (100ms) or checkmark
```

**Toggle switch:**
```
1. Click:     Knob slides to other side (200ms, ease-out)
2. Track:     Color fades from gray to green (200ms)
3. Haptic:    (Not in desktop, but imagine click feel)
```

**Input focus:**
```
1. Focus:   Border color change (150ms)
2. Focus:   Glow fade-in (150ms)
3. Typing:  Cursor blink (standard)
4. Blur:    Border & glow fade-out (150ms)
```

---

## 5. State Management & Feedback

### Every Action Needs Feedback

**User clicks button → Something must happen visibly:**

**Immediate (< 100ms):**
- Button press animation
- Cursor change
- Visual state change

**Loading (100ms - 2s):**
- Spinner appears
- Button disabled
- Status text updates

**Complete (> 2s):**
- Success state shown
- Button re-enabled
- Result displayed

**Error (any time):**
- Clear error message
- Actionable suggestion
- Retry option

### Loading States

**0-500ms: No indicator needed**
- Fast enough to feel instant
- Indicator would flash and annoy

**500ms-2s: Minimal indicator**
```
Spinner or "..." animation
No progress bar (unknown duration)
```

**2s-10s: Progress indicator**
```
Progress bar if progress is known
Spinner + "Processing..." text if not
```

**10s+: Detailed feedback**
```
Progress bar with percentage
Status text: "Downloading... 45%"
Time estimate: "30s remaining"
Cancel button
```

### Empty States

**Don't just show blank space:**
```
✗ BAD:
┌─────────────────────────────┐
│                             │  <- Empty, confusing
│                             │
└─────────────────────────────┘

✓ GOOD:
┌─────────────────────────────┐
│     No models installed     │
│                             │
│   [Download Your First Model]│
└─────────────────────────────┘
```

**Empty state checklist:**
- Clear message explaining why it's empty
- Illustration or icon (optional)
- Primary action to fix it
- Help link if needed

---

## 6. egui-Specific Polish Techniques

### Working with Immediate Mode

**Challenge:** egui rebuilds UI every frame
**Solution:** Cache computed values

```rust
// ✗ BAD (recomputes every frame)
ui.label(format_large_text(&huge_string));

// ✓ GOOD (cache formatted text)
let cached_text = use_memo(|| format_large_text(&huge_string));
ui.label(cached_text);
```

### Smooth Animations in egui

**egui doesn't have built-in animation system:**
```rust
// Manual animation state
struct AnimatedValue {
    current: f32,
    target: f32,
    speed: f32,
}

impl AnimatedValue {
    fn animate(&mut self, dt: f32) {
        // Smooth interpolation
        let diff = self.target - self.current;
        self.current += diff * self.speed * dt;

        // Request repaint if still animating
        if diff.abs() > 0.01 {
            ctx.request_repaint();
        }
    }
}
```

### Custom Widget Rendering

**For pixel-perfect control:**
```rust
// Use egui's Painter for custom shapes
let painter = ui.painter();

// Draw rounded rectangle
painter.rect_filled(
    rect,
    Rounding::same(8.0), // Corner radius
    Color32::from_rgb(76, 175, 140), // Sage green
);

// Draw text with precise positioning
painter.text(
    pos,
    Align2::CENTER_CENTER,
    "Text",
    FontId::proportional(14.0),
    Color32::WHITE,
);
```

### Performance Optimization

**Reduce redraws:**
```rust
// Only repaint when needed
if state_changed {
    ctx.request_repaint();
} else {
    ctx.request_repaint_after(Duration::from_secs(1));
}

// Avoid allocations in hot loop
// Cache strings, vecs, computations outside ui closure
```

---

## 7. Testing for Polish

### Visual Regression Testing

**Before/after screenshots:**
1. Take screenshot of each state
2. Make changes
3. Take new screenshots
4. Compare side-by-side
5. Look for:
   - Alignment shifts
   - Color changes
   - Spacing differences
   - Font rendering changes

### The "Feels Good" Test

**Use the UI yourself:**
- Does clicking feel responsive?
- Do animations feel smooth or janky?
- Is text easy to read for 5+ minutes?
- Do you notice visual glitches?
- Does it feel fast or sluggish?

**Have others use it:**
- Fresh eyes catch things you miss
- Different users have different standards
- Non-designers give honest feedback

### Comparison Testing

**Side-by-side with WisprFlow or similar:**
1. Open WisprFlow
2. Open Hush
3. Compare:
   - Spacing feels similar?
   - Colors have similar warmth?
   - Animations similar smoothness?
   - Overall vibe matches?

**Not about copying, about matching quality level**

### Performance Testing

**Check frame rate:**
```rust
// Log frame time
let frame_start = Instant::now();
// ... render UI ...
let frame_time = frame_start.elapsed();
if frame_time > Duration::from_millis(16) {
    warn!("Frame took {}ms (target: 16ms)", frame_time.as_millis());
}
```

**Target: 60 FPS (16.67ms per frame)**
- Occasional drops OK (garbage collection, etc.)
- Consistent drops = optimization needed

---

## 8. Common Pitfalls & How to Avoid

### Pitfall 1: "Spec Says X, So I Did Exactly X"

**Problem:** Specs are guidelines, not absolute rules
**Solution:** Use judgment - if 12px spacing looks wrong, try 16px

**Example:**
- Spec: "12px spacing between elements"
- Reality: Button looks cramped with 12px margin
- Fix: Use 16px for buttons, 12px for other elements

### Pitfall 2: Ignoring Context

**Problem:** Same component looks different in different contexts
**Solution:** Adapt component to its environment

**Example:**
- Primary button in modal: Large, centered
- Primary button in form: Medium, right-aligned
- Primary button in toolbar: Small, icon-only

### Pitfall 3: Animation Overload

**Problem:** Animating too many things
**Solution:** Animate what matters, make rest instant

**Example:**
- ✓ Modal open: Animate (draws attention)
- ✗ Form label: Don't animate (unnecessary)
- ✓ Success checkmark: Animate (celebration)
- ✗ Text color change: Don't animate (distracting)

### Pitfall 4: Pixel-Pushing Instead of User-Testing

**Problem:** Spending hours on 1px shifts
**Solution:** Test with users, fix actual problems

**Priority order:**
1. Functionality works (most important)
2. No visual glitches (critical)
3. Spacing feels balanced (important)
4. Perfect pixel alignment (nice-to-have)

### Pitfall 5: Forgetting Edge Cases

**Problem:** UI looks great with perfect data
**Solution:** Test with realistic messy data

**Test cases:**
- Very long text (truncate gracefully)
- Very short text (don't break layout)
- Empty state (show helpful message)
- Error state (clear actionable error)
- Loading state (smooth transition)

---

## 9. The "Polish Checklist"

Use this before marking any UI task complete:

### Visual
- [ ] Spacing feels balanced (not cramped, not too loose)
- [ ] Colors have appropriate contrast (readable, not harsh)
- [ ] Typography is crisp and readable
- [ ] Shadows create clear depth hierarchy
- [ ] All elements properly aligned (optically, not just grid)
- [ ] No jagged edges or pixelation
- [ ] Consistent corner radius throughout

### Interaction
- [ ] All interactive elements have hover state
- [ ] Click feedback is immediate (< 100ms)
- [ ] Animations are smooth (60 FPS)
- [ ] No janky transitions or jumps
- [ ] Loading states are clear
- [ ] Success/error feedback is obvious
- [ ] Disabled states are clearly disabled

### Attention to Detail
- [ ] Text truncates gracefully when too long
- [ ] Empty states have helpful messages
- [ ] Error messages are actionable
- [ ] Icons align with text optically
- [ ] Button sizes feel right for target size
- [ ] Touch targets are large enough (min 44×44px)
- [ ] No orphan elements (single item in list)

### Performance
- [ ] UI responds instantly to clicks
- [ ] Animations don't drop frames
- [ ] No visual lag when typing
- [ ] Window resizing is smooth
- [ ] No memory leaks over time

### Consistency
- [ ] Similar actions look similar
- [ ] Color usage is consistent
- [ ] Spacing follows system (4px grid)
- [ ] Typography uses defined scale
- [ ] Animation timing is consistent

---

## 10. When to Iterate vs When to Ship

### Ship It If:
- ✅ All functionality works
- ✅ No visual glitches
- ✅ Passes accessibility tests
- ✅ 80% of polish checklist met
- ✅ Users can complete tasks successfully

### Iterate If:
- ❌ Visual glitches or bugs
- ❌ Animations are janky
- ❌ Users get confused during testing
- ❌ Accessibility issues
- ❌ Performance problems

### Polish Later If:
- ⚠️ Minor spacing inconsistencies
- ⚠️ Could use better animations
- ⚠️ Some states could be prettier
- ⚠️ Would benefit from illustrations

**Perfect is the enemy of good.**
Ship when it's 80% polished, iterate to 95%, never reach 100%.

---

## Resources

### Inspiration (Apps with Great Polish)
- **WisprFlow** - Our north star for dictation UI
- **Linear** - Excellent keyboard nav, animations
- **Raycast** - Fast, minimal, polished launcher
- **Arc Browser** - Smooth animations, attention to detail
- **Superhuman** - Email app with incredible polish

### egui Examples to Study
- **egui demo app** - Shows what's possible
- **egui_extras** - Additional widgets
- **egui_plot** - Plotting with polish
- **rerun.io** - Production egui app (very polished)

### Tools
- **ColorSlurp** - Color picking and contrast checking
- **Contrast** - WCAG contrast checker
- **Loom** - Record animations for frame analysis
- **Ksnip** - Linux screenshots for comparison

---

## Final Thoughts

**Polish is not about perfection.**

It's about:
- Caring about details
- Testing with real users
- Iterating based on feedback
- Knowing when good enough is good enough

**You'll know it's polished when:**
- Users say "This feels nice"
- Interactions feel effortless
- You're not embarrassed to demo it
- It feels as good as WisprFlow

**Remember:**
- Start with functionality
- Add polish incrementally
- Test frequently
- Ship when it's good, iterate to great

---

**Next: Start implementing T-027 (Enhanced Settings Window) with these polish principles in mind** 🚀
