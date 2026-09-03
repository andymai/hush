//! Device access for hotkeys (`/dev/input`) and text insertion (`/dev/uinput`).
//!
//! A udev rule tags both kinds of node with `uaccess`, so systemd-logind grants
//! the physically seated user an ACL on them for the length of the session.
//! No group change, no logout, and nothing beyond that one rule runs as root.

use anyhow::{anyhow, Context, Result};
use std::path::Path;
use std::process::Command;

pub const UDEV_RULE_PATH: &str = "/etc/udev/rules.d/70-hush.rules";
pub const MODULES_LOAD_PATH: &str = "/etc/modules-load.d/hush-uinput.conf";

pub const UDEV_RULE: &str = r#"# Installed by `hush setup permissions`.
# Lets the logged-in user read keyboards (hotkeys) and write uinput (text insertion).
KERNEL=="uinput", SUBSYSTEM=="misc", OPTIONS+="static_node=uinput", TAG+="uaccess"
SUBSYSTEM=="input", KERNEL=="event*", TAG+="uaccess"
"#;

/// The commands that need root: write the rule, load uinput now and at boot,
/// and re-run the rules so the ACLs apply without a logout.
pub fn install_script() -> String {
    format!(
        "set -e\n\
         cat > {rule_path} <<'HUSH_UDEV_RULE'\n{rule}HUSH_UDEV_RULE\n\
         echo uinput > {modules_path}\n\
         modprobe uinput\n\
         udevadm control --reload\n\
         udevadm trigger --subsystem-match=input --subsystem-match=misc --action=change\n",
        rule_path = UDEV_RULE_PATH,
        rule = UDEV_RULE,
        modules_path = MODULES_LOAD_PATH,
    )
}

/// What the current process can reach right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionStatus {
    pub in_container: bool,
    pub rule_installed: bool,
    pub uinput_present: bool,
    pub uinput_writable: bool,
    pub event_nodes: usize,
    pub readable_event_nodes: usize,
    pub in_input_group: bool,
}

impl PermissionStatus {
    /// Both hotkeys and text insertion will work.
    pub fn ready(&self) -> bool {
        self.uinput_writable && self.readable_event_nodes > 0
    }
}

pub fn status() -> PermissionStatus {
    let (event_nodes, readable_event_nodes) = count_event_nodes();
    PermissionStatus {
        in_container: in_container(),
        rule_installed: Path::new(UDEV_RULE_PATH).exists(),
        uinput_present: Path::new("/dev/uinput").exists(),
        uinput_writable: std::fs::OpenOptions::new()
            .write(true)
            .open("/dev/uinput")
            .is_ok(),
        event_nodes,
        readable_event_nodes,
        in_input_group: in_input_group(),
    }
}

/// Toolbox, Distrobox, Podman, and Docker all mark their root filesystem.
/// Inside one, `/etc` belongs to the container, so the rule must be installed
/// on the host instead.
pub fn in_container() -> bool {
    ["/run/.containerenv", "/run/.toolboxenv", "/.dockerenv"]
        .iter()
        .any(|marker| Path::new(marker).exists())
}

fn count_event_nodes() -> (usize, usize) {
    let mut total = 0;
    let mut readable = 0;
    if let Ok(entries) = std::fs::read_dir("/dev/input") {
        for entry in entries.flatten() {
            if !entry.file_name().to_string_lossy().starts_with("event") {
                continue;
            }
            total += 1;
            if std::fs::File::open(entry.path()).is_ok() {
                readable += 1;
            }
        }
    }
    (total, readable)
}

fn in_input_group() -> bool {
    Command::new("id")
        .arg("-Gn")
        .output()
        .ok()
        .map(|out| {
            String::from_utf8_lossy(&out.stdout)
                .split_whitespace()
                .any(|group| group == "input")
        })
        .unwrap_or(false)
}

/// How to obtain root for the install script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elevation {
    /// polkit prompt; graphical dialog on a desktop, text agent elsewhere
    Pkexec,
    Sudo,
}

pub fn preferred_elevation() -> Option<Elevation> {
    let on_desktop =
        std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some();
    if on_desktop && command_exists("pkexec") {
        Some(Elevation::Pkexec)
    } else if command_exists("sudo") {
        Some(Elevation::Sudo)
    } else if command_exists("pkexec") {
        Some(Elevation::Pkexec)
    } else {
        None
    }
}

fn command_exists(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).any(|dir| dir.join(name).is_file()))
        .unwrap_or(false)
}

/// Run the install script as root through the given method.
pub fn install(elevation: Elevation) -> Result<()> {
    let program = match elevation {
        Elevation::Pkexec => "pkexec",
        Elevation::Sudo => "sudo",
    };
    let status = Command::new(program)
        .args(["sh", "-c", &install_script()])
        .status()
        .with_context(|| format!("Failed to run {}", program))?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!(
            "{} exited with {}; the rule was not installed",
            program,
            status
        ))
    }
}

/// Print the status table used by `hush setup permissions --check`.
pub fn print_status(status: &PermissionStatus) {
    let mark = |ok: bool| if ok { "✅" } else { "❌" };
    println!("🔎 Device access:");
    println!(
        "   {} udev rule {}",
        mark(status.rule_installed),
        UDEV_RULE_PATH
    );
    println!(
        "   {} /dev/uinput {}",
        mark(status.uinput_writable),
        if status.uinput_writable {
            "writable"
        } else if status.uinput_present {
            "present, not writable"
        } else {
            "missing (uinput module not loaded)"
        }
    );
    println!(
        "   {} /dev/input: {} of {} event devices readable",
        mark(status.readable_event_nodes > 0),
        status.readable_event_nodes,
        status.event_nodes
    );
    if status.in_input_group {
        println!("   ℹ️  user is in the input group");
    }
    if status.in_container {
        println!("   ℹ️  running inside a container");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_writes_rule_and_reloads_udev() {
        let script = install_script();
        assert!(script.contains(UDEV_RULE_PATH));
        assert!(script.contains(MODULES_LOAD_PATH));
        assert!(script.contains("TAG+=\"uaccess\""));
        assert!(script.contains("udevadm control --reload"));
        assert!(script.contains("udevadm trigger"));
        assert!(script.starts_with("set -e\n"));
    }

    #[test]
    fn rule_covers_both_device_kinds() {
        assert!(UDEV_RULE.contains("KERNEL==\"uinput\""));
        assert!(UDEV_RULE.contains("KERNEL==\"event*\""));
        assert_eq!(UDEV_RULE.matches("uaccess").count(), 2);
    }

    #[test]
    fn status_is_consistent() {
        let status = status();
        assert!(status.readable_event_nodes <= status.event_nodes);
        if status.uinput_writable {
            assert!(status.uinput_present);
        }
    }
}
