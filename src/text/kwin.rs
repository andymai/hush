//! Focused window on KDE Plasma (Wayland) through KWin scripting. KWin has
//! no D-Bus method that names the active window, but it runs scripts that can
//! call back over D-Bus, so each query loads a one-line script that reports
//! the active window's caption and class to a service Hush owns.
//!
//! All of it runs on one dedicated thread with its own runtime, so callers can
//! ask from anywhere, inside or outside the daemon's async context.

use super::window::WindowInfo;
use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tokio::sync::mpsc as async_mpsc;
use tracing::debug;
use zbus::{Connection, Proxy};

const SERVICE: &str = "io.github.andymai.hush.WindowQuery";
const OBJECT: &str = "/io/github/andymai/hush/WindowQuery";
const PLUGIN: &str = "hush-window-query";
/// How long KWin gets to load, run, and report.
const REPORT_TIMEOUT: Duration = Duration::from_millis(700);
/// How long a caller waits for the whole round trip.
const CALL_TIMEOUT: Duration = Duration::from_millis(1500);
const START_TIMEOUT: Duration = Duration::from_secs(5);

type Reply = mpsc::Sender<Option<WindowInfo>>;

struct Reporter {
    tx: async_mpsc::UnboundedSender<WindowInfo>,
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
    requests: async_mpsc::UnboundedSender<Reply>,
}

impl KwinQuery {
    pub fn new() -> Result<Self> {
        let (requests, request_rx) = async_mpsc::unbounded_channel::<Reply>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<()>>();
        thread::Builder::new()
            .name("hush-kwin".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(e) => {
                        let _ = ready_tx.send(Err(anyhow!("KWin query runtime: {}", e)));
                        return;
                    },
                };
                runtime.block_on(serve(request_rx, ready_tx));
            })
            .context("Failed to start the KWin query thread")?;
        ready_rx
            .recv_timeout(START_TIMEOUT)
            .map_err(|_| anyhow!("The KWin query thread did not start"))??;
        Ok(Self { requests })
    }

    pub fn focused(&self) -> Option<WindowInfo> {
        let (reply_tx, reply_rx) = mpsc::channel();
        if self.requests.send(reply_tx).is_err() {
            debug!("The KWin query thread has stopped");
            return None;
        }
        match reply_rx.recv_timeout(CALL_TIMEOUT) {
            Ok(info) => info,
            Err(_) => {
                debug!("The KWin query timed out");
                None
            },
        }
    }
}

async fn serve(
    mut requests: async_mpsc::UnboundedReceiver<Reply>,
    ready: mpsc::Sender<Result<()>>,
) {
    let (report_tx, mut reports) = async_mpsc::unbounded_channel();
    let (connection, path) = match connect(report_tx).await {
        Ok(state) => {
            let _ = ready.send(Ok(()));
            state
        },
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        },
    };
    while let Some(reply) = requests.recv().await {
        // Drop anything a previous, timed-out query left behind.
        while reports.try_recv().is_ok() {}
        let info = match query(&connection, &path, &mut reports).await {
            Ok(info) => info,
            Err(e) => {
                debug!("KWin window query failed: {}", e);
                None
            },
        };
        let _ = reply.send(info);
    }
    let _ = std::fs::remove_file(&path);
}

async fn connect(
    report_tx: async_mpsc::UnboundedSender<WindowInfo>,
) -> Result<(Connection, PathBuf)> {
    let connection = Connection::session().await.context("session bus")?;
    connection
        .object_server()
        .at(OBJECT, Reporter { tx: report_tx })
        .await
        .context("export the window query object")?;
    connection
        .request_name(SERVICE)
        .await
        .context("own the window query name")?;
    let path = write_script()?;
    Ok((connection, path))
}

async fn query(
    connection: &Connection,
    script: &Path,
    reports: &mut async_mpsc::UnboundedReceiver<WindowInfo>,
) -> Result<Option<WindowInfo>> {
    let scripting = Proxy::new(
        connection,
        "org.kde.KWin",
        "/Scripting",
        "org.kde.kwin.Scripting",
    )
    .await?;
    let _: bool = scripting
        .call("unloadScript", &(PLUGIN,))
        .await
        .unwrap_or(false);
    let id: i32 = scripting
        .call("loadScript", &(script.to_string_lossy().as_ref(), PLUGIN))
        .await?;
    if id < 0 {
        return Err(anyhow!("KWin refused the script"));
    }
    // Plasma 6 exposes scripts at /<id>, Plasma 5 at /Scripting/Script<id>.
    let mut started = false;
    for object in [format!("/{}", id), format!("/Scripting/Script{}", id)] {
        let Ok(proxy) = Proxy::new(connection, "org.kde.KWin", object, "org.kde.kwin.Script").await
        else {
            continue;
        };
        if proxy.call::<_, _, ()>("run", &()).await.is_ok() {
            started = true;
            break;
        }
    }
    if !started {
        let _: bool = scripting
            .call("unloadScript", &(PLUGIN,))
            .await
            .unwrap_or(false);
        return Err(anyhow!("could not start the KWin script"));
    }
    let report = tokio::time::timeout(REPORT_TIMEOUT, reports.recv())
        .await
        .ok()
        .flatten();
    let _: bool = scripting
        .call("unloadScript", &(PLUGIN,))
        .await
        .unwrap_or(false);
    Ok(report.filter(|info| !info.class.is_empty() || !info.title.is_empty()))
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
