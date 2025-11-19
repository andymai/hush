# Hush Documentation Index

**Last Updated**: 2025-11-19
**Purpose**: Central navigation hub for all Hush project documentation

---

## Quick Start

### For Users
1. **[README.md](README.md)** - Project overview and quick start guide
2. **[Voice Commands Guide](docs/VOICE_COMMANDS.md)** - How to use voice commands
3. **[UInput Quick Reference](docs/uinput-quick-reference.md)** - Fast setup for text insertion
4. **[UInput Setup Guide](docs/uinput-setup.md)** - Detailed setup and troubleshooting

### For Developers
1. **[Contributing Guide](CONTRIBUTING.md)** - How to contribute to Hush
2. **[Architecture Overview](docs/ARCHITECTURE.md)** - System architecture and design
3. **[Design Patterns](docs/architecture/DESIGN_PATTERNS.md)** - Core trait patterns and abstractions
4. **[Error Handling Guide](docs/ERROR_HANDLING.md)** - Error handling patterns and best practices
5. **[Refactoring Plan](docs/REFACTORING_PLAN.md)** - Ongoing code refactoring roadmap
6. **[Architecture Decision Records](docs/architecture/adrs/README.md)** - Key architectural decisions

---

## User Documentation

### Getting Started
- **[Main README](README.md)** - Installation, quick start, and usage examples
- **[Model Management](models/README.md)** - Downloading and managing Whisper models

### Features and Usage
- **[Voice Commands](docs/VOICE_COMMANDS.md)** - Complete voice command reference
  - Formatting commands (new paragraph, new line)
  - Capitalization commands (cap that, all caps)
  - Editing commands (undo, delete that)

### Setup Guides
- **[UInput Quick Reference](docs/uinput-quick-reference.md)** - TL;DR setup for most users
- **[UInput Setup Guide](docs/uinput-setup.md)** - Comprehensive setup with troubleshooting
  - Universal text insertion compatibility
  - Works with VMs, password fields, and all applications
  - Detailed permission setup

### Recent Features
- **[Polish & Distribution Features](POLISH_AND_DISTRIBUTION.md)** - Latest features (2025-11-14)
  - Desktop integration (install, autostart)
  - Settings UI overlay
  - Audio feedback system

---

## Developer Documentation

### Architecture
- **[Architecture Overview](docs/ARCHITECTURE.md)** - Trait-based architecture (Phase 1 complete)
  - Component abstractions
  - Mock implementations
  - Migration roadmap
  - Current status and metrics

- **[Design Patterns](docs/architecture/DESIGN_PATTERNS.md)** - Core abstractions and patterns
  - AudioSource, Transcriber, TextOutput, InputTrigger traits
  - Platform abstraction
  - State management
  - Implementation status

- **[Complete Improvements Summary](docs/CHANGELOG.md)** - Recent improvements and refactoring
  - Code review fixes
  - Rust best practices (Oct 2025)
  - Type safety enhancements

### Architecture Decision Records (ADRs)
- **[ADR Index](docs/architecture/adrs/README.md)** - All architectural decisions
- **[ADR-001: Trait-Based Architecture](docs/architecture/adrs/ADR-001-trait-based-architecture.md)** - Component decoupling
- **[ADR-002: Error Handling Strategy](docs/architecture/adrs/ADR-002-error-handling-strategy.md)** - Structured errors
- **[ADR-003: Centralized State Management](docs/architecture/adrs/ADR-003-centralized-state-management.md)** - State machine design

### Implementation Guides
- **[Contributing Guide](CONTRIBUTING.md)** - Development setup, coding standards, PR process
- **[Error Handling Guide](docs/ERROR_HANDLING.md)** - Error system usage and patterns
- **[Refactoring Plan](docs/REFACTORING_PLAN.md)** - CLI dispatcher refactoring roadmap
- **[Design Patterns Analysis](docs/archive/DESIGN_PATTERNS_ANALYSIS.md)** - UInput trait alignment analysis

---

## Archive (Historical Reference)

These documents capture previous planning, analysis, and implementation approaches. They may contain outdated information but are kept for historical reference.

