//! Desktop notifications, best effort: without a notification daemon the
//! message is only logged.
//!
//! Each notification runs on its own thread with its own runtime, because the
//! D-Bus client needs one and callers reach here from plain threads as well as
//! from the daemon's async context.

use tracing::debug;

pub fn notify(summary: &str, body: &str) {
    let summary = summary.to_string();
    let body = body.to_string();
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(e) => {
                debug!("Notification '{}' not shown: {}", summary, e);
                return;
            },
        };
        let result = runtime.block_on(
            notify_rust::Notification::new()
                .appname("Hush")
                .summary(&summary)
                .body(&body)
                .icon("io.github.andymai.hush")
                .timeout(notify_rust::Timeout::Milliseconds(4000))
                .show_async(),
        );
        if let Err(e) = result {
            debug!("Notification '{}' not shown: {}", summary, e);
        }
    });
}
