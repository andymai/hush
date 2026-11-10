//! A [`KeyTable`] for the session's XKB keymap, built with libxkbcommon.
//!
//! Hush compiles the keymap from its RMLVO names, holds the modifiers in an
//! XKB state as a compositor would, and reads which character each key
//! types in the active layout. A character that no key types can come from
//! a dead key and a base key, but only when the compose table for the
//! locale turns that pair into the character.
//!
//! Hush loads libxkbcommon at run time, so a system without it falls back
//! to the US table or to the clipboard.

use super::keymap::{KeyTable, Tap, EXTRA_KEYS, MAIN_KEYS, MODIFIER_KEYS};
use anyhow::{anyhow, bail, Result};
use input_linux::Key;
use std::collections::HashMap;
use std::ffi::{c_char, CString};
use xkbcommon_dl::{
    keysyms, xkb_compose_compile_flags, xkb_compose_feed_result, xkb_compose_state,
    xkb_compose_state_flags, xkb_compose_status, xkb_compose_table, xkb_context, xkb_context_flags,
    xkb_key_direction, xkb_keymap, xkb_keymap_compile_flags, xkb_keysym_t, xkb_log_level,
    xkb_rule_names, xkb_state, xkb_state_component, XkbCommon, XkbCommonCompose, XKB_MOD_NAME_ALT,
    XKB_MOD_NAME_CTRL, XKB_MOD_NAME_LOGO, XKB_MOD_NAME_SHIFT,
};

/// XKB key codes are evdev codes plus 8.
const EVDEV_OFFSET: u32 = 8;

/// The RMLVO names a keymap compiles from. An empty rules, model, or
/// layout takes the libxkbcommon default; an empty options list is no
/// options.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Names {
    pub rules: String,
    pub model: String,
    pub layout: String,
    pub variant: String,
    pub options: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Request {
    pub names: Names,
    /// The active layout, as an index into the layout list.
    pub group: u32,
    /// Whether characters on the third level (AltGr) count. It is off when
    /// the XKB options are unknown, because an option can move AltGr.
    pub level3: bool,
    /// evdev codes Hush must not press, such as a key another program
    /// grabs.
    pub reserved: Vec<u16>,
    /// The locale whose compose table confirms dead-key sequences.
    pub locale: String,
}

pub fn available() -> bool {
    xkbcommon_dl::xkbcommon_option().is_some()
}

/// The locale applications use for their compose table, found the way
/// the libxkbcommon documentation describes.
pub fn session_locale() -> String {
    ["LC_ALL", "LC_CTYPE", "LANG"]
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()))
        .unwrap_or_else(|| "C".to_string())
}

pub fn build(request: &Request) -> Result<KeyTable> {
    let lib =
        xkbcommon_dl::xkbcommon_option().ok_or_else(|| anyhow!("libxkbcommon is not installed"))?;
    let context = Context::new(lib)?;
    let keymap = Keymap::compile(&context, &request.names)?;
    let layouts = keymap.layouts();
    if request.group >= layouts {
        bail!(
            "layout {} is active, but the keymap has {} layouts",
            request.group + 1,
            layouts
        );
    }
    let free = |key: Key| !request.reserved.contains(&key.code());
    let shift = find_modifier(&keymap, request.group, free, Modifier::Shift)?;
    let level3 = if request.level3 {
        find_modifier(&keymap, request.group, free, Modifier::Level3)?
    } else {
        None
    };
    let found = scan(&keymap, request.group, shift, level3, free)?;
    let mut chars: HashMap<char, Vec<Tap>> = found
        .direct
        .iter()
        .map(|&(ch, tap, _)| (ch, vec![tap]))
        .collect();
    if !found.dead.is_empty() {
        if let Some(compose) = Compose::new(&context, &request.locale) {
            let sequences =
                dead_key_sequences(&found, &chars, |dead, base| compose.compose(dead, base));
            chars.extend(sequences);
        }
    }
    Ok(KeyTable::new(chars, shift.unwrap_or(Key::LeftShift)))
}

/// The keys that can hold Control or Shift, in the order Hush tries them.
pub const CHORD_MODIFIER_KEYS: [Key; 9] = [
    Key::LeftCtrl,
    Key::RightCtrl,
    Key::LeftShift,
    Key::RightShift,
    Key::CapsLock,
    Key::LeftMeta,
    Key::RightMeta,
    Key::LeftAlt,
    Key::RightAlt,
];

