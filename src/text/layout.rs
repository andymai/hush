//! Which keyboard layouts the session uses, and which one is active.
//!
//! Typing through uinput sends key codes, and the compositor turns those into
//! characters through the active layout. Hush reads the layout names here,
//! and [`super::typing`] compiles the keymap from them to find the keys for
//! each character. With several layouts, the compositor must say which one
//! is active; when it does not, insertion goes through the clipboard, which
//! is layout independent.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use tracing::debug;

/// The name of Hush's typing device. Hyprland lists it in lower case with
/// dashes.
const DEVICE_NAME: &str = "Hush Voice Keyboard";
const HYPRLAND_DEVICE_NAME: &str = "hush-voice-keyboard";
/// How long KWin gets to name the active layout.
const DBUS_TIMEOUT: Duration = Duration::from_millis(500);

/// The configured layouts as the session reports them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Layout {
    /// XKB names: a layout list such as `us,de` or `us(intl)`, and a variant
    /// list that can be empty.
    Xkb { layout: String, variant: String },
    /// A name that is not an XKB layout, such as Sway's `English (Dvorak)`
    /// when Hush cannot map it, or a GNOME input method.
    Described(String),
}

impl Layout {
    fn xkb(layout: impl Into<String>, variant: impl Into<String>) -> Self {
        Self::Xkb {
            layout: layout.into(),
            variant: variant.into(),
        }
    }

    /// Whether the first layout is plain US, the only common layout where
    /// Right Alt is not AltGr. An empty layout is unknown and counts as US.
    pub fn is_plain_us(&self) -> bool {
        match self {
            Self::Xkb { layout, variant } => {
                let (layout, inline_variant) = split_variant(first_entry(layout));
                let variant = match first_entry(variant) {
                    "" => inline_variant,
                    listed => listed,
                };
                plain_us(layout, variant)
            },
            Self::Described(name) => {
                let name = name.trim();
                name.is_empty() || name.eq_ignore_ascii_case("English (US)")
            },
        }
    }

    /// Each layout of the list as a layout name and a variant, or `None`
    /// for a described layout.
    pub fn entries(&self) -> Option<Vec<(String, String)>> {
        let Self::Xkb { layout, variant } = self else {
            return None;
        };
        let variants: Vec<&str> = variant.split(',').map(str::trim).collect();
        Some(
            layout
                .split(',')
                .enumerate()
                .map(|(i, entry)| {
                    let (base, inline) = split_variant(entry.trim());
                    let variant = match variants.get(i).copied().unwrap_or_default() {
                        "" => inline,
                        listed => listed,
                    };
                    (base.to_string(), variant.to_string())
                })
                .collect(),
        )
    }
}

impl std::fmt::Display for Layout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Xkb { layout, variant } if variant.replace(',', "").trim().is_empty() => {
                write!(f, "{}", layout)
            },
            Self::Xkb { layout, variant } => write!(f, "{} ({})", layout, variant),
            Self::Described(name) => write!(f, "{}", name),
        }
    }
}

/// Whether a layout name and variant are the plain US layout. An empty
/// layout is unknown and counts as US.
pub fn plain_us(layout: &str, variant: &str) -> bool {
    let layout = layout.trim().to_lowercase();
    let variant = variant.trim().to_lowercase();
    layout.is_empty() || (layout == "us" && (variant.is_empty() || variant == "basic"))
}

/// Which layout of the list is active.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Active {
    /// The compositor reports this index into the layout list.
    Index(usize),
    /// No source says. With one layout in the list, that one is active.
    Unknown,
    /// Hush cannot tell, for this reason.
    Unclear(String),
}

/// The session's keyboard: the layouts, the XKB rules, model, and options
/// they compile with, and the active layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub layout: Layout,
    /// Empty for the libxkbcommon default.
    pub rules: String,
    /// Empty for the libxkbcommon default.
    pub model: String,
    /// `None` when the source does not say, as on Sway.
    pub options: Option<String>,
    pub active: Active,
}

