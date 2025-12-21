use crate::text::uinput_keyboard::{check_uinput_availability, UinputKeyboard};
use crate::Result;
use enigo::{Direction, Enigo, Key, Keyboard};
use std::thread;
use std::time::Duration;
use tracing::{debug, info, warn};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, Window};
use x11rb::rust_connection::RustConnection;

pub struct TextInserter {
    uinput_keyboard: Option<UinputKeyboard>,
    enigo: Enigo,
    x11_conn: RustConnection,
    screen_num: usize,
    clipboard_fallback: bool,
    typing_delay_ms: u64,
}

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub id: Window,
    pub title: String,
    pub class: String,
    pub is_focused: bool,
}

#[derive(Debug)]
pub enum InsertionMethod {
    Uinput,
    Direct,
    Clipboard,
    Fallback,
}

impl TextInserter {
    pub fn new() -> Result<Self> {
        info!("Initializing text insertion system");

        // Try to initialize uinput keyboard (primary method)
        let uinput_keyboard = match UinputKeyboard::new() {
            Ok(kb) => {
                if kb.is_ready() {
                    info!("  Uinput keyboard: ready (primary method)");
                    Some(kb)
                } else {
                    warn!("  Uinput keyboard: not ready - will use fallback");
                    None
                }
            },
            Err(e) => {
                warn!("  Uinput keyboard: failed to initialize - {}", e);
                None
            },
        };

        // Initialize X11 connection
        let (x11_conn, screen_num) = x11rb::connect(None)
            .map_err(|e| anyhow::anyhow!("Failed to connect to X11 server: {:?}", e))?;

        // Initialize Enigo for keyboard simulation (fallback method)
        let enigo = Enigo::new(&enigo::Settings::default())
            .map_err(|e| anyhow::anyhow!("Failed to initialize keyboard controller: {:?}", e))?;

        info!("Text insertion system initialized successfully");
        info!("  X11 connection: established");
        info!("  Screen number: {}", screen_num);
        info!("  Enigo controller: ready (fallback method)");

        if uinput_keyboard.is_some() {
            info!("  Primary method: uinput (kernel-level)");
        } else {
            info!("  Primary method: enigo (X11 simulation)");
        }

        Ok(TextInserter {
            uinput_keyboard,
            enigo,
            x11_conn,
            screen_num,
            clipboard_fallback: true,
            typing_delay_ms: 10, // Small delay between keystrokes for reliability
        })
    }

    pub fn insert_text(&mut self, text: &str) -> Result<()> {
        info!(
            "Inserting text: '{}' ({} chars)",
            text.chars().take(50).collect::<String>(),
            text.len()
        );

        // Get the currently focused window for context
        let focused_window = self.get_focused_window()?;
        debug!(
            "Target window: {} ({})",
            focused_window.title, focused_window.class
        );

        // Choose insertion method based on text content and target application
        let method = self.choose_insertion_method(text, &focused_window);
        debug!("Using insertion method: {:?}", method);

        match method {
            InsertionMethod::Uinput => self.insert_uinput(text),
            InsertionMethod::Direct => self.insert_direct(text),
            InsertionMethod::Clipboard => self.insert_via_clipboard(text),
            InsertionMethod::Fallback => self.insert_with_fallback(text),
        }
    }

    pub fn get_focused_window(&self) -> Result<WindowInfo> {
        // Get the root window
        let screen = &self.x11_conn.setup().roots[self.screen_num];
        let root_window = screen.root;

        // Query for the currently focused window
        let reply = self
            .x11_conn
            .get_input_focus()
            .map_err(|e| anyhow::anyhow!("Failed to get input focus: {:?}", e))?
            .reply()
            .map_err(|e| anyhow::anyhow!("Failed to get focus reply: {:?}", e))?;

        let focused_window = if reply.focus == x11rb::NONE || reply.focus == root_window {
            // If no specific window is focused, use root window
            root_window
        } else {
            reply.focus
        };

        // Get window properties
        let title = self
            .get_window_property(focused_window, "_NET_WM_NAME")
            .or_else(|_| self.get_window_property(focused_window, "WM_NAME"))
            .unwrap_or_else(|_| "Unknown".to_string());

        let class = self
            .get_window_property(focused_window, "WM_CLASS")
            .unwrap_or_else(|_| "Unknown".to_string());

        Ok(WindowInfo {
            id: focused_window,
            title,
            class,
            is_focused: true,
        })
    }

