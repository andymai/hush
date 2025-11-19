# Voice Commands

---
**Last Updated**: 2025-11-19
**Status**: Active
**Purpose**: Complete reference for voice commands in Hush listen mode
**Related Documents**: [Main README](../README.md) | [Documentation Index](../DOCUMENTATION_INDEX.md)
---

Hush supports voice commands that allow you to control text formatting and editing hands-free during listen mode.

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

## Technical Details

### Architecture
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

### Configuration
Voice commands are enabled by default in listen mode. To disable:
```bash
# Not yet implemented - commands are always enabled
# Future: ./hush listen --no-commands
```

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

## Troubleshooting

### Command Not Recognized
- Speak clearly and use exact command phrases
- Commands are case-insensitive, so "NEW PARAGRAPH" works same as "new paragraph"
- Check logs: `tail -f ~/.local/share/hush/logs/*.log | grep -i command`

### Undo Not Working
- Ensure you're undoing within 5 minutes of insertion
- Check that the insertion was successful before trying to undo
- Only the last insertion can be undone at a time

### Commands Being Inserted as Text
- This shouldn't happen - if it does, it's a bug
- Report with example phrase and transcription in logs

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