/// The keys that hold Control and Shift in every keymap of a list, for a
/// keyboard that can have any of them. `None` when no key holds the
/// modifier in every keymap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Holders {
    pub control: Option<Key>,
    pub shift: Option<Key>,
}

pub fn holders(names: &[Names], group: u32) -> Result<Holders> {
    let lib =
        xkbcommon_dl::xkbcommon_option().ok_or_else(|| anyhow!("libxkbcommon is not installed"))?;
    let context = Context::new(lib)?;
    let keymaps = names
        .iter()
        .map(|names| Keymap::compile(&context, names))
        .collect::<Result<Vec<_>>>()?;
    Ok(Holders {
        control: common_holder(&keymaps, group, Held::Control)?,
        shift: common_holder(&keymaps, group, Held::Shift)?,
    })
}

/// The keys that can hold a modifier, for [`key_classes`].
const CLASS_KEYS: [Key; 11] = [
    Key::LeftCtrl,
    Key::RightCtrl,
    Key::LeftShift,
    Key::RightShift,
    Key::LeftAlt,
    Key::RightAlt,
    Key::LeftMeta,
    Key::RightMeta,
    Key::CapsLock,
    Key::Compose,
    Key::NonUsBackslashAndPipe,
];

/// What the keys that can hold a modifier type in one keymap.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyClasses {
    pub control: Vec<Key>,
    pub shift: Vec<Key>,
    pub alt: Vec<Key>,
    pub super_keys: Vec<Key>,
    /// The keys that type AltGr (ISO_Level3_Shift).
    pub altgr: Vec<Key>,
    /// The keys that hold another modifier: the fifth level, Mode_switch,
    /// or Hyper.
    pub other: Vec<Key>,
}

pub fn key_classes(names: &Names) -> Result<KeyClasses> {
    let lib =
        xkbcommon_dl::xkbcommon_option().ok_or_else(|| anyhow!("libxkbcommon is not installed"))?;
    let context = Context::new(lib)?;
    let keymap = Keymap::compile(&context, names)?;
    let mut classes = KeyClasses::default();
    for key in CLASS_KEYS {
        let class = if holds(&keymap, 0, key, Held::Control)? {
            &mut classes.control
        } else if holds(&keymap, 0, key, Held::Shift)? {
            &mut classes.shift
        } else if holds(&keymap, 0, key, Held::Alt)? {
            &mut classes.alt
        } else if holds(&keymap, 0, key, Held::Super)? {
            &mut classes.super_keys
        } else {
            match State::new(&keymap, 0)?.sym(key) {
                keysyms::ISO_Level3_Shift => &mut classes.altgr,
                keysyms::ISO_Level5_Shift
                | keysyms::Mode_switch
                | keysyms::Hyper_L
                | keysyms::Hyper_R => &mut classes.other,
                _ => continue,
            }
        };
        class.push(key);
    }
    Ok(classes)
}

#[derive(Clone, Copy)]
enum Held {
    Control,
    Shift,
    Alt,
    Super,
}

fn common_holder(keymaps: &[Keymap], group: u32, held: Held) -> Result<Option<Key>> {
    for key in CHORD_MODIFIER_KEYS {
        let mut every = true;
        for keymap in keymaps {
            every &= holds(keymap, group, key, held)?;
        }
        if every {
            return Ok(Some(key));
        }
    }
    Ok(None)
}

/// The key types the modifier's keysym, holds the modifier while it is
/// down, and changes nothing else.
fn holds(keymap: &Keymap, group: u32, key: Key, held: Held) -> Result<bool> {
    let state = State::new(keymap, group)?;
    let wanted: &[xkb_keysym_t] = match held {
        Held::Control => &[keysyms::Control_L, keysyms::Control_R],
        Held::Shift => &[keysyms::Shift_L, keysyms::Shift_R],
        Held::Alt => &[
            keysyms::Alt_L,
            keysyms::Alt_R,
            keysyms::Meta_L,
            keysyms::Meta_R,
        ],
        Held::Super => &[keysyms::Super_L, keysyms::Super_R],
    };
    if !wanted.contains(&state.sym(key)) {
        return Ok(false);
    }
    state.press(key);
    let active = match held {
        Held::Control => state.control_active(),
        Held::Shift => state.shift_active(),
        Held::Alt => state.mod_active(XKB_MOD_NAME_ALT),
        Held::Super => state.mod_active(XKB_MOD_NAME_LOGO),
    };
    let ok = active && state.layout() == group;
    state.release(key);
    Ok(ok && state.mods() == 0)
}