    pub fn set_clipboard_fallback(&mut self, enabled: bool) {
        self.clipboard_fallback = enabled;
        info!(
            "Clipboard fallback: {}",
            if enabled { "enabled" } else { "disabled" }
        );
    }

    pub fn set_typing_delay(&mut self, delay_ms: u64) {
        self.typing_delay_ms = delay_ms;
        info!("Typing delay set to {}ms", delay_ms);
    }

    fn choose_insertion_method(&self, text: &str, window: &WindowInfo) -> InsertionMethod {
        // If uinput is available, use it as the primary method for all cases
        // except when application specifically prefers clipboard
        if self.uinput_keyboard.is_some() {
            // Check for applications that strongly prefer clipboard
            let strongly_prefers_clipboard = self.window_strongly_prefers_clipboard(&window.class);

            // Check text length - extremely long text might still be better via clipboard
            let is_extremely_long = text.len() > 5000;

            if strongly_prefers_clipboard || is_extremely_long {
                if self.clipboard_fallback {
                    InsertionMethod::Clipboard
                } else {
                    InsertionMethod::Uinput // Still prefer uinput over enigo
                }
            } else {
                InsertionMethod::Uinput
            }
        } else {
            // Fallback to original logic when uinput is not available
            let has_special_chars = text.chars().any(|c| {
                !c.is_ascii_alphanumeric()
                    && !c.is_ascii_whitespace()
                    && !".,!?-_()[]{}:;\"'".contains(c)
            });

            let is_long_text = text.len() > 1000;
            let prefers_clipboard = self.window_prefers_clipboard(&window.class);

            if has_special_chars || is_long_text || prefers_clipboard {
                if self.clipboard_fallback {
                    InsertionMethod::Clipboard
                } else {
                    InsertionMethod::Fallback
                }
            } else {
                InsertionMethod::Direct
            }
        }
    }

    fn window_prefers_clipboard(&self, window_class: &str) -> bool {
        let class_lower = window_class.to_lowercase();

        // Applications known to work better with clipboard insertion
        let clipboard_preferred = [
            "terminal",
            "gnome-terminal",
            "konsole",
            "xterm",
            "terminator",
            "code",
            "sublime",
            "atom",
            "vim",
            "emacs",
            "firefox",
            "chrome",
            "chromium",
            "slack",
            "discord",
            "teams",
        ];

        clipboard_preferred
            .iter()
            .any(|&app| class_lower.contains(app))
    }

    fn window_strongly_prefers_clipboard(&self, window_class: &str) -> bool {
        let class_lower = window_class.to_lowercase();

        // Applications that strongly prefer clipboard even over uinput
        // (very few applications should be in this list)
        let strongly_preferred: &[&str] = &[
            // Add applications here that have issues with uinput
            // For now, keeping this minimal as uinput should work universally
        ];

        strongly_preferred
            .iter()
            .any(|&app| class_lower.contains(app))
    }

    fn insert_uinput(&mut self, text: &str) -> Result<()> {
        debug!("Inserting text via uinput (kernel-level)");

        if let Some(uinput) = &mut self.uinput_keyboard {
            // Cancel any Alt-triggered menu focus (common issue with Ctrl+Alt hotkeys on GTK/KDE)
            let _ = uinput.send_escape();

            // Set the typing delay to match our configured delay
            uinput.set_typing_delay(self.typing_delay_ms);

            // Use uinput to type the text
            uinput
                .type_text(text)
                .map_err(|e| anyhow::anyhow!("Uinput typing failed: {}", e))?;

            info!("Text inserted via uinput ({} characters)", text.len());
            Ok(())
        } else {
            Err(anyhow::anyhow!("Uinput keyboard not available"))
        }
    }

