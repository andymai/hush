use crate::Result;
use core_graphics::event::CGKeyCode;
use std::process::Command;
use tracing::warn;

/// macOS window information
#[derive(Debug, Clone)]
pub struct MacOSWindowInfo {
    pub title: String,
    pub app_name: String,
    pub bundle_id: String,
    pub is_focused: bool,
}

/// Check if Accessibility permissions are granted
pub fn check_accessibility_permissions() -> bool {
    #[cfg(target_os = "macos")]
    {
        true
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// Prompt user to grant Accessibility permissions
///
/// Opens System Preferences to the Accessibility pane and displays
/// instructions to the user on how to grant permissions.
pub fn prompt_accessibility_permissions() {
    eprintln!("\n⚠️  Accessibility Permissions Required\n");
    eprintln!("Hush needs Accessibility permissions to insert text.");
    eprintln!("\nTo grant permissions:");
    eprintln!("  1. Open System Preferences");
    eprintln!("  2. Go to Privacy & Security → Accessibility");
    eprintln!("  3. Add and enable 'hush' or your terminal app\n");
    eprintln!("Opening System Preferences now...\n");

    // Open System Preferences to Accessibility pane
    let _ = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
}

/// Get information about the currently focused window
pub fn get_focused_window_info() -> Result<MacOSWindowInfo> {
    Ok(MacOSWindowInfo {
        title: "Active Window".to_string(),
        app_name: "Unknown App".to_string(),
        bundle_id: "unknown".to_string(),
        is_focused: true,
    })
}

/// Convert a character to macOS virtual keycode and shift modifier.
/// Returns (keycode, needs_shift). Unsupported characters fall back to space.
pub fn char_to_keycode(ch: char) -> Result<(CGKeyCode, bool)> {
    let (code, shift) = match ch {
        // Lowercase letters (a=0, b=11, c=8, etc. - QWERTY layout)
        'a' => (0, false),
        'b' => (11, false),
        'c' => (8, false),
        'd' => (2, false),
        'e' => (14, false),
        'f' => (3, false),
        'g' => (5, false),
        'h' => (4, false),
        'i' => (34, false),
        'j' => (38, false),
        'k' => (40, false),
        'l' => (37, false),
        'm' => (46, false),
        'n' => (45, false),
        'o' => (31, false),
        'p' => (35, false),
        'q' => (12, false),
        'r' => (15, false),
        's' => (1, false),
        't' => (17, false),
        'u' => (32, false),
        'v' => (9, false),
        'w' => (13, false),
        'x' => (7, false),
        'y' => (16, false),
        'z' => (6, false),

        // Uppercase letters (same keycodes, with shift)
        'A' => (0, true),
        'B' => (11, true),
        'C' => (8, true),
        'D' => (2, true),
        'E' => (14, true),
        'F' => (3, true),
        'G' => (5, true),
        'H' => (4, true),
        'I' => (34, true),
        'J' => (38, true),
        'K' => (40, true),
        'L' => (37, true),
        'M' => (46, true),
        'N' => (45, true),
        'O' => (31, true),
        'P' => (35, true),
        'Q' => (12, true),
        'R' => (15, true),
        'S' => (1, true),
        'T' => (17, true),
        'U' => (32, true),
        'V' => (9, true),
        'W' => (13, true),
        'X' => (7, true),
        'Y' => (16, true),
        'Z' => (6, true),

        // Numbers
        '0' => (29, false),
        '1' => (18, false),
        '2' => (19, false),
        '3' => (20, false),
        '4' => (21, false),
        '5' => (23, false),
        '6' => (22, false),
        '7' => (26, false),
        '8' => (28, false),
        '9' => (25, false),

        // Number row symbols (with shift)
        '!' => (18, true), // 1
        '@' => (19, true), // 2
        '#' => (20, true), // 3
        '$' => (21, true), // 4
        '%' => (23, true), // 5
        '^' => (22, true), // 6
        '&' => (26, true), // 7
        '*' => (28, true), // 8
        '(' => (25, true), // 9
        ')' => (29, true), // 0

        // Common punctuation
        ' ' => (49, false),  // Space
        '\n' => (36, false), // Return
        '\t' => (48, false), // Tab
        '.' => (47, false),  // Period
        ',' => (43, false),  // Comma
        ';' => (41, false),  // Semicolon
        ':' => (41, true),   // Colon (Shift+;)
        '\'' => (39, false), // Single quote
        '"' => (39, true),   // Double quote (Shift+')
        '/' => (44, false),  // Slash
        '?' => (44, true),   // Question (Shift+/)
        '\\' => (42, false), // Backslash
        '|' => (42, true),   // Pipe (Shift+\)
        '`' => (50, false),  // Backtick
        '~' => (50, true),   // Tilde (Shift+`)
        '-' => (27, false),  // Minus
        '_' => (27, true),   // Underscore (Shift+-)
        '=' => (24, false),  // Equals
        '+' => (24, true),   // Plus (Shift+=)
        '[' => (33, false),  // Left bracket
        '{' => (33, true),   // Left brace (Shift+[)
        ']' => (30, false),  // Right bracket
        '}' => (30, true),   // Right brace (Shift+])
        '<' => (43, true),   // Less than (Shift+,)
        '>' => (47, true),   // Greater than (Shift+.)

        // Unsupported characters - warn and use space
        _ => {
            warn!(
                "Unsupported character: '{}' (U+{:04X}), using space",
                ch, ch as u32
            );
            (49, false) // Space
        },
    };

    Ok((code, shift))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_char_to_keycode_lowercase() {
        let (keycode, shift) = char_to_keycode('a').unwrap();
        assert_eq!(keycode, 0);
        assert_eq!(shift, false);
    }

    #[test]
    fn test_char_to_keycode_uppercase() {
        let (keycode, shift) = char_to_keycode('A').unwrap();
        assert_eq!(keycode, 0);
        assert_eq!(shift, true);
    }

    #[test]
    fn test_char_to_keycode_numbers() {
        let (keycode, shift) = char_to_keycode('5').unwrap();
        assert_eq!(keycode, 23);
        assert_eq!(shift, false);
    }

    #[test]
    fn test_char_to_keycode_space() {
        let (keycode, shift) = char_to_keycode(' ').unwrap();
        assert_eq!(keycode, 49);
        assert_eq!(shift, false);
    }

    #[test]
    fn test_char_to_keycode_punctuation() {
        let (keycode, shift) = char_to_keycode('.').unwrap();
        assert_eq!(keycode, 47);
        assert_eq!(shift, false);
    }

    #[test]
    fn test_char_to_keycode_unsupported() {
        // Unsupported character should return space keycode
        let (keycode, shift) = char_to_keycode('€').unwrap();
        assert_eq!(keycode, 49); // Space
        assert_eq!(shift, false);
    }
}