/// What the keys type with each set of modifiers, cheapest first.
struct Found {
    direct: Vec<(char, Tap, xkb_keysym_t)>,
    dead: Vec<(xkb_keysym_t, Tap)>,
}

fn scan(
    keymap: &Keymap,
    group: u32,
    shift: Option<Key>,
    level3: Option<Key>,
    free: impl Fn(Key) -> bool,
) -> Result<Found> {
    let mut levels = vec![(None, None)];
    if shift.is_some() {
        levels.push((shift, None));
    }
    if level3.is_some() {
        levels.push((None, level3));
        if shift.is_some() {
            levels.push((shift, level3));
        }
    }
    let mut found = Found {
        direct: Vec::new(),
        dead: Vec::new(),
    };
    for keys in [&MAIN_KEYS[..], &EXTRA_KEYS[..]] {
        for &(shift, level3) in &levels {
            let state = State::new(keymap, group)?;
            if let Some(key) = level3 {
                state.press(key);
            }
            if let Some(key) = shift {
                state.press(key);
            }
            if state.layout() != group {
                continue;
            }
            for &key in keys {
                if !free(key) || Some(key) == shift || Some(key) == level3 {
                    continue;
                }
                let sym = state.sym(key);
                let tap = Tap { key, shift, level3 };
                if is_dead(sym) {
                    if !found.dead.iter().any(|&(dead, _)| dead == sym) {
                        found.dead.push((sym, tap));
                    }
                } else if let Some(ch) = keymap.char_for(sym) {
                    if !found.direct.iter().any(|&(known, _, _)| known == ch) {
                        found.direct.push((ch, tap, sym));
                    }
                }
            }
        }
    }
    Ok(found)
}

/// For each character no key types, the cheapest dead key and base key
/// that `compose` turns into it.
fn dead_key_sequences(
    found: &Found,
    direct: &HashMap<char, Vec<Tap>>,
    compose: impl Fn(xkb_keysym_t, xkb_keysym_t) -> Option<String>,
) -> HashMap<char, Vec<Tap>> {
    let mut best: HashMap<char, (usize, Vec<Tap>)> = HashMap::new();
    for &(dead, dead_tap) in &found.dead {
        for &(_, base_tap, base) in &found.direct {
            let Some(text) = compose(dead, base) else {
                continue;
            };
            let mut chars = text.chars();
            let (Some(ch), None) = (chars.next(), chars.next()) else {
                continue;
            };
            if ch.is_control() || direct.contains_key(&ch) {
                continue;
            }
            let cost = held(&dead_tap) + held(&base_tap);
            if best.get(&ch).is_none_or(|(known, _)| cost < *known) {
                best.insert(ch, (cost, vec![dead_tap, base_tap]));
            }
        }
    }
    best.into_iter().map(|(ch, (_, taps))| (ch, taps)).collect()
}

fn held(tap: &Tap) -> usize {
    usize::from(tap.shift.is_some()) + usize::from(tap.level3.is_some())
}

fn is_dead(sym: xkb_keysym_t) -> bool {
    (keysyms::dead_grave..=keysyms::dead_longsolidusoverlay).contains(&sym)
}

#[derive(Clone, Copy)]
enum Modifier {
    Shift,
    Level3,
}

/// The key that holds the modifier in the group: Shift_L or Shift_R for
/// Shift, ISO_Level3_Shift for the third level. A press must hold a
/// modifier without a group change, and the release must clear it.
fn find_modifier(
    keymap: &Keymap,
    group: u32,
    free: impl Fn(Key) -> bool,
    modifier: Modifier,
) -> Result<Option<Key>> {
    let wanted: &[xkb_keysym_t] = match modifier {
        Modifier::Shift => &[keysyms::Shift_L, keysyms::Shift_R],
        Modifier::Level3 => &[keysyms::ISO_Level3_Shift],
    };
    for key in MODIFIER_KEYS {
        if !free(key) {
            continue;
        }
        let state = State::new(keymap, group)?;
        if !wanted.contains(&state.sym(key)) {
            continue;
        }
        state.press(key);
        let holds = state.layout() == group
            && match modifier {
                Modifier::Shift => state.shift_active(),
                Modifier::Level3 => state.mods() != 0 && !state.shift_active(),
            };
        state.release(key);
        if holds && state.mods() == 0 && state.layout() == group {
            return Ok(Some(key));
        }
    }
    Ok(None)
}