    fn insert_direct(&mut self, text: &str) -> Result<()> {
        debug!("Inserting text directly via keyboard simulation");

        // Type the text character by character
        for (i, ch) in text.char_indices() {
            if i > 0 && i % 100 == 0 {
                debug!("Typed {} characters so far", i);
            }

            // Handle special characters
            match ch {
                '\n' => {
                    self.enigo
                        .key(Key::Return, Direction::Click)
                        .map_err(|e| anyhow::anyhow!("Failed to type newline: {:?}", e))?;
                },
                '\t' => {
                    self.enigo
                        .key(Key::Tab, Direction::Click)
                        .map_err(|e| anyhow::anyhow!("Failed to type tab: {:?}", e))?;
                },
                _ => {
                    // For regular characters, use text input
                    let char_str = ch.to_string();
                    self.enigo
                        .text(&char_str)
                        .map_err(|e| anyhow::anyhow!("Failed to type '{}': {:?}", ch, e))?;
                },
            }

            // Small delay for reliability
            if self.typing_delay_ms > 0 {
                thread::sleep(Duration::from_millis(self.typing_delay_ms));
            }
        }

        info!("Text inserted directly ({} characters)", text.len());
        Ok(())
    }

    fn insert_via_clipboard(&mut self, text: &str) -> Result<()> {
        debug!("Inserting text via clipboard");

        // Set clipboard content
        self.set_clipboard_text(text)?;

        // Small delay to ensure clipboard is set
        thread::sleep(Duration::from_millis(50));

        // Paste using Ctrl+V
        self.enigo
            .key(Key::Control, Direction::Press)
            .map_err(|e| anyhow::anyhow!("Failed to press Ctrl: {:?}", e))?;

        self.enigo
            .key(Key::Unicode('v'), Direction::Click)
            .map_err(|e| anyhow::anyhow!("Failed to press V: {:?}", e))?;

        self.enigo
            .key(Key::Control, Direction::Release)
            .map_err(|e| anyhow::anyhow!("Failed to release Ctrl: {:?}", e))?;

        info!("Text inserted via clipboard ({} characters)", text.len());
        Ok(())
    }

    fn insert_with_fallback(&mut self, text: &str) -> Result<()> {
        warn!("Using fallback insertion method");

        // Priority order: uinput -> direct (enigo) -> clipboard -> fail

        // Try uinput first if available
        if self.uinput_keyboard.is_some() {
            match self.insert_uinput(text) {
                Ok(_) => {
                    info!("Fallback: uinput insertion succeeded");
                    return Ok(());
                },
                Err(e) => {
                    warn!("Uinput insertion failed: {:?}", e);
                },
            }
        }

        // Try direct (enigo) insertion next
        match self.insert_direct(text) {
            Ok(_) => {
                info!("Fallback: direct insertion succeeded");
                Ok(())
            },
            Err(e) => {
                warn!("Direct insertion failed: {:?}", e);
                if self.clipboard_fallback {
                    info!("Attempting clipboard fallback");
                    self.insert_via_clipboard(text)
                } else {
                    Err(anyhow::anyhow!("All insertion methods failed"))
                }
            },
        }
    }

    /// Undo last insertion by sending backspace keys
    pub fn undo_last_insertion(&mut self, char_count: usize) -> Result<()> {
        info!("Undoing last insertion ({} characters)", char_count);

        // Use uinput if available for more reliable backspace
        if let Some(uinput) = &mut self.uinput_keyboard {
            // Send backspace keys to delete the characters
            for i in 0..char_count {
                if i > 0 && i % 100 == 0 {
                    debug!("Deleted {} characters so far", i);
                }

                // Send backspace key
                uinput.send_backspace()?;

                // Small delay between backspaces for reliability
                thread::sleep(Duration::from_millis(self.typing_delay_ms));
            }

            info!("Undo complete via uinput");
            Ok(())
        } else {
            // Fallback to enigo for backspace
            debug!("Using enigo for undo (fallback)");

            for i in 0..char_count {
                if i > 0 && i % 100 == 0 {
                    debug!("Deleted {} characters so far", i);
                }

                self.enigo
                    .key(Key::Backspace, Direction::Click)
                    .map_err(|e| anyhow::anyhow!("Failed to send backspace: {:?}", e))?;

                thread::sleep(Duration::from_millis(self.typing_delay_ms));
            }

            info!("Undo complete via enigo");
            Ok(())
        }
    }

