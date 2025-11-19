# AI Coding Agent Protocol - Hush Voice-to-Text

## System Overview

You are an AI coding agent working autonomously on the **Hush** project - a fast, accurate voice-to-text application for Linux developers. Multiple agents (2-3) may work concurrently on different tasks using isolated git branches. Your job: complete assigned tasks without hallucinating, with full verification at each step.

**Core Principle:** Code is truth, search is verification. Never assume—always grep.

**Project Context:** Hush is a Rust application using trait-based architecture with GPU-accelerated Whisper transcription, local-first privacy, and universal text insertion.

## File Structure

```
/
├── CLAUDE.md                    # This file
├── .ai/
│   ├── knowledge/
│   │   ├── conventions.md       # "We use trait-based architecture with adapters"
│   │   └── architecture.md      # "Traits in src/core/, adapters in src/adapters/"
│   ├── tasks/
│   │   ├── available/           # Unclaimed tasks
│   │   ├── claimed/             # Tasks in progress (filename: T-XXX-{agent-id}.md)
│   │   ├── blocked/             # Failed tasks needing human review
│   │   └── complete/            # Finished tasks
│   └── sessions/                # Optional: append-only debug logs
├── src/
│   ├── core/                    # Core traits and types
│   ├── adapters/                # Platform-specific implementations
│   ├── application/             # Application orchestration
│   └── ...
└── docs/                        # Architecture documentation
```

## Task Lifecycle

### 1. Claim Task

```bash
# Pick first available task
TASK=$(ls .ai/tasks/available/ | head -1)
TASK_ID=$(basename "$TASK" .md)

# Atomic claim via rename
if mv ".ai/tasks/available/${TASK}" \
       ".ai/tasks/claimed/${TASK_ID}-${AGENT_ID}.md" 2>/dev/null; then
  echo "Claimed ${TASK_ID}"
else
  echo "Task unavailable, pick another"
  exit 1
fi
```

**Note:** With 2-3 agents, collision rate is <1%. If claim fails, just pick next task.

### 2. Create Isolated Branch

```bash
git checkout -b "agent/${AGENT_ID}/${TASK_ID}"

# Record starting point for context decay detection
BASE_COMMIT=$(git rev-parse HEAD)
echo "${BASE_COMMIT}" > .ai/sessions/${TASK_ID}.base
```

### 3. Load Context (Budget: ~18K tokens)

**Run these searches once at task start:**

```bash
# 1. Project structure (3K tokens)
find . -maxdepth 3 -type d | grep -v -E 'target|\.git|node_modules' | sort

# 2. Knowledge files (3K tokens)
cat .ai/knowledge/conventions.md
cat .ai/knowledge/architecture.md

# 3. Find relevant patterns (10K tokens)
# Extract key terms from task, search codebase
rg "{pattern_from_task}" --type rust --context=2 --max-count=5

# 4. Check existing tests (2K tokens)
fd "_test\.rs$|tests\.rs$|^test_" tests/ src/ | head -10
```

**Hush-Specific Context Essentials:**
- Trait definitions: `src/core/traits.rs`
- Error types: `src/core/error.rs`
- State management: `src/core/state.rs`
- Adapters: `src/adapters/*`
- Architecture docs: `docs/ARCHITECTURE.md`, `docs/architecture/DESIGN_PATTERNS.md`

**Context expires after 2 hours.** See "Context Decay" below.

### 4. Anti-Hallucination Protocol

**NEVER assume. ALWAYS verify.**

#### Before Writing Any Rust Code:

```bash
# Using a trait?
rg "pub trait TraitName" --type rust

# Importing a type?
rg "pub struct TypeName|pub enum TypeName" --type rust

# Using a function/method?
rg "pub fn function_name|fn function_name" --type rust --context=1

# Using a dependency?
grep "package-name" Cargo.toml

# Implementing a trait?
rg "impl.*TraitName.*for" --type rust

# Checking existing tests?
rg "#\[test\]|#\[tokio::test\]" tests/ --type rust
```

**Hush-Specific Verification:**

```bash
# Before implementing AudioSource trait
rg "pub trait AudioSource" src/core/traits.rs

# Before creating an adapter
ls src/adapters/ && rg "Adapter" --type rust

# Before using error types
rg "pub enum HushError|pub enum.*Error" src/core/error.rs

# Before modifying state machine
rg "pub enum AppState" src/core/state.rs
```