struct Context {
    lib: &'static XkbCommon,
    ptr: *mut xkb_context,
}

impl Context {
    fn new(lib: &'static XkbCommon) -> Result<Self> {
        // SAFETY: the flags are valid, and a null result is checked.
        let ptr =
            unsafe { (lib.xkb_context_new)(xkb_context_flags::XKB_CONTEXT_NO_ENVIRONMENT_NAMES) };
        if ptr.is_null() {
            bail!("libxkbcommon could not make a context");
        }
        // Without this, a layout that does not compile prints errors on stderr.
        // SAFETY: ptr is a live context.
        unsafe { (lib.xkb_context_set_log_level)(ptr, xkb_log_level::XKB_LOG_LEVEL_CRITICAL) };
        Ok(Self { lib, ptr })
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        // SAFETY: ptr came from xkb_context_new and is released once.
        unsafe { (self.lib.xkb_context_unref)(self.ptr) }
    }
}

struct Keymap {
    lib: &'static XkbCommon,
    ptr: *mut xkb_keymap,
}

impl Keymap {
    fn compile(context: &Context, names: &Names) -> Result<Self> {
        let field =
            |value: &str| CString::new(value).map_err(|_| anyhow!("an XKB name holds a NUL byte"));
        let rules = field(&names.rules)?;
        let model = field(&names.model)?;
        let layout = field(&names.layout)?;
        let variant = field(&names.variant)?;
        let options = field(&names.options)?;
        let rmlvo = xkb_rule_names {
            rules: rules.as_ptr(),
            model: model.as_ptr(),
            layout: layout.as_ptr(),
            variant: variant.as_ptr(),
            options: options.as_ptr(),
        };
        // SAFETY: the context is live, and every string outlives the call.
        let ptr = unsafe {
            (context.lib.xkb_keymap_new_from_names)(
                context.ptr,
                &rmlvo,
                xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
            )
        };
        if ptr.is_null() {
            bail!(
                "the XKB keymap for layout '{}', variant '{}', options '{}' did not compile",
                names.layout,
                names.variant,
                names.options
            );
        }
        Ok(Self {
            lib: context.lib,
            ptr,
        })
    }

    fn layouts(&self) -> u32 {
        // SAFETY: ptr is a live keymap.
        unsafe { (self.lib.xkb_keymap_num_layouts)(self.ptr) }
    }

    fn char_for(&self, sym: xkb_keysym_t) -> Option<char> {
        // SAFETY: the function takes any keysym value.
        let code = unsafe { (self.lib.xkb_keysym_to_utf32)(sym) };
        char::from_u32(code).filter(|ch| code != 0 && !ch.is_control())
    }
}

impl Drop for Keymap {
    fn drop(&mut self) {
        // SAFETY: ptr came from xkb_keymap_new_from_names and is released once.
        unsafe { (self.lib.xkb_keymap_unref)(self.ptr) }
    }
}

/// An XKB state with the group locked, as after a layout switch.
struct State {
    lib: &'static XkbCommon,
    ptr: *mut xkb_state,
}

impl State {
    fn new(keymap: &Keymap, group: u32) -> Result<Self> {
        // SAFETY: the keymap is live; the state takes its own reference.
        let ptr = unsafe { (keymap.lib.xkb_state_new)(keymap.ptr) };
        if ptr.is_null() {
            bail!("libxkbcommon could not make a keyboard state");
        }
        // SAFETY: ptr is a live state.
        unsafe { (keymap.lib.xkb_state_update_mask)(ptr, 0, 0, 0, 0, 0, group) };
        Ok(Self {
            lib: keymap.lib,
            ptr,
        })
    }

    fn press(&self, key: Key) {
        self.update(key, xkb_key_direction::XKB_KEY_DOWN);
    }

    fn release(&self, key: Key) {
        self.update(key, xkb_key_direction::XKB_KEY_UP);
    }

    fn update(&self, key: Key, direction: xkb_key_direction) {
        // SAFETY: ptr is a live state; an unknown key code is ignored.
        unsafe { (self.lib.xkb_state_update_key)(self.ptr, code(key), direction) };
    }

    fn sym(&self, key: Key) -> xkb_keysym_t {
        // SAFETY: ptr is a live state; an unknown key code gives NoSymbol.
        unsafe { (self.lib.xkb_state_key_get_one_sym)(self.ptr, code(key)) }
    }

