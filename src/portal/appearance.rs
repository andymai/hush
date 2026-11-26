//! The desktop's appearance through the Settings portal: light or dark, the
//! accent colour, higher contrast, and reduced motion
//! (`org.freedesktop.appearance`). An older portal lacks some of these
//! keys, so GNOME's and KDE's own keys fill the gaps.
//!
//! GNOME and KDE answer the portal. On wlroots desktops,
//! xdg-desktop-portal-wlr does not, so the gtk portal or darkman must.

use std::collections::HashMap;
use std::time::Duration;
use tracing::debug;
use zbus::zvariant::{OwnedValue, Value};

const DESTINATION: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";
const INTERFACE: &str = "org.freedesktop.portal.Settings";
const TIMEOUT: Duration = Duration::from_secs(2);

const APPEARANCE: &str = "org.freedesktop.appearance";
const GNOME_INTERFACE: &str = "org.gnome.desktop.interface";
const GNOME_A11Y: &str = "org.gnome.desktop.a11y.interface";
const KDE: &str = "org.kde.kdeglobals.KDE";
const NAMESPACES: [&str; 4] = [APPEARANCE, GNOME_INTERFACE, GNOME_A11Y, KDE];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scheme {
    #[default]
    NoPreference,
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Appearance {
    pub scheme: Scheme,
    /// sRGB
    pub accent: Option<[u8; 3]>,
    pub high_contrast: bool,
    pub reduced_motion: bool,
}

/// Each key as the portal last gave it.
#[derive(Debug, Clone, Default, PartialEq)]
struct Keys {
    scheme: Option<Scheme>,
    accent: Option<[u8; 3]>,
    contrast: Option<bool>,
    reduced_motion: Option<bool>,
    gnome_high_contrast: Option<bool>,
    gnome_animations: Option<bool>,
    kde_animation_factor: Option<f64>,
}

impl Keys {
    fn set(&mut self, namespace: &str, key: &str, value: &Value<'_>) {
        let value = unwrap(value);
        match (namespace, key) {
            (APPEARANCE, "color-scheme") => self.scheme = scheme_of(value),
            (APPEARANCE, "accent-color") => self.accent = accent_of(value),
            (APPEARANCE, "contrast") => self.contrast = number_of(value).map(|n| n == 1),
            (APPEARANCE, "reduced-motion") => {
                self.reduced_motion = number_of(value).map(|n| n == 1)
            },
            (GNOME_A11Y, "high-contrast") => self.gnome_high_contrast = flag_of(value),
            (GNOME_INTERFACE, "enable-animations") => self.gnome_animations = flag_of(value),
            (KDE, "AnimationDurationFactor") => self.kde_animation_factor = factor_of(value),
            _ => {},
        }
    }

    /// The freedesktop key wins. Without it, any desktop key that asks for
    /// less motion or more contrast turns that on: KDE also answers GNOME's
    /// keys, with GNOME's defaults.
    fn appearance(&self) -> Appearance {
        let gnome_still = self.gnome_animations.map(|on| !on);
        let kde_still = self.kde_animation_factor.map(|factor| factor <= 0.0);
        Appearance {
            scheme: self.scheme.unwrap_or_default(),
            accent: self.accent,
            high_contrast: self
                .contrast
                .unwrap_or(self.gnome_high_contrast == Some(true)),
            reduced_motion: self
                .reduced_motion
                .unwrap_or(gnome_still == Some(true) || kde_still == Some(true)),
        }
    }
}

/// Read the appearance on a thread of its own, then call `on_change` with
/// it and again after each change. Without a portal, `on_change` never
/// runs.
pub fn watch(on_change: impl FnMut(Appearance) + Send + 'static) {
    // zbus blocks on a runtime of its own, so it gets a thread of its own.
    let spawned = std::thread::Builder::new()
        .name("hush-appearance".into())
        .spawn(move || {
            if let Err(e) = listen(on_change) {
                debug!("No appearance from the desktop portal: {}", e);
            }
        });
    if let Err(e) = spawned {
        debug!("Could not watch the desktop appearance: {}", e);
    }
}

fn listen(mut on_change: impl FnMut(Appearance)) -> zbus::Result<()> {
    let connection = zbus::blocking::connection::Builder::session()?
        .method_timeout(TIMEOUT)
        .build()?;
    let proxy = zbus::blocking::Proxy::new(&connection, DESTINATION, PATH, INTERFACE)?;
    // Listen before the read, so a change between the two is not lost.
    let changes = proxy.receive_signal("SettingChanged")?;
    let mut keys = Keys::default();
    let read: zbus::Result<HashMap<String, HashMap<String, OwnedValue>>> =
        proxy.call("ReadAll", &(NAMESPACES.as_slice(),));
    match read {
        Ok(namespaces) => {
            for (namespace, values) in &namespaces {
                for (key, value) in values {
                    keys.set(namespace, key, value);
                }
            }
        },
        // A portal that starts late after login can still send each change.
        Err(e) => debug!("The desktop portal gave no appearance yet: {}", e),
    }
    let mut shown = keys.appearance();
    on_change(shown);
    for message in changes {
        let Ok((namespace, key, value)) =
            message.body().deserialize::<(String, String, OwnedValue)>()
        else {
            continue;
        };
        keys.set(&namespace, &key, &value);
        let now = keys.appearance();
        if now != shown {
            shown = now;
            on_change(now);
        }
    }
    Ok(())
}

/// The older `Read` method, and some backends, wrap the value in one more
/// variant.
fn unwrap<'a>(value: &'a Value<'a>) -> &'a Value<'a> {
    match value {
        Value::Value(inner) => unwrap(inner),
        other => other,
    }
}

