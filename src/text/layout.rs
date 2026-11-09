//! Which keyboard layout the session uses.
//!
//! Typing through uinput sends key codes, and the compositor turns those into
//! characters through the active layout. Hush's code table is the plain US
//! layout, so on any other layout the letters come out wrong (`z` for `y` on
//! German, other letters on Dvorak and Colemak, dead keys on US
//! International). When the layout is not plain US, insertion goes through
//! the clipboard instead, which is layout independent.

use std::path::Path;
use std::process::Command;
use tracing::debug;

/// The active layout as the session reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Layout {
    /// XKB names: a layout list such as `us,de` or `us(intl)`, and a variant
    /// list that can be empty.
    Xkb { layout: String, variant: String },
    /// Sway gives only the description, such as `English (Dvorak)`.
    Described(String),
}

impl Layout {
    fn xkb(layout: impl Into<String>, variant: impl Into<String>) -> Self {
        Self::Xkb {
            layout: layout.into(),
            variant: variant.into(),
        }
    }

    /// Only the plain US layout puts each character where Hush's code table
    /// expects it. An empty layout is unknown and counts as US.
    pub fn types_directly(&self) -> bool {
        match self {
            Self::Xkb { layout, variant } => {
                let (layout, inline_variant) = split_variant(first_entry(layout));
                let variant = match first_entry(variant) {
                    "" => inline_variant,
                    listed => listed,
                };
                let layout = layout.to_lowercase();
                let variant = variant.to_lowercase();
                layout.is_empty() || (layout == "us" && (variant.is_empty() || variant == "basic"))
            },
            Self::Described(name) => {
                let name = name.trim();
                name.is_empty() || name.eq_ignore_ascii_case("English (US)")
            },
        }
    }
}

impl std::fmt::Display for Layout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Xkb { layout, variant } if variant.trim().is_empty() => write!(f, "{}", layout),
            Self::Xkb { layout, variant } => write!(f, "{} ({})", layout, variant),
            Self::Described(name) => write!(f, "{}", name),
        }
    }
}

/// The first item of a comma list: the layout that is active at login.
fn first_entry(list: &str) -> &str {
    list.split(',').next().unwrap_or_default().trim()
}

/// `us(intl)` from XKB and `us+intl` from GNOME carry the variant inline.
fn split_variant(layout: &str) -> (&str, &str) {
    if let Some((base, rest)) = layout.split_once('(') {
        return (base.trim(), rest.trim_end_matches(')').trim());
    }
    if let Some((base, rest)) = layout.split_once('+') {
        return (base.trim(), rest.trim());
    }
    (layout, "")
}

/// The configured layout, from the most specific source that answers.
pub fn current() -> Option<Layout> {
    let kxkbrc = dirs::config_dir().map(|dir| dir.join("kxkbrc"));
    detect(kxkbrc.as_deref())
}

/// [`current`] with `kxkbrc` in place of KDE's `~/.config/kxkbrc`.
fn detect(kxkbrc: Option<&Path>) -> Option<Layout> {
    if let Some(layout) = from_env() {
        return Some(layout);
    }
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        if let Some(layout) = run("hyprctl", &["devices", "-j"]).and_then(|s| parse_hyprland(&s)) {
            return Some(layout);
        }
    }
    if std::env::var_os("SWAYSOCK").is_some() {
        if let Some(layout) =
            run("swaymsg", &["-t", "get_inputs", "-r"]).and_then(|s| parse_sway(&s))
        {
            return Some(layout);
        }
    }
    if desktop_is("kde") {
        if let Some(layout) = kxkbrc.and_then(read).and_then(|s| parse_kxkbrc(&s)) {
            return Some(layout);
        }
    }
    if desktop_is("gnome") {
        if let Some(layout) = run(
            "gsettings",
            &["get", "org.gnome.desktop.input-sources", "sources"],
        )
        .and_then(|s| parse_gnome_sources(&s))
        {
            return Some(layout);
        }
    }
    // Under Wayland, setxkbmap asks XWayland, whose rules often stay at the
    // US default whatever the compositor uses.
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        if let Some(layout) =
            run("setxkbmap", &["-query"]).and_then(|s| parse_fields(&s, "layout:", "variant:"))
        {
            return Some(layout);
        }
    }
    run("localectl", &["status"]).and_then(|s| parse_localectl(&s))
}

fn from_env() -> Option<Layout> {
    let layout = std::env::var("XKB_DEFAULT_LAYOUT").ok()?;
    if layout.trim().is_empty() {
        return None;
    }
    let variant = std::env::var("XKB_DEFAULT_VARIANT").unwrap_or_default();
    Some(Layout::xkb(layout, variant))
}

