//! The daemon's side of the overlay. It starts `hush overlay` with one end
//! of a socket pair as fd 3, serves the child's watch on the other end, and
//! starts the child again after a crash. Dictation never waits for it.

use super::NO_SURFACE;
use crate::ipc::protocol::OverlayStatus;
use crate::ipc::{server, SessionCommand, SharedState};
use crate::notify::{notify, NoticeKind};
use anyhow::{Context, Result};
use std::collections::VecDeque;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// A run this long forgets the earlier failures.
const STABLE: Duration = Duration::from_secs(60);
const MAX_FAILURES: u32 = 5;
/// Restarts after a lost display connection, in one minute, before such an
/// exit counts as a failure (a compositor that drops the client each time).
const CLEAN_PER_MINUTE: usize = 3;

/// `close_range` flag: mark the fds close-on-exec instead of closing them,
/// which keeps the pipe through which std reports a failed exec.
const CLOSE_RANGE_CLOEXEC: libc::c_uint = 1 << 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    /// The display connection ended
    Clean,
    NoSurface,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    Restart(Duration),
    Stop(OverlayStatus),
}

/// When to start the child again.
#[derive(Debug, Default)]
pub struct Policy {
    failures: u32,
    clean: VecDeque<Instant>,
}

impl Policy {
    pub fn after(&mut self, ended: Ended, ran: Duration, now: Instant) -> Next {
        if ran >= STABLE {
            self.failures = 0;
        }
        match ended {
            Ended::NoSurface => Next::Stop(OverlayStatus::Unavailable),
            Ended::Clean => {
                self.clean
                    .retain(|at| now.saturating_duration_since(*at) < Duration::from_secs(60));
                self.clean.push_back(now);
                if self.clean.len() <= CLEAN_PER_MINUTE {
                    Next::Restart(Duration::ZERO)
                } else {
                    self.fail()
                }
            },
            Ended::Failed => self.fail(),
        }
    }

    fn fail(&mut self) -> Next {
        self.failures += 1;
        if self.failures >= MAX_FAILURES {
            Next::Stop(OverlayStatus::Failed)
        } else {
            Next::Restart(Duration::from_secs(1 << (self.failures - 1).min(2)))
        }
    }
}

