# Architecture Decision Records (ADRs)

---
**Last Updated**: 2025-11-19
**Status**: Active
**Purpose**: Index of all architectural decisions for the Hush project
**Related Documents**: [Architecture](../ARCHITECTURE.md) | [Design Patterns](../DESIGN_PATTERNS.md) | [Documentation Index](../../../DOCUMENTATION_INDEX.md)
---

This directory contains Architecture Decision Records for the Hush Voice-to-Text project.

## What is an ADR?

An Architecture Decision Record (ADR) documents an important architectural decision made along with its context and consequences.

## Format

Each ADR follows this structure:

- **Title**: Numbered sequentially (ADR-001, ADR-002, etc.)
- **Status**: Proposed, Accepted, Rejected, Superseded, Deprecated
- **Context**: The problem or situation requiring a decision
- **Decision**: What we decided to do and why
- **Consequences**: Positive and negative outcomes of the decision
- **Alternatives**: Other options considered and why they were rejected

## Active ADRs

### [ADR-001: Trait-Based Architecture](./ADR-001-trait-based-architecture.md)
**Status**: Partially Implemented (UInput complete Oct 2025, other components in progress)
**Summary**: Adopt trait-based architecture using the Adapter Pattern to decouple components and enable dependency injection.

**Key Points**:
- Define traits for all component interfaces
- Implement adapters for existing concrete types
- Refactor HushApp to accept trait objects
- Enable mockability and testing

**Impact**: High - Affects all components

---

### [ADR-002: Error Handling Strategy](./ADR-002-error-handling-strategy.md)
**Status**: Partially Implemented (Oct 2025)
**Summary**: Structured error handling using thiserror for domain errors while keeping anyhow for application-level errors.

**Key Points**:
- Layered error handling (application vs domain)
- Custom error types with pattern matching
- User-friendly error messages
- Automatic retry logic for transient errors

**Impact**: High - Affects error handling throughout codebase

---

### [ADR-003: Centralized State Management](./ADR-003-centralized-state-management.md)
**Status**: Partially Implemented (Oct 2025)
**Summary**: Implement centralized state machine with type-safe state transitions using Observer Pattern.

**Key Points**:
- Single source of truth for application state
- Type-safe state transitions
- Observer pattern for component reactions
- State history for debugging

**Impact**: High - Eliminates duplicated state across components

---

## Decision Process

1. **Identify** the need for an architectural decision
2. **Research** alternatives and trade-offs
3. **Draft** an ADR following the template
4. **Review** with team and iterate
5. **Accept** or reject the decision
6. **Implement** accepted decisions
7. **Update** ADR status as implementation progresses

## ADR Lifecycle

```
Proposed → Accepted → Implemented
    ↓
Rejected
    ↓
Superseded (by newer ADR)
    ↓
Deprecated
```

## Templates

See [ADR Template](./template.md) for creating new ADRs.

## References

- [ADR GitHub Organization](https://adr.github.io/)
- [Documenting Architecture Decisions](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions)
- [Architecture Decision Records at Spotify](https://engineering.atspotify.com/2020/04/when-should-i-write-an-architecture-decision-record/)