    fn set_clipboard_text(&self, text: &str) -> Result<()> {
        use std::process::Command;

        // Use xclip to set clipboard content (most reliable method)
        let mut child = Command::new("xclip")
            .arg("-selection")
            .arg("clipboard")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn xclip: {:?}", e))?;

        if let Some(stdin) = child.stdin.as_mut() {
            use std::io::Write;
            stdin
                .write_all(text.as_bytes())
                .map_err(|e| anyhow::anyhow!("Failed to write to xclip stdin: {:?}", e))?;
        }

        let status = child
            .wait()
            .map_err(|e| anyhow::anyhow!("Failed to wait for xclip: {:?}", e))?;

        if !status.success() {
            return Err(anyhow::anyhow!("xclip failed with status: {:?}", status));
        }

        debug!("Clipboard content set successfully");
        Ok(())
    }

    fn get_window_property(&self, window: Window, property: &str) -> Result<String> {
        // Get atom for the property name
        let atom_reply = self
            .x11_conn
            .intern_atom(false, property.as_bytes())
            .map_err(|e| anyhow::anyhow!("Failed to intern atom: {:?}", e))?
            .reply()
            .map_err(|e| anyhow::anyhow!("Failed to get atom reply: {:?}", e))?;

        // Get the property value
        let property_reply = self
            .x11_conn
            .get_property(
                false, // delete
                window,
                atom_reply.atom,
                AtomEnum::ANY, // type
                0,             // long_offset
                u32::MAX,      // long_length
            )
            .map_err(|e| anyhow::anyhow!("Failed to get property: {:?}", e))?
            .reply()
            .map_err(|e| anyhow::anyhow!("Failed to get property reply: {:?}", e))?;

        // Convert bytes to string
        let property_str = String::from_utf8_lossy(&property_reply.value).into_owned();

        // Clean up the string (remove null terminators, etc.)
        Ok(property_str.trim_end_matches('\0').to_string())
    }

    pub fn test_insertion(&mut self) -> Result<()> {
        info!("Running text insertion test");

        // Test basic text insertion
        let test_text = "Hello from Hush voice-to-text!";
        self.insert_text(test_text)?;

        // Small delay
        thread::sleep(Duration::from_millis(500));

        // Test special characters
        let special_text = "\nThis includes: special-chars, numbers123, and symbols!@#";
        self.insert_text(special_text)?;

        info!("Text insertion test completed successfully");
        Ok(())
    }
}

impl Drop for TextInserter {
    fn drop(&mut self) {
        info!("Cleaning up text insertion system");
        // X11 connection will be closed automatically
    }
}

// Helper function to check if required system dependencies are available
pub fn check_dependencies() -> Result<()> {
    use std::process::Command;

    // Check for uinput availability (primary insertion method)
    match check_uinput_availability() {
        Ok(_) => info!("✅ UInput available - using optimal text insertion method"),
        Err(e) => {
            warn!("❌ UInput not available - {}", e);
            warn!("⚠️  Some applications (VMs, password fields) may not work optimally");
            warn!("📖 Run 'hush setup-uinput' for detailed setup instructions");
            info!("Will fallback to X11 simulation and clipboard methods");
        },
    }

    // Check for xclip (required for clipboard operations)
    match Command::new("xclip").arg("-version").output() {
        Ok(_) => debug!("xclip is available"),
        Err(_) => {
            warn!("xclip not found - clipboard operations may fail");
            warn!("Install with: sudo apt-get install xclip");
        },
    }

    // Check X11 display
    if std::env::var("DISPLAY").is_err() {
        return Err(anyhow::anyhow!(
            "No X11 display found - DISPLAY environment variable not set"
        ));
    }

    info!("Text insertion dependencies check completed");
    Ok(())
}