/// Start the overlay on a thread of its own, unless `HUSH_NO_OVERLAY` is set
/// or the config turns it off. The thread lives as long as the daemon,
/// because the child's parent-death signal follows the thread that forked it.
pub fn start(
    enabled: bool,
    shared: Arc<SharedState>,
    control: mpsc::Sender<SessionCommand>,
    runtime: tokio::runtime::Handle,
) {
    if !enabled || std::env::var_os("HUSH_NO_OVERLAY").is_some() {
        shared.set_overlay(OverlayStatus::Off);
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("hush-overlay".into())
        .spawn(move || supervise(&shared, &control, &runtime));
    if let Err(e) = spawned {
        warn!("The overlay did not start: {}", e);
    }
}

fn supervise(
    shared: &Arc<SharedState>,
    control: &mpsc::Sender<SessionCommand>,
    runtime: &tokio::runtime::Handle,
) {
    let mut policy = Policy::default();
    loop {
        if binary_replaced() {
            // A package upgrade replaced the binary, and its libraries next
            // to it are new: a new overlay would mix the two versions.
            shared.set_overlay(OverlayStatus::Failed);
            notify(
                NoticeKind::Problem,
                "Hush was updated",
                "Restart Hush to see the recording overlay again: hush daemon restart",
            );
            return;
        }
        let started = Instant::now();
        let ended = match run_child(shared, control, runtime) {
            Ok(ended) => ended,
            Err(e) => {
                warn!("The overlay failed to start: {:#}", e);
                Ended::Failed
            },
        };
        // A daemon that quits closes the overlay's socket, so the overlay
        // ends cleanly; a new one would find a runtime that serves nothing.
        if shared.stopping() {
            return;
        }
        match policy.after(ended, started.elapsed(), Instant::now()) {
            Next::Restart(delay) => {
                info!(?ended, "The overlay starts again in {} s", delay.as_secs());
                std::thread::sleep(delay);
                if shared.stopping() {
                    return;
                }
            },
            Next::Stop(status) => {
                match status {
                    OverlayStatus::Failed => warn!(
                        "The overlay failed {} times, so the daemon runs without it",
                        MAX_FAILURES
                    ),
                    _ => info!("The daemon runs without the overlay"),
                }
                shared.set_overlay(status);
                return;
            },
        }
    }
}

fn run_child(
    shared: &Arc<SharedState>,
    control: &mpsc::Sender<SessionCommand>,
    runtime: &tokio::runtime::Handle,
) -> Result<Ended> {
    let (ours, theirs) = UnixStream::pair().context("Failed to make the overlay's socket")?;
    let child_fd = theirs.as_raw_fd();
    let parent = std::process::id();
    let mut command = Command::new("/proc/self/exe");
    command
        .arg0("hush")
        .arg("overlay")
        .stdin(Stdio::null())
        .stdout(Stdio::null());
    // SAFETY: between fork and exec the closure calls only async-signal-safe
    // functions (dup2, fcntl, prctl, getppid, syscall) and allocates nothing.
    unsafe {
        command.pre_exec(move || {
            let fd = super::link::FD;
            if child_fd == fd {
                // dup2 onto itself would keep close-on-exec set.
                if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
            } else if libc::dup2(child_fd, fd) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            // The daemon died before prctl took effect.
            if libc::getppid() as u32 != parent {
                libc::_exit(0);
            }
            // C libraries in the daemon can leave fds without close-on-exec.
            libc::syscall(libc::SYS_close_range, 4u32, u32::MAX, CLOSE_RANGE_CLOEXEC);
            Ok(())
        });
    }
    let mut child = command.spawn().context("Failed to start hush overlay")?;
    // The child holds its own copy; with ours open, the daemon's end would
    // never see the child exit.
    drop(theirs);
    shared.set_overlay(OverlayStatus::Running);
    ours.set_nonblocking(true)?;
    let shared_serve = Arc::clone(shared);
    let control_serve = control.clone();
    runtime.spawn(async move { server::serve_stream(ours, &shared_serve, &control_serve).await });
    let status = child.wait().context("Failed to wait for hush overlay")?;
    Ok(match status.code() {
        Some(0) => Ended::Clean,
        Some(NO_SURFACE) => Ended::NoSurface,
        _ => Ended::Failed,
    })
}

/// `/proc/self/exe` names a deleted file after a package upgrade.
fn binary_replaced() -> bool {
    std::fs::read_link("/proc/self/exe")
        .is_ok_and(|path| path.to_string_lossy().ends_with(" (deleted)"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_failures_back_off_and_then_give_up() {
        let mut policy = Policy::default();
        let now = Instant::now();
        let quick = Duration::from_secs(1);
        let delays: Vec<Next> = (0..MAX_FAILURES)
            .map(|_| policy.after(Ended::Failed, quick, now))
            .collect();
        assert_eq!(
            delays,
            [
                Next::Restart(Duration::from_secs(1)),
                Next::Restart(Duration::from_secs(2)),
                Next::Restart(Duration::from_secs(4)),
                Next::Restart(Duration::from_secs(4)),
                Next::Stop(OverlayStatus::Failed),
            ]
        );
    }

    #[test]
    fn a_stable_run_forgets_old_failures() {
        let mut policy = Policy::default();
        let now = Instant::now();
        for _ in 0..4 {
            policy.after(Ended::Failed, Duration::from_secs(1), now);
        }
        assert_eq!(
            policy.after(Ended::Failed, Duration::from_secs(600), now),
            Next::Restart(Duration::from_secs(1))
        );
    }

    #[test]
    fn a_lost_display_restarts_at_once_a_few_times_a_minute() {
        let mut policy = Policy::default();
        let now = Instant::now();
        for _ in 0..CLEAN_PER_MINUTE {
            assert_eq!(
                policy.after(Ended::Clean, Duration::from_secs(1), now),
                Next::Restart(Duration::ZERO)
            );
        }
        assert_eq!(
            policy.after(Ended::Clean, Duration::from_secs(1), now),
            Next::Restart(Duration::from_secs(1)),
            "a fourth in a minute counts as a failure"
        );
        assert_eq!(
            policy.after(
                Ended::Clean,
                Duration::from_secs(1),
                now + Duration::from_secs(61)
            ),
            Next::Restart(Duration::ZERO),
            "a minute later the display may drop the client again"
        );
    }

    #[test]
    fn no_surface_stops_for_good() {
        let mut policy = Policy::default();
        assert_eq!(
            policy.after(Ended::NoSurface, Duration::ZERO, Instant::now()),
            Next::Stop(OverlayStatus::Unavailable)
        );
    }
}
