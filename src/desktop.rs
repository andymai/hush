//! The application and autostart entries, so the launcher, the installer, and
//! the settings window all agree on one file name and one set of actions.

use std::path::{Path, PathBuf};

pub const APP_ID: &str = "io.github.andymai.hush";
pub const ENTRY_FILE: &str = "io.github.andymai.hush.desktop";
/// Written by versions before the entry took the application id as its name.
pub const LEGACY_ENTRY_FILE: &str = "hush.desktop";

const ENTRY_TEMPLATE: &str = include_str!("../packaging/io.github.andymai.hush.desktop");

/// The launcher entry, pointing at this binary.
pub fn application_entry(binary: &Path) -> String {
    ENTRY_TEMPLATE.replace("Exec=hush", &format!("Exec={}", binary.display()))
}

/// The login entry, which starts the daemon rather than the window.
pub fn autostart_entry(binary: &Path) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Hush\n\
         Comment=Local voice-to-text: hold a key, speak, and the text appears at your cursor\n\
         Exec={} daemon start --foreground\n\
         Icon={}\n\
         Terminal=false\n\
         Categories=Utility;Accessibility;\n\
         X-GNOME-Autostart-enabled=true\n",
        binary.display(),
        APP_ID
    )
}

pub fn applications_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("applications")
}

pub fn autostart_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("autostart")
}

pub fn entry_path() -> PathBuf {
    applications_dir().join(ENTRY_FILE)
}

pub fn autostart_path() -> PathBuf {
    autostart_dir().join(ENTRY_FILE)
}

/// Both names count, so an entry written by an older version still reads as on.
pub fn autostart_installed() -> bool {
    let dir = autostart_dir();
    dir.join(ENTRY_FILE).exists() || dir.join(LEGACY_ENTRY_FILE).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_launcher_entry_points_at_this_binary() {
        let entry = application_entry(Path::new("/opt/hush/bin/hush"));
        assert!(entry.contains("Exec=/opt/hush/bin/hush settings"));
        assert!(entry.contains("Exec=/opt/hush/bin/hush toggle"));
        assert!(!entry.contains("Exec=hush"));
        assert!(entry.contains(&format!("Icon={}", APP_ID)));
    }

    #[test]
    fn the_autostart_entry_starts_the_daemon() {
        let entry = autostart_entry(Path::new("/usr/bin/hush"));
        assert!(entry.contains("Exec=/usr/bin/hush daemon start --foreground"));
        assert!(entry.contains("X-GNOME-Autostart-enabled=true"));
    }

    #[test]
    fn entries_live_beside_their_kind() {
        assert!(entry_path().ends_with(format!("applications/{}", ENTRY_FILE)));
        assert!(autostart_path().ends_with(format!("autostart/{}", ENTRY_FILE)));
    }
}
