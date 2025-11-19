# Voice Commands Reference

**Last Updated:** 2025-11-19
**Status:** Active reference for AI agents

Complete reference for voice commands in Hush listen mode.

## Supported Commands

### Formatting Commands

#### New Paragraph
Insert a double newline (paragraph break):
```
"Hello world new paragraph This is a new paragraph"
```
Result:
```
Hello world

This is a new paragraph
```

Alternative phrases:
- "new paragraph"
- "next paragraph"

#### New Line
Insert a single newline:
```
"First line new line Second line"
```
Result:
```
First line
Second line
```

Alternative phrases:
- "new line"
- "next line"

### Capitalization Commands

#### Capitalize That
Capitalize the first letter of the preceding text:
```
"hello world cap that"
```
Result:
```
Hello world
```

Alternative phrases:
- "cap that"
- "capitalize that"

#### All Caps
Convert preceding text to uppercase:
```
"important all caps"
```
Result:
```
IMPORTANT
```

Alternative phrases:
- "all caps"
- "upper case"

### Editing Commands

#### Undo / Delete That
Remove the last text insertion:
```
# Say: "Hello world"
# (text is inserted)
# Say: "undo that"
# (text is removed)
```

Alternative phrases:
- "undo"
- "undo that"
- "delete that"
- "scratch that"

## Command Behavior

### Standalone Commands
Commands can be used alone:
```
"undo that" → Removes last insertion
"new paragraph" → Inserts double newline
```

### Commands with Text
Commands can be mixed with dictation:
```
"First paragraph new paragraph Second paragraph"
```
Result:
```
First paragraph

Second paragraph
```

### Multiple Commands
You can chain multiple commands:
```
"Line one new line Line two new paragraph Line three"
```
Result:
```
Line one
Line two

Line three
```

### Command Processing
- Commands are detected case-insensitively
- Commands work with both rule-based and LLM text processing
- Command-formatted text (with newlines) skips filler word removal to preserve formatting
- Undo history is maintained for up to 50 insertions or 5 minutes

## Technical Architecture

The voice command system consists of three main components:

1. **CommandParser** (`src/text_processing/commands.rs`)
   - Parses transcribed text for command phrases using regex
   - Returns a `ParsedCommand` with segments of text and commands

2. **CommandExecutor** (`src/text_processing/executor.rs`)
   - Executes parsed commands
   - Builds formatted output text
   - Returns `ExecutionResult` indicating what action to take

3. **InsertionHistory** (`src/text_processing/history.rs`)
   - Tracks text insertions for undo functionality
   - Maintains up to 50 entries or 5 minutes of history
   - Automatically prunes old entries

### Integration
Voice commands are integrated into the `listen` mode pipeline:

1. Audio → Transcription → **Command Parsing**
2. **Command Execution** → Text/Undo Action
3. Text Processing (if needed) → Insertion
4. **History Recording** for undo

### File Locations

When working on voice commands, check:
- `src/text_processing/commands.rs` - Command parsing logic
- `src/text_processing/executor.rs` - Command execution
- `src/text_processing/history.rs` - Undo history tracking
- Tests in each file's `#[cfg(test)]` module

## Usage Examples

### Writing an Email
```
"Dear Sarah comma new paragraph

I wanted to follow up on our meeting yesterday period
new paragraph

The main points were colon new line
dash Project timeline new line
dash Budget approval new line
dash Next steps new paragraph

Let me know if you have any questions period new paragraph

Best comma new line
John"
```

### Writing Code Comments
```
"slash slash Main function all caps new line
slash slash Processes user input and returns result"
```

### Quick Corrections
```
"This is a test document"
# (inserted)

"undo that"
# (removed)

"This is the final version"
# (inserted)
```

## Limitations

- Commands are only available in `listen` mode (not in `record` or `manual` modes)
- Undo only works for the last 50 insertions or 5 minutes
- Cannot undo multiple insertions at once (must say "undo" multiple times)
- Select/highlight commands are not yet implemented

## Future Enhancements

Planned commands for future releases:
- "select [text]" - Highlight specific text
- "insert [snippet name]" - Expand saved snippets
- "bold that" / "italic that" - Formatting (for supported applications)
- "backspace" / "delete [n] words" - Granular deletion
- "caps on" / "caps off" - Toggle capitalization mode

## Testing

Run the voice command tests:
```bash
cargo test text_processing::commands
cargo test text_processing::executor
cargo test text_processing::history
```

Test manually:
```bash
./hush listen

# Then say:
"hello new paragraph world"

# Should insert:
# hello
#
# world
```

## Implementation Notes for AI Agents

When modifying voice command functionality:

1. **Always use ripgrep to find existing patterns:**
   ```bash
   rg "CommandParser" src/text_processing/
   rg "ExecutionResult" src/text_processing/
   ```

2. **Check existing tests before implementing:**
   ```bash
   rg "#\[test\]" src/text_processing/commands.rs
   ```

3. **Maintain backward compatibility:**
   - Don't change existing command phrases
   - Add new alternatives, don't remove old ones
   - Preserve undo history behavior

4. **Test with real speech patterns:**
   - Commands should be case-insensitive
   - Handle variations in pronunciation
   - Test with LLM processing enabled and disabled