    fn layout(&self) -> u32 {
        // SAFETY: ptr is a live state.
        unsafe {
            (self.lib.xkb_state_serialize_layout)(
                self.ptr,
                xkb_state_component::XKB_STATE_LAYOUT_EFFECTIVE,
            )
        }
    }

    fn mods(&self) -> u32 {
        // SAFETY: ptr is a live state.
        unsafe {
            (self.lib.xkb_state_serialize_mods)(
                self.ptr,
                xkb_state_component::XKB_STATE_MODS_EFFECTIVE,
            )
        }
    }

    fn shift_active(&self) -> bool {
        self.mod_active(XKB_MOD_NAME_SHIFT)
    }

    fn control_active(&self) -> bool {
        self.mod_active(XKB_MOD_NAME_CTRL)
    }

    /// `name` is one of the NUL-terminated `XKB_MOD_NAME` constants.
    fn mod_active(&self, name: &'static [u8]) -> bool {
        // SAFETY: ptr is a live state, and each caller passes a
        // NUL-terminated name.
        unsafe {
            (self.lib.xkb_state_mod_name_is_active)(
                self.ptr,
                name.as_ptr().cast::<c_char>(),
                xkb_state_component::XKB_STATE_MODS_EFFECTIVE,
            ) == 1
        }
    }
}

impl Drop for State {
    fn drop(&mut self) {
        // SAFETY: ptr came from xkb_state_new and is released once.
        unsafe { (self.lib.xkb_state_unref)(self.ptr) }
    }
}

fn code(key: Key) -> u32 {
    u32::from(key.code()) + EVDEV_OFFSET
}

/// The compose table for a locale, with one state to feed it.
struct Compose {
    lib: &'static XkbCommonCompose,
    table: *mut xkb_compose_table,
    state: *mut xkb_compose_state,
}

impl Compose {
    fn new(context: &Context, locale: &str) -> Option<Self> {
        let lib = xkbcommon_dl::xkbcommon_compose_option()?;
        let locale = CString::new(locale).ok()?;
        // SAFETY: the context is live, and the locale outlives the call.
        let table = unsafe {
            (lib.xkb_compose_table_new_from_locale)(
                context.ptr,
                locale.as_ptr(),
                xkb_compose_compile_flags::XKB_COMPOSE_COMPILE_NO_FLAGS,
            )
        };
        if table.is_null() {
            return None;
        }
        // SAFETY: the table is live; the state takes its own reference.
        let state = unsafe {
            (lib.xkb_compose_state_new)(table, xkb_compose_state_flags::XKB_COMPOSE_STATE_NO_FLAGS)
        };
        if state.is_null() {
            // SAFETY: the table came from xkb_compose_table_new_from_locale.
            unsafe { (lib.xkb_compose_table_unref)(table) };
            return None;
        }
        Some(Self { lib, table, state })
    }

    /// The text a dead key and then a base key compose to.
    fn compose(&self, dead: xkb_keysym_t, base: xkb_keysym_t) -> Option<String> {
        // SAFETY: state is a live compose state, and the buffer length is
        // passed with the buffer.
        unsafe {
            (self.lib.xkb_compose_state_reset)(self.state);
            if (self.lib.xkb_compose_state_feed)(self.state, dead)
                != xkb_compose_feed_result::XKB_COMPOSE_FEED_ACCEPTED
                || (self.lib.xkb_compose_state_get_status)(self.state)
                    != xkb_compose_status::XKB_COMPOSE_COMPOSING
            {
                return None;
            }
            (self.lib.xkb_compose_state_feed)(self.state, base);
            if (self.lib.xkb_compose_state_get_status)(self.state)
                != xkb_compose_status::XKB_COMPOSE_COMPOSED
            {
                return None;
            }
            let mut buffer = [0u8; 64];
            let written = (self.lib.xkb_compose_state_get_utf8)(
                self.state,
                buffer.as_mut_ptr().cast::<c_char>(),
                buffer.len(),
            );
            let written = usize::try_from(written).ok()?;
            let bytes = buffer.get(..written).filter(|_| written < buffer.len())?;
            String::from_utf8(bytes.to_vec()).ok()
        }
    }
}

