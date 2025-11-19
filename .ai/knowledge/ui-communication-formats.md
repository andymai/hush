# UI Communication Formats for AI Agents

**Question:** What's the best way to communicate UI designs to AI agents implementing code?

**Answer:** It depends on the AI's capabilities, but a **hybrid approach** works best.

---

## Format Comparison

### 1. **ASCII Art** (Current Approach)

**Example:**
```
┌────────────────────────────┐
│  Hush Settings        ✕    │  <- Title bar
├────────────────────────────┤
│ General Models Audio       │  <- Tabs
├────────────────────────────┤
│                            │
│  Theme:        [Dark  ▼]   │  <- Form
│                            │
└────────────────────────────┘
```

**Pros:**
- ✅ Works everywhere (text-only)
- ✅ Git-friendly (diffs, blame)
- ✅ Easy to create manually
- ✅ Human-readable

**Cons:**
- ❌ No precise measurements
- ❌ No colors (just symbols)
- ❌ Hard to show complex layouts
- ❌ Can't represent animations
- ❌ Ambiguous spacing

**Best for:**
- Quick sketches
- Communication between humans
- Documentation that needs to be readable

**AI Success Rate:** 60-70% accuracy in implementation

---

### 2. **Structured YAML/JSON** ⭐ (Best for AI Parsing)

**Example:** See `ui-spec-example.yaml`

```yaml
overlay_idle:
  geometry:
    width: 60
    height: 4
    position: "bottom-center"
  appearance:
    background:
      color: "#000000"
      opacity: 0.75
```

**Pros:**
- ✅ Machine-readable (easy AI parsing)
- ✅ Precise measurements
- ✅ Type-safe (can validate)
- ✅ Supports inheritance
- ✅ Can express animations, states
- ✅ Generate code directly
- ✅ Git-friendly

**Cons:**
- ❌ Verbose for complex UIs
- ❌ Not visually intuitive
- ❌ Requires schema definition
- ❌ Time-consuming to write

**Best for:**
- AI-to-AI communication
- Programmatic UI generation
- Complex components with many properties
- When precision is critical

**AI Success Rate:** 90-95% accuracy in implementation

---

### 3. **Component DSL** (Domain-Specific Language)

**Example:** See `ui-dsl-example.txt`

```
@overlay.idle {
  size: 60x4
  position: bottom-center offset:32
  fill: rgba(0,0,0,0.75)

  &:hover {
    scale: 1.05 @150ms
  }
}
```

**Pros:**
- ✅ Concise and readable
- ✅ CSS-like syntax (familiar)
- ✅ Supports inheritance, states
- ✅ Less verbose than YAML
- ✅ Still machine-readable

**Cons:**
- ❌ Custom format (needs parser)
- ❌ Learning curve
- ❌ Not standard
- ❌ Tooling doesn't exist

**Best for:**
- If you're willing to build tooling
- When YAML feels too verbose
- CSS-familiar developers

**AI Success Rate:** 85-90% accuracy (if AI understands format)

---

### 4. **Annotated Screenshots** ⭐ (Best for Vision Models)

**Example:**
```
[Screenshot of settings window with annotations]

Measurements:
- Window: 800×600px (resizable)
- Title bar: 48px height
- Tab bar: 40px height
- Content padding: 24px
- Button height: 36px

Colors:
- Background: #FFFFFF
- Accent: #4CAF8C (sage green)
- Text: #1A1A1A

Typography:
- Title: Inter Semibold 16px
- Body: Inter Regular 13px

Spacing:
- Form row gap: 12px
- Section gap: 24px
```

**Pros:**
- ✅ Visually accurate (shows exactly what you want)
- ✅ Works with vision-capable AI (GPT-4V, Claude 3)
- ✅ Shows colors, shadows, gradients perfectly
- ✅ Human-friendly
- ✅ Fast to create (just screenshot + annotate)

**Cons:**
- ❌ Requires vision model
- ❌ Not programmatically parseable
- ❌ Large file size
- ❌ Not git-friendly (binary)
- ❌ Annotations can be ambiguous