/// Provides detailed guidance for setting up uinput permissions
pub fn print_uinput_setup_guidance() {
    println!("\n🔧 UInput Setup Guide for Hush Voice-to-Text");
    println!("═══════════════════════════════════════════════");
    println!();

    println!("UInput provides the best text insertion experience, working universally");
    println!("across all applications, including VMs and secure password fields.\n");

    println!("📋 Quick Setup (choose one method):\n");

    println!("Method 1 - Add user to input group (RECOMMENDED):");
    println!("  sudo usermod -a -G input $USER");
    println!("  sudo modprobe uinput");
    println!("  echo 'uinput' | sudo tee /etc/modules-load.d/uinput.conf");
    println!("  # Then log out and log back in\n");

    println!("Method 2 - Temporary fix (until reboot):");
    println!("  sudo modprobe uinput");
    println!("  sudo chmod 666 /dev/uinput\n");

    println!("Method 3 - Persistent udev rule:");
    println!("  sudo tee /etc/udev/rules.d/99-uinput.rules << 'EOF'");
    println!("  SUBSYSTEM==\"misc\", KERNEL==\"uinput\", GROUP=\"input\", MODE=\"0664\", TAG+=\"uaccess\"");
    println!("  EOF");
    println!("  sudo udevadm control --reload-rules && sudo udevadm trigger");
    println!("  sudo modprobe uinput\n");

    println!("🔍 Verification:");
    println!("  groups | grep input     # Should show 'input' in your groups");
    println!("  ls -la /dev/uinput      # Should show read/write access");
    println!("  lsmod | grep uinput     # Should show uinput module loaded\n");

    println!("ℹ️  Note: Hush will work without uinput using fallback methods,");
    println!("   but uinput provides the best compatibility and reliability.\n");

    println!("📖 For detailed setup instructions and troubleshooting:");
    println!("   See docs/uinput-setup.md or docs/uinput-quick-reference.md\n");
}

/// Analyzes the current uinput setup and provides specific guidance
pub fn diagnose_uinput_issues() -> Result<()> {
    use std::path::Path;
    use std::process::Command;

    println!("\n🔍 Diagnosing UInput Setup");
    println!("═════════════════════════");

    // Check if /dev/uinput exists
    if !Path::new("/dev/uinput").exists() {
        println!("❌ /dev/uinput not found");
        println!("   Solution: sudo modprobe uinput\n");
        return Ok(());
    } else {
        println!("✅ /dev/uinput exists");
    }

    // Check if uinput module is loaded
    match Command::new("lsmod").output() {
        Ok(output) => {
            if String::from_utf8_lossy(&output.stdout).contains("uinput") {
                println!("✅ uinput module is loaded");
            } else {
                println!("⚠️  uinput module not found in lsmod");
                println!("   This might be built into the kernel (which is fine)");
            }
        },
        Err(_) => println!("⚠️  Could not check loaded modules (lsmod unavailable)"),
    }

    // Check permissions
    match std::fs::OpenOptions::new().write(true).open("/dev/uinput") {
        Ok(_) => println!("✅ Can write to /dev/uinput"),
        Err(e) => {
            println!("❌ Cannot write to /dev/uinput: {}", e);

            // Check current user groups
            if let Ok(output) = Command::new("groups").output() {
                let groups = String::from_utf8_lossy(&output.stdout);
                if groups.contains("input") {
                    println!("   You are in the 'input' group, but still can't write.");
                    println!("   Try logging out and logging back in.");
                } else {
                    println!("   You are not in the 'input' group.");
                    println!("   Solution: sudo usermod -a -G input $USER");
                    println!("   Then log out and log back in.");
                }
            }

            // Check file permissions
            if let Ok(output) = Command::new("ls").args(["-la", "/dev/uinput"]).output() {
                println!(
                    "   Current permissions: {}",
                    String::from_utf8_lossy(&output.stdout).trim()
                );
                println!("   Should be: crw-rw---- 1 root input ...");
            }
        },
    }

    println!("\n💡 Quick fixes:");
    println!("   Temporary:  sudo chmod 666 /dev/uinput");
    println!("   Permanent:  sudo usermod -a -G input $USER (then logout/login)");

    Ok(())
}
