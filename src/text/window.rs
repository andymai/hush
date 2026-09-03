//! Best-effort focused-window lookup for logging and per-app decisions.
//! Nothing here is required for insertion to work.

use std::process::Command;
use tracing::debug;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInfo {
    pub title: String,
    pub class: String,
}

pub enum WindowProvider {
    X11 {
        conn: Box<x11rb::rust_connection::RustConnection>,
        screen: usize,
    },
    Hyprland,
    Sway,
    Unavailable,
}

impl WindowProvider {
    /// Pick a provider from the session environment, preferring the
    /// compositor's own IPC over an XWayland connection.
    pub fn detect() -> Self {
        if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
            return Self::Hyprland;
        }
        if std::env::var_os("SWAYSOCK").is_some() {
            return Self::Sway;
        }
        if std::env::var_os("DISPLAY").is_some() {
            if let Ok((conn, screen)) = x11rb::connect(None) {
                return Self::X11 {
                    conn: Box::new(conn),
                    screen,
                };
            }
        }
        Self::Unavailable
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::X11 { .. } => "x11",
            Self::Hyprland => "hyprland",
            Self::Sway => "sway",
            Self::Unavailable => "none",
        }
    }

    pub fn focused(&self) -> Option<WindowInfo> {
        match self {
            Self::X11 { conn, screen } => x11_focused(conn, *screen),
            Self::Hyprland => {
                run_json("hyprctl", &["activewindow", "-j"]).and_then(|s| parse_hyprland(&s))
            },
            Self::Sway => run_json("swaymsg", &["-t", "get_tree"]).and_then(|s| parse_sway(&s)),
            Self::Unavailable => None,
        }
    }
}

fn run_json(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        debug!("{} exited with {}", program, output.status);
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse_hyprland(json: &str) -> Option<WindowInfo> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let class = value.get("class")?.as_str()?.to_string();
    let title = value
        .get("title")
        .and_then(|t| t.as_str())
        .unwrap_or_default()
        .to_string();
    Some(WindowInfo { title, class })
}

fn parse_sway(json: &str) -> Option<WindowInfo> {
    let tree: serde_json::Value = serde_json::from_str(json).ok()?;
    let node = find_focused(&tree)?;
    let class = node
        .get("app_id")
        .and_then(|v| v.as_str())
        .or_else(|| {
            node.get("window_properties")
                .and_then(|p| p.get("class"))
                .and_then(|c| c.as_str())
        })?
        .to_string();
    let title = node
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or_default()
        .to_string();
    Some(WindowInfo { title, class })
}

fn find_focused(node: &serde_json::Value) -> Option<&serde_json::Value> {
    if node.get("focused").and_then(|f| f.as_bool()) == Some(true)
        && node
            .get("nodes")
            .is_some_and(|n| n.as_array().is_some_and(|a| a.is_empty()))
    {
        return Some(node);
    }
    for key in ["nodes", "floating_nodes"] {
        if let Some(children) = node.get(key).and_then(|n| n.as_array()) {
            for child in children {
                if let Some(found) = find_focused(child) {
                    return Some(found);
                }
            }
        }
    }
    None
}

fn x11_focused(conn: &x11rb::rust_connection::RustConnection, screen: usize) -> Option<WindowInfo> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, Window};

    let root = conn.setup().roots.get(screen)?.root;
    let focus = conn.get_input_focus().ok()?.reply().ok()?.focus;
    let window: Window = if focus == x11rb::NONE || focus == root {
        return None;
    } else {
        focus
    };
    let property = |name: &str| -> Option<String> {
        let atom = conn
            .intern_atom(false, name.as_bytes())
            .ok()?
            .reply()
            .ok()?
            .atom;
        let reply = conn
            .get_property(false, window, atom, AtomEnum::ANY, 0, u32::MAX)
            .ok()?
            .reply()
            .ok()?;
        Some(
            String::from_utf8_lossy(&reply.value)
                .trim_end_matches('\0')
                .to_string(),
        )
    };
    let title = property("_NET_WM_NAME")
        .or_else(|| property("WM_NAME"))
        .unwrap_or_default();
    let class = property("WM_CLASS")?;
    Some(WindowInfo { title, class })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hyprland_json_yields_class_and_title() {
        let json = r#"{"address":"0x1","class":"kitty","title":"~ — kitty","pid":42}"#;
        assert_eq!(
            parse_hyprland(json),
            Some(WindowInfo {
                title: "~ — kitty".into(),
                class: "kitty".into()
            })
        );
        assert_eq!(parse_hyprland("{}"), None);
    }

    #[test]
    fn sway_tree_finds_the_focused_leaf() {
        let json = r#"{"focused":false,"nodes":[{"focused":true,"nodes":[
            {"focused":true,"nodes":[],"floating_nodes":[],"name":"Firefox","app_id":"firefox"}]}]}"#;
        let found = parse_sway(json).unwrap();
        assert_eq!(found.class, "firefox");
        assert_eq!(found.title, "Firefox");

        let xwayland =
            r#"{"focused":true,"nodes":[],"name":"Term","window_properties":{"class":"XTerm"}}"#;
        assert_eq!(parse_sway(xwayland).unwrap().class, "XTerm");
        assert_eq!(parse_sway(r#"{"focused":false,"nodes":[]}"#), None);
    }

    #[test]
    fn detect_never_panics() {
        let provider = WindowProvider::detect();
        assert!(!provider.name().is_empty());
    }
}
