/// Simple test for text insertion functionality
/// This tests just the text insertion without voice recognition
use hush::{Result, text::TextInserter};
use std::io::{self, Write};
use tracing::{info, error};

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    info!("🔧 Hush Text Insertion Test");
    info!("===========================");
    info!("");

    // Check system dependencies
    info!("🔍 Checking system dependencies...");
    match hush::text::check_dependencies() {
        Ok(_) => info!("✅ All dependencies are available"),
        Err(e) => {
            error!("❌ System dependency check failed: {}", e);
            info!("💡 Make sure you have:");
            info!("  - X11 running (check DISPLAY environment variable)");
            info!("  - xclip installed (sudo apt install xclip)");
            return Err(e);
        }
    }

    // Initialize text inserter
    info!("🔧 Initializing text insertion system...");
    let mut text_inserter = match TextInserter::new() {
        Ok(inserter) => {
            info!("✅ Text insertion system initialized successfully");
            inserter
        }
        Err(e) => {
            error!("❌ Failed to initialize text insertion system: {}", e);
            return Err(e);
        }
    };

    // Show current focused window
    match text_inserter.get_focused_window() {
        Ok(window) => {
            info!("🎯 Currently focused window:");
            info!("   Title: '{}'", window.title);
            info!("   Class: '{}'", window.class);
            info!("   Window ID: {}", window.id);
        }
        Err(e) => {
            error!("⚠️  Could not get focused window info: {}", e);
        }
    }

    info!("");
    info!("🚀 Text Insertion Test Ready!");
    info!("");
    info!("Instructions:");
    info!("1. Open a text editor, document, or any text field");
    info!("2. Click in the text field to focus it");
    info!("3. Come back to this terminal");
    info!("4. Type some text and press Enter");
    info!("5. The text will be inserted into the focused application");
    info!("");

    loop {
        // Get current focused window before each insertion
        match text_inserter.get_focused_window() {
            Ok(window) => {
                info!("🎯 Target: {} ({})", window.title, window.class);
            }
            Err(e) => {
                error!("⚠️  Warning: Could not get focused window: {}", e);
            }
        }

        // Prompt for text input
        print!("Enter text to insert (or 'quit' to exit): ");
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => {
                let text_to_insert = input.trim();
                
                if text_to_insert.is_empty() {
                    continue;
                }
                
                if text_to_insert.eq_ignore_ascii_case("quit") || text_to_insert.eq_ignore_ascii_case("exit") {
                    break;
                }
                
                info!("⌨️  Inserting text: \"{}\"", text_to_insert);
                
                match text_inserter.insert_text(text_to_insert) {
                    Ok(_) => {
                        info!("✅ Text inserted successfully!");
                    }
                    Err(e) => {
                        error!("❌ Failed to insert text: {}", e);
                        error!("💡 Make sure a text field is focused and accessible");
                        
                        // Suggest troubleshooting
                        info!("🔧 Troubleshooting tips:");
                        info!("  - Make sure the target application accepts text input");
                        info!("  - Try clicking in the text field again");
                        info!("  - Some applications may require specific focus");
                        info!("  - Check if the application is responding");
                    }
                }
            }
            Err(e) => {
                error!("❌ Failed to read input: {}", e);
                break;
            }
        }
        
        info!("");
    }

    info!("👋 Text insertion test completed. Goodbye!");
    Ok(())
}