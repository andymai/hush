//! The application and autostart entries, so the launcher, the installer, and
//! the settings window all agree on one file name and one set of actions.

use crate::service::ServiceState;
use std::ffi::OsStr;
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

/// Whether Hush starts at login: in portal mode, the last answer of the
/// Background portal; else the entry file or the enabled unit.
pub fn starts_at_login() -> bool {
    starts_at_login_with(ServiceState::probe)
}

/// [`starts_at_login`], with the unit's state from `service` when the
/// answer needs it.
pub fn starts_at_login_with(service: impl FnOnce() -> ServiceState) -> bool {
    #[cfg(target_os = "linux")]
    if crate::portal::InputMode::current().is_portal() {
        return crate::portal::background::autostart_on();
    }
    autostart_installed()
        || std::env::current_exe().is_ok_and(|exe| service().starts_at_login(&exe))
}

/// The binary that a launcher or autostart entry runs. An AppImage's binary
/// lives in a mount that ends with the process, so the entry names the
/// AppImage file. A Nix store path can vanish in a garbage collection, so
/// the entry names the profile link to it.
pub fn entry_binary() -> std::io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    Ok(entry_binary_for(
        &exe,
        std::env::var_os("APPIMAGE").as_deref(),
        std::env::var_os("APPDIR").as_deref(),
        &nix_profile_links(),
    ))
}

fn entry_binary_for(
    exe: &Path,
    appimage: Option<&OsStr>,
    appdir: Option<&OsStr>,
    links: &[PathBuf],
) -> PathBuf {
    // A process that another AppImage started inherits APPIMAGE, so the
    // binary must also run from inside this AppImage's mount.
    if let (Some(appimage), Some(appdir)) = (appimage, appdir) {
        if !appimage.is_empty() && !appdir.is_empty() && exe.starts_with(appdir) {
            return PathBuf::from(appimage);
        }
    }
    if exe.starts_with("/nix/store") {
        if let Some(link) = stable_link(exe, links) {
            return link;
        }
    }
    exe.to_path_buf()
}

/// The first of `links` that resolves to `exe`.
fn stable_link(exe: &Path, links: &[PathBuf]) -> Option<PathBuf> {
    links
        .iter()
        .find(|link| std::fs::canonicalize(link).is_ok_and(|target| target == exe))
        .cloned()
}

fn nix_profile_links() -> Vec<PathBuf> {
    let mut links = Vec::new();
    if let Some(home) = dirs::home_dir() {
        links.push(home.join(".nix-profile/bin/hush"));
        links.push(home.join(".local/state/nix/profile/bin/hush"));
    }
    if let Ok(user) = std::env::var("USER") {
        links.push(PathBuf::from(format!(
            "/etc/profiles/per-user/{}/bin/hush",
            user
        )));
    }
    links.push(PathBuf::from("/run/current-system/sw/bin/hush"));
    links
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
    fn entries_name_a_binary_that_outlives_the_process() {
        let none: &[PathBuf] = &[];
        let mounted = Path::new("/tmp/.mount_hushAbc/usr/bin/hush");
        assert_eq!(
            entry_binary_for(
                mounted,
                Some(OsStr::new("/home/me/Apps/Hush.AppImage")),
                Some(OsStr::new("/tmp/.mount_hushAbc")),
                none
            ),
            PathBuf::from("/home/me/Apps/Hush.AppImage")
        );
        assert_eq!(
            entry_binary_for(
                Path::new("/usr/bin/hush"),
                Some(OsStr::new("/home/me/Apps/Other.AppImage")),
                Some(OsStr::new("/tmp/.mount_otherXyz")),
                none
            ),
            PathBuf::from("/usr/bin/hush"),
            "APPIMAGE inherited from another AppImage"
        );
        assert_eq!(
            entry_binary_for(Path::new("/home/me/.local/bin/hush"), None, None, none),
            PathBuf::from("/home/me/.local/bin/hush")
        );

        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("store-hush");
        std::fs::write(&target, "").unwrap();
        let target = std::fs::canonicalize(&target).unwrap();
        let link = dir.path().join("profile-hush");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let elsewhere = dir.path().join("missing");
        assert_eq!(stable_link(&target, &[elsewhere, link.clone()]), Some(link));
        assert_eq!(stable_link(Path::new("/nowhere/hush"), &[]), None);
    }

    #[test]
    fn entries_live_beside_their_kind() {
        assert!(entry_path().ends_with(format!("applications/{}", ENTRY_FILE)));
        assert!(autostart_path().ends_with(format!("autostart/{}", ENTRY_FILE)));
    }
}
