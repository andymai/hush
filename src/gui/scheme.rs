//! The desktop's light or dark preference, accent, and interface font for
//! the window, from [`crate::portal::appearance`]. The window changes with
//! the desktop.

use super::desktop_font::{self, Faces};
use crate::portal::appearance::{self, DesktopFont, Scheme};
use egui::{Context, Theme};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use tracing::info;

const UNKNOWN: u8 = 0;
const LIGHT: u8 = 1;
const DARK: u8 = 2;

/// The accent and the faces that the window takes from the desktop.
#[derive(Debug, Clone, Default)]
pub struct DesktopStyle {
    pub accent: Option<[u8; 3]>,
    /// `None` keeps Geist: the desktop names no font, or no installed font
    /// has its family.
    pub faces: Option<Faces>,
}

/// The latest preference, written by the listening thread.
#[derive(Clone, Default)]
pub struct DesktopScheme {
    latest: Arc<AtomicU8>,
    /// A style that the window has not taken yet
    style: Arc<Mutex<Option<DesktopStyle>>>,
}

impl DesktopScheme {
    /// Read the preference, then repaint `ctx` each time it changes. The
    /// listening thread loads a new font, so the window never waits for it.
    pub fn watch(ctx: &Context) -> Self {
        let scheme = Self::default();
        let latest = Arc::clone(&scheme.latest);
        let pending = Arc::clone(&scheme.style);
        let ctx = ctx.clone();
        let mut seen: Option<(Option<[u8; 3]>, Option<DesktopFont>)> = None;
        let mut faces: Option<Faces> = None;
        appearance::watch(move |appearance| {
            store(&latest, &ctx, appearance.scheme);
            let wanted = (appearance.accent, appearance.font.clone());
            if seen.as_ref() == Some(&wanted) {
                return;
            }
            if seen.as_ref().map(|(_, font)| font) != Some(&wanted.1) {
                faces = wanted.1.as_ref().and_then(desktop_font::load);
                if let Some(loaded) = &faces {
                    info!(
                        "The settings window uses the desktop's font, {}",
                        loaded.family
                    );
                }
            }
            *pending.lock() = Some(DesktopStyle {
                accent: wanted.0,
                faces: faces.clone(),
            });
            seen = Some(wanted);
            ctx.request_repaint();
        });
        scheme
    }

    /// The desktop's accent and font, once after each change.
    pub fn take_style(&self) -> Option<DesktopStyle> {
        self.style.lock().take()
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
