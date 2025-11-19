# Hush UI Design Specification

**Last Updated:** 2025-11-19
**Design Goal:** Match WisprFlow's level of polish with Hush's minimalist, privacy-first philosophy
**Target Feel:** Warm, elegant, unobtrusive, fast, professional

---

## Design Philosophy

### Core Principles

1. **Intentional Invisibility**
   - UI should "disappear into muscle memory"
   - Feel like a power you have, not a tool you use
   - Present but not obtrusive

2. **Warmth & Clarity**
   - Avoid cold, clinical AI aesthetic
   - Use gentle curves, generous spacing, soft corners
   - Humanistic, lived-in quality

3. **Speed & Responsiveness**
   - Instant visual feedback
   - Smooth animations (60 FPS minimum)
   - No lag, no waiting

4. **Privacy-First Visual Language**
   - Reinforce that transcription is local
   - No cloud icons or uploading indicators
   - Emphasize on-device processing

---

## Color Palette

### Primary Colors

**Background Tones (Inspired by WisprFlow's "Lumen"):**
```
Cream/Warm White:  #FAF8F5  (light mode background)
Soft Charcoal:     #1C1C1E  (dark mode background)
Panel Background:  #FFFFFF  (light mode panels)
Panel Dark:        #2C2C2E  (dark mode panels)
```

**Accent Colors (Inspired by WisprFlow's "Pulse"):**
```
Primary Accent:    #4CAF8C  (sage green - success, active states)
Secondary Accent:  #B4A5D9  (soft lavender - hover, subtle emphasis)
Warning:           #F5A623  (amber - caution, important actions)
Error:             #E74C3C  (coral red - errors, destructive actions)
```

**Text Colors:**
```
Primary Text (Light):    #1A1A1A  (near-black)
Secondary Text (Light):  #666666  (medium gray)
Tertiary Text (Light):   #999999  (light gray)

Primary Text (Dark):     #FAFAFA  (off-white)
Secondary Text (Dark):   #B0B0B0  (light gray)
Tertiary Text (Dark):    #707070  (medium gray)
```

**Overlay Specific:**
```
Overlay Idle:        rgba(0, 0, 0, 0.75)     (dark mode)
Overlay Idle Light:  rgba(255, 255, 255, 0.85) (light mode)
Overlay Recording:   rgba(76, 175, 140, 0.95) (sage green glow)
Border Subtle:       rgba(255, 255, 255, 0.2) (dark mode)
Border Subtle Light: rgba(0, 0, 0, 0.1)       (light mode)
```

### Color Usage Guidelines

- **Green (Sage)**: Success states, recording active, confirmation buttons
- **Lavender**: Hover states, secondary actions, subtle emphasis
- **Amber**: Warnings, important decisions, upgrade prompts
- **Red (Coral)**: Errors, destructive actions (delete, clear)
- **Gray Scale**: Text hierarchy, borders, backgrounds

---

## Typography

### Font Stack

**Sans-Serif (Primary UI):**
```
Inter, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif
```

**Monospace (Code, Technical):**
```
"JetBrains Mono", "Fira Code", "SF Mono", Monaco, Consolas, monospace
```

**Serif (Headlines, Emphasis - Optional):**
```
"EB Garamond", Georgia, "Times New Roman", serif
```

### Type Scale

```
Display Large:  32px / 40px line-height (window titles, wizard steps)
Display Medium: 24px / 32px line-height (section headers)
Heading Large:  20px / 28px line-height (tab titles, card headers)
Heading Medium: 16px / 24px line-height (subsection titles)
Body Large:     14px / 22px line-height (primary content)
Body Medium:    13px / 20px line-height (secondary content)
Body Small:     12px / 18px line-height (captions, metadata)
Tiny:           10px / 16px line-height (overlay states, timestamps)
```

### Font Weights

```
Regular:   400  (body text)
Medium:    500  (emphasis, buttons)
Semibold:  600  (headings, active states)
Bold:      700  (strong emphasis, rarely used)
```

### Typography Guidelines

- **Overlay text**: Small (10-12px), semibold, high contrast
- **Settings window**: Body Medium (13px) for most content
- **Buttons**: Body Medium (13px), Medium weight (500)
- **Labels**: Body Small (12px), Regular weight (400)
- **Headings**: Heading Medium/Large (16-20px), Semibold (600)

---

## Spacing System

### Base Unit: 4px

```
Tiny:       4px   (tight spacing, icon padding)
Small:      8px   (compact lists, button padding)
Medium:     12px  (default spacing between elements)
Large:      16px  (section spacing, card padding)
XLarge:     24px  (major section breaks)
XXLarge:    32px  (window margins, hero spacing)
Huge:       48px  (dramatic spacing, wizard steps)
```

### Layout Grids

**Settings Window:**
- Content width: 600-800px (centered)
- Side margins: 32px
- Tab bar height: 48px
- Content padding: 24px

**Overlay:**
- Idle: 60×4px (tiny pill)
- Recording: 80×24px (expanded pill)
- Settings panel: 220×140px (popup)

**First-Run Wizard:**
- Content width: 560px (centered)
- Step height: 480px minimum
- Button bar height: 64px
- Progress indicator: 8px tall

---

## Component Design

### 1. Overlay (Main UI)

#### Idle State (Minimal Presence)
```
Dimensions:  60px wide × 4px tall
Shape:       Perfect pill (border-radius: 2px)
Background:  rgba(0, 0, 0, 0.75) dark / rgba(255, 255, 255, 0.85) light
Border:      1px rgba(255, 255, 255, 0.2) / rgba(0, 0, 0, 0.1)
Shadow:      0px 4px 12px rgba(0, 0, 0, 0.3)
Position:    Bottom-center, 32px from bottom
Cursor:      pointer (entire area clickable)
```

**Visual Cues:**
- Subtle pulsing glow (1s cycle, 10% opacity change)
- On hover: Scale to 105%, glow intensity +20%

#### Recording State (Active Feedback)
```
Dimensions:  80px wide × 24px tall (smooth expansion, 200ms ease-out)
Shape:       Perfect pill (border-radius: 12px)
Background:  rgba(76, 175, 140, 0.95) - sage green glow
Border:      None (or 1px brighter green)
Shadow:      0px 6px 20px rgba(76, 175, 140, 0.4) - green glow
```

**Waveform Animation:**
- 12 vertical bars, 2.5px wide each
- Bar spacing: 1px
- Height: 3px base + (amplitude × time × 16px max)
- Color: Pure white (#FFFFFF)
- Animation: Sine wave, 10Hz frequency, phase offset per bar
- Frame rate: 20 FPS (50ms update)

**Duration Display:**
- Position: Below waveform
- Font: 10px, Medium weight
- Color: White with 80% opacity
- Format: "0:03" (mm:ss)

#### Processing State
```
Dimensions:  80px wide × 24px tall
Background:  rgba(255, 255, 255, 0.9) light / rgba(40, 40, 40, 0.95) dark
Text:        "processing..." 10px, centered
Animation:   Subtle spinner or pulsing dots
```

#### Success State (Brief, 1.5s)
```
Dimensions:  80px wide × 24px tall
Background:  rgba(76, 175, 140, 0.95) - green
Text:        "done ✓" 10px, centered
Animation:   Fade in, hold 1s, fade out
```

#### Error State (2s, then fade)
```
Dimensions:  80px wide × 24px tall
Background:  rgba(231, 76, 60, 0.95) - coral red
Text:        "error" 10px, centered
Animation:   Subtle shake (3px left-right, 3 times)
```

### 2. Settings Window (Comprehensive UI)

#### Window Chrome
```
Dimensions:  800px wide × 600px tall (resizable, min 600×400)
Background:  #FFFFFF (light) / #1C1C1E (dark)
Border:      None (native window decorations on Linux)
Shadow:      0px 12px 48px rgba(0, 0, 0, 0.2)
Corner Radius: 12px (if custom window)
```

#### Title Bar
```
Height:      48px
Background:  #FAF8F5 (light) / #2C2C2E (dark)
Bottom Border: 1px rgba(0, 0, 0, 0.1) / rgba(255, 255, 255, 0.1)
Title:       "Hush Settings" - 16px, Semibold, left-aligned, 16px margin
Close Button: Top-right, 32×32px hit area, hover: rgba bg
```

#### Tab Bar
```
Height:      40px
Background:  #FFFFFF (light) / #2C2C2E (dark)
Border:      Bottom 1px rgba(0, 0, 0, 0.1)
Tab Width:   Auto (equal distribution)
Tab Padding: 12px horizontal
Tab Text:    13px, Medium weight
```

**Tab States:**
- **Inactive**: #666666 text, no background
- **Hover**: #4CAF8C text (sage), background rgba(76, 175, 140, 0.1)
- **Active**: #4CAF8C text (sage), bottom border 2px solid #4CAF8C
- **Transition**: 150ms ease-out

**Tab Names:**
- General
- Models
- Audio
- Hotkeys
- Advanced

#### Content Area
```
Padding:     24px all sides
Max Width:   600px (centered if window wider)
Background:  Transparent (inherits window bg)
```

### 3. Settings Components

#### Section Header
```
Margin:      0 0 16px 0 (first section: 0 top margin)
Text:        16px, Semibold, #1A1A1A (light) / #FAFAFA (dark)
Bottom Border: 1px rgba(0, 0, 0, 0.1) / rgba(255, 255, 255, 0.1)
Padding:     0 0 8px 0
```

#### Form Row (Label + Control)
```
Margin:      12px 0
Display:     Flex, space-between, align-center
Min-Height:  32px
```

**Label:**
```
Text:        13px, Regular, #666666 (light) / #B0B0B0 (dark)
Flex:        0 0 40% (fixed width)
Align:       Right (for left-side labels)
Padding:     0 16px 0 0
```

**Control:**
```
Flex:        1 (grows to fill)
Max-Width:   300px
```

#### Button (Primary)
```
Height:      36px
Padding:     0 20px
Border-Radius: 8px
Background:  #4CAF8C (sage green)
Text:        13px, Medium weight, #FFFFFF
Border:      None
Shadow:      0px 2px 4px rgba(0, 0, 0, 0.1)
Cursor:      pointer

Hover:       Background #3D9E7A (darker green), Shadow 0px 3px 6px
Active:      Background #2E8B68 (even darker), Shadow 0px 1px 2px
Disabled:    Background #CCCCCC, Text #999999, Cursor not-allowed
```

#### Button (Secondary)
```
Height:      36px
Padding:     0 20px
Border-Radius: 8px
Background:  Transparent
Text:        13px, Medium weight, #4CAF8C
Border:      1px solid #4CAF8C
Cursor:      pointer

Hover:       Background rgba(76, 175, 140, 0.1)
Active:      Background rgba(76, 175, 140, 0.2)
Disabled:    Border #CCCCCC, Text #999999, Cursor not-allowed
```

#### Button (Destructive)
```
Height:      36px
Padding:     0 20px
Border-Radius: 8px
Background:  #E74C3C (coral red)
Text:        13px, Medium weight, #FFFFFF
Border:      None

Hover:       Background #D43D2E
Active:      Background #C12E21
```

#### Toggle Switch
```
Width:       44px
Height:      24px
Border-Radius: 12px
Background (Off): #CCCCCC
Background (On):  #4CAF8C
Knob:        20px diameter, white, 2px margin
Transition:  200ms ease-out
Shadow:      0px 1px 3px rgba(0, 0, 0, 0.2)
```

#### Slider
```
Height:      4px
Border-Radius: 2px
Background:  #E0E0E0 (track)
Fill:        #4CAF8C (progress)
Thumb:       16px diameter, white, border 2px #4CAF8C
Shadow:      0px 2px 4px rgba(0, 0, 0, 0.1)
```

#### Text Input
```
Height:      36px
Padding:     0 12px
Border-Radius: 6px
Background:  #FFFFFF (light) / #2C2C2E (dark)
Border:      1px #CCCCCC (light) / #4A4A4A (dark)
Text:        13px, Regular
Placeholder: #999999

Focus:       Border #4CAF8C, Shadow 0px 0px 0px 3px rgba(76, 175, 140, 0.2)
Error:       Border #E74C3C, Shadow 0px 0px 0px 3px rgba(231, 76, 60, 0.2)
```

#### Dropdown / Select
```
Height:      36px
Padding:     0 12px
Border-Radius: 6px
Background:  #FFFFFF (light) / #2C2C2E (dark)
Border:      1px #CCCCCC (light) / #4A4A4A (dark)
Text:        13px, Regular
Arrow:       Right-aligned, 8px chevron

Hover:       Border #4CAF8C
Open:        Border #4CAF8C, Shadow 0px 0px 0px 3px rgba(76, 175, 140, 0.2)
```

#### Radio Button
```
Size:        20px diameter
Border:      2px #CCCCCC
Background:  Transparent
Dot:         10px diameter, #4CAF8C (when selected)

Hover:       Border #4CAF8C
Selected:    Border #4CAF8C, Dot visible
```

#### Checkbox
```
Size:        20px square
Border-Radius: 4px
Border:      2px #CCCCCC
Background:  Transparent
Checkmark:   White, 12px (when checked)

Hover:       Border #4CAF8C
Checked:     Border #4CAF8C, Background #4CAF8C
```

### 4. Model Management UI

#### Model Card
```
Width:       Full width
Height:      Auto (min 80px)
Padding:     16px
Margin:      8px 0
Border-Radius: 8px
Background:  #F5F5F5 (light) / #2C2C2E (dark)
Border:      1px transparent

Hover:       Border #4CAF8C, Background slight lighter
Selected:    Border 2px #4CAF8C, Background slight green tint
```

**Card Layout:**
```
┌─────────────────────────────────────────────┐
│ ● Base Model           ✓ Installed    [Set] │
│ 145 MB • CPU: 4-6s • GPU: 0.5s • Good       │
│ ━━━━━━━━━━━━━━━━ 45% Downloaded             │
└─────────────────────────────────────────────┘
```

**Card Elements:**
- **Title**: 16px, Semibold, left
- **Status Badge**: 12px, Regular, right (✓ Installed / ⌛ Not installed)
- **Action Button**: 32px height, 80px width, right
- **Specs**: 12px, Regular, #666666, second line
- **Progress Bar**: 4px height, 100% width, bottom (if downloading)

#### Model Info (Hover Tooltip)
```
Max Width:   300px
Padding:     12px
Border-Radius: 6px
Background:  #2C2C2E (dark) / #FFFFFF (light)
Shadow:      0px 4px 12px rgba(0, 0, 0, 0.3)
Text:        12px, Regular

Content:
- Model description
- Use case recommendation
- Accuracy level explanation
```

#### Download Progress Bar
```
Height:      4px
Border-Radius: 2px
Background:  #E0E0E0 (track)
Fill:        #4CAF8C (progress) - animated gradient
Text Above:  "72 MB / 145 MB • 2.4 MB/s • 30s remaining" - 11px, #666666
```

### 5. Audio Device Picker

#### Device List Item
```
Height:      48px
Padding:     12px 16px
Display:     Flex, align-center
Border-Bottom: 1px rgba(0, 0, 0, 0.05)

Hover:       Background rgba(76, 175, 140, 0.05)
Selected:    Background rgba(76, 175, 140, 0.1), Left border 3px #4CAF8C
```

**Item Layout:**
```
● USB Microphone                    [Test]  ▲▲▲▲▲___  -12dB
```

- **Radio Button**: 20px, left
- **Device Name**: 14px, Medium, flex-grow
- **Test Button**: 32px height, 60px width
- **Level Meter**: 80px width, 8px height, right
- **dB Label**: 11px, right of meter

#### Live Audio Level Meter
```
Width:       Full width (or 200px in compact)
Height:      8px
Border-Radius: 4px
Background:  #E0E0E0 (track)

Level Colors:
- 0-60%:     #4CAF8C (green - good)
- 60-85%:    #F5A623 (amber - loud)
- 85-100%:   #E74C3C (red - clipping)

Peak Indicator: 2px wide vertical line, white, holds for 500ms
Animation:   Smooth, 60 FPS, exponential decay
```

#### Waveform Visualization (Test Recording)
```
Width:       Full width
Height:      100px
Background:  #F5F5F5 (light) / #2C2C2E (dark)
Border-Radius: 6px
Padding:     8px

Waveform:    Filled area chart, #4CAF8C
Playback:    Red vertical line scrubber
```

### 6. Hotkey Configurator

#### Hotkey Display
```
Height:      36px
Padding:     0 12px
Border-Radius: 6px
Background:  #F5F5F5 (light) / #2C2C2E (dark)
Border:      1px #CCCCCC
Display:     Inline-flex, gap 4px

Key Badge:   Background #FFFFFF, Border 1px #CCCCCC, Padding 4px 8px,
             Border-Radius 4px, Font 12px monospace
```

**Example:**
```
┌──────────────────────────┐
│  Ctrl  +  Alt  +  V      │
└──────────────────────────┘
```

#### Hotkey Recorder Modal
```
Width:       400px
Height:      200px
Position:    Center of window
Background:  #FFFFFF (light) / #2C2C2E (dark)
Shadow:      0px 20px 60px rgba(0, 0, 0, 0.4)
Border-Radius: 12px
Backdrop:    rgba(0, 0, 0, 0.5) - blurred

Content:
- "Press keys..." prompt - 20px, centered
- Live key display - 24px, centered, updates as keys pressed
- Error message - 13px, red, if invalid
- [Cancel] [Apply] buttons - bottom, right-aligned
```

**Recording States:**
- Waiting: "Press keys..." gray text
- Recording: Live update "Ctrl + Alt + " as keys pressed
- Valid: Green checkmark, "Apply" enabled
- Invalid: Red X, error message, "Apply" disabled

### 7. First-Run Setup Wizard

#### Wizard Window
```
Dimensions:  640px wide × 560px tall
Background:  #FAF8F5 (light) / #1C1C1E (dark)
Border-Radius: 12px
Shadow:      0px 20px 80px rgba(0, 0, 0, 0.3)
Padding:     0 (content manages own padding)
```

#### Progress Indicator (Top)
```
Height:      8px
Width:       Full width
Background:  #E0E0E0
Progress:    #4CAF8C, smooth animation
Text:        "Step 2 of 7" - 12px, centered, above bar
```

#### Step Content Area
```
Padding:     48px 64px
Min-Height:  400px
Display:     Flex, flex-direction column, justify center
```

#### Step Title
```
Font:        24px, Semibold
Color:       #1A1A1A (light) / #FAFAFA (dark)
Margin:      0 0 16px 0
Text-Align:  Center
```

#### Step Description
```
Font:        14px, Regular
Color:       #666666 (light) / #B0B0B0 (dark)
Margin:      0 0 32px 0
Text-Align:  Center
Max-Width:   480px (centered)
```

#### Button Bar (Bottom)
```
Height:      64px
Padding:     12px 24px
Border-Top:  1px rgba(0, 0, 0, 0.1)
Display:     Flex, space-between, align-center

Buttons:     36px height
- [< Back]   - Secondary button, left
- [Skip]     - Text button, center (if skippable)
- [Next >]   - Primary button, right
```

#### Welcome Screen (Step 1)
```
Icon:        Hush logo, 80px, centered
Title:       "Welcome to Hush" - 32px
Subtitle:    "Fast, private voice-to-text for Linux" - 16px
Description: 2-3 sentences about Hush
Privacy:     🔒 "All transcription happens locally" - badge
CTA:         Large primary button "Get Started" - 44px height
```

#### Model Download Step
```
Layout:
- Model comparison table (3 columns)
- Recommended badge on suggested model
- Download progress (if downloading)
- "Why download?" explanation

Visual:
- GPU detected: Green badge "✓ GPU Detected - Faster models recommended"
- No GPU: Amber badge "⚠ No GPU - Small models recommended"
```

#### Completion Screen
```
Icon:        Green checkmark, 80px, animated (scale in + rotate)
Title:       "You're all set!" - 32px
Summary:     4-5 bullet points of configuration
Quick Start: Illustrated guide (hold hotkey → speak → release)
CTA:         "Start Using Hush" - Large primary button
```

### 8. System Tray Icon

#### Icon States
```
Size:        22×22px (standard Linux tray size)
Format:      SVG (scalable)
Style:       Monochrome (matches system theme)
```

**Icon Designs:**
- **Idle**: Microphone outline, simple
- **Recording**: Microphone filled, pulse animation
- **Processing**: Spinner overlay on microphone
- **Error**: Microphone with red dot/badge

**Animation:**
- Pulse: 1s cycle, scale 90%-100%
- Spin: 1s rotation for processing
- Smooth transitions between states

#### Context Menu
```
Width:       220px
Padding:     4px 0
Background:  System default
Border:      1px system border
Shadow:      System shadow

Menu Item:
- Height:    28px
- Padding:   6px 12px
- Font:      System font, 13px
- Icon:      16px, left-aligned, 6px margin

Separator:
- Height:    1px
- Background: rgba(0, 0, 0, 0.1)
- Margin:    4px 0

Submenu:
- Right arrow indicator
- Flyout appears to right
```

### 9. Notifications

#### System Notification
```
Width:       360px (system default)
Height:      Auto (80-120px typical)
Style:       Native system notification

Content:
- App name:  "Hush"
- Icon:      22×22px app icon
- Title:     16px, Semibold (e.g., "Transcription Complete")
- Body:      14px, Regular (e.g., "45 words inserted")
- Actions:   [Copy] [View] buttons (if supported)
```

**Notification Types:**
- Success: Green icon, "Transcription Complete"
- Error: Red icon, "Transcription Failed"
- Info: Blue icon, "Model Downloaded"
- Warning: Amber icon, "Low Disk Space"

---

## Animation Guidelines

### Timing Functions

```css
/* Fast actions (hovers, clicks) */
transition: all 150ms cubic-bezier(0.4, 0, 0.2, 1);

/* Medium actions (modals, overlays) */
transition: all 250ms cubic-bezier(0.4, 0, 0.2, 1);

/* Slow actions (page transitions) */
transition: all 400ms cubic-bezier(0.4, 0, 0.2, 1);

/* Bounce (success, completion) */
animation: 600ms cubic-bezier(0.34, 1.56, 0.64, 1);
```

### Key Animations

**Overlay State Changes:**
```
Idle → Recording:    200ms ease-out (expand width, change color)
Recording → Processing: 150ms ease-in (change color, start spinner)
Processing → Success: 200ms ease-out (flash green, show checkmark)
Success → Idle:      300ms ease-in (fade out, shrink)
```

**Modal/Window:**
```
Open:  250ms ease-out (scale 0.95 → 1.0, opacity 0 → 1)
Close: 200ms ease-in (scale 1.0 → 0.95, opacity 1 → 0)
```

**Button:**
```
Hover:  150ms ease-out (background color, shadow)
Click:  100ms ease-in (scale 0.98, shadow reduce)
Release: 150ms ease-out (scale 1.0, shadow restore)
```

**Progress Bar:**
```
Fill: Linear progression, smooth 60 FPS
Indeterminate: 1.5s linear infinite (sliding shimmer)
```

**Waveform (Overlay):**
```
Frame Rate: 20 FPS (50ms per frame)
Height:     Sine wave based on time + amplitude
Smoothing:  Exponential decay (feels more natural)
```

### Loading States

**Spinner:**
```
Size:        20px
Thickness:   2px
Color:       #4CAF8C (or current context color)
Speed:       800ms per rotation
Style:       Partial circle (270° arc), rotating
```

**Skeleton Screens:**
```
Background:  #F0F0F0 (light) / #2A2A2A (dark)
Shimmer:     Linear gradient, 1.5s animation
Style:       Rounded rectangles matching content layout
```

### Micro-interactions

**Success Feedback:**
- Green flash (100ms)
- Checkmark scale-in with bounce (400ms)
- Optional subtle sound effect

**Error Feedback:**
- Red border pulse (200ms)
- Shake animation (300ms, ±3px horizontal)
- Optional error sound

**Recording Start:**
- Overlay expands smoothly (200ms)
- Green glow appears (150ms fade-in)
- Waveform starts immediately

**Hotkey Pressed:**
- Visual feedback in overlay (50ms response time)
- Overlay state change (immediate)

---

## Accessibility

### Keyboard Navigation

**Focus Indicators:**
```
Outline:     2px solid #4CAF8C
Offset:      2px
Border-Radius: Inherit from element + 2px
```

**Tab Order:**
- Logical flow: top-to-bottom, left-to-right
- Skip navigation available
- Trapped focus in modals

**Keyboard Shortcuts:**
- Esc: Close modal/cancel action
- Enter: Confirm/submit
- Space: Toggle checkbox/radio
- Arrow keys: Navigate lists/options
- Ctrl+W: Close window (standard)

### Screen Reader Support

**ARIA Labels:**
- All interactive elements labeled
- Status announcements for state changes
- Progress updates announced
- Error messages announced immediately

**Semantic HTML (in web contexts):**
- Proper heading hierarchy
- Form labels associated with inputs
- Button vs. link distinction
- List markup for lists

### Color Contrast

**WCAG AA Compliance (4.5:1 minimum):**
- Primary text on background: 12:1 (light) / 14:1 (dark)
- Secondary text on background: 7:1 (light) / 8:1 (dark)
- UI controls: 3:1 minimum
- Focus indicators: 3:1 minimum

**Color Blindness:**
- Don't rely solely on color for information
- Use icons + text for status
- Patterns in addition to colors
- Test with color blindness simulators

### Motion

**Respect prefers-reduced-motion:**
```css
@media (prefers-reduced-motion: reduce) {
  * {
    animation-duration: 0.01ms !important;
    transition-duration: 0.01ms !important;
  }
}
```

**Alternatives:**
- Instant state changes instead of animations
- Fade only (no scale/rotate)
- Disable parallax effects

---

## Responsive Behavior

### Settings Window Scaling

**Breakpoints:**
```
Minimum:  600px wide × 400px tall
Small:    600-750px wide
Medium:   750-900px wide (default)
Large:    900-1200px wide
```

**Adaptations:**
- Small: Single column layout, reduced padding
- Medium: Standard two-column forms
- Large: More whitespace, larger type

### Overlay Position

**Default:** Bottom-center, 32px from edge
**Options:** Top/bottom/left/right, custom offset
**Multi-monitor:** Remembers position per monitor

### High DPI / Retina

- All raster assets at 2x and 3x
- Prefer SVG for icons
- Fractional pixel rendering
- Crisp borders at all scales

---

## Dark Mode

### Implementation

**Automatic Detection:**
```
Detect system preference
Allow manual override in settings
Persist user choice
```

**Color Adaptations:**
- Invert lightness, preserve hue
- Reduce contrast slightly (easier on eyes)
- Adjust shadows (lighter in dark mode)
- Glow effects more prominent

**Testing:**
- Check contrast ratios
- Verify all states (hover, active, disabled)
- Test with real content
- Long-session usability

---

## Platform-Specific Considerations

### Linux Desktop Environments

**GNOME:**
- Follows GNOME HIG for window decorations
- Uses Adwaita-compatible colors
- System tray in top panel

**KDE Plasma:**
- Follows Breeze design language
- Respects Plasma colors
- System tray in bottom panel

**XFCE / Other:**
- Standard GTK themes
- Graceful degradation
- Minimal custom chrome

### Window Decorations

**Native (Preferred):**
- Use system window decorations
- Follows theme
- Familiar to users

**Custom (If needed):**
- Match system style closely
- Provide minimize/maximize/close
- Double-click titlebar to maximize

---

## Implementation Notes

### egui Integration

**Theme Application:**
```rust
let mut style = (*ctx.style()).clone();
style.visuals.window_fill = Color32::from_rgb(250, 248, 245); // Cream
style.visuals.window_stroke.color = Color32::from_rgba_unmultiplied(0, 0, 0, 26); // 10% black
style.spacing.item_spacing = egui::vec2(12.0, 12.0);
style.spacing.button_padding = egui::vec2(20.0, 8.0);
// ... more styling
ctx.set_style(style);
```

**Custom Widgets:**
- Extend egui::Widget trait
- Cache computed layouts
- Use painter for custom rendering
- Respect user's theme preferences

**Performance:**
- Minimize allocations per frame
- Cache expensive computations
- Use ctx.request_repaint_after() for appropriate frame rates
- Profile with debug tools

### Asset Organization

```
assets/
├── icons/
│   ├── app-icon.svg
│   ├── app-icon@2x.png
│   ├── tray-idle.svg
│   ├── tray-recording.svg
│   └── ...
├── fonts/
│   ├── Inter-Regular.ttf
│   ├── Inter-Medium.ttf
│   ├── Inter-SemiBold.ttf
│   └── JetBrainsMono-Regular.ttf
└── illustrations/
    ├── wizard-welcome.svg
    ├── wizard-complete.svg
    └── ...
```

---

## Design Review Checklist

Before marking a UI task as complete:

**Visual Polish:**
- [ ] All colors match spec
- [ ] Typography is consistent
- [ ] Spacing follows 4px grid
- [ ] Animations are smooth (60 FPS)
- [ ] Hover states work correctly
- [ ] Focus indicators visible
- [ ] Icons are sharp at all sizes

**Functionality:**
- [ ] All interactions work as expected
- [ ] Keyboard navigation works
- [ ] Error states display correctly
- [ ] Loading states are smooth
- [ ] Success feedback is clear
- [ ] Edge cases handled

**Accessibility:**
- [ ] WCAG AA contrast met
- [ ] Screen reader tested
- [ ] Keyboard-only navigation works
- [ ] Focus order is logical
- [ ] Reduced motion respected

**Responsiveness:**
- [ ] Works at minimum size
- [ ] Scales to large sizes
- [ ] High DPI tested
- [ ] Dark mode works
- [ ] RTL tested (if applicable)

**Performance:**
- [ ] No frame drops
- [ ] Fast startup
- [ ] Smooth animations
- [ ] No memory leaks
- [ ] Efficient redraws

---

## Future Enhancements

### Advanced Animations
- Lottie animation support
- Particle effects for success states
- Fluid transitions between views

### Themes
- User-customizable color themes
- Import/export theme files
- Community theme repository

### Advanced Layouts
- Adjustable panel sizes
- Detachable settings panels
- Multi-window support

---

## Resources

**Design References:**
- WisprFlow: https://wisprflow.ai
- Human Interface Guidelines
- Material Design 3
- Fluent Design System

**Tools:**
- Figma (for mockups)
- egui inspector (for debugging)
- ColorSlurp (for color picking)
- Contrast checker (WCAG)

**Fonts:**
- Inter: https://rsms.me/inter/
- JetBrains Mono: https://jetbrains.com/mono/

---

**Last Updated:** 2025-11-19
**Next Review:** After T-027 completion (update based on implementation learnings)
