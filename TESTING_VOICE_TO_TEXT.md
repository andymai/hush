# Testing Voice-to-Text with Text Insertion

Now that transcription is working, here's how to test the complete voice-to-text pipeline including actual text insertion into applications.

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

### Simple Text Editors (Best for initial testing):
- **Gedit** - `gedit` (GNOME Text Editor)
- **Kate** - `kate` (KDE Advanced Text Editor)  
- **Mousepad** - `mousepad` (simple text editor)
- **Nano** in terminal - `nano test.txt`

### Advanced Applications:
- **VS Code** - Great for code editing
- **LibreOffice Writer** - Word processing
- **Firefox/Chrome** - Web forms and text areas
- **Slack/Discord** - Chat applications
- **Terminal applications** - Any terminal with text input

### Web-Based (browser):
- Open any website with a text field
- Try Google search box, email compose, etc.
- Social media post boxes

## 🔧 Troubleshooting

### If Text Insertion Doesn't Work:

1. **Check focused window:**
   - Make sure you clicked in the text field
   - The application should show a cursor/focus indicator

2. **Try different applications:**
   - Some apps may not accept simulated input
   - Start with simple text editors first

3. **Check terminal output:**
   - Look for error messages in the test output
   - Window focus information is shown

4. **Verify system dependencies:**
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

- **Direct typing**: Character-by-character simulation (most compatible)
- **Clipboard**: Sets clipboard and pastes with Ctrl+V (for special characters)
- **Fallback**: Tries direct first, then clipboard if that fails

Different applications may work better with different methods.

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