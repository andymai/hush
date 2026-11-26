//! The `hush.service` user unit that the .deb, the .rpm, the AUR package,
//! and the Nix flake ship. When the unit runs this binary and a graphical
//! session runs under systemd, `hush daemon start` and `restart` and the
//! autostart switch go through the unit, so systemd owns the daemon.
//! Otherwise Hush starts the daemon itself, as on a compositor started from a
//! TTY, where the user manager lacks the display variables.

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const UNIT: &str = "hush.service";

/// What systemd says about the unit and the session. All false when
/// systemctl is missing, as in the Flatpak sandbox.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServiceState {
    /// systemd found the unit file
    pub installed: bool,
    pub enabled: bool,
    /// The PID of the unit's process while it runs
    pub main_pid: Option<u32>,
    /// The binary in the unit's `ExecStart`
    pub exec_path: Option<PathBuf>,
    /// `graphical-session.target` is active
    pub graphical_session: bool,
}

impl ServiceState {
    pub fn probe() -> Self {
        #[cfg(target_os = "linux")]
        if crate::portal::sandboxed() {
            return Self::default();
        }
        let show = systemctl(&[
            "show",
            UNIT,
            "-p",
            "LoadState",
            "-p",
            "UnitFileState",
            "-p",
            "MainPID",
            "-p",
            "ExecStart",
        ]);
        let mut state = show.map(|text| parse_show(&text)).unwrap_or_default();
        state.graphical_session =
            systemctl(&["is-active", "--quiet", "graphical-session.target"]).is_some();
        state
    }

    /// The unit runs `exe`, and the session has the display variables that
    /// the daemon needs, so the unit should own the daemon.
    pub fn applies_to(&self, exe: &Path) -> bool {
        self.installed && self.graphical_session && self.runs(exe)
    }

    /// The unit's `ExecStart` is `exe`. A unit left by another install, or a
    /// build run from a source tree beside a package, does not count.
    pub fn runs(&self, exe: &Path) -> bool {
        self.exec_path
            .as_deref()
            .is_some_and(|unit| same_file(unit, exe))
    }

    /// The unit starts this binary at login.
    pub fn starts_at_login(&self, exe: &Path) -> bool {
        self.installed && self.enabled && self.runs(exe)
    }
}

/// How `hush daemon restart` brings the daemon back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartRoute {
    /// The unit runs the daemon: `systemctl --user restart`.
    RestartUnit,
    /// Stop the daemon, if one runs outside the unit, then start the unit.
    /// This also brings back a daemon that an older restart moved out of it.
    StopThenStartUnit,
    /// Stop the daemon, then start it detached.
    StopThenDetached,
}

/// `daemon_pid` is the PID of the running daemon, `default_options` whether
/// the restart keeps the options that the unit's `ExecStart` uses.
pub fn restart_route(
    state: &ServiceState,
    exe: &Path,
    daemon_pid: Option<u32>,
    default_options: bool,
) -> RestartRoute {
    if daemon_pid.is_some() && daemon_pid == state.main_pid {
        RestartRoute::RestartUnit
    } else if default_options && state.applies_to(exe) {
        RestartRoute::StopThenStartUnit
    } else {
        RestartRoute::StopThenDetached
    }
}

/// The unit's process runs this code: its cgroup is the unit's.
pub fn in_unit() -> bool {
    std::fs::read_to_string("/proc/self/cgroup").is_ok_and(|text| cgroup_is_unit(&text))
}

fn cgroup_is_unit(cgroup: &str) -> bool {
    let unit = format!("/{}", UNIT);
    cgroup.lines().any(|line| {
        let path = line.rsplit(':').next().unwrap_or_default();
        path.ends_with(&unit) || path.contains(&format!("{}/", unit))
    })
}

pub fn start() -> Result<()> {
    run(&["start", UNIT])
}

pub fn restart() -> Result<()> {
    run(&["restart", UNIT])
}