**If search returns nothing:**

1. Search broader: `rg "FunctionName" src/`
2. Check similar files: `fd "similar-pattern" | xargs rg "FunctionName"`
3. Still not found? **STOP.** Add to task notes:

   ```
   UNKNOWN DEPENDENCY: FunctionName
   - Searched: [list patterns tried]
   - Not found in codebase
   - Options: Create it, or flag for human review
   ```

**NEVER fill gaps with assumptions like:**

- "This probably exists in…"
- "Typically this would be…"
- "Based on common Rust patterns…"

If uncertain, explicitly state what you don't know and block the task.

### 5. Execution with Verification

**80/20 Rule:** Spend 80% of effort searching/verifying, 20% writing code.

#### After Every Code Change:

```bash
# 1. Type check (fast, do this frequently)
cargo check

# 2. Run affected tests
cargo test --test {test_module} -- --nocapture
# Or: cargo test {test_function_name}

# 3. Run all tests (before commit)
cargo test

# 4. Lint check (required before commit)
cargo clippy -- -D warnings

# 5. Format check (required before commit)
cargo fmt --check

# 6. Commit as checkpoint
git add {changed_files}
git commit -m "Step N: {what_changed_and_why}"
```

**Hush-Specific Verification Checklist (Every Change):**

- [ ] Trait bounds verified (Send + Sync for async traits)
- [ ] Error types used correctly (HushError variants)
- [ ] State transitions are valid (check StateMachine)
- [ ] Imports verified via ripgrep before writing
- [ ] `cargo check` passes
- [ ] Tests run and pass
- [ ] `cargo clippy` has no warnings
- [ ] `cargo fmt` applied
- [ ] Change committed to branch

### 6. Failure Handling

Track failure diversity, not just count:

**STOP immediately if:**

- Same fix attempted 3 times (logic error, not execution error)
- Total time exceeds 2 hours (context decay risk)
- Token budget exceeds 100K (~$3)

**Continue exploring if:**

- 5+ different approaches tried (valid exploration)
- Tests passing but functionality incomplete
- Making measurable progress (e.g., reducing compiler errors)

**When Blocked:**

```bash
# 1. Create detailed block report
cat > .ai/tasks/blocked/${TASK_ID}.md << EOF
# Blocked: ${TASK_ID}

## Task Description
{original_task_from_claimed_file}

## Branch
agent/${AGENT_ID}/${TASK_ID}

## Attempts
{count} attempts over {duration}

## Last Error
\`\`\`
{error_message_or_test_output}
\`\`\`

## Compiler Output
\`\`\`
{cargo_check_output}
\`\`\`

## Suspected Issue
{hypothesis_about_root_cause}

## What I Tried
1. {approach_1} - Result: {outcome}
2. {approach_2} - Result: {outcome}
...

## Context Age
Started: {start_time}
Base commit: ${BASE_COMMIT}
Current main: $(git rev-parse origin/main)
EOF

# 2. Push branch for human review
git push origin "agent/${AGENT_ID}/${TASK_ID}"

# 3. Move claimed task to blocked
mv ".ai/tasks/claimed/${TASK_ID}-${AGENT_ID}.md" \
   ".ai/tasks/blocked/${TASK_ID}.md"
```

### 7. Context Decay Detection

After 2 hours OR before pushing:

```bash
BASE_COMMIT=$(cat .ai/sessions/${TASK_ID}.base)
CURRENT_MAIN=$(git rev-parse origin/main)

if [ "${BASE_COMMIT}" != "${CURRENT_MAIN}" ]; then
  echo "WARNING: Base branch advanced during task execution"
  echo "Base: ${BASE_COMMIT}"
  echo "Current: ${CURRENT_MAIN}"

  # Rerun critical searches
  rg "{key_patterns_from_task}" --type rust --max-count=3

  # Check for API changes in modified Rust files
  git diff ${BASE_COMMIT}..${CURRENT_MAIN} --name-only | \
    grep "\.rs$" | \
    head -5

  # Decision point: Continue or restart?
  # If trait definitions/APIs changed, consider restarting task with fresh context
fi
```

**Automatic refresh required:**

- Sessions exceeding 2 hours
- Before final push
- After any test failure (to catch codebase changes)

### 8. Complete Task

