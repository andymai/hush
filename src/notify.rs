//! Desktop notifications, best effort: without a notification daemon the
//! message is only logged.

use tracing::debug;

pub fn notify(summary: &str, body: &str) {
    let summary = summary.to_string();
    let body = body.to_string();
    std::thread::spawn(move || {
        let result = notify_rust::Notification::new()
            .appname("Hush")
            .summary(&summary)
            .body(&body)
            .icon("io.github.andymai.hush")
            .timeout(notify_rust::Timeout::Milliseconds(4000))
            .show();
        if let Err(e) = result {
            debug!("Notification '{}' not shown: {}", summary, e);
        }
    });
}