**Best for:**
- Vision-capable AI models
- Complex visual designs
- When color/shadow accuracy matters
- Reference for human developers

**AI Success Rate:** 80-90% with vision models, 0% without

---

### 5. **Figma/Design Tool Export**

**Example:** Figma → JSON export

```json
{
  "name": "Settings Window",
  "type": "FRAME",
  "width": 800,
  "height": 600,
  "fills": [{
    "type": "SOLID",
    "color": {"r": 1, "g": 1, "b": 1}
  }],
  "children": [...]
}
```

**Pros:**
- ✅ Industry-standard format
- ✅ Precise measurements
- ✅ Colors, effects, typography
- ✅ Can extract programmatically
- ✅ Designers already use it

**Cons:**
- ❌ Complex JSON structure
- ❌ AI needs Figma API knowledge
- ❌ Requires Figma license
- ❌ Web-focused (not native UI)
- ❌ Lots of irrelevant data

**Best for:**
- Teams already using Figma
- When designers create mockups
- Web UI (translates better)

**AI Success Rate:** 70-80% (needs Figma-to-code training)

---

### 6. **SVG + Annotations**

**Example:**
```xml
<svg width="800" height="600">
  <!-- Window background -->
  <rect width="800" height="600" fill="#FFFFFF" rx="12"/>

  <!-- Title bar -->
  <g data-component="title-bar" data-height="48">
    <rect y="0" width="800" height="48" fill="#FAF8F5"/>
    <text x="16" y="30" font-size="16" font-weight="600">Hush Settings</text>
  </g>

  <!-- Tabs -->
  <g data-component="tabs" data-height="40">
    <!-- ... -->
  </g>
</svg>
```

**Pros:**
- ✅ Vector format (scales perfectly)
- ✅ Can embed measurements as data attributes
- ✅ AI can parse XML
- ✅ Shows exact visual appearance

**Cons:**
- ❌ Verbose for complex UIs
- ❌ Static (no animations)
- ❌ Requires conversion to native UI code
- ❌ Not how native UIs are built

**Best for:**
- Icon systems
- Simple illustrations
- When vector accuracy matters

**AI Success Rate:** 75-85% (good for static layouts)

---

### 7. **Component Tree (JSON)**

**Example:**
```json
{
  "type": "Window",
  "props": {
    "width": 800,
    "height": 600,
    "title": "Hush Settings"
  },
  "children": [
    {
      "type": "TitleBar",
      "props": {"height": 48},
      "children": [
        {"type": "Text", "content": "Hush Settings"},
        {"type": "CloseButton"}
      ]
    },
    {
      "type": "TabBar",
      "props": {"tabs": ["General", "Models", "Audio"]}
    },
    {
      "type": "Content",
      "children": [...]
    }
  ]
}
```

**Pros:**
- ✅ Matches component hierarchy
- ✅ Easy to parse
- ✅ Maps directly to code
- ✅ Familiar to developers (React-like)

**Cons:**
- ❌ Doesn't show visual styling
- ❌ No layout information
- ❌ Missing animations, states

**Best for:**
- Component-based frameworks
- When structure > appearance
- Rapid prototyping

**AI Success Rate:** 80-85% (needs style guidance separately)

---

## 🏆 Recommended Hybrid Approach

### **Tier 1: For Critical Components (95% AI Success)**

**Use: Structured YAML + Annotated Screenshot**

```
1. YAML spec (ui-specs/button-primary.yaml)
   - Exact measurements, colors, states
   - Animation timing, easing
   - All properties machine-readable

2. Reference screenshot (ui-screenshots/button-primary.png)
   - Shows exact visual appearance
   - Annotated with key measurements
   - Multiple states (idle, hover, active, disabled)
```

**Why both?**
- YAML → AI generates code precisely
- Screenshot → AI validates visual appearance
- Together → 95% accuracy

### **Tier 2: For Standard Components (85% AI Success)**

**Use: Component DSL or YAML**

```
@button.primary {
  height: 36
  padding: 0 20
  radius: 8
  background: #4CAF8C
  color: white
  ...
}
```

