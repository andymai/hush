# Task Documentation Reference

**For all tasks T-027 through T-035:**

## Essential Documentation

### 1. **UI Implementation Guide** (START HERE)
**File:** `.ai/knowledge/UI-IMPLEMENTATION-GUIDE.md`
**Use for:** Quick reference, checklists, getting started
**Read first:** Yes - gives overview and links to other docs

### 2. **Design Specification**
**File:** `.ai/knowledge/ui-design-specification.md`
**Use for:** Exact colors (#4CAF8C), typography (Inter), spacing (4px grid), component specs
**Read when:** Implementing any visual element

### 3. **Visual Mockups**
**File:** `.ai/knowledge/ui-mockups.md`
**Use for:** Layout structure, screen flows, component composition
**Read when:** Understanding how screens should look

### 4. **Polish Principles**
**File:** `.ai/knowledge/ui-polish-principles.md`
**Use for:** The "feel" - spacing that breathes, optical alignment, animation timing
**Read when:** Basic implementation done, refining for polish

### 5. **Implementation Roadmap**
**File:** `.ai/tasks/available/UI-EXPANSION-ROADMAP.md`
**Use for:** Task order, dependencies, timeline, strategy
**Read when:** Planning which task to work on next

---

## Quick Reference

**Colors:**
```
Primary:    #4CAF8C  (sage green)
Background: #FAF8F5  (light) / #1C1C1E (dark)
Error:      #E74C3C  (coral red)
Warning:    #F5A623  (amber)
```

**Typography:**
```
Font:  Inter
Heading: 16-32px, Semibold (600)
Body:    13-14px, Regular (400)
```

**Spacing:**
```
4px base grid: 8px, 12px, 16px, 24px, 32px
```

**Components:**
```
Buttons: 36px height, 8px radius
Inputs:  36px height, 6px radius
```

**Animations:**
```
Fast:    150ms (hover)
Medium:  250ms (modal)
Easing:  cubic-bezier(0.4, 0, 0.2, 1)
```

---

## Implementation Checklist

- [ ] Functionality works
- [ ] Colors match spec
- [ ] Spacing follows 4px grid
- [ ] Animations smooth (60 FPS)
- [ ] No visual glitches
- [ ] Keyboard nav works
- [ ] cargo check passes
- [ ] cargo clippy passes
- [ ] cargo test passes

Target: 85% polish (perfect is not required!)

---

## Document Updates (2025-11-19)

**Consolidated from 150KB → 88KB:**
- Removed format comparison docs (not using alternate formats)
- Removed YAML/DSL examples (sticking with ASCII + specs)
- Removed Loveable deep-dive (interesting but not actionable)
- Consolidated polish guide (trimmed from 27KB → 7KB)
- Created unified implementation guide

**What remains:**
- Core design system (specification + mockups)
- Polish principles (the "feel" guide)
- Implementation guide (quick reference)
- Roadmap (strategy)
- Individual task files (requirements)

**Focus:** Actionable, task-supporting documentation only.