```bash
# 1. Final verification
cargo test                      # All tests pass
cargo clippy -- -D warnings     # No lint warnings
cargo fmt --check               # Code formatted
cargo build --release           # Release build works

# 2. Push branch
git push origin "agent/${AGENT_ID}/${TASK_ID}"

# 3. Create PR
gh pr create \
  --title "${TASK_ID}: {brief_description}" \
  --body "$(cat .ai/tasks/claimed/${TASK_ID}-${AGENT_ID}.md)" \
  --label "ai-agent"

# 4. Mark complete
mv ".ai/tasks/claimed/${TASK_ID}-${AGENT_ID}.md" \
   ".ai/tasks/complete/${TASK_ID}.md"
```

## Grounding Rules - Rust & Hush Specific

### Absolute Requirements

1. **NEVER assume traits exist** → Run `rg "pub trait {name}" src/core/` first
2. **NEVER trust imports from memory** → Run `rg "pub (struct|enum|trait) {Name}"` every session
3. **NEVER claim tests pass without running them** → Execute `cargo test` and verify output
4. **NEVER skip type checking** → Run `cargo check` after every meaningful change
5. **STOP after 3 identical failures** → Create BLOCKED report
6. **CONTINUE after 5 different approaches** → Exploration is valid
7. **REFRESH context after 2 hours** → Base commit may have advanced
8. **ALWAYS use trait objects properly** → Verify `Box<dyn Trait>` or `Arc<dyn Trait>` usage
9. **NEVER ignore compiler warnings** → `cargo clippy -- -D warnings` must pass

### Hush-Specific Rules

1. **Trait-First Development**: Always check if a trait exists before creating concrete types
2. **Adapter Pattern**: New platform code goes in `src/adapters/`, not `src/core/`
3. **Error Handling**: Use `HushError` variants, never generic `anyhow!()` at boundaries
4. **State Management**: All state transitions go through `StateMachine`
5. **Async Boundaries**: Use `#[async_trait]` for async trait methods
6. **Testing**: Create mock implementations in `src/core/mocks.rs` for traits
7. **Documentation**: All public APIs must have doc comments

### When Uncertain

```bash
# 1. Search for trait definitions
rg "pub trait" src/core/traits.rs --context=3

# 2. Check adapter implementations
fd "adapter" src/adapters/ --type f

# 3. Review error types
rg "pub enum.*Error" src/core/error.rs

# 4. Check state machine
cat src/core/state.rs | grep "pub enum AppState" -A 10

# 5. Examine existing tests for patterns
fd "test" tests/ | head -5 | xargs cat

# 6. Still unknown?
# Document in task notes, DO NOT guess
echo "⚠ UNKNOWN: {what_you_don't_know}" >> .ai/sessions/${TASK_ID}.log
```

**NEVER say:**

- "This likely exists in…"
- "Based on typical Rust patterns…"
- "Assuming the trait is…"

**Instead say:**

- "I cannot find trait X in src/core/"
- "Search returned no results for Y"
- "Blocking task: Unknown dependency Z"

## Token Budget Awareness

**Target per task:** 40-70K tokens (~$1-2)
**Hard limit:** 100K tokens (~$3)

**Context loads:**

- Cold start: ~18K tokens
- Warm refresh: ~3.5K tokens
- Per verification cycle: ~3K tokens
- Compiler output: ~2-5K tokens per error batch

**If approaching limit:**

1. Check if making progress (fewer compiler errors, tests passing)
2. If yes: Continue to completion
3. If no: Block task, don't waste tokens on thrashing

## Success Criteria

A task is complete when:

- [ ] All tests pass (`cargo test`)
- [ ] No compiler warnings (`cargo clippy -- -D warnings`)
- [ ] Code formatted (`cargo fmt`)
- [ ] Type checking passes (`cargo check`)
- [ ] Code matches conventions in `.ai/knowledge/`
- [ ] Trait implementations are correct (Send + Sync where needed)
- [ ] Error handling uses structured HushError types
- [ ] Documentation added for public APIs
- [ ] Branch pushed to remote
- [ ] PR created with clear description
- [ ] Task moved to `complete/`

A task should be blocked when:

- [ ] Same compiler error after 3 identical fix attempts
- [ ] Unknown traits/types that need human clarification
- [ ] Context >2 hours old without progress
- [ ] Token budget exceeded without resolution
- [ ] Test failures you cannot diagnose
- [ ] Borrow checker errors you cannot resolve after 3 attempts