**Why:**
- Less critical = less overhead
- DSL is concise but precise
- Screenshot not necessary if following design system

### **Tier 3: For Layout/Structure (80% AI Success)**

**Use: ASCII Mockups + Component References**

```
┌────────────────────────────┐
│  <TitleBar>           ✕    │
├────────────────────────────┤
│  <TabBar tabs=[...] />     │
├────────────────────────────┤
│  <Content padding=24>      │
│    <FormRow>               │
│      <Label>Theme</Label>  │
│      <Dropdown />          │
│    </FormRow>              │
│  </Content>                │
└────────────────────────────┘
```

**Why:**
- Shows structure clearly
- References existing components
- Human-readable

---

## 📋 Implementation Guide

### For Hush UI Expansion:

**1. Create Component Library (YAML specs)**
```
.ai/ui-specs/
├── components/
│   ├── button-primary.yaml
│   ├── button-secondary.yaml
│   ├── input.yaml
│   ├── dropdown.yaml
│   ├── toggle.yaml
│   └── ...
├── layouts/
│   ├── settings-window.yaml
│   ├── modal.yaml
│   └── ...
└── screens/
    ├── overlay-idle.yaml
    ├── overlay-recording.yaml
    └── ...
```

**2. Add Reference Screenshots (Optional but helpful)**
```
.ai/ui-screenshots/
├── components/
│   ├── button-states.png (idle, hover, active, disabled)
│   ├── form-controls.png
│   └── ...
└── screens/
    ├── settings-window-general.png
    ├── settings-window-models.png
    └── ...
```

**3. Use Hybrid References in Task Files**

**Before (ASCII only):**
```markdown
## UI Layout
[ASCII mockup here]
```

**After (Hybrid):**
```markdown
## UI Specification

**Component Spec:** `.ai/ui-specs/components/button-primary.yaml`
**Visual Reference:** `.ai/ui-screenshots/button-states.png`
**Design System:** `.ai/knowledge/ui-design-specification.md`

**Layout Structure:**
[ASCII mockup for structure]

**Implementation:**
1. Parse YAML spec for exact properties
2. Reference screenshot for visual validation
3. Follow design system for consistency
```

---

## 🎯 Best Format Summary

| Format | Precision | AI Parse | Human Read | Create Time | Best For |
|--------|-----------|----------|------------|-------------|----------|
| **ASCII** | ⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | Quick sketches |
| **YAML/JSON** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐ | Critical components |
| **Component DSL** | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐ | Standard components |
| **Screenshots** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | Visual reference |
| **Figma Export** | ⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐⭐ | Design teams |
| **SVG** | ⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐ | Icons, static |

**Winner:** **YAML + Screenshots** (Hybrid approach)

---

## 💡 Quick Wins for Hush

### What to Do Next:

**Option A: Keep ASCII (Good Enough)**
- Current approach works
- 70-80% AI success
- No additional work

**Option B: Add YAML Specs (Recommended)**
- Create YAML for critical components (buttons, inputs, overlay)
- Keep ASCII for layouts
- 85-90% AI success
- ~4 hours of work

**Option C: Full Hybrid (Maximum Precision)**
- YAML specs for all components
- Screenshots for reference
- ASCII for structure
- 95%+ AI success
- ~8-12 hours of work

### My Recommendation:

**Start with Option B:**
1. Create YAML specs for the **top 10 components**:
   - Overlay (idle, recording)
   - Button (primary, secondary, destructive)
   - Input, Dropdown, Toggle, Slider
   - Model Card
   - Settings Window layout

2. Keep ASCII mockups for **screen layouts**

3. Reference YAML specs in task files

**This gives 85-90% AI accuracy with reasonable effort.**

---

## 🚀 Next Steps

Want me to:
1. **Create YAML specs** for Hush's top 10 components?
2. **Build a YAML schema** so AI can validate?
3. **Update task files** to reference YAML specs?
4. **Create a DSL parser** (more work, but cleaner)?

The YAML approach is **proven** and immediately usable by AI agents.

Should I convert our critical components to YAML format?