impl Session {
    fn new(layout: Layout) -> Self {
        Self {
            layout,
            rules: String::new(),
            model: String::new(),
            options: Some(String::new()),
            active: Active::Unknown,
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

/// The configured layouts, from the most specific source that answers.
pub fn current() -> Option<Layout> {
    session().map(|session| session.layout)
}

/// The session's keyboard, with the active layout when the compositor
/// reports it.
pub fn session() -> Option<Session> {
    let kxkbrc = dirs::config_dir().map(|dir| dir.join("kxkbrc"));
    let mut session = detect(kxkbrc.as_deref())?;
    if session.active == Active::Unknown && desktop_is("kde") {
        if let Some((names, index)) = kwin_layouts() {
            session.active = kde_active(&session.layout, &names, index);
        }
    }
    if session.active == Active::Unknown && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        if let Some(group) = x11_group() {
            session.active = Active::Index(group);
        }
    }
    Some(session)
}

/// [`session`] without the active-layout queries, with `kxkbrc` in place of
/// KDE's `~/.config/kxkbrc`. The compositor's own report comes first, then
/// the variables that compositors read when nothing else is set.
fn detect(kxkbrc: Option<&Path>) -> Option<Session> {
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        let registry = Registry::load();
        if let Some(session) =
            run("hyprctl", &["devices", "-j"]).and_then(|s| parse_hyprland(&s, registry.as_ref()))
        {
            return Some(session);
        }
    }
    if std::env::var_os("SWAYSOCK").is_some() {
        let registry = Registry::load();
        if let Some(session) = run("swaymsg", &["-t", "get_inputs", "-r"])
            .and_then(|s| parse_sway(&s, registry.as_ref()))
        {
            return Some(session);
        }
    }
    if desktop_is("kde") {
        let config = kxkbrc.and_then(read).map(|s| parse_kxkbrc(&s));
        let options = config.as_ref().and_then(|config| config.options.clone());
        if let Some(mut session) = config.and_then(|config| config.session) {
            session.options = options.or_else(env_options);
            return Some(session);
        }
        if let Some(mut session) = from_env() {
            session.options = options.or_else(env_options);
            return Some(session);
        }
    }
    if desktop_is("gnome") {
        let get = |key: &str| {
            run(
                "gsettings",
                &["get", "org.gnome.desktop.input-sources", key],
            )
        };
        if let Some(session) = get("sources").and_then(|sources| {
            parse_gnome(
                &sources,
                get("mru-sources").as_deref().unwrap_or_default(),
                get("xkb-options").as_deref().unwrap_or_default(),
            )
        }) {
            return Some(session);
        }
    }
    if let Some(session) = from_env() {
        return Some(session);
    }
    // Under Wayland, setxkbmap asks XWayland, whose rules often stay at the
    // US default whatever the compositor uses.
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        if let Some(session) = run("setxkbmap", &["-query"]).and_then(|s| parse_setxkbmap(&s)) {
            return Some(session);
        }
    }
    run("localectl", &["status"]).and_then(|s| parse_localectl(&s))
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_options() -> Option<String> {
    Some(env_var("XKB_DEFAULT_OPTIONS").unwrap_or_default())
}

fn from_env() -> Option<Session> {
    let layout = env_var("XKB_DEFAULT_LAYOUT")?;
    let variant = env_var("XKB_DEFAULT_VARIANT").unwrap_or_default();
    Some(Session {
        rules: env_var("XKB_DEFAULT_RULES").unwrap_or_default(),
        model: env_var("XKB_DEFAULT_MODEL").unwrap_or_default(),
        options: env_options(),
        ..Session::new(Layout::xkb(layout, variant))
    })
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

/// `setxkbmap -query` prints each name on its own line, and leaves out an
/// empty variant or option list.
fn parse_setxkbmap(text: &str) -> Option<Session> {
    let layout = field(text, "layout:")?;
    Some(Session {
        rules: field(text, "rules:").unwrap_or_default().to_string(),
        model: field(text, "model:").unwrap_or_default().to_string(),
        options: Some(field(text, "options:").unwrap_or_default().to_string()),
        ..Session::new(Layout::xkb(
            layout,
            field(text, "variant:").unwrap_or_default(),
        ))
    })
}

/// The X11 layout when one is set, else the console keymap.
fn parse_localectl(text: &str) -> Option<Session> {
    if let Some(layout) = field(text, "X11 Layout:") {
        return Some(Session {
            model: field(text, "X11 Model:").unwrap_or_default().to_string(),
            options: Some(field(text, "X11 Options:").unwrap_or_default().to_string()),
            ..Session::new(Layout::xkb(
                layout,
                field(text, "X11 Variant:").unwrap_or_default(),
            ))
        });
    }
    field(text, "VC Keymap:").map(|keymap| Session::new(Layout::xkb(keymap, "")))
}

/// What KDE's kxkbrc says.
#[derive(Debug, PartialEq, Eq)]
struct Kxkbrc {
    /// The layouts, when `Use` is true and the list is not empty.
    session: Option<Session>,
    /// The options, which KWin applies only when `ResetOldOptions` is true.
    options: Option<String>,
}

/// KDE keeps its layouts, model, and options in kxkbrc.
fn parse_kxkbrc(text: &str) -> Kxkbrc {
    let mut in_layout = false;
    let mut used = false;
    let mut reset_options = false;
    let mut layouts = None;
    let mut variants = String::new();
    let mut model = String::new();
    let mut options = String::new();
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_layout = line == "[Layout]";
            continue;
        }
        if !in_layout {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "Use" => used = value == "true",
            "ResetOldOptions" => reset_options = value == "true",
            "LayoutList" => layouts = Some(value.to_string()),
            "VariantList" => variants = value.to_string(),
            "Model" => model = value.to_string(),
            "Options" => options = value.to_string(),
            _ => {},
        }
    }
    let options = reset_options.then_some(options);
    let session = layouts
        .filter(|list| used && !first_entry(list).is_empty())
        .map(|layouts| Session {
            // KWin's default when kxkbrc names no model.
            model: if model.is_empty() {
                "pc104".to_string()
            } else {
                model
            },
            options: options.clone(),
            ..Session::new(Layout::xkb(layouts, variants))
        });
    Kxkbrc { session, options }
}

