//! Which keyboard layouts the session uses, and which one is active.
//!
//! Typing through uinput sends key codes, and the compositor turns those into
//! characters through the active layout. Hush reads the layout names here,
//! and [`super::typing`] compiles the keymap from them to find the keys for
//! each character. With several layouts, the compositor must say which one
//! is active; when it does not, insertion goes through the clipboard, which
//! is layout independent.

use std::collections::BTreeSet;
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
    /// Hush cannot tell, for this reason, and `fix` says what lets it type.
    Unclear { why: String, fix: &'static str },
}

/// The fix when the session has several layouts and no word on the active
/// one.
pub const FIX_SEVERAL: &str = "to type, keep one layout, or use a desktop that reports the \
     active layout (KDE Plasma, GNOME, Hyprland, Sway, X11)";
const FIX_KWIN: &str = "to type, set the layouts in System Settings, then restart Hush";
const FIX_PER_WINDOW: &str = "to type, turn off one source for each window: gsettings set \
     org.gnome.desktop.input-sources per-window false";
const FIX_GNOME_RECENT: &str =
    "to type, keep one input source, or run IBus, which GNOME needs to record a switch";
const FIX_X11: &str =
    "to type, make the session layout the X server default with localectl set-x11-keymap";

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
    /// A second option list the keyboard can have instead, when Hush
    /// cannot tell which one applies. Hush then types only the characters
    /// whose keys are the same with both lists.
    pub other_options: Option<String>,
    pub active: Active,
}

impl Session {
    fn new(layout: Layout) -> Self {
        Self {
            layout,
            rules: String::new(),
            model: String::new(),
            options: Some(String::new()),
            other_options: None,
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
    let x11 = !wayland() && std::env::var_os("DISPLAY").is_some();
    if x11 {
        match_server_default(&mut session, run("localectl", &["status"]).as_deref());
    }
    if session.active == Active::Unknown && desktop_is("kde") {
        if let Some((names, index)) = kwin_layouts() {
            apply_kwin(&mut session, &names, index);
        }
    }
    if session.active == Active::Unknown && x11 {
        if let Some(group) = x11_group() {
            session.active = Active::Index(group);
        }
    }
    Some(session)
}

fn wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
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
    // KWin always compiles a keymap, so on KDE Wayland its rules decide.
    // On KDE X11, kded applies the layouts to the X server, and setxkbmap
    // reads them back.
    if desktop_is("kde") && wayland() {
        let config = kxkbrc.and_then(read).unwrap_or_default();
        return Some(kwin_session(&parse_kxkbrc(&config), &XkbEnv::read()));
    }
    if desktop_is("gnome") {
        let get = |key: &str| {
            run(
                "gsettings",
                &["get", "org.gnome.desktop.input-sources", key],
            )
        };
        if let Some(sources) = get("sources") {
            let gnome = Gnome {
                sources: &sources,
                mru: &get("mru-sources").unwrap_or_default(),
                options: &get("xkb-options").unwrap_or_default(),
                per_window: get("per-window").is_some_and(|value| value.trim() == "true"),
            };
            if let Some(session) = parse_gnome(&gnome, || run("ibus", &["engine"])) {
                return Some(session);
            }
        }
    }
    // Under Wayland, setxkbmap asks XWayland, whose rules often stay at the
    // US default whatever the compositor uses. Under X11, the X server's
    // keymap is the truth, and the variables mean nothing to it.
    if !wayland() {
        if let Some(session) = run("setxkbmap", &["-query"]).and_then(|s| parse_setxkbmap(&s)) {
            return Some(session);
        }
    }
    if let Some(session) = from_env(&XkbEnv::read()) {
        return Some(session);
    }
    run("localectl", &["status"]).and_then(|s| parse_localectl(&s))
}

/// The `XKB_DEFAULT_*` variables, which compositors read when their own
/// configuration names no layout. An unset variable is empty.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct XkbEnv {
    rules: String,
    model: String,
    layout: String,
    variant: String,
    options: String,
}

impl XkbEnv {
    fn read() -> Self {
        let var = |name: &str| {
            std::env::var(name)
                .map(|value| value.trim().to_string())
                .unwrap_or_default()
        };
        Self {
            rules: var("XKB_DEFAULT_RULES"),
            model: var("XKB_DEFAULT_MODEL"),
            layout: var("XKB_DEFAULT_LAYOUT"),
            variant: var("XKB_DEFAULT_VARIANT"),
            options: var("XKB_DEFAULT_OPTIONS"),
        }
    }
}