pub fn enable() -> Result<()> {
    run(&["enable", UNIT])
}

pub fn disable() -> Result<()> {
    run(&["disable", UNIT])
}

fn run(args: &[&str]) -> Result<()> {
    let output = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
        .with_context(|| format!("Failed to run systemctl --user {}", args.join(" ")))?;
    if output.status.success() {
        return Ok(());
    }
    Err(anyhow!(
        "systemctl --user {} failed: {}. See: journalctl --user -u {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr).trim(),
        UNIT
    ))
}

/// The output of a successful `systemctl --user` call, `None` on a failure
/// or when systemctl is missing.
fn systemctl(args: &[&str]) -> Option<String> {
    let output = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Reads `systemctl show` lines such as `LoadState=loaded`, and the
/// `ExecStart={ path=/usr/bin/hush ; argv[]=... }` form.
fn parse_show(text: &str) -> ServiceState {
    let mut state = ServiceState::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "LoadState" => state.installed = value == "loaded",
            "UnitFileState" => {
                state.enabled = matches!(value, "enabled" | "enabled-runtime" | "linked")
            },
            "MainPID" => state.main_pid = value.parse().ok().filter(|pid| *pid != 0),
            "ExecStart" => {
                state.exec_path = value
                    .split_once("path=")
                    .and_then(|(_, rest)| rest.split(" ;").next())
                    .map(|path| PathBuf::from(path.trim()))
                    .filter(|path| !path.as_os_str().is_empty());
            },
            _ => {},
        }
    }
    state
}

/// After a package upgrade, `/proc/self/exe` names the old file with
/// " (deleted)" appended; the path still names the binary of the unit.
fn same_file(unit: &Path, exe: &Path) -> bool {
    let exe = exe
        .to_str()
        .and_then(|text| text.strip_suffix(" (deleted)"))
        .map_or_else(|| exe.to_path_buf(), PathBuf::from);
    let canonical =
        |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    unit == exe || canonical(unit) == canonical(&exe)
}

/// A compositor that runs no autostart entries when it starts without
/// systemd's graphical session, such as Sway or Hyprland from a TTY.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compositor {
    Sway,
    Hyprland,
    Niri,
}

impl Compositor {
    /// From the sockets that each compositor exports to its clients.
    pub fn detect(var: impl Fn(&str) -> Option<String>) -> Option<Self> {
        if var("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
            Some(Compositor::Hyprland)
        } else if var("NIRI_SOCKET").is_some() {
            Some(Compositor::Niri)
        } else if var("SWAYSOCK").is_some() {
            Some(Compositor::Sway)
        } else {
            None
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Compositor::Sway => "Sway",
            Compositor::Hyprland => "Hyprland",
            Compositor::Niri => "niri",
        }
    }

    /// The config line that starts the daemon with the compositor.
    pub fn startup_line(self) -> &'static str {
        match self {
            Compositor::Sway => "exec hush daemon start",
            Compositor::Hyprland => "exec-once = hush daemon start",
            Compositor::Niri => "spawn-at-startup \"hush\" \"daemon\" \"start\"",
        }
    }
}

