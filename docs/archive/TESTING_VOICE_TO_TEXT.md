# Testing Voice-to-Text with Text Insertion

---
**STATUS**: 🗄️ **ARCHIVED** - Historical Reference Only
**Last Updated**: 2025-11-19
**Purpose**: Historical testing instructions from development phase
**Related Documents**: [Main README](../../README.md) | [UInput Setup](../uinput-setup.md) | [Documentation Index](../../DOCUMENTATION_INDEX.md)
---

> **⚠️ ARCHIVED DOCUMENT**: This document contains testing instructions from the development phase. For current usage and testing instructions, see the [main README](../../README.md) and [UInput Setup Guide](../uinput-setup.md).

This document describes how to test the complete voice-to-text pipeline including actual text insertion into applications.

## 🧪 Available Test Binaries

### 1. `test-text-insertion-simple`
**Purpose**: Test just text insertion without voice recognition  
**Best for**: Verifying that text insertion works with your applications

```bash
cargo run --bin test-text-insertion-simple
```

**How to use:**
1. Run the command in terminal
2. Open any text editor (gedit, kate, VS Code, browser text field, etc.)
3. Click in the text field to focus it
4. Switch back to the terminal
5. Type text and press Enter
6. Watch it appear in the text field!

### 2. `test-voice-to-text` 
**Purpose**: Complete voice-to-text pipeline with real transcription  
**Best for**: Full end-to-end testing with actual speech

```bash
cargo run --bin test-voice-to-text
```

**How to use:**
1. Run the command in terminal  
2. Open any text editor or text field
3. Click in the text field to focus it
4. Follow the on-screen instructions
5. Speak when prompted
6. Watch your speech appear as text!

## 📝 Step-by-Step Testing Guide

### Step 1: Test Text Insertion First

1. **Open a terminal** and navigate to the hush project:
   ```bash
   cd /home/andy/Git/hush
   ```

2. **Run the text insertion test:**
   ```bash
   cargo run --bin test-text-insertion-simple
   ```

3. **Open a simple text editor** (in another window):
   - Gedit: `gedit` or search for "Text Editor" in applications
   - Kate: `kate` (if available)  
   - Or any text field in a browser

4. **Click in the text field** to focus it

5. **Switch back to terminal** and type some test text like:
   ```
   Hello, this is a test from Hush!
   ```

6. **Press Enter** - the text should appear in your text editor!

### Step 2: Test Complete Voice-to-Text

Once text insertion is working:

1. **Run the complete voice-to-text test:**
   ```bash
   cargo run --bin test-voice-to-text
   ```

2. **Follow the setup instructions** shown in the terminal

3. **Open your preferred text editor or application**

4. **Click in a text field** to focus it

5. **Press Enter** in the terminal to start recording

6. **Speak clearly** when prompted (you have 5 seconds)

7. **Watch your speech get transcribed and inserted!**

## 🎯 Recommended Test Applications

ℹ️ **Note**: With the new UInput integration, Hush now works with **ALL applications** including VMs, password fields, and secure contexts!

### Universal Compatibility (UInput Method):
- **All applications** - Works universally at kernel level
- **Password managers** - KeePass, 1Password, etc.
- **Virtual machines** - VMware, VirtualBox, QEMU
- **Games and fullscreen apps** - Any application
- **Secure contexts** - Lock screens, sudo prompts
- **Terminal applications** - SSH sessions, vim, nano
- **Web applications** - Any browser-based app

### Simple Text Editors (Great for testing):
- **Gedit** - `gedit` (GNOME Text Editor)
- **Kate** - `kate` (KDE Advanced Text Editor)  
- **Mousepad** - `mousepad` (simple text editor)
- **Nano** in terminal - `nano test.txt`

### Advanced Applications:
- **VS Code** - Code editing (now works perfectly!)
- **LibreOffice Writer** - Word processing
- **Firefox/Chrome** - Web forms and text areas
- **Slack/Discord** - Chat applications
- **IntelliJ IDEA** - IDEs that previously had issues

### Previously Problematic (Now Fixed with UInput!):
- **Password fields** - Now work perfectly
- **VirtualBox/VMware guests** - Full compatibility
- **Games with anti-cheat** - Kernel-level emulation bypasses restrictions
- **Wayland applications** - Universal compatibility

## 🔧 Troubleshooting

### If Text Insertion Doesn't Work:

1. **Check UInput setup (recommended method):**
   ```bash
   # Check if UInput is working
   hush diagnose-uinput
   
   # Get setup instructions if needed
   hush setup-uinput
   ```

2. **Quick UInput fix:**
   ```bash
   sudo modprobe uinput
   sudo chmod 666 /dev/uinput
   ```

3. **Check focused window:**
   - Make sure you clicked in the text field
   - The application should show a cursor/focus indicator

4. **Try different applications:**
   - With UInput, ALL applications should work
   - If only some apps work, UInput setup may be incomplete

5. **Check terminal output:**
   - Look for error messages in the test output
   - UInput availability is shown during startup

6. **Verify fallback dependencies (if UInput unavailable):**
   - X11 is running (DISPLAY is set)
   - xclip is installed (`which xclip`)

### If Voice Transcription Doesn't Work:

1. **Check audio:**
   - Make sure your microphone is working
   - Test with `arecord -f cd -t wav -d 5 test.wav` then `aplay test.wav`

2. **Check model:**
   - Make sure you have a model downloaded
   - Run `cargo run --bin simple-model-manager list`

3. **Speak clearly:**
   - Speak directly into microphone
   - Avoid background noise
   - Try simple phrases first

## 🎨 Insertion Methods

The text inserter automatically chooses the best method:

1. **UInput (Primary)**: Hardware-level keyboard emulation via Linux kernel
   - ✅ Works with ALL applications universally
   - ✅ VMs, password fields, secure contexts
   - ✅ Games, fullscreen apps, Wayland/X11
   - ⚠️ Requires setup: `hush setup-uinput`

2. **X11/enigo (Fallback)**: X11 synthetic events
   - ✅ Good for most GUI applications
   - ❌ May not work in VMs or secure contexts
   - ❌ Limited compatibility with some apps

3. **Clipboard (Last resort)**: Sets clipboard and pastes with Ctrl+V
   - ✅ Works when other methods fail
   - ✅ Good for special characters and long text
   - ❌ Overwrites current clipboard content

**Automatic Selection**: Hush intelligently chooses the best method for each situation.

## 📊 Expected Performance

### Transcription Quality:
- **Tiny model**: Fast but lower accuracy
- **Base model**: Good balance of speed and accuracy  
- **Larger models**: Higher accuracy but slower

### Text Insertion:
- **Speed**: Nearly instantaneous for short text
- **Compatibility**: Works with most GUI applications
- **Reliability**: Very high with proper focus

## 🚀 Next Steps

Once both work well:

1. **Try different model sizes** for better accuracy
2. **Test with various applications** you use daily  
3. **Experiment with different speaking styles**
4. **Consider integrating into your workflow**

## 💡 Tips for Best Results

### For Voice Recognition:
- Speak clearly and at normal pace
- Minimize background noise
- Use a good microphone if available
- Try shorter phrases initially

### For Text Insertion:
- Ensure the target application is focused
- Start with simple text editors
- Check that the application accepts text input
- Some applications may require specific focus methods

---

**Ready to test?** Start with the simple text insertion test first, then move to the complete voice-to-text pipeline!