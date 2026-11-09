//! Desktop notifications, best effort: without a notification daemon the
//! message is only logged.
//!
//! Each notification runs on its own thread with its own runtime, because the
//! D-Bus client needs one and callers reach here from plain threads as well as
//! from the daemon's async context.
//!
//! Each kind of notice reuses the server ID of its last notice, so a new notice
//! replaces the old one instead of stacking under it.

use notify_rust::{Hint, Notification, Timeout};
use parking_lot::Mutex;
use tracing::debug;

/// Pick `Status` for the reply to what the user just did and for the progress
/// of a take: the next action makes it stale. Pick `Problem` for a fault that
/// outlives the action (a device, a setting, a service) or for text that did
/// not reach the cursor. Pick `Release` for a new Hush version, so that a
/// later problem does not replace it in the history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    Status,
    Problem,
    Release,
}

impl NoticeKind {
    fn is_transient(self) -> bool {
        matches!(self, Self::Status)
    }
}

static LAST_IDS: LastIds = LastIds::new();

struct LastIds {
    status: Mutex<Option<u32>>,
    problem: Mutex<Option<u32>>,
    release: Mutex<Option<u32>>,
}

impl LastIds {
    const fn new() -> Self {
        Self {
            status: Mutex::new(None),
            problem: Mutex::new(None),
            release: Mutex::new(None),
        }
    }

    /// The slot stays locked while `send` runs, so a quick second notice of
    /// the same kind waits for the first ID instead of stacking.
    fn send_with(&self, kind: NoticeKind, send: impl FnOnce(Option<u32>) -> Option<u32>) {
        let slot = match kind {
            NoticeKind::Status => &self.status,
            NoticeKind::Problem => &self.problem,
            NoticeKind::Release => &self.release,
        };
        let mut last = slot.lock();
        if let Some(id) = send(*last) {
            *last = Some(id);
        }
    }
}

fn build(kind: NoticeKind, summary: &str, body: &str, replaces: Option<u32>) -> Notification {
    let mut notification = Notification::new();
    notification
        .appname("Hush")
        .summary(summary)
        .body(body)
        .icon("io.github.andymai.hush")
        .timeout(Timeout::Milliseconds(4000));
    if let Some(id) = replaces {
        notification.id(id);
    }
    if kind.is_transient() {
        notification.hint(Hint::Transient(true));
    }
    notification
}

pub fn notify(kind: NoticeKind, summary: &str, body: &str) {
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
        LAST_IDS.send_with(kind, |replaces| {
            let notification = build(kind, &summary, &body, replaces);
            match runtime.block_on(notification.show_async()) {
                Ok(handle) => Some(handle.id()),
                Err(e) => {
                    debug!("Notification '{}' not shown: {}", summary, e);
                    None
                },
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_starts_without_an_id() {
        let ids = LastIds::new();
        for kind in [NoticeKind::Status, NoticeKind::Problem, NoticeKind::Release] {
            ids.send_with(kind, |last| {
                assert_eq!(last, None);
                None
            });
        }
    }

    #[test]
    fn next_notice_gets_the_last_id_of_its_kind() {
        let ids = LastIds::new();
        ids.send_with(NoticeKind::Status, |_| Some(7));
        ids.send_with(NoticeKind::Status, |last| {
            assert_eq!(last, Some(7));
            Some(9)
        });
        ids.send_with(NoticeKind::Status, |last| {
            assert_eq!(last, Some(9));
            None
        });
    }

    #[test]
    fn kinds_keep_separate_ids() {
        let ids = LastIds::new();
        ids.send_with(NoticeKind::Status, |_| Some(3));
        ids.send_with(NoticeKind::Problem, |last| {
            assert_eq!(last, None);
            Some(4)
        });
        ids.send_with(NoticeKind::Release, |last| {
            assert_eq!(last, None);
            None
        });
        ids.send_with(NoticeKind::Status, |last| {
            assert_eq!(last, Some(3));
            None
        });
    }

    #[test]
    fn failed_send_keeps_the_last_id() {
        let ids = LastIds::new();
        ids.send_with(NoticeKind::Problem, |_| Some(5));
        ids.send_with(NoticeKind::Problem, |_| None);
        ids.send_with(NoticeKind::Problem, |last| {
            assert_eq!(last, Some(5));
            None
        });
    }

    #[test]
    fn only_status_notices_are_transient() {
        let transient = |kind| {
            build(kind, "summary", "body", None)
                .hints
                .contains(&Hint::Transient(true))
        };
        assert!(transient(NoticeKind::Status));
        assert!(!transient(NoticeKind::Problem));
        assert!(!transient(NoticeKind::Release));
    }
}