/// `color-scheme`: 1 prefers dark, 2 prefers light, and 0 has no preference.
fn scheme_of(value: &Value<'_>) -> Option<Scheme> {
    match number_of(value)? {
        1 => Some(Scheme::Dark),
        2 => Some(Scheme::Light),
        _ => Some(Scheme::NoPreference),
    }
}

/// `accent-color`: three sRGB numbers from 0 to 1. A number outside that
/// range means the desktop has no accent.
fn accent_of(value: &Value<'_>) -> Option<[u8; 3]> {
    let Value::Structure(fields) = value else {
        return None;
    };
    let channels: Vec<f64> = fields
        .fields()
        .iter()
        .filter_map(|field| match unwrap(field) {
            Value::F64(channel) => Some(*channel),
            _ => None,
        })
        .collect();
    let [r, g, b] = channels.as_slice() else {
        return None;
    };
    let byte = |channel: f64| {
        (0.0..=1.0)
            .contains(&channel)
            .then(|| (channel * 255.0).round() as u8)
    };
    Some([byte(*r)?, byte(*g)?, byte(*b)?])
}

fn number_of(value: &Value<'_>) -> Option<u32> {
    match value {
        Value::U32(number) => Some(*number),
        _ => None,
    }
}

fn flag_of(value: &Value<'_>) -> Option<bool> {
    match value {
        Value::Bool(flag) => Some(*flag),
        _ => None,
    }
}

/// KDE's portal gives kdeglobals values as text.
fn factor_of(value: &Value<'_>) -> Option<f64> {
    match value {
        Value::F64(factor) => Some(*factor),
        Value::Str(text) => text.as_str().trim().parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::Structure;

    fn accent(r: f64, g: f64, b: f64) -> Value<'static> {
        Value::Structure(Structure::from((r, g, b)))
    }

    #[test]
    fn the_portal_number_names_the_scheme() {
        assert_eq!(scheme_of(&Value::U32(1)), Some(Scheme::Dark));
        assert_eq!(scheme_of(&Value::U32(2)), Some(Scheme::Light));
        assert_eq!(scheme_of(&Value::U32(0)), Some(Scheme::NoPreference));
        assert_eq!(scheme_of(&Value::U32(7)), Some(Scheme::NoPreference));
        assert_eq!(scheme_of(&Value::from("dark")), None);
        let mut keys = Keys::default();
        keys.set(
            APPEARANCE,
            "color-scheme",
            &Value::Value(Box::new(Value::U32(1))),
        );
        assert_eq!(
            keys.appearance().scheme,
            Scheme::Dark,
            "the reply of the older Read method"
        );
    }

    #[test]
    fn the_accent_is_three_channels_from_zero_to_one() {
        // KDE Breeze's blue, as this desktop's portal gives it
        assert_eq!(
            accent_of(&accent(0.239_215_686, 0.682_352_96, 0.913_725_495)),
            Some([61, 174, 233])
        );
        assert_eq!(accent_of(&accent(1.5, 0.0, 0.0)), None, "no accent");
        assert_eq!(accent_of(&accent(-1.0, -1.0, -1.0)), None, "no accent");
        assert_eq!(accent_of(&Value::U32(1)), None);
    }

    #[test]
    fn the_freedesktop_keys_win_over_the_desktop_keys() {
        let mut keys = Keys::default();
        keys.set(GNOME_INTERFACE, "enable-animations", &Value::Bool(false));
        keys.set(GNOME_A11Y, "high-contrast", &Value::Bool(true));
        assert!(keys.appearance().reduced_motion);
        assert!(keys.appearance().high_contrast);

        keys.set(APPEARANCE, "reduced-motion", &Value::U32(0));
        keys.set(APPEARANCE, "contrast", &Value::U32(0));
        assert!(!keys.appearance().reduced_motion);
        assert!(!keys.appearance().high_contrast);

        keys.set(APPEARANCE, "reduced-motion", &Value::U32(1));
        keys.set(APPEARANCE, "contrast", &Value::U32(1));
        assert!(keys.appearance().reduced_motion);
        assert!(keys.appearance().high_contrast);
    }

    #[test]
    fn kde_stops_animations_with_a_zero_factor() {
        let mut keys = Keys::default();
        // KDE also answers GNOME's key, with its default.
        keys.set(GNOME_INTERFACE, "enable-animations", &Value::Bool(true));
        keys.set(
            KDE,
            "AnimationDurationFactor",
            &Value::from("0.08838834764831843"),
        );
        assert!(!keys.appearance().reduced_motion);
        keys.set(KDE, "AnimationDurationFactor", &Value::from("0"));
        assert!(keys.appearance().reduced_motion);
        keys.set(KDE, "AnimationDurationFactor", &Value::F64(1.0));
        assert!(!keys.appearance().reduced_motion);
    }

    #[test]
    fn other_keys_change_nothing() {
        let mut keys = Keys::default();
        keys.set(GNOME_INTERFACE, "cursor-size", &Value::I32(24));
        keys.set("org.example", "color-scheme", &Value::U32(1));
        assert_eq!(keys.appearance(), Appearance::default());
    }
}