## Best Practices - Rust Edition

**Do:**

- Run `cargo check` after every change
- Search for trait definitions before implementing
- Use `rg "pub trait.*{Name}"` to verify traits exist
- Run `cargo test` after every commit
- Block early if uncertain about trait bounds
- Document what you tried when blocking
- Use `cargo clippy` suggestions
- Follow existing error handling patterns

**Don't:**

- Trust your memory of trait definitions
- Skip type checking to save time
- Continue past 3 identical borrow checker errors
- Guess at trait bounds or lifetimes
- Use `unwrap()` or `expect()` in production code
- Ignore compiler warnings
- Merge main into your branch (use rebase or flag for review)

## Common Pitfalls - Rust Specific

1. **"I'll just implement this trait, it probably has these methods"** → NO. Grep first.
2. **"The tests passed last time"** → NO. Run `cargo test` again.
3. **"This is the 4th borrow checker error but I'll try once more"** → NO. Block the task.
4. **"Main branch advanced but I'm almost done"** → NO. Refresh context.
5. **"I don't see the trait but maybe it's in a dependency"** → Check Cargo.toml first.
6. **"This needs Send + Sync probably"** → NO. Check the trait definition.
7. **"I'll use anyhow for this error"** → NO. Use HushError variants.

## Emergency Procedures

**If you realize mid-task you hallucinated:**

```bash
# 1. Assess damage
git diff HEAD~N  # Review questionable commits

# 2. Check if it compiles
cargo check

# 3. If caught early (1-2 commits)
git reset --soft HEAD~N
# Fix issues, recommit properly

# 4. If caught late (many commits, types wrong)
# Add note to blocked task:
echo "⚠ POSSIBLE HALLUCINATION: {what_you_assumed}" >> blocked_notes.md
# Push branch anyway, let human review
```

**If cargo check suddenly fails after working:**

```bash
# Check if base branch advanced
git fetch origin
git log HEAD..origin/main --oneline

# If changed, context is stale
# Check for trait/API changes
git diff HEAD..origin/main -- src/core/

# Option 1: Rebase and fix conflicts
git rebase origin/main

# Option 2: Block task, flag for review on fresh branch
```

**If stuck on borrow checker errors:**

```bash
# After 3 attempts, STOP
# Document the error:
cat > .ai/sessions/${TASK_ID}-borrow-error.md << EOF
# Borrow Checker Issue

## Error
\`\`\`
{cargo_check_output}
\`\`\`

## What I Tried
1. {attempt_1}
2. {attempt_2}
3. {attempt_3}

## Hypothesis
{what_you_think_is_wrong}
EOF

# Block the task
```

## Agent Identity

Set at start of each session:

```bash
export AGENT_ID="agent-$(date +%s)-$$"  # timestamp-processid
```

This uniquely identifies your work across claimed files, branches, and logs.

## Hush Project Quick Reference

### Project Structure
```
src/
├── core/              # Traits, errors, state, mocks
├── adapters/          # Platform implementations (audio, text, hotkey, transcription)
├── application/       # Application orchestration
├── audio/             # Audio capture and feedback
├── cli/               # Command-line interface
├── overlay/           # Visual overlay UI
├── text/              # Text insertion
├── text_processing/   # Intelligent text processing
├── transcription/     # Whisper integration
└── ...
```

### Common Commands

```bash
# Build debug
make build

# Run application
./hush listen

# Run tests
cargo test

# Check types
cargo check

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt

# Build release
make release

# Run specific test
cargo test test_name -- --nocapture
```

### Key Files to Check Before Changes

1. `src/core/traits.rs` - All trait definitions
2. `src/core/error.rs` - Error types
3. `src/core/state.rs` - State machine
4. `src/core/mocks.rs` - Mock implementations for testing
5. `docs/ARCHITECTURE.md` - Architecture overview
6. `docs/architecture/DESIGN_PATTERNS.md` - Design patterns
7. `.ai/knowledge/conventions.md` - Project conventions
8. `.ai/knowledge/architecture.md` - Architecture summary

-----

**Remember:** You're not writing Rust from memory. You're orchestrating a codebase through search and verification. The codebase is the source of truth—your role is to discover what's there through `rg` and `fd`, verify with `cargo check`, and never invent what isn't.

**When in doubt:** `rg`, `cargo check`, `cargo test`. In that order. Every time.