impl Drop for Compose {
    fn drop(&mut self) {
        // SAFETY: both came from their constructors and are released once.
        unsafe {
            (self.lib.xkb_compose_state_unref)(self.state);
            (self.lib.xkb_compose_table_unref)(self.table);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::path::Path;

    const LOCALE: &str = "en_US.UTF-8";

    /// Whether the keymap tests can run here: libxkbcommon, the
    /// xkeyboard-config data, and the X11 compose tables.
    pub(crate) fn xkb_ready() -> bool {
        let root = std::env::var("XKB_CONFIG_ROOT").unwrap_or_else(|_| "/usr/share/X11/xkb".into());
        let missing = if !available() {
            Some("libxkbcommon does not load")
        } else if !Path::new(&root).join("symbols/us").is_file() {
            Some("the xkeyboard-config data (xkb-data) is not installed")
        } else if !Path::new("/usr/share/X11/locale/en_US.UTF-8/Compose").is_file() {
            Some("the X11 compose tables (libx11-data) are not installed")
        } else {
            None
        };
        if let Some(why) = missing {
            eprintln!("Skipping the XKB keymap test: {}", why);
        }
        missing.is_none()
    }

    pub(crate) fn names(layout: &str, variant: &str) -> Names {
        Names {
            layout: layout.into(),
            variant: variant.into(),
            ..Names::default()
        }
    }

    fn request(layout: &str, variant: &str, group: u32) -> Request {
        Request {
            names: names(layout, variant),
            group,
            level3: true,
            reserved: Vec::new(),
            locale: LOCALE.into(),
        }
    }

    fn table(layout: &str, variant: &str) -> KeyTable {
        build(&request(layout, variant, 0)).expect("the keymap compiles")
    }

    fn plain(key: Key) -> Vec<Tap> {
        vec![Tap::plain(key)]
    }

    fn shifted(key: Key) -> Vec<Tap> {
        vec![Tap::shifted(key)]
    }

    fn altgr(key: Key, shift: bool) -> Tap {
        Tap {
            key,
            shift: shift.then_some(Key::LeftShift),
            level3: Some(Key::RightAlt),
        }
    }

    fn taps(table: &KeyTable, ch: char) -> Option<Vec<Tap>> {
        table.taps(ch).map(<[Tap]>::to_vec)
    }

    #[test]
    fn the_us_keymap_matches_the_built_in_us_table() {
        if !xkb_ready() {
            return;
        }
        let us = table("us", "");
        let built_in = KeyTable::us();
        for ch in (' '..='~').filter(|ch| !ch.is_control()) {
            assert_eq!(us.taps(ch), built_in.taps(ch), "{ch:?}");
        }
        assert_eq!(us.taps('é'), None, "plain US has no dead keys");
        assert!(!us.covers("café"));
        assert!(!us.covers("👍"));
    }

    #[test]
    fn german_swaps_y_and_z_and_uses_altgr() {
        if !xkb_ready() {
            return;
        }
        let de = table("de", "");
        assert_eq!(taps(&de, 'z'), Some(plain(Key::Y)));
        assert_eq!(taps(&de, 'Y'), Some(shifted(Key::Z)));
        assert_eq!(taps(&de, 'ö'), Some(plain(Key::Semicolon)));
        assert_eq!(taps(&de, 'ß'), Some(plain(Key::Minus)));
        assert_eq!(taps(&de, '@'), Some(vec![altgr(Key::Q, false)]));
        assert_eq!(taps(&de, '€'), Some(vec![altgr(Key::E, false)]));
        assert_eq!(taps(&de, '{'), Some(vec![altgr(Key::Num7, false)]));
        assert_eq!(taps(&de, '¿'), Some(vec![altgr(Key::Minus, true)]));
        assert_eq!(
            taps(&de, '<'),
            Some(plain(Key::NonUsBackslashAndPipe)),
            "only the ISO key types <"
        );
        assert_eq!(
            taps(&de, 'é'),
            Some(vec![Tap::plain(Key::Equal), Tap::plain(Key::E)]),
            "the dead acute, then e"
        );
        assert_eq!(
            taps(&de, '^'),
            Some(vec![Tap::plain(Key::Grave), Tap::plain(Key::Space)]),
            "the dead circumflex, then Space"
        );
        assert!(de.covers("Größe: 3 € @ café ^_^"));
        assert!(!de.covers("señor"), "German has no dead tilde");
        assert!(!de.covers("日本語"));
    }

    #[test]
    fn french_puts_digits_on_shift() {
        if !xkb_ready() {
            return;
        }
        let fr = table("fr", "");
        assert_eq!(taps(&fr, 'a'), Some(plain(Key::Q)));
        assert_eq!(taps(&fr, 'q'), Some(plain(Key::A)));
        assert_eq!(taps(&fr, '1'), Some(shifted(Key::Num1)));
        assert_eq!(taps(&fr, 'é'), Some(plain(Key::Num2)));
        assert_eq!(
            taps(&fr, 'ê'),
            Some(vec![Tap::plain(Key::LeftBrace), Tap::plain(Key::E)])
        );
        assert!(fr.covers("Où êtes-vous ? À l'école, 100 %."));
    }

    #[test]
    fn us_international_types_quotes_with_altgr() {
        if !xkb_ready() {
            return;
        }
        let intl = table("us", "intl");
        assert_eq!(
            taps(&intl, '\''),
            Some(vec![altgr(Key::Apostrophe, false)]),
            "the plain key is a dead acute"
        );
        assert_eq!(taps(&intl, 'é'), Some(vec![altgr(Key::E, false)]));
        assert_eq!(
            taps(&intl, 'ã'),
            Some(vec![Tap::shifted(Key::Grave), Tap::plain(Key::A)])
        );
        assert!(intl.covers("don't say \"ça\""));
        assert_eq!(
            table("us(intl)", ""),
            intl,
            "the variant can also be inline"
        );
    }

    #[test]
    fn dvorak_and_spanish_map_their_own_keys() {
        if !xkb_ready() {
            return;
        }
        let dvorak = table("us", "dvorak");
        assert_eq!(taps(&dvorak, 'a'), Some(plain(Key::A)));
        assert_eq!(taps(&dvorak, 'o'), Some(plain(Key::S)));
        assert_eq!(taps(&dvorak, ','), Some(plain(Key::W)));
        assert_eq!(taps(&dvorak, '\''), Some(plain(Key::Q)));
        assert_eq!(
            dvorak.plain_key('v'),
            Some(Key::Dot),
            "Ctrl+V presses this key"
        );

        let es = table("es", "");
        assert_eq!(taps(&es, 'ñ'), Some(plain(Key::Semicolon)));
        assert_eq!(taps(&es, '@'), Some(vec![altgr(Key::Num2, false)]));
        assert_eq!(
            taps(&es, 'á'),
            Some(vec![Tap::plain(Key::Apostrophe), Tap::plain(Key::A)])
        );
        assert!(es.covers("¿Qué tal, señora? ¡Bien!"));
    }

    #[test]
    fn two_layouts_type_through_the_active_group() {
        if !xkb_ready() {
            return;
        }
        let us = build(&request("us,de", "", 0)).expect("the keymap compiles");
        let de = build(&request("us,de", "", 1)).expect("the keymap compiles");
        assert_eq!(taps(&us, 'z'), Some(plain(Key::Z)));
        assert_eq!(taps(&de, 'z'), Some(plain(Key::Y)));
        assert_eq!(taps(&de, '@'), Some(vec![altgr(Key::Q, false)]));
        assert_eq!(taps(&us, 'ö'), None);
        assert!(build(&request("us,de", "", 2)).is_err(), "no third layout");
    }

    #[test]
    fn without_the_third_level_altgr_characters_go() {
        if !xkb_ready() {
            return;
        }
        let de = build(&Request {
            level3: false,
            ..request("de", "", 0)
        })
        .expect("the keymap compiles");
        assert_eq!(taps(&de, '@'), None);
        assert_eq!(taps(&de, 'z'), Some(plain(Key::Y)));
        let reserved = build(&Request {
            reserved: vec![Key::RightAlt.code()],
            ..request("de", "", 0)
        })
        .expect("the keymap compiles");
        assert_eq!(
            taps(&reserved, '@'),
            None,
            "a grabbed AltGr key is never pressed"
        );
    }

    #[test]
    fn an_option_moves_the_third_level() {
        if !xkb_ready() {
            return;
        }
        let de = build(&Request {
            names: Names {
                options: "lv3:caps_switch,lv3:ralt_alt".into(),
                ..names("de", "")
            },
            ..request("de", "", 0)
        })
        .expect("the keymap compiles");
        assert_eq!(
            taps(&de, '@'),
            Some(vec![Tap {
                key: Key::Q,
                shift: None,
                level3: Some(Key::CapsLock)
            }])
        );
    }

    #[test]
    fn each_modifier_key_gets_the_class_it_types() {
        if !xkb_ready() {
            return;
        }
        let classes = |layout: &str, options: &str| {
            key_classes(&Names {
                layout: layout.into(),
                options: options.into(),
                ..Names::default()
            })
            .unwrap()
        };
        let us = classes("us", "");
        assert_eq!(us.control, [Key::LeftCtrl, Key::RightCtrl]);
        assert_eq!(us.alt, [Key::LeftAlt, Key::RightAlt]);
        assert_eq!(us.super_keys, [Key::LeftMeta, Key::RightMeta]);
        assert!(us.altgr.is_empty() && us.other.is_empty());

        let swapped = classes("us", "ctrl:swap_lwin_lctl,ctrl:swap_rwin_rctl");
        assert_eq!(swapped.control, [Key::LeftMeta, Key::RightMeta]);
        assert_eq!(swapped.super_keys, [Key::LeftCtrl, Key::RightCtrl]);

        let swapcaps = classes("us", "ctrl:swapcaps");
        assert_eq!(swapcaps.control, [Key::RightCtrl, Key::CapsLock]);
        assert!(
            !swapcaps.super_keys.contains(&Key::LeftCtrl)
                && !swapcaps.other.contains(&Key::LeftCtrl),
            "Left Ctrl types Caps Lock, which holds no modifier"
        );

        let alt_win = classes("us", "altwin:swap_alt_win");
        assert_eq!(alt_win.alt, [Key::LeftMeta, Key::RightMeta]);
        assert_eq!(alt_win.super_keys, [Key::LeftAlt, Key::RightAlt]);

        let german = classes("de", "");
        assert_eq!(german.alt, [Key::LeftAlt]);
        assert_eq!(german.altgr, [Key::RightAlt]);
        assert_eq!(classes("us", "lv3:caps_switch").altgr, [Key::CapsLock]);
    }

    #[test]
    fn control_is_on_the_key_that_the_options_give_it() {
        if !xkb_ready() {
            return;
        }
        let us = |options: &str| Names {
            layout: "us".into(),
            options: options.into(),
            ..Names::default()
        };
        let control = |names: &[Names]| holders(names, 0).unwrap().control;
        assert_eq!(
            holders(&[us("")], 0).unwrap(),
            Holders {
                control: Some(Key::LeftCtrl),
                shift: Some(Key::LeftShift)
            }
        );
        assert_eq!(
            control(&[us("ctrl:swapcaps")]),
            Some(Key::RightCtrl),
            "Left Ctrl types Caps Lock"
        );
        assert_eq!(
            holders(&[us("ctrl:swap_lwin_lctl,ctrl:swap_rwin_rctl")], 0).unwrap(),
            Holders {
                control: Some(Key::LeftMeta),
                shift: Some(Key::LeftShift)
            },
            "both Ctrl keys type Super, and Left Win types Control_L"
        );
        assert_eq!(
            control(&[us(""), us("ctrl:swap_lwin_lctl")]),
            Some(Key::RightCtrl),
            "the key must hold Control with each option list"
        );
        assert_eq!(
            control(&[us("ctrl:swap_lwin_lctl"), us("ctrl:swap_rwin_rctl")]),
            None,
            "no key holds Control with both lists"
        );
        assert!(holders(
            &[
                us(""),
                Names {
                    layout: "no-such-layout".into(),
                    ..Names::default()
                }
            ],
            0
        )
        .is_err());
    }

    #[test]
    fn an_unknown_layout_does_not_compile() {
        if !xkb_ready() {
            return;
        }
        assert!(build(&request("no-such-layout", "", 0)).is_err());
    }

    #[test]
    fn a_dead_key_sequence_needs_the_compose_table() {
        let found = Found {
            direct: vec![('e', Tap::plain(Key::E), u32::from('e'))],
            dead: vec![(keysyms::dead_acute, Tap::plain(Key::Equal))],
        };
        let direct = HashMap::from([('e', vec![Tap::plain(Key::E)])]);
        assert!(
            dead_key_sequences(&found, &direct, |_, _| None).is_empty(),
            "no sequence without the compose table's word"
        );
        let confirmed = dead_key_sequences(&found, &direct, |_, _| Some("é".into()));
        assert_eq!(
            confirmed.get(&'é'),
            Some(&vec![Tap::plain(Key::Equal), Tap::plain(Key::E)])
        );
        assert!(
            dead_key_sequences(&found, &direct, |_, _| Some("e\u{301}".into())).is_empty(),
            "a result of two characters does not count"
        );
    }
}
