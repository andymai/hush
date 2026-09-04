//! Which keyboard layout the session uses.
//!
//! Typing through uinput sends key codes, and the compositor turns those into
//! characters through the active layout. Hush's code table is the US layout,
//! so on any other layout the letters come out wrong (`z` for `y` on German,
//! and worse on AZERTY). When the layout is not US-like, insertion goes
//! through the clipboard instead, which is layout independent.

use std::process::Command;
use tracing::debug;

/// Layouts whose letters and digits sit where the US layout has them.
const US_LIKE: [&str; 6] = ["us", "en", "english", "us-acentos", "dvorak", "colemak"];

fn looks_us(layout: &str) -> bool {
    let first = layout
        .split([',', '+', '(', ' '])
        .next()
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    if first.is_empty() {
        return true;
    }
    US_LIKE.contains(&first.as_str())
}

/// The first configured layout, from the session or the system.
pub fn current() -> Option<String> {
    if let Some(layout) = std::env::var_os("XKB_DEFAULT_LAYOUT") {
        let layout = layout.to_string_lossy().into_owned();
        if !layout.trim().is_empty() {
            return Some(layout);
        }
    }
    if let Some(layout) = query("setxkbmap", &["-query"], "layout:") {
        return Some(layout);
    }
    if let Some(layout) = query("localectl", &["status"], "X11 Layout:") {
        return Some(layout);
    }
    query("localectl", &["status"], "VC Keymap:")
}

fn query(program: &str, args: &[&str], field: &str) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.trim().strip_prefix(field))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value != "n/a")
}

/// Whether typing key by key produces the characters Hush means. Unknown
/// layouts are treated as US, which is what the code table assumes.
pub fn types_like_us() -> bool {
    match current() {
        Some(layout) => {
            let us = looks_us(&layout);
            debug!(
                "Keyboard layout {} ({})",
                layout,
                if us { "US-like" } else { "not US" }
            );
            us
        },
        None => {
            debug!("Keyboard layout unknown; assuming US");
            true
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn us_and_its_variants_type_directly() {
        for layout in ["us", "US", "us,de", "us(intl)", "dvorak", "colemak", "en"] {
            assert!(looks_us(layout), "{layout} is US-like");
        }
    }

    #[test]
    fn other_layouts_go_through_the_clipboard() {
        for layout in ["de", "fr", "de,us", "ru", "gb", "es+cat"] {
            assert!(!looks_us(layout), "{layout} is not US");
        }
    }

    #[test]
    fn an_empty_layout_is_treated_as_us() {
        assert!(looks_us(""));
        assert!(looks_us("   "));
    }

    #[test]
    fn detection_never_panics() {
        let _ = types_like_us();
    }
}