/// The compositor whose config must start Hush, because nothing else will:
/// no graphical session runs under systemd, so neither the unit nor an
/// autostart entry starts at login.
pub fn compositor_without_autostart(state: &ServiceState) -> Option<Compositor> {
    if state.graphical_session {
        return None;
    }
    Compositor::detect(|name| std::env::var(name).ok().filter(|value| !value.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHOW: &str = "LoadState=loaded\n\
        UnitFileState=enabled\n\
        MainPID=4242\n\
        ExecStart={ path=/usr/bin/hush ; argv[]=/usr/bin/hush daemon start --foreground ; \
        ignore_errors=no ; start_time=[n/a] ; stop_time=[n/a] ; pid=0 ; code=(null) ; status=0/0 }\n";

    fn session(show: &str) -> ServiceState {
        ServiceState {
            graphical_session: true,
            ..parse_show(show)
        }
    }

    #[test]
    fn show_output_gives_the_unit_state() {
        assert_eq!(
            parse_show(SHOW),
            ServiceState {
                installed: true,
                enabled: true,
                main_pid: Some(4242),
                exec_path: Some(PathBuf::from("/usr/bin/hush")),
                graphical_session: false,
            }
        );
        let missing = parse_show("LoadState=not-found\nUnitFileState=\nMainPID=0\nExecStart=\n");
        assert_eq!(missing, ServiceState::default());
    }

    #[test]
    fn the_unit_applies_only_to_its_own_binary_in_a_graphical_session() {
        let state = session(SHOW);
        assert!(state.applies_to(Path::new("/usr/bin/hush")));
        assert!(
            state.applies_to(Path::new("/usr/bin/hush (deleted)")),
            "a binary that a package upgrade replaced"
        );
        assert!(
            !state.applies_to(Path::new("/home/me/hush/target/debug/hush")),
            "a source build beside the package"
        );
        let tty = parse_show(SHOW);
        assert!(
            !tty.applies_to(Path::new("/usr/bin/hush")),
            "Sway from a TTY: the unit would lack the display variables"
        );
        let stale = session(
            "LoadState=loaded\nUnitFileState=disabled\nMainPID=0\n\
             ExecStart={ path=/home/me/.local/bin/hush-listen ; argv[]=x }\n",
        );
        assert!(!stale.applies_to(Path::new("/home/me/.local/bin/hush")));
        assert!(!stale.starts_at_login(Path::new("/home/me/.local/bin/hush-listen")));
    }

    #[test]
    fn a_restart_follows_the_daemon_that_runs() {
        let exe = Path::new("/usr/bin/hush");
        let state = session(SHOW);
        assert_eq!(
            restart_route(&state, exe, Some(4242), true),
            RestartRoute::RestartUnit
        );
        assert_eq!(
            restart_route(&state, exe, Some(4242), false),
            RestartRoute::RestartUnit,
            "the unit's daemon restarts in the unit even with other options"
        );
        assert_eq!(
            restart_route(&state, exe, Some(999), true),
            RestartRoute::StopThenStartUnit,
            "a daemon that an older restart moved out of the unit goes back"
        );
        assert_eq!(
            restart_route(&state, exe, None, true),
            RestartRoute::StopThenStartUnit
        );
        assert_eq!(
            restart_route(&state, exe, Some(999), false),
            RestartRoute::StopThenDetached,
            "the unit cannot pass other options"
        );
        assert_eq!(
            restart_route(&ServiceState::default(), exe, Some(999), true),
            RestartRoute::StopThenDetached
        );
    }

    #[test]
    fn the_unit_cgroup_names_the_unit() {
        assert!(cgroup_is_unit(
            "0::/user.slice/user-1000.slice/user@1000.service/app.slice/hush.service\n"
        ));
        assert!(!cgroup_is_unit(
            "0::/user.slice/user-1000.slice/user@1000.service/app.slice/app-io.github.andymai.hush@autostart.service\n"
        ));
        assert!(!cgroup_is_unit(
            "0::/user.slice/user-1000.slice/user@1000.service/app.slice/fakehush.service\n"
        ));
    }

    #[test]
    fn compositors_come_from_their_sockets() {
        let only = |name: &'static str| move |var: &str| (var == name).then(|| "x".to_string());
        assert_eq!(Compositor::detect(only("SWAYSOCK")), Some(Compositor::Sway));
        assert_eq!(
            Compositor::detect(only("HYPRLAND_INSTANCE_SIGNATURE")),
            Some(Compositor::Hyprland)
        );
        assert_eq!(
            Compositor::detect(only("NIRI_SOCKET")),
            Some(Compositor::Niri)
        );
        assert_eq!(Compositor::detect(|_| None), None);
        assert_eq!(
            Compositor::Hyprland.startup_line(),
            "exec-once = hush daemon start"
        );
    }
}