fn desktop_is(name: &str) -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .any(|part| part.eq_ignore_ascii_case(name))
}

fn run(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

fn field<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    text.lines()
        .find_map(|line| line.trim().strip_prefix(name))
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "n/a")
}

/// `setxkbmap -query` and `localectl status` print the variant on its own
/// line.
fn parse_fields(text: &str, layout: &str, variant: &str) -> Option<Layout> {
    let layout = field(text, layout)?;
    Some(Layout::xkb(
        layout,
        field(text, variant).unwrap_or_default(),
    ))
}

/// The X11 layout when one is set, else the console keymap.
fn parse_localectl(text: &str) -> Option<Layout> {
    parse_fields(text, "X11 Layout:", "X11 Variant:")
        .or_else(|| field(text, "VC Keymap:").map(|keymap| Layout::xkb(keymap, "")))
}

/// KDE keeps its layouts in kxkbrc, and uses them only when `Use` is true.
fn parse_kxkbrc(text: &str) -> Option<Layout> {
    let mut in_layout = false;
    let mut used = false;
    let mut layouts = None;
    let mut variants = String::new();
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_layout = line == "[Layout]";
            continue;
        }
        if !in_layout {
            continue;
        }
        match line.split_once('=') {
            Some(("Use", value)) => used = value.trim() == "true",
            Some(("LayoutList", value)) => layouts = Some(value.trim().to_string()),
            Some(("VariantList", value)) => variants = value.trim().to_string(),
            _ => {},
        }
    }
    let layouts = layouts.filter(|list| used && !first_entry(list).is_empty())?;
    Some(Layout::xkb(layouts, variants))
}

/// `hyprctl devices -j`: the main keyboard's layout and variant.
fn parse_hyprland(json: &str) -> Option<Layout> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let keyboards = value.get("keyboards")?.as_array()?;
    let keyboard = keyboards
        .iter()
        .find(|k| k.get("main").and_then(|m| m.as_bool()) == Some(true))
        .or_else(|| keyboards.first())?;
    let text = |key: &str| {
        keyboard
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    };
    let layout = text("layout");
    if layout.trim().is_empty() {
        return None;
    }
    Some(Layout::xkb(layout, text("variant")))
}

/// `swaymsg -t get_inputs -r`: the first keyboard's active layout name.
fn parse_sway(json: &str) -> Option<Layout> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    value
        .as_array()?
        .iter()
        .filter(|input| input.get("type").and_then(|t| t.as_str()) == Some("keyboard"))
        .find_map(|input| input.get("xkb_active_layout_name")?.as_str())
        .map(|name| Layout::Described(name.to_string()))
}

/// GNOME's first input source, such as `[('xkb', 'us+dvorak'), ...]`. An
/// input method engine first (`('ibus', 'mozc-jp')`) is never plain US.
fn parse_gnome_sources(text: &str) -> Option<Layout> {
    let start = text.find("('")? + 1;
    let end = start + text[start..].find(')')?;
    let entry = &text[start..end];
    let mut parts = entry.split(',').map(|part| part.trim().trim_matches('\''));
    let kind = parts.next()?;
    let name = parts.next()?;
    if name.is_empty() {
        return None;
    }
    if kind == "xkb" {
        Some(Layout::xkb(name, ""))
    } else {
        Some(Layout::Described(format!("{} {}", kind, name)))
    }
}

