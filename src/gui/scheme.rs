//! The desktop's light or dark preference, read from the XDG desktop portal
//! (`org.freedesktop.appearance` `color-scheme`), which GNOME, KDE, and the
//! wlroots portals answer. A thread keeps listening, so the window changes
//! with the desktop.

use egui::{Context, Theme};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;
use zbus::zvariant::{OwnedValue, Value};

const DESTINATION: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";
const INTERFACE: &str = "org.freedesktop.portal.Settings";
const NAMESPACE: &str = "org.freedesktop.appearance";
const KEY: &str = "color-scheme";
const TIMEOUT: Duration = Duration::from_secs(2);

const UNKNOWN: u8 = 0;
const LIGHT: u8 = 1;
const DARK: u8 = 2;

/// The latest preference, written by the listening thread.
#[derive(Clone, Default)]
pub struct DesktopScheme {
    latest: Arc<AtomicU8>,
}

impl DesktopScheme {
    /// Read the preference, then repaint `ctx` each time it changes.
    pub fn watch(ctx: &Context) -> Self {
        let scheme = Self::default();
        let latest = Arc::clone(&scheme.latest);
        let ctx = ctx.clone();
        // zbus blocks on a runtime of its own, so it gets a thread of its own.
        let spawned = std::thread::Builder::new()
            .name("hush-color-scheme".into())
            .spawn(move || {
                if let Err(e) = listen(&latest, &ctx) {
                    debug!("No colour scheme from the desktop portal: {}", e);
                }
            });
        if let Err(e) = spawned {
            debug!("Could not watch the colour scheme: {}", e);
        }
        scheme
    }

    /// `None` until the portal answers, and when the desktop has no
    /// preference.
    pub fn get(&self) -> Option<Theme> {
        match self.latest.load(Ordering::Acquire) {
            LIGHT => Some(Theme::Light),
            DARK => Some(Theme::Dark),
            _ => None,
        }
    }
}

fn listen(latest: &AtomicU8, ctx: &Context) -> zbus::Result<()> {
    let connection = zbus::blocking::connection::Builder::session()?
        .method_timeout(TIMEOUT)
        .build()?;
    let proxy = zbus::blocking::Proxy::new(&connection, DESTINATION, PATH, INTERFACE)?;
    // Listen before the read, so a change between the two is not lost.
    let changes = proxy.receive_signal("SettingChanged")?;
    let value: OwnedValue = proxy
        .call("ReadOne", &(NAMESPACE, KEY))
        .or_else(|_| proxy.call("Read", &(NAMESPACE, KEY)))?;
    store(latest, ctx, &value);
    for message in changes {
        let Ok((namespace, key, value)) =
            message.body().deserialize::<(String, String, OwnedValue)>()
        else {
            continue;
        };
        if namespace == NAMESPACE && key == KEY {
            store(latest, ctx, &value);
        }
    }
    Ok(())
}

fn store(latest: &AtomicU8, ctx: &Context, value: &Value<'_>) {
    let code = match scheme_of(value) {
        Some(Theme::Light) => LIGHT,
        Some(Theme::Dark) => DARK,
        None => UNKNOWN,
    };
    if latest.swap(code, Ordering::AcqRel) != code {
        ctx.request_repaint();
    }
}

/// The portal's `color-scheme`: 1 prefers dark, 2 prefers light, and 0 has
/// no preference. The older `Read` method wraps the number in one more
/// variant.
fn scheme_of(value: &Value<'_>) -> Option<Theme> {
    match value {
        Value::Value(inner) => scheme_of(inner),
        Value::U32(1) => Some(Theme::Dark),
        Value::U32(2) => Some(Theme::Light),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_portal_number_names_the_scheme() {
        assert_eq!(scheme_of(&Value::U32(1)), Some(Theme::Dark));
        assert_eq!(scheme_of(&Value::U32(2)), Some(Theme::Light));
        assert_eq!(scheme_of(&Value::U32(0)), None, "no preference");
        assert_eq!(scheme_of(&Value::U32(7)), None);
        assert_eq!(scheme_of(&Value::from("dark")), None);
        assert_eq!(
            scheme_of(&Value::Value(Box::new(Value::U32(1)))),
            Some(Theme::Dark),
            "the reply of the older Read method"
        );
    }

    #[test]
    fn a_change_repaints_only_when_the_scheme_differs() {
        let ctx = Context::default();
        let latest = AtomicU8::new(UNKNOWN);
        let scheme = DesktopScheme {
            latest: Arc::new(AtomicU8::new(UNKNOWN)),
        };
        assert_eq!(scheme.get(), None);
        store(&latest, &ctx, &Value::U32(1));
        assert_eq!(latest.load(Ordering::Acquire), DARK);
        store(&latest, &ctx, &Value::U32(0));
        assert_eq!(latest.load(Ordering::Acquire), UNKNOWN);
        scheme.latest.store(LIGHT, Ordering::Release);
        assert_eq!(scheme.get(), Some(Theme::Light));
    }
}