/// KWin's layout list and the index of the active layout, over D-Bus.
fn kwin_layouts() -> Option<(Vec<String>, u32)> {
    // zbus blocks on a runtime of its own, which panics on a thread that
    // already drives one, so the query gets a thread of its own.
    std::thread::Builder::new()
        .name("hush-kwin-layout".into())
        .spawn(ask_kwin)
        .ok()?
        .join()
        .ok()?
}

fn ask_kwin() -> Option<(Vec<String>, u32)> {
    let connection = zbus::blocking::connection::Builder::session()
        .ok()?
        .method_timeout(DBUS_TIMEOUT)
        .build()
        .ok()?;
    let call = |method: &str| {
        connection.call_method(
            Some("org.kde.keyboard"),
            "/Layouts",
            Some("org.kde.KeyboardLayouts"),
            method,
            &(),
        )
    };
    let list: Vec<(String, String, String)> =
        call("getLayoutsList").ok()?.body().deserialize().ok()?;
    let index: u32 = call("getLayout").ok()?.body().deserialize().ok()?;
    Some((list.into_iter().map(|(short, _, _)| short).collect(), index))
}

/// KWin names its layouts as the raw items of the layout list. When they
/// match the list Hush read, KWin's index is the active layout.
fn kde_active(layout: &Layout, kwin: &[String], index: u32) -> Active {
    let Layout::Xkb { layout: list, .. } = layout else {
        return Active::Unknown;
    };
    let read: Vec<&str> = list.split(',').map(str::trim).collect();
    if read.len() == kwin.len() && read.iter().zip(kwin).all(|(a, b)| *a == b.trim()) {
        Active::Index(index as usize)
    } else {
        Active::Unclear(format!(
            "KWin uses the layouts {}, but Hush read {}",
            kwin.join(","),
            list
        ))
    }
}

fn x11_group() -> Option<usize> {
    use x11rb::protocol::xkb::{self, ConnectionExt as _};
    let (conn, _) = x11rb::rust_connection::RustConnection::connect(None).ok()?;
    conn.xkb_use_extension(1, 0).ok()?.reply().ok()?;
    let state = conn
        .xkb_get_state(xkb::ID::USE_CORE_KBD.into())
        .ok()?
        .reply()
        .ok()?;
    Some(usize::from(u8::from(state.group)))
}