/// Whether typing key by key produces the characters Hush means. An unknown
/// layout counts as US, which is what the code table assumes.
pub fn types_like_us() -> bool {
    match current() {
        Some(layout) => {
            let direct = layout.types_directly();
            debug!(
                "Keyboard layout {} ({})",
                layout,
                if direct { "plain US" } else { "not plain US" }
            );
            direct
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

    fn direct(layout: &str, variant: &str) -> bool {
        Layout::xkb(layout, variant).types_directly()
    }

    #[test]
    fn plain_us_types_directly() {
        for (layout, variant) in [
            ("us", ""),
            ("US", ""),
            ("us", "basic"),
            ("us(basic)", ""),
            ("us,de", ""),
            ("us,de", ",nodeadkeys"),
            (" us ", " "),
        ] {
            assert!(direct(layout, variant), "{layout} / {variant} is plain US");
        }
        assert!(Layout::Described("English (US)".into()).types_directly());
    }

    #[test]
    fn dvorak_colemak_and_us_variants_paste() {
        for (layout, variant) in [
            ("dvorak", ""),
            ("colemak", ""),
            ("us(intl)", ""),
            ("us(alt-intl)", ""),
            ("us(dvorak)", ""),
            ("us(colemak)", ""),
            ("us", "intl"),
            ("us", "dvorak"),
            ("us", "colemak"),
            ("us+dvorak", ""),
            ("us,de", "dvorak,"),
            ("us-acentos", ""),
        ] {
            assert!(!direct(layout, variant), "{layout} / {variant} pastes");
        }
        for name in ["English (Dvorak)", "English (US, intl., with dead keys)"] {
            assert!(!Layout::Described(name.into()).types_directly(), "{name}");
        }
    }

    #[test]
    fn other_layouts_go_through_the_clipboard() {
        for layout in ["de", "fr", "de,us", "ru", "gb", "es+cat", "en"] {
            assert!(!direct(layout, ""), "{layout} is not US");
        }
        assert!(!Layout::Described("German".into()).types_directly());
    }

    #[test]
    fn an_empty_layout_is_treated_as_us() {
        assert!(direct("", ""));
        assert!(direct("   ", ""));
        assert!(Layout::Described(String::new()).types_directly());
    }

    #[test]
    fn setxkbmap_and_localectl_give_the_variant_on_its_own_line() {
        let setxkbmap =
            "rules:      evdev\nmodel:      pc105\nlayout:     us\nvariant:    dvorak\n";
        assert_eq!(
            parse_fields(setxkbmap, "layout:", "variant:"),
            Some(Layout::xkb("us", "dvorak"))
        );
        let localectl = "   System Locale: LANG=en_US.UTF-8\n       VC Keymap: us\n      X11 Layout: us\n     X11 Variant: colemak\n";
        let layout = parse_localectl(localectl).unwrap();
        assert_eq!(layout, Layout::xkb("us", "colemak"));
        assert!(!layout.types_directly());

        let console_only = "       VC Keymap: dvorak\n      X11 Layout: n/a\n";
        assert_eq!(
            parse_localectl(console_only),
            Some(Layout::xkb("dvorak", ""))
        );
    }

    #[test]
    fn kde_layouts_count_only_when_used() {
        let kxkbrc = "[$Version]\nupdate_info=kxkb.upd:remove-empty-lists\n\n[Layout]\nDisplayNames=,\nLayoutList=us,de\nUse=true\nVariantList=dvorak,\n";
        assert_eq!(parse_kxkbrc(kxkbrc), Some(Layout::xkb("us,de", "dvorak,")));
        assert!(!parse_kxkbrc(kxkbrc).unwrap().types_directly());
        assert_eq!(parse_kxkbrc(&kxkbrc.replace("Use=true", "Use=false")), None);
        assert_eq!(parse_kxkbrc("[Layout]\nUse=true\n"), None);
    }

    #[test]
    fn hyprland_reports_the_main_keyboard() {
        let json = r#"{"mice":[],"keyboards":[
            {"name":"hush-voice-keyboard","layout":"us","variant":"","main":false},
            {"name":"at-translated-set-2-keyboard","layout":"us","variant":"colemak","main":true}]}"#;
        assert_eq!(parse_hyprland(json), Some(Layout::xkb("us", "colemak")));
        assert_eq!(parse_hyprland(r#"{"keyboards":[]}"#), None);
    }

    #[test]
    fn sway_reports_the_layout_description() {
        let json = r#"[{"type":"pointer","name":"mouse"},
            {"type":"keyboard","xkb_active_layout_name":"English (Dvorak)"}]"#;
        assert_eq!(
            parse_sway(json),
            Some(Layout::Described("English (Dvorak)".into()))
        );
        assert_eq!(parse_sway("[]"), None);
    }

    #[test]
    fn gnome_puts_the_variant_after_a_plus() {
        let layout = parse_gnome_sources("[('xkb', 'us+dvorak'), ('xkb', 'de')]\n").unwrap();
        assert_eq!(layout, Layout::xkb("us+dvorak", ""));
        assert!(!layout.types_directly());
        assert!(parse_gnome_sources("[('xkb', 'us')]")
            .unwrap()
            .types_directly());
        assert!(!parse_gnome_sources("[('ibus', 'mozc-jp'), ('xkb', 'us')]")
            .unwrap()
            .types_directly());
        assert_eq!(parse_gnome_sources("@a(ss) []"), None);
    }

    #[test]
    fn detection_never_panics() {
        let dir = tempfile::tempdir().unwrap();
        let _ = detect(Some(&dir.path().join("kxkbrc")));
    }
}
