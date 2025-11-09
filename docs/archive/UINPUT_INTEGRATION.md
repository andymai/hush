# UInput Integration - Enhanced Text Insertion for Hush

This document describes the comprehensive UInput integration that has been added to Hush to provide optimal text insertion across all Linux applications.

## What's New

### 1. Linux UInput Keyboard Implementation
- **File**: `src/text/uinput_keyboard.rs`
- **Features**: 
  - Low-level Linux uinput-based keyboard emulation
  - Wide character support with automatic shift key handling
  - Configurable typing delays
  - Comprehensive error handling

### 2. Enhanced Text Insertion System
- **File**: `src/text/insertion.rs`
- **Improvements**:
  - **Priority-based method selection**: UInput (primary) → X11/enigo → clipboard (fallback)
  - **Application-aware insertion**: Detects applications that prefer clipboard
  - **Intelligent fallback**: Automatically switches methods if one fails
  - **Enhanced window detection**: Better application identification

### 3. CLI Tools for Setup
New command-line tools to help users set up UInput:

```bash
# Show comprehensive setup guide
cargo run --bin hush -- setup-uinput

# Diagnose UInput issues and get specific solutions
cargo run --bin hush -- diagnose-uinput
```

### 4. Comprehensive Documentation
- **`docs/uinput-setup.md`**: Complete setup guide with troubleshooting
- **`docs/uinput-quick-reference.md`**: Quick commands for fast setup

## Usage

### For Users

1. **Check if UInput is working**: `hush diagnose-uinput`
2. **Get setup instructions**: `hush setup-uinput`
3. **Follow the recommended method** (usually adding user to input group)

### For Developers

```rust
use hush::text::TextInserter;

let mut inserter = TextInserter::new()?;
inserter.insert_text("Hello, world!")?; // Automatically uses best method
```

## Technical Details

### Method Priority
1. **UInput** (highest priority): Works everywhere, including VMs and secure fields
2. **X11/enigo**: Good for most X11 applications
3. **Clipboard** (fallback): For applications that prefer paste operations

### Window Detection
The system detects certain applications that work better with clipboard:
- Terminals (gnome-terminal, konsole, xterm, etc.)
- Some password managers
- Secure input fields

### Error Handling
- Graceful fallback between methods
- Clear error messages with actionable guidance
- Comprehensive logging for troubleshooting

## Benefits

### Universal Compatibility
- Works with **VMs and remote applications**
- Works with **secure password fields**  
- Works with **applications that block synthetic events**
- Works with **all window managers and desktop environments**

### User Experience
- **Automatic setup guidance**: No need to figure out permissions manually
- **Diagnostic tools**: Quickly identify and fix issues
- **Intelligent method selection**: Uses the best method for each application
- **Reliable fallback**: Always attempts to insert text somehow

## Installation Requirements

### Dependencies
- `input-linux` crate for UInput functionality
- `xclip` for clipboard operations (install with `apt-get install xclip`)

### Permissions
UInput requires special permissions. The easiest setup:

```bash
sudo usermod -a -G input $USER
sudo modprobe uinput
echo 'uinput' | sudo tee /etc/modules-load.d/uinput.conf
# Then log out and log back in
```

## Testing

Use the comprehensive test suite:

```bash
# Interactive UInput testing
cargo run --bin test-working-uinput

# Test all text insertion methods
cargo run --bin test-text-insertion
```

## Architecture

```
TextInserter
├── UinputKeyboard (primary)
│   ├── Character mapping
│   ├── Key event generation  
│   └── Device management
├── enigo/X11 (secondary)
│   ├── X11 synthetic events
│   └── Window focus handling
└── Clipboard (fallback)
    ├── xclip integration
    └── Auto-paste via Ctrl+V
```

This integration makes Hush's text insertion system one of the most robust available for Linux, with universal compatibility and intelligent method selection.