fn from_env(env: &XkbEnv) -> Option<Session> {
    if env.layout.is_empty() {
        return None;
    }
    Some(Session {
        rules: env.rules.clone(),
        model: env.model.clone(),
        options: Some(env.options.clone()),
        ..Session::new(Layout::xkb(env.layout.clone(), env.variant.clone()))
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

/// The `[Layout]` group of KDE's kxkbrc. An unset key is empty.
#[derive(Debug, Default, PartialEq, Eq)]
struct Kxkbrc {
    layouts: String,
    variants: String,
    model: String,
    /// The options, which KWin applies only when `ResetOldOptions` is true.
    options: Option<String>,
}

fn parse_kxkbrc(text: &str) -> Kxkbrc {
    let mut config = Kxkbrc::default();
    let mut in_layout = false;
    let mut reset_options = false;
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
        let value = value.trim().to_string();
        match key.trim() {
            "ResetOldOptions" => reset_options = value == "true",
            "LayoutList" => config.layouts = value,
            "VariantList" => config.variants = value,
            "Model" => config.model = value,
            "Options" => options = value,
            _ => {},
        }
    }
    config.options = reset_options.then_some(options);
    config
}

/// The names KWin compiles its keymap from. KWin takes kxkbrc's
/// `LayoutList` whatever `Use` says, else the `XKB_DEFAULT_*` layout, else
/// the libxkbcommon default. Its model defaults to pc104, and it applies the
/// kxkbrc options only with `ResetOldOptions=true`, else the variable.
fn kwin_session(config: &Kxkbrc, env: &XkbEnv) -> Session {
    let (layouts, variants) = if config.layouts.is_empty() {
        (env.layout.clone(), env.variant.clone())
    } else {
        (config.layouts.clone(), config.variants.clone())
    };
    Session {
        rules: env.rules.clone(),
        model: if config.model.is_empty() {
            "pc104".to_string()
        } else {
            config.model.clone()
        },
        options: Some(
            config
                .options
                .clone()
                .unwrap_or_else(|| env.options.clone()),
        ),
        ..Session::new(Layout::xkb(layouts, variants))
    }
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

/// KWin names its layouts as the raw items of its layout list. One empty
/// name means KWin found no layout and compiled the libxkbcommon default,
/// which Hush compiles the same way. Otherwise, when the list matches the
/// one Hush read, KWin's index is the active layout.
fn apply_kwin(session: &mut Session, kwin: &[String], index: u32) {
    if let [only] = kwin {
        if only.trim().is_empty() {
            session.layout = Layout::xkb("", "");
            session.active = Active::Index(0);
            return;
        }
    }
    let Layout::Xkb { layout: list, .. } = &session.layout else {
        return;
    };
    let read: Vec<&str> = list.split(',').map(str::trim).collect();
    session.active =
        if read.len() == kwin.len() && read.iter().zip(kwin).all(|(a, b)| *a == b.trim()) {
            Active::Index(index as usize)
        } else {
            Active::Unclear {
                why: format!(
                    "KWin uses the layouts {}, but Hush read {}",
                    kwin.join(","),
                    list
                ),
                fix: FIX_KWIN,
            }
        };
}

/// On X11, a keyboard that appears after login, as Hush's own does, gets the
/// X server's default keymap, not a later `setxkbmap` change. The default is
/// the X11 layout of `localectl`, or US when it names none. Hush types only
/// when the session's layouts are that default. When the options differ,
/// Hush's keyboard can have either list, because a desktop daemon can apply
/// the session options to it, so both lists go to the key table.
fn match_server_default(session: &mut Session, localectl: Option<&str>) {
    let Some(text) = localectl else {
        session.active = Active::Unclear {
            why: "on X11, Hush cannot read the default layout of the X server, which its own \
                  keyboard gets (localectl does not answer)"
                .to_string(),
            fix: FIX_X11,
        };
        return;
    };
    let default = Layout::xkb(
        field(text, "X11 Layout:").unwrap_or("us"),
        field(text, "X11 Variant:").unwrap_or_default(),
    );
    if !same_layouts(&session.layout, &default) {
        session.active = Active::Unclear {
            why: format!(
                "on X11, Hush's own keyboard gets the default layout {} of the X server, but \
                 the session uses {}",
                default, session.layout
            ),
            fix: FIX_X11,
        };
        return;
    }
    let default_options = field(text, "X11 Options:").unwrap_or_default();
    if session.options.as_deref().map(option_set) != Some(option_set(default_options)) {
        session.other_options = Some(default_options.to_string());
    }
}

fn same_layouts(a: &Layout, b: &Layout) -> bool {
    let normal = |layout: &Layout| {
        layout.entries().map(|entries| {
            entries
                .into_iter()
                .map(|(layout, variant)| {
                    let variant = variant.to_lowercase();
                    let variant = if variant == "basic" {
                        String::new()
                    } else {
                        variant
                    };
                    (layout.to_lowercase(), variant)
                })
                .collect::<Vec<_>>()
        })
    };
    matches!((normal(a), normal(b)), (Some(a), Some(b)) if a == b)
}

fn option_set(options: &str) -> BTreeSet<&str> {
    options
        .split(',')
        .map(str::trim)
        .filter(|option| !option.is_empty())
        .collect()
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

/// What GNOME's input-sources settings say.
struct Gnome<'a> {
    sources: &'a str,
    mru: &'a str,
    options: &'a str,
    per_window: bool,
}

/// GNOME's active input source. With one source, that one. With several,
/// the most recently used one, but only when GNOME keeps one source for all
/// windows and IBus runs the same source: gnome-shell writes `mru-sources`
/// only on a switch by the user, and only when IBus is ready. With
/// `per-window`, a focus change switches the source with no write at all.
/// An input method engine (`('ibus', 'mozc-jp')`) has no XKB names.
fn parse_gnome(gnome: &Gnome, engine: impl FnOnce() -> Option<String>) -> Option<Session> {
    let sources = gnome_sources(gnome.sources);
    let first = sources.first()?;
    let active = if sources.len() == 1 {
        Ok(first.clone())
    } else if gnome.per_window {
        Err(Active::Unclear {
            fix: FIX_PER_WINDOW,
            why: format!(
                "GNOME keeps an input source for each window (per-window is on), so Hush cannot \
             tell which of the {} sources is active",
                sources.len()
            ),
        })
    } else {
        match gnome_sources(gnome.mru)
            .into_iter()
            .find(|recent| sources.contains(recent))
        {
            None => Err(Active::Unclear {
                why: format!(
                    "GNOME has {} input sources and names no recent one",
                    sources.len()
                ),
                fix: FIX_GNOME_RECENT,
            }),
            Some(recent) => match engine() {
                Some(engine) if engine_is(&engine, &recent) => Ok(recent),
                Some(engine) => Err(Active::Unclear {
                    why: format!(
                        "IBus runs {}, but the most recent GNOME input source is {}",
                        engine.trim(),
                        recent.1
                    ),
                    fix: FIX_GNOME_RECENT,
                }),
                None => Err(Active::Unclear {
                    why: "IBus does not name the active input source, so the list of recent \
                          GNOME sources can be out of date"
                        .to_string(),
                    fix: FIX_GNOME_RECENT,
                }),
            },
        }
    };
    let options: Vec<&str> = gnome
        .options
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
        Ok(source) => (to_layout(&source), Active::Index(0)),
        Err(unclear) => (to_layout(first), unclear),
    };
    Some(Session {
        options: Some(options.join(",")),
        active,
        ..Session::new(layout)
    })
}

/// gnome-shell sets the IBus engine `xkb:LAYOUT:VARIANT:LANGUAGE` for an
/// XKB source, and the engine's own name for an IBus source.
fn engine_is(engine: &str, (kind, id): &(String, String)) -> bool {
    let engine = engine.trim();
    if kind != "xkb" {
        return engine == id;
    }
    let (layout, variant) = id.split_once('+').unwrap_or((id.as_str(), ""));
    let mut parts = engine.split(':');
    parts.next() == Some("xkb")
        && parts.next() == Some(layout)
        && parts.next().unwrap_or_default() == variant
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

    fn env(layout: &str, variant: &str, options: &str) -> XkbEnv {
        XkbEnv {
            layout: layout.into(),
            variant: variant.into(),
            options: options.into(),
            model: "pc105".into(),
            ..XkbEnv::default()
        }
    }

    #[test]
    fn kwin_reads_the_layout_list_whatever_use_says() {
        let kxkbrc = "[$Version]\nupdate_info=kxkb.upd:remove-empty-lists\n\n[Layout]\nDisplayNames=,\nLayoutList=us,de\nModel=pc105\nOptions=lv3:caps_switch\nResetOldOptions=true\nUse=true\nVariantList=dvorak,\n";
        let session = kwin_session(&parse_kxkbrc(kxkbrc), &env("fr", "", ""));
        assert_eq!(session.layout, layout("us,de", "dvorak,"));
        assert_eq!(session.model, "pc105");
        assert_eq!(session.options.as_deref(), Some("lv3:caps_switch"));
        let unused = kwin_session(
            &parse_kxkbrc(&kxkbrc.replace("Use=true", "Use=false")),
            &env("fr", "", ""),
        );
        assert_eq!(
            unused.layout,
            layout("us,de", "dvorak,"),
            "KWin never reads Use"
        );
    }

    #[test]
    fn without_a_layout_list_kwin_takes_the_variables_then_the_default() {
        let kxkbrc = "[Layout]\nOptions=ctrl:swap_lwin_lctl\nResetOldOptions=true\n";
        let session = kwin_session(
            &parse_kxkbrc(kxkbrc),
            &env("de", "nodeadkeys", "compose:ralt"),
        );
        assert_eq!(session.layout, layout("de", "nodeadkeys"));
        assert_eq!(
            session.options.as_deref(),
            Some("ctrl:swap_lwin_lctl"),
            "the kxkbrc options win with ResetOldOptions"
        );
        assert_eq!(
            session.model, "pc104",
            "KWin's default model, not the variable"
        );

        let kept = "[Layout]\nOptions=ctrl:swap_lwin_lctl\nResetOldOptions=false\n";
        let session = kwin_session(&parse_kxkbrc(kept), &env("de", "", "compose:ralt"));
        assert_eq!(session.options.as_deref(), Some("compose:ralt"));

        let bare = kwin_session(&parse_kxkbrc(""), &XkbEnv::default());
        assert_eq!(bare.layout, layout("", ""), "the libxkbcommon default");
        assert_eq!(bare.options.as_deref(), Some(""));
    }

    #[test]
    fn kwin_names_the_active_layout_when_its_list_matches() {
        let kwin = vec!["us".to_string(), "de".to_string()];
        let mut session = Session::new(layout("us,de", ",nodeadkeys"));
        apply_kwin(&mut session, &kwin, 1);
        assert_eq!(session.active, Active::Index(1));

        let mut session = Session::new(layout("de", ""));
        apply_kwin(&mut session, &kwin, 0);
        assert!(matches!(session.active, Active::Unclear { .. }));

        let mut session = Session::new(Layout::Described("German".into()));
        apply_kwin(&mut session, &kwin, 0);
        assert_eq!(session.active, Active::Unknown);
    }

    #[test]
    fn an_empty_kwin_layout_is_the_default_keymap() {
        let mut session = Session::new(layout("us", ""));
        apply_kwin(&mut session, &[String::new()], 0);
        assert_eq!(session.layout, layout("", ""));
        assert_eq!(session.active, Active::Index(0));
        assert!(session.layout.is_plain_us());
    }

    fn localectl(layout: &str, variant: &str, options: &str) -> String {
        let mut text = "   System Locale: LANG=de_DE.UTF-8\n       VC Keymap: de\n".to_string();
        for (name, value) in [
            ("X11 Layout", layout),
            ("X11 Variant", variant),
            ("X11 Options", options),
        ] {
            if !value.is_empty() {
                text.push_str(&format!("{:>16}: {}\n", name, value));
            }
        }
        text
    }

    #[test]
    fn x11_types_only_on_the_default_layout_of_the_server() {
        let setxkbmap = |layout_list: &str, options: &str| Session {
            options: Some(options.into()),
            ..Session::new(layout(layout_list, ""))
        };
        let mut session = setxkbmap("de", "compose:ralt");
        match_server_default(&mut session, Some(&localectl("de", "", "compose:ralt")));
        assert_eq!(session.active, Active::Unknown);
        assert_eq!(session.options.as_deref(), Some("compose:ralt"));

        let mut session = setxkbmap("de", "");
        match_server_default(&mut session, Some(&localectl("us", "", "")));
        assert!(
            matches!(&session.active, Active::Unclear { why, .. } if why.contains("default layout us")),
            "a setxkbmap change does not reach Hush's own keyboard: {:?}",
            session.active
        );

        let mut session = setxkbmap("us", "");
        match_server_default(&mut session, Some(&localectl("", "", "")));
        assert_eq!(
            session.active,
            Active::Unknown,
            "the X server defaults to US"
        );

        let mut session = setxkbmap("us(intl)", "");
        match_server_default(&mut session, Some(&localectl("us", "intl", "")));
        assert_eq!(
            session.active,
            Active::Unknown,
            "inline and listed variants agree"
        );

        let mut session = setxkbmap("de", "caps:escape,compose:102");
        match_server_default(&mut session, Some(&localectl("de", "", "compose:102")));
        assert_eq!(session.active, Active::Unknown);
        assert_eq!(session.options.as_deref(), Some("caps:escape,compose:102"));
        assert_eq!(
            session.other_options.as_deref(),
            Some("compose:102"),
            "Hush's keyboard can have the default options instead"
        );
        let mut same = setxkbmap("de", "compose:102, caps:escape");
        match_server_default(
            &mut same,
            Some(&localectl("de", "", "caps:escape,compose:102")),
        );
        assert_eq!(same.other_options, None, "the order does not matter");

        let mut session = setxkbmap("us", "");
        match_server_default(&mut session, None);
        assert!(matches!(session.active, Active::Unclear { .. }));
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

    fn gnome<'a>(sources: &'a str, mru: &'a str, options: &'a str) -> Gnome<'a> {
        Gnome {
            sources,
            mru,
            options,
            per_window: false,
        }
    }

    fn engine(name: &str) -> impl FnOnce() -> Option<String> + '_ {
        move || Some(format!("{name}\n"))
    }

    #[test]
    fn gnome_types_with_the_most_recent_source_that_ibus_runs() {
        let sources = "[('xkb', 'us+dvorak'), ('xkb', 'de')]\n";
        let mru = "[('xkb', 'de'), ('xkb', 'us+dvorak')]";
        let options = "['lv3:ralt_switch', 'compose:menu']";
        let session = parse_gnome(&gnome(sources, mru, options), engine("xkb:de::ger")).unwrap();
        assert_eq!(session.layout, layout("de", ""));
        assert_eq!(session.active, Active::Index(0));
        assert_eq!(
            session.options.as_deref(),
            Some("lv3:ralt_switch,compose:menu")
        );

        let dvorak = "[('xkb', 'us+dvorak'), ('xkb', 'de')]";
        let session =
            parse_gnome(&gnome(sources, dvorak, ""), engine("xkb:us:dvorak:eng")).unwrap();
        assert_eq!(session.layout, layout("us+dvorak", ""));
        assert_eq!(session.active, Active::Index(0));

        let session = parse_gnome(
            &gnome(sources, "@a(ss) []", "@as []"),
            engine("xkb:de::ger"),
        )
        .unwrap();
        assert_eq!(session.layout, layout("us+dvorak", ""));
        assert!(matches!(session.active, Active::Unclear { .. }));
        assert_eq!(session.options.as_deref(), Some(""));
    }

    #[test]
    fn gnome_pastes_when_the_recent_list_can_be_stale() {
        let sources = "[('xkb', 'us'), ('xkb', 'de')]";
        let mru = "[('xkb', 'us'), ('xkb', 'de')]";
        let stale = parse_gnome(&gnome(sources, mru, ""), engine("xkb:de::ger")).unwrap();
        assert!(
            matches!(&stale.active, Active::Unclear { why, .. } if why.contains("IBus runs xkb:de::ger")),
            "{:?}",
            stale.active
        );
        let no_ibus = parse_gnome(&gnome(sources, mru, ""), || None).unwrap();
        assert!(matches!(no_ibus.active, Active::Unclear { .. }));

        let per_window = Gnome {
            per_window: true,
            ..gnome(sources, mru, "")
        };
        let session =
            parse_gnome(&per_window, || panic!("per-window needs no IBus query")).unwrap();
        assert!(
            matches!(&session.active, Active::Unclear { why, .. } if why.contains("per-window")),
            "{:?}",
            session.active
        );
    }

    #[test]
    fn one_gnome_source_is_active_whatever_the_settings_say() {
        let single = Gnome {
            per_window: true,
            ..gnome("[('xkb', 'de')]", "@a(ss) []", "@as []")
        };
        let session = parse_gnome(&single, || None).unwrap();
        assert_eq!(session.layout, layout("de", ""));
        assert_eq!(session.active, Active::Index(0));
        assert!(parse_gnome(&gnome("[('xkb', 'us')]", "", ""), || None)
            .unwrap()
            .layout
            .is_plain_us());

        let ime = parse_gnome(
            &gnome(
                "[('ibus', 'mozc-jp'), ('xkb', 'us')]",
                "[('ibus', 'mozc-jp')]",
                "",
            ),
            engine("mozc-jp"),
        )
        .unwrap();
        assert_eq!(ime.layout, Layout::Described("ibus mozc-jp".into()));
        assert_eq!(ime.active, Active::Index(0));
        assert_eq!(parse_gnome(&gnome("@a(ss) []", "", ""), || None), None);
    }

    #[test]
    fn detection_never_panics() {
        let dir = tempfile::tempdir().unwrap();
        let _ = detect(Some(&dir.path().join("kxkbrc")));
    }
}
