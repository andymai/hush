/// Test binary for the working uinput keyboard implementation
/// This validates that the input-linux crate based implementation works correctly
/// 
/// Run with: cargo run --bin test-working-uinput
/// 
/// Note: Requires appropriate permissions for /dev/uinput

use hush::text::{UinputKeyboard, check_uinput_availability};
use tracing::{info, error};
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    info!("🧪 Testing UInput Keyboard Implementation");
    info!("==========================================");

    // First check if uinput is available
    match check_uinput_availability() {
        Ok(()) => info!("✅ uinput device is accessible"),
        Err(e) => {
            error!("❌ uinput not available: {}", e);
            eprintln!("\nTo fix this, you may need to:");
            eprintln!("1. Load the uinput kernel module: sudo modprobe uinput");
            eprintln!("2. Set permissions: sudo chmod 666 /dev/uinput");
            eprintln!("3. Or add your user to the input group: sudo usermod -a -G input $USER");
            eprintln!("4. Then log out and log back in");
            return Err(e);
        }
    }

    // Create the keyboard
    let mut keyboard = match UinputKeyboard::new() {
        Ok(kb) => kb,
        Err(e) => {
            error!("Failed to create uinput keyboard: {}", e);
            return Err(e);
        }
    };

    if !keyboard.is_ready() {
        error!("Keyboard is not ready - cannot proceed with test");
        return Err(anyhow::anyhow!("Keyboard not ready"));
    }

    info!("Keyboard device: {}", keyboard.get_device_info());
    keyboard.set_typing_delay(50); // 50ms between keystrokes for testing

    println!("\n🎹 UInput Keyboard Test Suite");
    println!("==========================");

    loop {
        println!("\nChoose a test:");
        println!("1. Type basic text");
        println!("2. Type with special characters");
        println!("3. Type numbers and symbols");
        println!("4. Type multiline text");
        println!("5. Speed test (fast typing)");
        println!("6. Interactive typing");
        println!("q. Quit");
        
        print!("\nSelect option: ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        match input.trim() {
            "1" => test_basic_text(&mut keyboard)?,
            "2" => test_special_characters(&mut keyboard)?,
            "3" => test_numbers_and_symbols(&mut keyboard)?,
            "4" => test_multiline_text(&mut keyboard)?,
            "5" => test_speed_typing(&mut keyboard)?,
            "6" => test_interactive_typing(&mut keyboard)?,
            "q" | "quit" | "exit" => break,
            _ => println!("Invalid option, please try again."),
        }
    }

    println!("\n✅ Test session completed");
    Ok(())
}

fn test_basic_text(keyboard: &mut UinputKeyboard) -> anyhow::Result<()> {
    println!("\n📝 Testing basic text typing...");
    println!("Focus on the target application and wait 3 seconds...");
    countdown(3);
    
    let text = "Hello world! This is a test of basic text typing.";
    keyboard.type_text(text)?;
    
    println!("✅ Basic text test completed");
    Ok(())
}

fn test_special_characters(keyboard: &mut UinputKeyboard) -> anyhow::Result<()> {
    println!("\n🔤 Testing special characters...");
    println!("Focus on the target application and wait 3 seconds...");
    countdown(3);
    
    let text = "Testing: !@#$%^&*()_+-={}[]|\\:;\"'<>?/~`";
    keyboard.type_text(text)?;
    
    println!("✅ Special characters test completed");
    Ok(())
}

fn test_numbers_and_symbols(keyboard: &mut UinputKeyboard) -> anyhow::Result<()> {
    println!("\n🔢 Testing numbers and symbols...");
    println!("Focus on the target application and wait 3 seconds...");
    countdown(3);
    
    let text = "Numbers: 1234567890\nSymbols: !@#$%^&*()\nMixed: Price is $19.99 (20% off!)";
    keyboard.type_text(text)?;
    
    println!("✅ Numbers and symbols test completed");
    Ok(())
}

fn test_multiline_text(keyboard: &mut UinputKeyboard) -> anyhow::Result<()> {
    println!("\n📄 Testing multiline text...");
    println!("Focus on the target application and wait 3 seconds...");
    countdown(3);
    
    let text = r#"This is line 1
This is line 2
This is line 3 with symbols: !@#$%
	This line has a tab at the beginning
And this is the final line."#;
    
    keyboard.type_text(text)?;
    
    println!("✅ Multiline text test completed");
    Ok(())
}

fn test_speed_typing(keyboard: &mut UinputKeyboard) -> anyhow::Result<()> {
    println!("\n⚡ Testing fast typing (no delays)...");
    println!("Focus on the target application and wait 3 seconds...");
    countdown(3);
    
    // Temporarily set typing delay to 0 for speed test
    let original_delay = 50;
    keyboard.set_typing_delay(0);
    
    let text = "This text will be typed very quickly without delays between keystrokes to test performance.";
    
    let start = std::time::Instant::now();
    keyboard.type_text(text)?;
    let duration = start.elapsed();
    
    // Restore original delay
    keyboard.set_typing_delay(original_delay);
    
    println!("✅ Speed test completed in {:?}", duration);
    println!("  Typed {} characters", text.len());
    println!("  Rate: {:.1} chars/second", text.len() as f64 / duration.as_secs_f64());
    Ok(())
}

fn test_interactive_typing(keyboard: &mut UinputKeyboard) -> anyhow::Result<()> {
    println!("\n💬 Interactive typing test");
    println!("Type text and press Enter to send it via uinput (empty line to stop):");
    
    loop {
        print!("\nText to type: ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        let text = input.trim();
        if text.is_empty() {
            break;
        }
        
        println!("Focus target application, typing in 2 seconds...");
        countdown(2);
        
        keyboard.type_text(text)?;
        println!("✅ Text typed successfully");
    }
    
    Ok(())
}

fn countdown(seconds: u32) {
    for i in (1..=seconds).rev() {
        print!("{}... ", i);
        io::stdout().flush().unwrap();
        thread::sleep(Duration::from_secs(1));
    }
    println!("GO!");
}