/// The keyboard to read in a compositor's device list: Hush's own typing
/// device when it is there, because the compositor types its keys with
/// that device's layout, else the main keyboard, else the first.
fn pick_keyboard<'a>(
    keyboards: &'a [serde_json::Value],
    own_name: &str,
    is_main: impl Fn(&serde_json::Value) -> bool,
) -> Option<&'a serde_json::Value> {
    let name = |keyboard: &serde_json::Value| {
        keyboard
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or_default()
            .to_string()
    };
    keyboards
        .iter()
        .find(|k| name(k) == own_name)
        .or_else(|| keyboards.iter().find(|k| is_main(k)))
        .or_else(|| keyboards.first())
}

/// `hyprctl devices -j`: the keyboard's XKB names and its active layout.
fn parse_hyprland(json: &str, registry: Option<&Registry>) -> Option<Session> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let keyboards = value.get("keyboards")?.as_array()?;
    let keyboard = pick_keyboard(keyboards, HYPRLAND_DEVICE_NAME, |k| {
        k.get("main").and_then(|m| m.as_bool()) == Some(true)
    })?;
    let text = |key: &str| {
        keyboard
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    let layout = text("layout");
    if layout.is_empty() {
        return None;
    }
    let layout = Layout::xkb(layout, text("variant"));
    let active = match keyboard.get("active_layout_index").and_then(|i| i.as_u64()) {
        Some(index) => Active::Index(index as usize),
        None => registry
            .zip(layout.entries())
            .and_then(|(registry, entries)| registry.index_of(&entries, &text("active_keymap")))
            .map_or(Active::Unknown, Active::Index),
    };
    Some(Session {
        rules: text("rules"),
        model: text("model"),
        options: Some(text("options")),
        active,
        ..Session::new(layout)
    })
}

