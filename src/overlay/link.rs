//! The overlay's end of the socket pair that the daemon passes as fd 3. It
//! asks for a watch with the level and events, then reads whole lines.

use crate::ipc::protocol::{DaemonCommand, WatchLine};
use anyhow::{anyhow, Context, Result};
use std::io::{ErrorKind, Read, Write};
use std::os::fd::{AsFd, BorrowedFd, FromRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use tracing::debug;

/// The fd that the daemon gives its overlay.
pub const FD: i32 = 3;

pub struct Link {
    stream: UnixStream,
    buffer: Vec<u8>,
    closed: bool,
}

impl Link {
    /// Take fd 3 and ask for the watch.
    pub fn open() -> Result<Self> {
        if !is_socket(FD) {
            return Err(anyhow!(
                "hush overlay runs only as the daemon's child, which passes it a socket as fd {}",
                FD
            ));
        }
        // SAFETY: fd 3 is an open socket (checked above) that the daemon
        // passed for this process alone, and nothing else here owns it.
        let fd = unsafe { OwnedFd::from_raw_fd(FD) };
        Self::over(UnixStream::from(fd))
    }

    pub fn over(mut stream: UnixStream) -> Result<Self> {
        let mut request = serde_json::to_string(&DaemonCommand::Watch {
            meter: true,
            events: true,
        })?;
        request.push('\n');
        stream
            .write_all(request.as_bytes())
            .context("Failed to ask the daemon for its state")?;
        stream.set_nonblocking(true)?;
        Ok(Self {
            stream,
            buffer: Vec::new(),
            closed: false,
        })
    }

    pub fn fd(&self) -> BorrowedFd<'_> {
        self.stream.as_fd()
    }

    /// The daemon closed its end, so the overlay has nothing left to show.
    pub fn closed(&self) -> bool {
        self.closed
    }

    /// The whole lines that arrived since the last call.
    pub fn read(&mut self) -> Vec<WatchLine> {
        let mut chunk = [0u8; 4096];
        loop {
            match self.stream.read(&mut chunk) {
                Ok(0) => {
                    self.closed = true;
                    break;
                },
                Ok(count) => self.buffer.extend_from_slice(&chunk[..count]),
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => {
                    debug!("The daemon's socket failed: {}", e);
                    self.closed = true;
                    break;
                },
            }
        }
        let mut lines = Vec::new();
        while let Some(end) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.buffer.drain(..=end).collect();
            match serde_json::from_slice::<WatchLine>(&line) {
                Ok(line) => lines.push(line),
                Err(e) => debug!("Skipped a line from the daemon: {}", e),
            }
        }
        lines
    }
}

fn is_socket(fd: i32) -> bool {
    // SAFETY: fstat only writes the stat buffer, and fails on a closed fd.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    let found = unsafe { libc::fstat(fd, &mut stat) } == 0;
    found && stat.st_mode & libc::S_IFMT == libc::S_IFSOCK
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::protocol::{DaemonResponse, DaemonState, Level};
    use std::io::{BufRead, BufReader};

    #[test]
    fn the_link_asks_for_a_full_watch_and_reads_whole_lines() {
        let (ours, mut daemon) = UnixStream::pair().unwrap();
        let mut link = Link::over(ours).unwrap();
        let mut request = String::new();
        BufReader::new(daemon.try_clone().unwrap())
            .read_line(&mut request)
            .unwrap();
        assert_eq!(
            request,
            "{\"command\":\"watch\",\"meter\":true,\"events\":true}\n"
        );

        assert!(link.read().is_empty(), "nothing arrived yet");
        daemon
            .write_all(b"{\"ok\":true,\"state\":{\"state\":\"idle\"}}\n{\"level\":0.")
            .unwrap();
        assert_eq!(
            link.read(),
            [WatchLine::State(Box::new(DaemonResponse::ok(
                DaemonState::Idle
            )))]
        );
        daemon.write_all(b"5}\nnot json\n").unwrap();
        assert_eq!(link.read(), [WatchLine::Level(Level { level: 0.5 })]);
        assert!(!link.closed());
        drop(daemon);
        assert!(link.read().is_empty());
        assert!(link.closed(), "the daemon's end closed");
    }

    #[test]
    fn a_plain_file_is_not_the_daemons_socket() {
        let file = tempfile::tempfile().unwrap();
        use std::os::fd::AsRawFd;
        assert!(!is_socket(file.as_raw_fd()));
        let (socket, _other) = UnixStream::pair().unwrap();
        assert!(is_socket(socket.as_raw_fd()));
    }
}