### Planning Documents
- **[Wispr Flow UX Plan](docs/archive/WISPR_FLOW_UX_PLAN.md)** - Original overlay UI planning
- **[Design Patterns Analysis](docs/archive/DESIGN_PATTERNS_ANALYSIS.md)** - UInput refactoring analysis

### Implementation Notes
- **[Simple Whisper Integration](docs/archive/SIMPLE_WHISPER_INTEGRATION.md)** - Whisper.cpp integration notes
- **[UInput Integration](docs/archive/UINPUT_INTEGRATION.md)** - Original UInput implementation doc
- **[Testing Voice-to-Text](docs/archive/TESTING_VOICE_TO_TEXT.md)** - Testing guidelines

---

## Documentation by Topic

### Audio & Transcription
- Model management: [models/README.md](models/README.md)
- Whisper integration: [docs/archive/SIMPLE_WHISPER_INTEGRATION.md](docs/archive/SIMPLE_WHISPER_INTEGRATION.md)
- Testing: [docs/archive/TESTING_VOICE_TO_TEXT.md](docs/archive/TESTING_VOICE_TO_TEXT.md)

### Text Insertion
- Quick setup: [docs/uinput-quick-reference.md](docs/uinput-quick-reference.md)
- Detailed guide: [docs/uinput-setup.md](docs/uinput-setup.md)
- Integration details: [docs/archive/UINPUT_INTEGRATION.md](docs/archive/UINPUT_INTEGRATION.md)
- Design analysis: [docs/archive/DESIGN_PATTERNS_ANALYSIS.md](docs/archive/DESIGN_PATTERNS_ANALYSIS.md)

### User Interface
- Voice commands: [docs/VOICE_COMMANDS.md](docs/VOICE_COMMANDS.md)
- Settings UI: [POLISH_AND_DISTRIBUTION.md](POLISH_AND_DISTRIBUTION.md) (Settings UI section)
- Overlay planning: [docs/archive/WISPR_FLOW_UX_PLAN.md](docs/archive/WISPR_FLOW_UX_PLAN.md)

### Architecture & Design
- System overview: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- Core patterns: [docs/architecture/DESIGN_PATTERNS.md](docs/architecture/DESIGN_PATTERNS.md)
- Error handling: [docs/ERROR_HANDLING.md](docs/ERROR_HANDLING.md)
- Refactoring plan: [docs/REFACTORING_PLAN.md](docs/REFACTORING_PLAN.md)
- All ADRs: [docs/architecture/adrs/](docs/architecture/adrs/)
- Improvements log: [docs/CHANGELOG.md](docs/CHANGELOG.md)
- Contributing: [CONTRIBUTING.md](CONTRIBUTING.md)

---

## Document Status Legend

- **Active**: Currently accurate and maintained
- **Reference**: Accurate but not actively updated
- **Archived**: Historical reference, may contain outdated information
- **Deprecated**: Replaced by newer documentation

---

## Contributing to Documentation

When adding or updating documentation:

1. **Add standardized header** with:
   - Last updated date (YYYY-MM-DD format)
   - Status (Active/Reference/Archived/Deprecated)
   - Purpose/scope
   - Related documents

2. **Update this index** when adding new documentation

3. **Cross-reference** related documents

4. **Archive outdated docs** instead of deleting them

5. **Keep user docs separate** from developer docs

---

## Getting Help

### Quick Troubleshooting
- Text insertion not working? → [UInput Quick Reference](docs/uinput-quick-reference.md)
- Audio issues? → [Main README - Troubleshooting](README.md#troubleshooting)
- Model issues? → [Model README](models/README.md#troubleshooting)
- Voice commands not working? → [Voice Commands - Troubleshooting](docs/VOICE_COMMANDS.md#troubleshooting)

### Deep Dives
- Understanding architecture → [Architecture Overview](docs/ARCHITECTURE.md)
- Understanding design decisions → [ADRs](docs/architecture/adrs/README.md)
- Contributing to codebase → [Contributing Guide](CONTRIBUTING.md)
- Error handling patterns → [Error Handling Guide](docs/ERROR_HANDLING.md)
- Ongoing refactoring → [Refactoring Plan](docs/REFACTORING_PLAN.md)

---

**Note**: This index is maintained to help both users and developers quickly find relevant documentation. If you find broken links or missing documentation, please update this index.
