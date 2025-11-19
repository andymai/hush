# Dispatcher Refactoring Plan

## Goal

Break down the monolithic `src/cli/dispatcher.rs` (1,718 lines) into modular command handlers for better maintainability, testability, and code organization.

## Current Status

### ✅ Completed

1. **Infrastructure Created**
   - Created `src/cli/commands/` directory
   - Created `src/cli/commands/mod.rs` with module organization
   - Created `src/cli/commands/utils.rs` for shared utilities

2. **Commands Extracted** (2/10)
   - ✅ `status.rs` - System status display
   - ✅ `record.rs` - Single audio recording

3. **Documentation**
   - Module-level documentation in `commands/mod.rs`
   - Function-level documentation in extracted commands
   - Unit test examples in `record.rs`

### 📋 Remaining Work

#### High Priority (Frequently Used)

1. **`listen.rs`** - Intelligent listening mode (COMPLEX: ~400 lines)
   - Push-to-talk recording
   - Overlay UI integration
   - Text processing pipeline
   - LLM integration
   - Voice command handling
   - **Estimated effort:** 4-6 hours

2. **`manual.rs`** - Manual recording mode (~80 lines)
   - Interactive recording loop
   - Multiple recordings support
   - **Estimated effort:** 1 hour

3. **`setup.rs`** - System setup commands (~200 lines)
   - UInput setup and diagnostics
   - Audio device configuration
   - Hotkey setup and testing
   - **Estimated effort:** 2-3 hours

#### Medium Priority

4. **`test.rs`** - Testing commands (~150 lines)
   - Audio capture tests
   - Transcription tests
   - Text insertion tests
   - Pipeline tests
   - **Estimated effort:** 2 hours

5. **`models.rs`** - Model management (~100 lines)
   - Model downloading
   - Model listing
   - Model verification
   - **Estimated effort:** 1-2 hours

#### Low Priority (Less Frequently Used)

6. **`install.rs`** - Installation commands (~50 lines)
   - Desktop integration
   - Autostart setup
   - System-wide installation
   - **Estimated effort:** 1 hour

7. **`uninstall.rs`** - Uninstallation commands (~50 lines)
   - Remove desktop integration
   - Remove autostart
   - Clean up system files
   - **Estimated effort:** 1 hour

8. **`start.rs`** - Start daemon (~40 lines)
   - Daemon mode
   - Elevated mode
   - CLI mode
   - **Estimated effort:** 1 hour

## Architecture

### Module Structure

```
src/cli/
├── mod.rs (exports commands module)
├── dispatcher.rs (routing only - will shrink to ~200 lines)
└── commands/
    ├── mod.rs (module organization)
    ├── utils.rs (shared utilities)
    ├── status.rs ✅
    ├── record.rs ✅
    ├── listen.rs TODO
    ├── manual.rs TODO
    ├── setup.rs TODO
    ├── test.rs TODO
    ├── models.rs TODO
    ├── install.rs TODO
    ├── uninstall.rs TODO
    └── start.rs TODO
```

### Pattern to Follow

Each command module should:

1. **Have clear function signature:**
   ```rust
   pub async fn handle_<command>(args...) -> Result<()>
   ```

2. **Include documentation:**
   ```rust
   /// Brief description
   ///
   /// # Arguments
   /// * `arg1` - Description
   ///
   /// # Examples
   /// ```no_run
   /// handle_command(args).await?;
   /// ```
   ```

3. **Use shared utilities from `utils.rs`:**
   ```rust
   use super::utils::{show_config_status, setup_audio};
   ```

4. **Include unit tests where applicable:**
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       // Add tests
   }
   ```

## Refactoring Steps

### For Each Command

1. **Create new file** `src/cli/commands/<command>.rs`

2. **Extract handler function** from `dispatcher.rs`
   - Copy the `handle_<command>` function
   - Extract any helper functions it uses
   - Add imports for Hush components

3. **Update `commands/mod.rs`**
   - Add `pub mod <command>;`
   - Add `pub use <command>::handle_<command>;`

4. **Update `dispatcher.rs`**
   - Add import: `use crate::cli::commands::handle_<command>;`
   - Replace method call with function call:
     ```rust
     // Before
     self.handle_<command>(args).await

     // After
     crate::cli::commands::handle_<command>(args).await
     ```

5. **Test the command**
   ```bash
   cargo build
   ./hush <command> [args]
   ```

6. **Add unit tests** if applicable

### Final Cleanup

Once all commands are extracted:

1. **Remove `CommandDispatcher` struct** (no longer needed)
2. **Simplify `dispatcher.rs`** to just routing logic (~200 lines)
3. **Update documentation** to reflect new structure
4. **Run full test suite**
5. **Update `ARCHITECTURE.md`** with new CLI structure

## Benefits

### Before (Current)
- ❌ Single 1,718-line file
- ❌ Difficult to navigate
- ❌ Hard to test individual commands
- ❌ Merge conflicts likely
- ❌ Poor code organization

### After (Goal)
- ✅ ~10 focused files (~100-400 lines each)
- ✅ Easy to find specific command
- ✅ Individual command testing
- ✅ Parallel development possible
- ✅ Clear separation of concerns
- ✅ Better documentation

## Testing Strategy

### Per-Command Tests

Each command module should have:

1. **Smoke tests** - Command doesn't panic
2. **Argument validation** - Correct handling of parameters
3. **Error cases** - Proper error messages

### Integration Tests

Create `tests/cli_commands_tests.rs`:

```rust
#[tokio::test]
async fn test_record_command() {
    // Test record command end-to-end
}

#[tokio::test]
async fn test_status_command() {
    // Test status command end-to-end
}
// ... etc
```

## Timeline

| Priority | Commands | Estimated Time | Dependencies |
|----------|----------|----------------|--------------|
| High | listen, manual, setup | 7-10 hours | None |
| Medium | test, models | 3-4 hours | High priority done |
| Low | install, uninstall, start | 3 hours | None (parallel) |
| **Total** | **8 commands** | **13-17 hours** | |

## Rollout Strategy

### Phase 1: Infrastructure (✅ DONE)
- Create module structure
- Extract 2 simple commands as examples
- Document the pattern

### Phase 2: High Priority
1. Extract `manual` (simple, low risk)
2. Extract `setup` (moderate complexity)
3. Extract `listen` (complex, high value)

### Phase 3: Medium Priority
4. Extract `test`
5. Extract `models`

### Phase 4: Low Priority
6. Extract `install`, `uninstall`, `start` in parallel

### Phase 5: Cleanup
7. Remove CommandDispatcher struct
8. Simplify dispatcher.rs to routing only
9. Update documentation
10. Run full test suite

## Success Criteria

- [ ] All 10 commands extracted to separate files
- [ ] Each file < 500 lines
- [ ] All commands have documentation
- [ ] All commands have basic tests
- [ ] Full test suite passes
- [ ] No regressions in functionality
- [ ] `dispatcher.rs` < 300 lines
- [ ] Architecture docs updated

## Notes

- **Backward compatibility:** Command behavior should not change
- **Error handling:** Maintain existing error handling patterns
- **Testing:** Test each extraction before moving to next
- **Documentation:** Update as you go, not at the end
- **Code review:** Each extraction should be a separate commit

## References

- Current implementation: `src/cli/dispatcher.rs`
- Example extractions: `src/cli/commands/status.rs`, `src/cli/commands/record.rs`
- Utilities: `src/cli/commands/utils.rs`
- Module organization: `src/cli/commands/mod.rs`
