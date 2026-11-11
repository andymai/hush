//! Whether Hush uses the portals or evdev and uinput.

use crate::config::settings::InputBackend;
use std::path::Path;

/// How Hush reads the hotkeys and types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// evdev for the hotkeys, uinput for typing.
    Native,
    /// The RemoteDesktop, GlobalShortcuts, and Background portals.
    Portal,
}

impl InputMode {
    /// `auto` means portals in a sandbox and evdev elsewhere. An explicit
    /// choice wins, so a tester can run portal mode outside Flatpak, and a
    /// sandbox that gives device access can keep evdev.
    pub fn choose(backend: InputBackend, sandboxed: bool) -> Self {
        match backend {
            InputBackend::Portal => Self::Portal,
            InputBackend::Evdev => Self::Native,
            InputBackend::Auto if sandboxed => Self::Portal,
            InputBackend::Auto => Self::Native,
        }
    }

    pub fn detect(backend: InputBackend) -> Self {
        Self::choose(backend, sandboxed())
    }

    /// The mode of the user's configuration. A configuration that does
    /// not load counts as `auto`.
    pub fn current() -> Self {
        let backend = crate::Config::load()
            .map(|config| config.input.backend)
            .unwrap_or_default();
        Self::detect(backend)
    }

    pub fn is_portal(self) -> bool {
        self == Self::Portal
    }

    /// For logs and `hush doctor`.
    pub fn describe(self, backend: InputBackend, sandboxed: bool) -> String {
        match (self, backend) {
            (Self::Portal, InputBackend::Portal) => {
                "portals (input.backend = \"portal\")".to_string()
            },
            (Self::Portal, _) => "portals (Flatpak sandbox)".to_string(),
            (Self::Native, InputBackend::Evdev) if sandboxed => {
                "evdev and uinput (input.backend = \"evdev\" in a sandbox)".to_string()
            },
            (Self::Native, _) => "evdev and uinput".to_string(),
        }
    }
}

/// Whether Hush runs in a Flatpak sandbox. Only the file that Flatpak puts
/// at the root of each sandbox counts: `FLATPAK_ID` can reach a host
/// process, such as a shell that a Flatpak editor starts on the host.
pub fn sandboxed() -> bool {
    sandboxed_at(Path::new("/.flatpak-info"))
}

fn sandboxed_at(info_file: &Path) -> bool {
    info_file.exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_uses_portals_only_in_a_sandbox() {
        assert_eq!(
            InputMode::choose(InputBackend::Auto, true),
            InputMode::Portal
        );
        assert_eq!(
            InputMode::choose(InputBackend::Auto, false),
            InputMode::Native
        );
    }

    #[test]
    fn an_explicit_backend_wins() {
        for sandboxed in [true, false] {
            assert_eq!(
                InputMode::choose(InputBackend::Portal, sandboxed),
                InputMode::Portal,
                "a tester runs portal mode outside Flatpak"
            );
            assert_eq!(
                InputMode::choose(InputBackend::Evdev, sandboxed),
                InputMode::Native,
                "a sandbox with device access keeps evdev"
            );
        }
    }

    #[test]
    fn only_the_info_file_shows_the_sandbox() {
        let root = tempfile::tempdir().unwrap();
        let info = root.path().join(".flatpak-info");
        assert!(!sandboxed_at(&info));
        std::fs::write(&info, "[Application]\nname=io.github.andymai.hush\n").unwrap();
        assert!(sandboxed_at(&info));
    }

    #[test]
    fn the_description_says_why() {
        assert_eq!(
            InputMode::Portal.describe(InputBackend::Auto, true),
            "portals (Flatpak sandbox)"
        );
        assert_eq!(
            InputMode::Portal.describe(InputBackend::Portal, false),
            "portals (input.backend = \"portal\")"
        );
        assert_eq!(
            InputMode::Native.describe(InputBackend::Auto, false),
            "evdev and uinput"
        );
    }

    #[test]
    fn the_config_takes_the_backend_and_rejects_others() {
        let config = crate::Config::from_user_table(
            toml::from_str("[input]\nbackend = \"portal\"").unwrap(),
        )
        .unwrap();
        assert_eq!(config.input.backend, InputBackend::Portal);
        assert_eq!(
            crate::Config::defaults().unwrap().input.backend,
            InputBackend::Auto
        );
        let error = crate::Config::from_user_table(
            toml::from_str("[input]\nbackend = \"uinput\"").unwrap(),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("unknown variant `uinput`"), "{error}");
        assert!(error.contains("portal"), "{error}");
    }
}