/// `swaymsg -t get_inputs -r`: the keyboard's layout descriptions and
/// active index. Sway names no XKB layout and no options, so Hush maps
/// each description back to XKB names through the registry.
fn parse_sway(json: &str, registry: Option<&Registry>) -> Option<Session> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let keyboards: Vec<serde_json::Value> = value
        .as_array()?
        .iter()
        .filter(|input| input.get("type").and_then(|t| t.as_str()) == Some("keyboard"))
        .cloned()
        .collect();
    let keyboard = pick_keyboard(&keyboards, DEVICE_NAME, |_| false)?;
    let active_name = keyboard
        .get("xkb_active_layout_name")
        .and_then(|n| n.as_str())
        .unwrap_or_default()
        .to_string();
    let mut names: Vec<String> = keyboard
        .get("xkb_layout_names")
        .and_then(|n| n.as_array())
        .map(|names| {
            names
                .iter()
                .filter_map(|n| n.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let index = match keyboard
        .get("xkb_active_layout_index")
        .and_then(|i| i.as_u64())
    {
        Some(index) => index as usize,
        None if names.is_empty() => 0,
        None => return Some(described(active_name)),
    };
    if names.is_empty() {
        names.push(active_name.clone());
    }
    if active_name.is_empty() && names.get(index).is_none_or(|name| name.is_empty()) {
        return None;
    }
    let resolved: Option<Vec<(String, String)>> =
        registry.and_then(|registry| names.iter().map(|name| registry.names_for(name)).collect());
    let Some(resolved) = resolved else {
        let name = names.get(index).cloned().unwrap_or(active_name);
        return Some(described(name));
    };
    let layouts: Vec<&str> = resolved.iter().map(|(layout, _)| layout.as_str()).collect();
    let variants: Vec<&str> = resolved
        .iter()
        .map(|(_, variant)| variant.as_str())
        .collect();
    Some(Session {
        options: None,
        active: Active::Index(index),
        ..Session::new(Layout::xkb(layouts.join(","), variants.join(",")))
    })
}

fn described(name: String) -> Session {
    Session {
        options: None,
        ..Session::new(Layout::Described(name))
    }
}

/// The items of a GNOME `a(ss)` list, such as `[('xkb', 'us+dvorak'),
/// ('ibus', 'mozc-jp')]`.
fn gnome_sources(text: &str) -> Vec<(String, String)> {
    let mut sources = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("('") {
        let Some(end) = rest[start..].find(')') else {
            break;
        };
        let entry = &rest[start + 1..start + end];
        let mut parts = entry.split(',').map(|part| part.trim().trim_matches('\''));
        if let (Some(kind), Some(name)) = (parts.next(), parts.next()) {
            if !name.is_empty() {
                sources.push((kind.to_string(), name.to_string()));
            }
        }
        rest = &rest[start + end..];
    }
    sources
}

/// GNOME's input sources: the most recently used one is active. Without a
/// recent one, a single source is active, and with several Hush cannot
/// tell. An input method engine (`('ibus', 'mozc-jp')`) has no XKB names.
fn parse_gnome(sources: &str, mru: &str, options: &str) -> Option<Session> {
    let sources = gnome_sources(sources);
    let first = sources.first()?;
    let active = gnome_sources(mru)
        .into_iter()
        .find(|recent| sources.contains(recent))
        .or_else(|| (sources.len() == 1).then(|| first.clone()));
    let options: Vec<&str> = options
        .split(['[', ']', ',', '\''])
        .map(str::trim)
        .filter(|option| !option.is_empty() && !option.starts_with('@'))
        .collect();
    let to_layout = |(kind, name): &(String, String)| {
        if kind == "xkb" {
            Layout::xkb(name.clone(), "")
        } else {
            Layout::Described(format!("{} {}", kind, name))
        }
    };
    // GNOME groups its sources into keymaps of its own, so Hush compiles
    // the active source alone.
    let (layout, active) = match active {
        Some(source) => (to_layout(&source), Active::Index(0)),
        None => (
            to_layout(first),
            Active::Unclear(format!(
                "GNOME has {} input sources and names no recent one",
                sources.len()
            )),
        ),
    };
    Some(Session {
        options: Some(options.join(",")),
        active,
        ..Session::new(layout)
    })
}

/// The xkeyboard-config registry: each layout and variant with the
/// description that Sway and Hyprland report.
#[derive(Debug, Default)]
struct Registry {
    entries: Vec<(String, String, String)>,
}

impl Registry {
    fn load() -> Option<Self> {
        let root = std::env::var_os("XKB_CONFIG_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/usr/share/X11/xkb"));
        let text = read(&root.join("rules/evdev.lst"))?;
        let registry = Self::parse(&text);
        debug!(
            "XKB registry: {} layouts and variants",
            registry.entries.len()
        );
        Some(registry)
    }

    /// `evdev.lst`: a `! layout` section of `name description` lines and a
    /// `! variant` section of `name layout: description` lines.
    fn parse(text: &str) -> Self {
        let mut entries = Vec::new();
        let mut section = "";
        for line in text.lines() {
            if let Some(name) = line.strip_prefix('!') {
                section = name.trim();
                continue;
            }
            let line = line.trim();
            let Some((name, rest)) = line.split_once(char::is_whitespace) else {
                continue;
            };
            let rest = rest.trim();
            match section {
                "layout" => entries.push((name.to_string(), String::new(), rest.to_string())),
                "variant" => {
                    if let Some((layout, description)) = rest.split_once(": ") {
                        entries.push((
                            layout.trim().to_string(),
                            name.to_string(),
                            description.trim().to_string(),
                        ));
                    }
                },
                _ => {},
            }
        }
        Self { entries }
    }

    /// The layout and variant with this description, when exactly one has
    /// it.
    fn names_for(&self, description: &str) -> Option<(String, String)> {
        let description = description.trim();
        let mut matches = self
            .entries
            .iter()
            .filter(|(_, _, known)| known == description);
        let (layout, variant, _) = matches.next()?;
        matches
            .next()
            .is_none()
            .then(|| (layout.clone(), variant.clone()))
    }

    fn description(&self, layout: &str, variant: &str) -> Option<&str> {
        let variant = if variant == "basic" { "" } else { variant };
        self.entries
            .iter()
            .find(|(known, known_variant, _)| known == layout && known_variant == variant)
            .map(|(_, _, description)| description.as_str())
    }

    /// The index of the one entry whose description is `active`.
    fn index_of(&self, entries: &[(String, String)], active: &str) -> Option<usize> {
        let active = active.trim();
        let mut matches = entries
            .iter()
            .enumerate()
            .filter(|(_, (layout, variant))| self.description(layout, variant) == Some(active));
        let (index, _) = matches.next()?;
        matches.next().is_none().then_some(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(layout: &str, variant: &str) -> Layout {
        Layout::xkb(layout, variant)
    }

    fn us(layout_list: &str, variant: &str) -> bool {
        layout(layout_list, variant).is_plain_us()
    }

    const EVDEV_LST: &str = "! model\n  pc105           Generic 105-key PC\n\n\
        ! layout\n  us              English (US)\n  de              German\n  fr              French\n\n\
        ! variant\n  intl            us: English (US, intl., with dead keys)\n  dvorak          us: English (Dvorak)\n  nodeadkeys      de: German (no dead keys)\n  dvorak          fr: French (Dvorak)\n\n\
        ! option\n  grp             Switching to another layout\n";

    fn registry() -> Registry {
        Registry::parse(EVDEV_LST)
    }

    #[test]
    fn plain_us_is_the_first_layout_without_a_variant() {
        for (list, variant) in [
            ("us", ""),
            ("US", ""),
            ("us", "basic"),
            ("us(basic)", ""),
            ("us,de", ""),
            ("us,de", ",nodeadkeys"),
            (" us ", " "),
            ("", ""),
        ] {
            assert!(us(list, variant), "{list} / {variant} is plain US");
        }
        for (list, variant) in [
            ("dvorak", ""),
            ("us(intl)", ""),
            ("us", "dvorak"),
            ("us+dvorak", ""),
            ("us,de", "dvorak,"),
            ("de", ""),
            ("de,us", ""),
        ] {
            assert!(!us(list, variant), "{list} / {variant} is not plain US");
        }
        assert!(Layout::Described("English (US)".into()).is_plain_us());
        assert!(!Layout::Described("English (Dvorak)".into()).is_plain_us());
    }

    #[test]
    fn entries_split_the_lists_and_inline_variants() {
        assert_eq!(
            layout("us,de", ",nodeadkeys").entries(),
            Some(vec![
                ("us".into(), String::new()),
                ("de".into(), "nodeadkeys".into())
            ])
        );
        assert_eq!(
            layout("us(intl),us+dvorak", "").entries(),
            Some(vec![
                ("us".into(), "intl".into()),
                ("us".into(), "dvorak".into())
            ])
        );
        assert_eq!(Layout::Described("German".into()).entries(), None);
        assert_eq!(layout("us,de", "").to_string(), "us,de");
        assert_eq!(layout("us,de", "dvorak,").to_string(), "us,de (dvorak,)");
    }

    #[test]
    fn setxkbmap_and_localectl_give_each_name_on_its_own_line() {
        let setxkbmap = "rules:      evdev\nmodel:      pc105\nlayout:     us,de\nvariant:    dvorak,\noptions:    grp:alt_shift_toggle\n";
        let session = parse_setxkbmap(setxkbmap).unwrap();
        assert_eq!(session.layout, layout("us,de", "dvorak,"));
        assert_eq!(session.rules, "evdev");
        assert_eq!(session.model, "pc105");
        assert_eq!(session.options.as_deref(), Some("grp:alt_shift_toggle"));
        assert_eq!(session.active, Active::Unknown);
        assert_eq!(
            parse_setxkbmap("layout: us\n").unwrap().options.as_deref(),
            Some(""),
            "setxkbmap leaves out an empty option list"
        );

        let localectl = "   System Locale: LANG=en_US.UTF-8\n       VC Keymap: us\n      X11 Layout: us\n       X11 Model: pc105\n     X11 Variant: colemak\n     X11 Options: compose:ralt\n";
        let session = parse_localectl(localectl).unwrap();
        assert_eq!(session.layout, layout("us", "colemak"));
        assert_eq!(session.options.as_deref(), Some("compose:ralt"));
        let console_only = "       VC Keymap: dvorak\n      X11 Layout: n/a\n";
        assert_eq!(
            parse_localectl(console_only).unwrap().layout,
            layout("dvorak", "")
        );
    }

    #[test]
    fn kde_layouts_count_only_when_used() {
        let kxkbrc = "[$Version]\nupdate_info=kxkb.upd:remove-empty-lists\n\n[Layout]\nDisplayNames=,\nLayoutList=us,de\nModel=pc105\nOptions=lv3:caps_switch\nResetOldOptions=true\nUse=true\nVariantList=dvorak,\n";
        let config = parse_kxkbrc(kxkbrc);
        let session = config.session.unwrap();
        assert_eq!(session.layout, layout("us,de", "dvorak,"));
        assert_eq!(session.model, "pc105");
        assert_eq!(session.options.as_deref(), Some("lv3:caps_switch"));
        assert_eq!(config.options.as_deref(), Some("lv3:caps_switch"));
        assert_eq!(
            parse_kxkbrc(&kxkbrc.replace("Use=true", "Use=false")).session,
            None
        );
        assert_eq!(parse_kxkbrc("[Layout]\nUse=true\n").session, None);
    }

    #[test]
    fn kwin_applies_options_only_with_reset_old_options() {
        let kxkbrc = "[Layout]\nOptions=ctrl:swap_lwin_lctl\nResetOldOptions=false\n";
        assert_eq!(parse_kxkbrc(kxkbrc).options, None);
        let kxkbrc = "[Layout]\nOptions=ctrl:swap_lwin_lctl\nResetOldOptions=true\n";
        let config = parse_kxkbrc(kxkbrc);
        assert_eq!(config.options.as_deref(), Some("ctrl:swap_lwin_lctl"));
        assert_eq!(
            config.session, None,
            "no layouts, so KWin reads the environment"
        );
        let used = parse_kxkbrc("[Layout]\nLayoutList=de\nUse=true\n");
        assert_eq!(used.session.unwrap().model, "pc104", "KWin's default model");
    }

    #[test]
    fn kwin_names_the_active_layout_when_its_list_matches() {
        let kwin = vec!["us".to_string(), "de".to_string()];
        assert_eq!(
            kde_active(&layout("us,de", ",nodeadkeys"), &kwin, 1),
            Active::Index(1)
        );
        assert!(matches!(
            kde_active(&layout("de", ""), &kwin, 0),
            Active::Unclear(_)
        ));
        assert_eq!(
            kde_active(&Layout::Described("German".into()), &kwin, 0),
            Active::Unknown
        );
    }

    #[test]
    fn hyprland_reads_hushs_own_keyboard_first() {
        let json = r#"{"mice":[],"keyboards":[
            {"name":"at-translated-set-2-keyboard","rules":"","model":"pc105","layout":"us,de","variant":",nodeadkeys","options":"grp:alt_shift_toggle","active_keymap":"German (no dead keys)","main":true},
            {"name":"hush-voice-keyboard","rules":"","model":"","layout":"us,de","variant":",nodeadkeys","options":"","active_keymap":"English (US)","main":false}]}"#;
        let session = parse_hyprland(json, Some(&registry())).unwrap();
        assert_eq!(session.layout, layout("us,de", ",nodeadkeys"));
        assert_eq!(
            session.active,
            Active::Index(0),
            "Hush's device types in US"
        );
        assert_eq!(session.options.as_deref(), Some(""));

        let main_only = r#"{"keyboards":[
            {"name":"at-translated-set-2-keyboard","layout":"us,de","variant":",nodeadkeys","options":"grp:alt_shift_toggle","active_keymap":"German (no dead keys)","main":true}]}"#;
        let session = parse_hyprland(main_only, Some(&registry())).unwrap();
        assert_eq!(session.active, Active::Index(1));
        assert_eq!(session.options.as_deref(), Some("grp:alt_shift_toggle"));
        assert_eq!(
            parse_hyprland(main_only, None).unwrap().active,
            Active::Unknown,
            "without the registry the description maps to nothing"
        );

        let indexed = r#"{"keyboards":[{"name":"kb","layout":"us,de","active_layout_index":1,"active_keymap":"?","main":true}]}"#;
        assert_eq!(
            parse_hyprland(indexed, None).unwrap().active,
            Active::Index(1)
        );
        assert_eq!(parse_hyprland(r#"{"keyboards":[]}"#, None), None);
    }

    #[test]
    fn sway_descriptions_map_to_xkb_names() {
        let json = r#"[{"type":"pointer","name":"mouse"},
            {"type":"keyboard","name":"AT Keyboard","xkb_layout_names":["English (US)","German"],"xkb_active_layout_index":0,"xkb_active_layout_name":"English (US)"},
            {"type":"keyboard","name":"Hush Voice Keyboard","xkb_layout_names":["English (Dvorak)","German"],"xkb_active_layout_index":1,"xkb_active_layout_name":"German"}]"#;
        let session = parse_sway(json, Some(&registry())).unwrap();
        assert_eq!(session.layout, layout("us,de", "dvorak,"));
        assert_eq!(session.active, Active::Index(1));
        assert_eq!(session.options, None, "Sway does not report its options");

        let unknown = r#"[{"type":"keyboard","name":"kb","xkb_layout_names":["Klingon","German"],"xkb_active_layout_index":1,"xkb_active_layout_name":"German"}]"#;
        let session = parse_sway(unknown, Some(&registry())).unwrap();
        assert_eq!(session.layout, Layout::Described("German".into()));
        assert_eq!(
            parse_sway(json, None).unwrap().layout,
            Layout::Described("German".into())
        );
        assert_eq!(parse_sway("[]", None), None);
    }

    #[test]
    fn the_registry_needs_a_unique_description() {
        let registry = registry();
        assert_eq!(
            registry.names_for("English (Dvorak)"),
            Some(("us".into(), "dvorak".into()))
        );
        assert_eq!(
            registry.names_for("German"),
            Some(("de".into(), String::new()))
        );
        assert_eq!(registry.names_for("Klingon"), None);
        let doubled = Registry::parse(&format!("{EVDEV_LST}! variant\n  other  de: German\n"));
        assert_eq!(doubled.names_for("German"), None, "two entries share it");
        assert_eq!(registry.description("us", "basic"), Some("English (US)"));
        let entries = vec![
            ("us".to_string(), String::new()),
            ("us".to_string(), "basic".to_string()),
        ];
        assert_eq!(
            registry.index_of(&entries, "English (US)"),
            None,
            "two layouts of the list share the description"
        );
    }

    #[test]
    fn gnome_types_with_the_most_recent_source() {
        let sources = "[('xkb', 'us+dvorak'), ('xkb', 'de')]\n";
        let session = parse_gnome(
            sources,
            "[('xkb', 'de'), ('xkb', 'us+dvorak')]",
            "['lv3:ralt_switch', 'compose:menu']",
        )
        .unwrap();
        assert_eq!(session.layout, layout("de", ""));
        assert_eq!(session.active, Active::Index(0));
        assert_eq!(
            session.options.as_deref(),
            Some("lv3:ralt_switch,compose:menu")
        );

        let session = parse_gnome(sources, "@a(ss) []", "@as []").unwrap();
        assert_eq!(session.layout, layout("us+dvorak", ""));
        assert!(matches!(session.active, Active::Unclear(_)));
        assert_eq!(session.options.as_deref(), Some(""));

        let single = parse_gnome("[('xkb', 'us')]", "@a(ss) []", "@as []").unwrap();
        assert_eq!(single.active, Active::Index(0));
        assert!(single.layout.is_plain_us());

        let ime = parse_gnome(
            "[('ibus', 'mozc-jp'), ('xkb', 'us')]",
            "[('ibus', 'mozc-jp')]",
            "",
        )
        .unwrap();
        assert_eq!(ime.layout, Layout::Described("ibus mozc-jp".into()));
        assert_eq!(parse_gnome("@a(ss) []", "", ""), None);
    }

    #[test]
    fn detection_never_panics() {
        let dir = tempfile::tempdir().unwrap();
        let _ = detect(Some(&dir.path().join("kxkbrc")));
    }
}
