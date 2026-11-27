//! The desktop's light or dark preference for the window, from
//! [`crate::portal::appearance`]. The window changes with the desktop.

use crate::portal::appearance::{self, Scheme};
use egui::{Context, Theme};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

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
        appearance::watch(move |appearance| store(&latest, &ctx, appearance.scheme));
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

fn store(latest: &AtomicU8, ctx: &Context, scheme: Scheme) {
    let code = match scheme {
        Scheme::Light => LIGHT,
        Scheme::Dark => DARK,
        Scheme::NoPreference => UNKNOWN,
    };
    if latest.swap(code, Ordering::AcqRel) != code {
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A context with no repaint waiting, as after its first frames.
    fn settled() -> Context {
        let ctx = Context::default();
        for _ in 0..10 {
            if !ctx.has_requested_repaint() {
                break;
            }
            ctx.run_ui(egui::RawInput::default(), |_| {})
                .drop_without_applying_deltas();
        }
        assert!(!ctx.has_requested_repaint());
        ctx
    }

    #[test]
    fn a_change_repaints_only_when_the_scheme_differs() {
        let scheme = DesktopScheme::default();
        assert_eq!(scheme.get(), None);

        let ctx = settled();
        store(&scheme.latest, &ctx, Scheme::Dark);
        assert!(ctx.has_requested_repaint(), "no preference to dark");
        assert_eq!(scheme.get(), Some(Theme::Dark));

        let ctx = settled();
        store(&scheme.latest, &ctx, Scheme::Dark);
        assert!(!ctx.has_requested_repaint(), "dark again");
        assert_eq!(scheme.get(), Some(Theme::Dark));

        store(&scheme.latest, &ctx, Scheme::Light);
        assert!(ctx.has_requested_repaint(), "dark to light");
        assert_eq!(scheme.get(), Some(Theme::Light));

        let ctx = settled();
        store(&scheme.latest, &ctx, Scheme::NoPreference);
        assert!(ctx.has_requested_repaint(), "light to no preference");
        assert_eq!(scheme.get(), None);
    }
}
