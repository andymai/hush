# AI Agent Directory

This directory contains the AI Coding Agent Protocol structure for the Hush project.

## Directory Structure

```
.ai/
├── README.md                 # This file
├── knowledge/                # Project knowledge base
│   ├── conventions.md       # Coding conventions, patterns, standards
│   └── architecture.md      # Architecture summary and quick reference
├── tasks/
│   ├── available/           # Tasks ready to be claimed
│   ├── claimed/             # Tasks currently being worked on
│   ├── blocked/             # Tasks that are blocked and need human review
│   └── complete/            # Completed tasks (archive)
└── sessions/                # Session logs and debugging info (optional)
```

## Purpose

This structure enables:

1. **Multiple AI agents** to work concurrently on different tasks
2. **Knowledge preservation** across sessions
3. **Task coordination** without conflicts
4. **Debugging support** with session logs

## How It Works

### Knowledge Base

- **`conventions.md`**: Coding standards, patterns, testing conventions, and best practices for Hush
- **`architecture.md`**: Quick reference for architecture, traits, directory structure, and common patterns

AI agents should load these files at the start of each task to understand project context.

### Task Management

Tasks follow a lifecycle:

```
available/ → claimed/ → complete/
                ↓
            blocked/  (if stuck)
```

**Task File Naming:**
- Available: `T-XXX-description.md` (e.g., `T-001-add-wayland-support.md`)
- Claimed: `T-XXX-{agent-id}.md` (e.g., `T-001-agent-1732012345-12345.md`)
- Blocked: `T-XXX.md` (moves back to original name)
- Complete: `T-XXX.md` (archived)

### Sessions

Optional directory for agents to log debugging information:
- `{task-id}.base` - Base commit hash for context decay detection
- `{task-id}.log` - Debug logs and notes
- `{task-id}-error.md` - Error analysis

## Creating Tasks

To create a new task for AI agents:

```bash
cat > .ai/tasks/available/T-XXX-task-name.md << 'EOF'
# Task: Task Name

## Description
What needs to be done

## Requirements
- [ ] Requirement 1
- [ ] Requirement 2

## Success Criteria
- Tests pass
- Code is documented
- No clippy warnings

## Context
Any additional context or constraints

## Files to Check
- `src/core/traits.rs`
- `src/adapters/`
EOF
```

## Agent Workflow

See `CLAUDE.md` in the project root for the complete AI Coding Agent Protocol.

Quick summary:

1. **Claim task** - Atomic move from `available/` to `claimed/`
2. **Create branch** - `agent/{agent-id}/{task-id}`
3. **Load context** - Read knowledge files + search codebase
4. **Verify constantly** - Use `rg`, `cargo check`, `cargo test`
5. **Commit often** - After each logical change
6. **Complete or block** - Move task to `complete/` or `blocked/`

## For Humans

### Reviewing Blocked Tasks

When an agent blocks a task (moves it to `blocked/`):

1. Read the blocked task file for context
2. Check the agent's branch: `agent/{agent-id}/{task-id}`
3. Review what was attempted
4. Either:
   - Fix the issue and move task back to `available/`
   - Provide clarification in the task file
   - Close the task if not needed

### Monitoring Progress

```bash
# See available tasks
ls .ai/tasks/available/

# See tasks in progress
ls .ai/tasks/claimed/

# See blocked tasks
ls .ai/tasks/blocked/

# See completed tasks
ls .ai/tasks/complete/
```

## Maintenance

- **Update knowledge files** when conventions or architecture change
- **Clean up sessions/** periodically (after PRs merge)
- **Archive complete tasks** if the directory gets too large
- **Review blocked tasks** regularly to unblock agents

## Integration with Development

This directory is **git-tracked** so that:
- Knowledge is preserved across sessions
- Task history is maintained
- Blocked tasks can be reviewed

The `.ai/` directory is a **reference implementation** of the AI Coding Agent Protocol adapted specifically for the Hush project.

---

**For the full protocol, see:** `CLAUDE.md` in the project root
**For project architecture:** `.ai/knowledge/architecture.md`
**For coding conventions:** `.ai/knowledge/conventions.md`
**For all knowledge files:** `.ai/knowledge/` directory
