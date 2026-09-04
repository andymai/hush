//! Focused window on KDE Plasma (Wayland) through KWin scripting. KWin has
//! no D-Bus method that names the active window, but it runs scripts that
//! can call back over D-Bus, so each query loads a one-line script that
//! reports the active window's caption and class to a service Hush owns.

use super::window::WindowInfo;
use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;
use tracing::debug;
use zbus::blocking::{Connection, Proxy};

const SERVICE: &str = "io.github.andymai.hush.WindowQuery";
const OBJECT: &str = "/io/github/andymai/hush/WindowQuery";
const PLUGIN: &str = "hush-window-query";
const TIMEOUT: Duration = Duration::from_millis(600);

struct Reporter {
    tx: mpsc::Sender<WindowInfo>,
}

#[zbus::interface(name = "io.github.andymai.hush.WindowQuery")]
impl Reporter {
    #[zbus(name = "report")]
    fn report(&self, caption: String, class: String) {
        let _ = self.tx.send(WindowInfo {
            title: caption,
            class,
        });
    }
}

pub struct KwinQuery {
    conn: Connection,
    rx: Mutex<mpsc::Receiver<WindowInfo>>,
    script: PathBuf,
}

impl KwinQuery {
    pub fn new() -> Result<Self> {
        let conn = Connection::session().context("session bus")?;
        let (tx, rx) = mpsc::channel();
        conn.object_server()
            .at(OBJECT, Reporter { tx })
            .context("export window query object")?;
        conn.request_name(SERVICE)
            .context("own window query name")?;
        let script = write_script()?;
        Ok(Self {
            conn,
            rx: Mutex::new(rx),
            script,
        })
    }

    pub fn focused(&self) -> Option<WindowInfo> {
        match self.query() {
            Ok(info) => info,
            Err(e) => {
                debug!("KWin window query failed: {}", e);
                None
            },
        }
    }

    fn query(&self) -> Result<Option<WindowInfo>> {
        let rx = self.rx.lock();
        while rx.try_recv().is_ok() {}
        let scripting = Proxy::new(
            &self.conn,
            "org.kde.KWin",
            "/Scripting",
            "org.kde.kwin.Scripting",
        )?;
        let _: bool = scripting.call("unloadScript", &(PLUGIN,)).unwrap_or(false);
        let id: i32 = scripting.call(
            "loadScript",
            &(self.script.to_string_lossy().as_ref(), PLUGIN),
        )?;
        if id < 0 {
            return Err(anyhow!("KWin refused the script"));
        }
        // Plasma 6 exposes scripts at /<id>, Plasma 5 at /Scripting/Script<id>.
        let mut started = false;
        for path in [format!("/{}", id), format!("/Scripting/Script{}", id)] {
            let script = Proxy::new(
                &self.conn,
                "org.kde.KWin",
                path.as_str(),
                "org.kde.kwin.Script",
            )?;
            if script.call::<_, _, ()>("run", &()).is_ok() {
                started = true;
                break;
            }
        }
        if !started {
            let _: bool = scripting.call("unloadScript", &(PLUGIN,)).unwrap_or(false);
            return Err(anyhow!("could not start the KWin script"));
        }
        let result = rx.recv_timeout(TIMEOUT).ok();
        let _: bool = scripting.call("unloadScript", &(PLUGIN,)).unwrap_or(false);
        Ok(result.filter(|info| !info.class.is_empty() || !info.title.is_empty()))
    }
}

impl Drop for KwinQuery {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.script);
    }
}

fn write_script() -> Result<PathBuf> {
    let dir = crate::ipc::paths::runtime_dir();
    std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let path = dir.join("window-query.js");
    let script = format!(
        "var w = workspace.activeWindow || workspace.activeClient;\n\
         var caption = w ? String(w.caption) : \"\";\n\
         var cls = w ? String(w.resourceClass) : \"\";\n\
         callDBus(\"{SERVICE}\", \"{OBJECT}\", \"{SERVICE}\", \"report\", caption, cls);\n"
    );
    std::fs::write(&path, script).with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}
