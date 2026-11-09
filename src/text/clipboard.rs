//! The clipboard side of a paste.
//!
//! Before the paste, every format on the clipboard is saved, within a size
//! and a time limit. The transcript goes on the clipboard with a hint that
//! keeps it out of clipboard history (Klipper, cliphist, CopyQ). After the
//! paste, the saved formats come back, but only while the clipboard still
//! holds the transcript, so a copy the user makes in between stays.
//!
//! A Wayland compositor with a data-control protocol (ext or wlr) gives
//! access to every format. GNOME has no such protocol and X11 goes through
//! arboard, so those sessions keep only text.

use super::clock::Clock;
use crate::Result;
use anyhow::{anyhow, Context};
use std::fmt;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tracing::{debug, warn};

pub const MAX_SNAPSHOT_BYTES: usize = 32 * 1024 * 1024;
pub const SNAPSHOT_BUDGET: Duration = Duration::from_secs(1);
const CHECK_BUDGET: Duration = Duration::from_millis(250);
pub const HISTORY_HINT_MIME: &str = "x-kde-passwordManagerHint";
const HISTORY_HINT: &[u8] = b"secret";
/// Carries a token per paste, so a later check tells Hush's own transcript
/// apart from the same text copied by the user.
const MARKER_MIME: &str = "application/x-hush-transcript";
const TEXT_MIMES: [&str; 5] = [
    "text/plain;charset=utf-8",
    "text/plain",
    "UTF8_STRING",
    "STRING",
    "TEXT",
];
/// X11 selection targets that describe the selection and hold no data.
const PSEUDO_TARGETS: [&str; 5] = ["TARGETS", "TIMESTAMP", "MULTIPLE", "SAVE_TARGETS", "DELETE"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Selection {
    Clipboard,
    /// What a middle click, and in some applications Shift+Insert, pastes.
    Primary,
}

impl fmt::Display for Selection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Clipboard => "clipboard",
            Self::Primary => "primary selection",
        })
    }
}

/// One MIME type and its data.
pub type Format = (String, Vec<u8>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Snapshot {
    Empty,
    Formats(Vec<Format>),
}

/// Why the clipboard keeps the transcript after a paste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoRestore {
    Unreadable(String),
    TooLarge,
    TooSlow,
}

impl fmt::Display for NoRestore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable(why) => write!(f, "the old content could not be saved ({})", why),
            Self::TooLarge => write!(
                f,
                "the old content was larger than {} MiB",
                MAX_SNAPSHOT_BYTES / (1024 * 1024)
            ),
            Self::TooSlow => write!(
                f,
                "the old content took longer than {} ms to read",
                SNAPSHOT_BUDGET.as_millis()
            ),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("the data is larger than the limit")]
    TooLarge,
    #[error("the owner did not send the data in time")]
    TimedOut,
    #[error(transparent)]
    Failed(#[from] anyhow::Error),
}

/// The text of one paste, and the token that identifies it.
pub struct Transcript {
    text: String,
    token: String,
}

impl Transcript {
    pub fn new(text: &str) -> Self {
        static PASTES: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Self {
            text: text.to_string(),
            token: format!(
                "{}-{}-{}",
                std::process::id(),
                nanos,
                PASTES.fetch_add(1, Ordering::Relaxed)
            ),
        }
    }

    /// The text in the usual text types, the history hint, and the marker.
    pub fn formats(&self) -> Vec<Format> {
        let mut formats = text_formats(&self.text);
        formats.push((HISTORY_HINT_MIME.to_string(), HISTORY_HINT.to_vec()));
        formats.push((MARKER_MIME.to_string(), self.token.as_bytes().to_vec()));
        formats
    }
}

/// Plain text in every type that applications ask for.
pub fn text_formats(text: &str) -> Vec<Format> {
    TEXT_MIMES
        .iter()
        .map(|mime| (mime.to_string(), text.as_bytes().to_vec()))
        .collect()
}

fn is_plain_text(mime: &str) -> bool {
    TEXT_MIMES.contains(&mime) || mime.starts_with("text/plain")
}

/// Access to one session's clipboard.
pub trait ClipboardBackend: Send {
    /// How much a restore brings back, for the logs.
    fn describe(&self) -> &'static str;
    /// Whether every offered type survives, so the marker can identify the
    /// transcript. Otherwise the check compares the text.
    fn carries_marker(&self) -> bool;
    /// The offered MIME types; empty when nothing owns the selection.
    fn offered(&mut self, selection: Selection) -> Result<Vec<String>>;
    /// The data of one type, at most `limit` bytes, before `deadline`.
    fn read(
        &mut self,
        selection: Selection,
        mime: &str,
        limit: usize,
        deadline: Instant,
    ) -> std::result::Result<Vec<u8>, ReadError>;
    /// Own the selection and offer exactly these formats.
    fn offer(&mut self, selection: Selection, formats: Vec<Format>) -> Result<()>;
    fn clear(&mut self, selection: Selection) -> Result<()>;
}

/// Save every format of a selection, or say why it will not come back.
pub fn snapshot(
    backend: &mut dyn ClipboardBackend,
    selection: Selection,
    clock: &dyn Clock,
) -> std::result::Result<Snapshot, NoRestore> {
    snapshot_within(
        backend,
        selection,
        MAX_SNAPSHOT_BYTES,
        SNAPSHOT_BUDGET,
        clock,
    )
}

fn snapshot_within(
    backend: &mut dyn ClipboardBackend,
    selection: Selection,
    max_bytes: usize,
    budget: Duration,
    clock: &dyn Clock,
) -> std::result::Result<Snapshot, NoRestore> {
    let deadline = clock.now() + budget;
    let offered = backend
        .offered(selection)
        .map_err(|e| NoRestore::Unreadable(format!("{:#}", e)))?;
    let mut mimes: Vec<String> = Vec::with_capacity(offered.len());
    for mime in offered {
        if !PSEUDO_TARGETS.contains(&mime.as_str()) && !mimes.contains(&mime) {
            mimes.push(mime);
        }
    }
    if mimes.is_empty() {
        return Ok(Snapshot::Empty);
    }
    let mut total = 0usize;
    let mut formats = Vec::with_capacity(mimes.len());
    for mime in mimes {
        if clock.now() >= deadline {
            return Err(NoRestore::TooSlow);
        }
        let data = match backend.read(selection, &mime, max_bytes - total, deadline) {
            Ok(data) => data,
            Err(ReadError::TooLarge) => return Err(NoRestore::TooLarge),
            Err(ReadError::TimedOut) => return Err(NoRestore::TooSlow),
            Err(ReadError::Failed(e)) => {
                return Err(NoRestore::Unreadable(format!("{}: {:#}", mime, e)))
            },
        };
        total += data.len();
        if total > max_bytes {
            return Err(NoRestore::TooLarge);
        }
        if clock.now() > deadline {
            return Err(NoRestore::TooSlow);
        }
        formats.push((mime, data));
    }
    Ok(Snapshot::Formats(formats))
}

/// Whether the selection still holds this transcript.
pub fn holds(
    backend: &mut dyn ClipboardBackend,
    selection: Selection,
    transcript: &Transcript,
    clock: &dyn Clock,
) -> Result<bool> {
    let offered = backend.offered(selection)?;
    let (mime, expected) = if backend.carries_marker() {
        (MARKER_MIME, transcript.token.as_bytes())
    } else {
        match offered.iter().find(|mime| is_plain_text(mime)) {
            Some(mime) => (mime.as_str(), transcript.text.as_bytes()),
            None => return Ok(false),
        }
    };
    if !offered.iter().any(|offered| offered == mime) {
        return Ok(false);
    }
    let deadline = clock.now() + CHECK_BUDGET;
    match backend.read(selection, mime, expected.len(), deadline) {
        Ok(data) => Ok(data == expected),
        Err(ReadError::TooLarge) => Ok(false),
        Err(e) => Err(anyhow!(e)).with_context(|| format!("could not read the {}", selection)),
    }
}

/// What happened to a selection after the paste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Restore {
    /// The saved formats are back.
    Restored,
    /// The selection was empty before the paste and is empty again.
    Cleared,
    /// The snapshot failed, so the transcript stays.
    Kept(NoRestore),
    /// Something else owns the selection now, and keeps it.
    Replaced,
    /// The ownership check failed, so nothing changes.
    CheckFailed(String),
    /// The restore itself failed.
    Failed(String),
}

/// Put the saved formats back if the selection still holds the transcript.
pub fn restore(
    backend: &mut dyn ClipboardBackend,
    selection: Selection,
    saved: std::result::Result<Snapshot, NoRestore>,
    transcript: &Transcript,
    clock: &dyn Clock,
) -> Restore {
    let snapshot = match saved {
        Ok(snapshot) => snapshot,
        Err(why) => return Restore::Kept(why),
    };
    match holds(backend, selection, transcript, clock) {
        Ok(true) => {},
        Ok(false) => return Restore::Replaced,
        Err(e) => return Restore::CheckFailed(format!("{:#}", e)),
    }
    let result = match snapshot {
        Snapshot::Empty => backend.clear(selection).map(|()| Restore::Cleared),
        Snapshot::Formats(formats) => backend
            .offer(selection, formats)
            .map(|()| Restore::Restored),
    };
    result.unwrap_or_else(|e| Restore::Failed(format!("{:#}", e)))
}

/// How much of the clipboard this session can save and restore.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    /// Wayland with ext-data-control or wlr-data-control: every format.
    DataControl,
    /// Wayland without a data-control protocol, such as GNOME: text only,
    /// through XWayland.
    NoDataControl,
    /// X11: text only.
    X11,
}

pub fn support() -> Support {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return Support::X11;
    }
    match wl_clipboard_rs::utils::is_primary_selection_supported() {
        Ok(_) => Support::DataControl,
        Err(e) => {
            debug!("No Wayland data-control clipboard: {}", e);
            Support::NoDataControl
        },
    }
}

/// The best clipboard access for this session, or `None` when there is
/// none.
pub fn open() -> Option<Box<dyn ClipboardBackend>> {
    let support = support();
    if support == Support::DataControl {
        return Some(Box::new(DataControl));
    }
    if support == Support::NoDataControl {
        warn!(
            "The compositor has no Wayland data-control protocol (GNOME has none); \
             after a paste, only text comes back on the clipboard"
        );
    }
    match TextOnly::new() {
        Ok(backend) => Some(Box::new(backend)),
        Err(e) => {
            warn!("Clipboard unavailable, paste fallback disabled: {:#}", e);
            None
        },
    }
}

/// Every format, through `ext-data-control-v1` or `wlr-data-control`.
pub struct DataControl;

impl DataControl {
    fn paste_type(selection: Selection) -> wl_clipboard_rs::paste::ClipboardType {
        match selection {
            Selection::Clipboard => wl_clipboard_rs::paste::ClipboardType::Regular,
            Selection::Primary => wl_clipboard_rs::paste::ClipboardType::Primary,
        }
    }

    fn copy_type(selection: Selection) -> wl_clipboard_rs::copy::ClipboardType {
        match selection {
            Selection::Clipboard => wl_clipboard_rs::copy::ClipboardType::Regular,
            Selection::Primary => wl_clipboard_rs::copy::ClipboardType::Primary,
        }
    }
}

impl ClipboardBackend for DataControl {
    fn describe(&self) -> &'static str {
        "every format through Wayland data-control"
    }

    fn carries_marker(&self) -> bool {
        true
    }

    fn offered(&mut self, selection: Selection) -> Result<Vec<String>> {
        use wl_clipboard_rs::paste;
        match paste::get_mime_types_ordered(Self::paste_type(selection), paste::Seat::Unspecified) {
            Ok(types) => Ok(types),
            Err(paste::Error::ClipboardEmpty) => Ok(Vec::new()),
            Err(e) => Err(e).with_context(|| format!("could not list the {} formats", selection)),
        }
    }

    fn read(
        &mut self,
        selection: Selection,
        mime: &str,
        limit: usize,
        deadline: Instant,
    ) -> std::result::Result<Vec<u8>, ReadError> {
        use wl_clipboard_rs::paste;
        let (pipe, _) = paste::get_contents(
            Self::paste_type(selection),
            paste::Seat::Unspecified,
            paste::MimeType::Specific(mime),
        )
        .map_err(|e| ReadError::Failed(anyhow!(e)))?;
        read_until(pipe, limit, deadline)
    }

    fn offer(&mut self, selection: Selection, formats: Vec<Format>) -> Result<()> {
        use wl_clipboard_rs::copy;
        let mut options = copy::Options::new();
        options
            .clipboard(Self::copy_type(selection))
            .omit_additional_text_mime_types(true);
        let sources = formats
            .into_iter()
            .map(|(mime, data)| copy::MimeSource {
                source: copy::Source::Bytes(data.into_boxed_slice()),
                mime_type: copy::MimeType::Specific(mime),
            })
            .collect();
        options
            .copy_multi(sources)
            .with_context(|| format!("could not set the {}", selection))
    }

    fn clear(&mut self, selection: Selection) -> Result<()> {
        use wl_clipboard_rs::copy;
        copy::clear(Self::copy_type(selection), copy::Seat::All)
            .with_context(|| format!("could not clear the {}", selection))
    }
}

/// Read a pipe to its end, but stop at `limit` bytes or at `deadline`.
fn read_until(
    mut pipe: impl Read + AsRawFd,
    limit: usize,
    deadline: Instant,
) -> std::result::Result<Vec<u8>, ReadError> {
    let mut data = Vec::new();
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() || !wait_readable(pipe.as_raw_fd(), remaining)? {
            return Err(ReadError::TimedOut);
        }
        match pipe.read(&mut chunk) {
            Ok(0) => return Ok(data),
            Ok(n) if data.len() + n > limit => return Err(ReadError::TooLarge),
            Ok(n) => data.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {},
            Err(e) => return Err(ReadError::Failed(anyhow!(e))),
        }
    }
}

/// Whether the descriptor has data or reached its end before the timeout.
fn wait_readable(fd: i32, timeout: Duration) -> std::result::Result<bool, ReadError> {
    let mut pollfd = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    let timeout_ms = i32::try_from(timeout.as_millis().max(1)).unwrap_or(i32::MAX);
    // SAFETY: pollfd is a valid, initialized struct and nfds is 1.
    let ready = unsafe { libc::poll(&mut pollfd, 1, timeout_ms) };
    if ready < 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() == std::io::ErrorKind::Interrupted {
            return Ok(true);
        }
        return Err(ReadError::Failed(anyhow!(err)));
    }
    Ok(ready > 0)
}

/// Text only, through arboard: X11, and Wayland sessions without
/// data-control, which arboard reaches through XWayland.
pub struct TextOnly {
    clipboard: arboard::Clipboard,
    /// Tells an empty selection from one that holds no text.
    x11: Option<x11rb::rust_connection::RustConnection>,
    cached: Option<(Selection, String)>,
}

impl TextOnly {
    pub fn new() -> Result<Self> {
        let clipboard = arboard::Clipboard::new().context("no clipboard is available")?;
        let x11 = x11rb::connect(None).ok().map(|(conn, _)| conn);
        Ok(Self {
            clipboard,
            x11,
            cached: None,
        })
    }

    fn kind(selection: Selection) -> arboard::LinuxClipboardKind {
        match selection {
            Selection::Clipboard => arboard::LinuxClipboardKind::Clipboard,
            Selection::Primary => arboard::LinuxClipboardKind::Primary,
        }
    }

    fn text(&mut self, selection: Selection) -> std::result::Result<String, arboard::Error> {
        use arboard::GetExtLinux;
        self.clipboard.get().clipboard(Self::kind(selection)).text()
    }

    /// Whether an X11 client owns the selection, or `None` when unknown.
    fn owned(&self, selection: Selection) -> Option<bool> {
        use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};
        let conn = self.x11.as_ref()?;
        let atom = match selection {
            Selection::Clipboard => {
                conn.intern_atom(false, b"CLIPBOARD")
                    .ok()?
                    .reply()
                    .ok()?
                    .atom
            },
            Selection::Primary => AtomEnum::PRIMARY.into(),
        };
        let owner = conn.get_selection_owner(atom).ok()?.reply().ok()?.owner;
        Some(owner != x11rb::NONE)
    }
}

impl ClipboardBackend for TextOnly {
    fn describe(&self) -> &'static str {
        "text only"
    }

    fn carries_marker(&self) -> bool {
        false
    }

    fn offered(&mut self, selection: Selection) -> Result<Vec<String>> {
        self.cached = None;
        match self.text(selection) {
            Ok(text) => {
                self.cached = Some((selection, text));
                Ok(vec![TEXT_MIMES[0].to_string()])
            },
            Err(arboard::Error::ContentNotAvailable) => match self.owned(selection) {
                Some(false) => Ok(Vec::new()),
                Some(true) => Err(anyhow!(
                    "the {} holds no text, and this session restores only text",
                    selection
                )),
                None => Err(anyhow!(
                    "the {} holds no text, and Hush cannot tell whether it is empty",
                    selection
                )),
            },
            Err(e) => Err(anyhow!(e)).with_context(|| format!("could not read the {}", selection)),
        }
    }

    fn read(
        &mut self,
        selection: Selection,
        mime: &str,
        limit: usize,
        deadline: Instant,
    ) -> std::result::Result<Vec<u8>, ReadError> {
        if !is_plain_text(mime) {
            return Err(ReadError::Failed(anyhow!("{} is not text", mime)));
        }
        let text = match self.cached.take() {
            Some((cached, text)) if cached == selection => text,
            _ => self
                .text(selection)
                .map_err(|e| ReadError::Failed(anyhow!(e)))?,
        };
        if text.len() > limit {
            return Err(ReadError::TooLarge);
        }
        if Instant::now() > deadline {
            return Err(ReadError::TimedOut);
        }
        Ok(text.into_bytes())
    }

    fn offer(&mut self, selection: Selection, formats: Vec<Format>) -> Result<()> {
        use arboard::SetExtLinux;
        self.cached = None;
        let private = formats.iter().any(|(mime, _)| mime == HISTORY_HINT_MIME);
        let (_, text) = formats
            .into_iter()
            .find(|(mime, _)| is_plain_text(mime))
            .ok_or_else(|| anyhow!("only text can go on the clipboard in this session"))?;
        let text = String::from_utf8(text).context("the text is not UTF-8")?;
        let set = self.clipboard.set().clipboard(Self::kind(selection));
        let set = if private {
            set.exclude_from_history()
        } else {
            set
        };
        set.text(text)
            .with_context(|| format!("could not set the {}", selection))
    }

    fn clear(&mut self, selection: Selection) -> Result<()> {
        use arboard::ClearExtLinux;
        self.cached = None;
        self.clipboard
            .clear_with()
            .clipboard(Self::kind(selection))
            .with_context(|| format!("could not clear the {}", selection))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::clock::fake::FakeClock;
    use std::collections::HashMap;
    use std::sync::Arc;

    /// An in-memory clipboard whose reads take time on a fake clock.
    struct Memory {
        selections: HashMap<Selection, Vec<Format>>,
        marker: bool,
        read_delay: Duration,
        clock: Arc<FakeClock>,
        reads: usize,
        fail_on: Option<String>,
    }

    impl Memory {
        fn new(clock: &Arc<FakeClock>) -> Self {
            Self {
                selections: HashMap::new(),
                marker: true,
                read_delay: Duration::ZERO,
                clock: Arc::clone(clock),
                reads: 0,
                fail_on: None,
            }
        }

        fn with(mut self, selection: Selection, formats: &[(&str, &[u8])]) -> Self {
            self.selections.insert(
                selection,
                formats
                    .iter()
                    .map(|(mime, data)| (mime.to_string(), data.to_vec()))
                    .collect(),
            );
            self
        }

        fn current(&self, selection: Selection) -> Option<&Vec<Format>> {
            self.selections.get(&selection)
        }
    }

    impl ClipboardBackend for Memory {
        fn describe(&self) -> &'static str {
            "memory"
        }

        fn carries_marker(&self) -> bool {
            self.marker
        }

        fn offered(&mut self, selection: Selection) -> Result<Vec<String>> {
            Ok(self
                .current(selection)
                .map(|formats| formats.iter().map(|(mime, _)| mime.clone()).collect())
                .unwrap_or_default())
        }

        fn read(
            &mut self,
            selection: Selection,
            mime: &str,
            limit: usize,
            deadline: Instant,
        ) -> std::result::Result<Vec<u8>, ReadError> {
            self.reads += 1;
            self.clock.advance(self.read_delay);
            if self.fail_on.as_deref() == Some(mime) {
                return Err(ReadError::Failed(anyhow!("broken pipe")));
            }
            if self.clock.now() > deadline {
                return Err(ReadError::TimedOut);
            }
            let data = self
                .current(selection)
                .and_then(|formats| formats.iter().find(|(m, _)| m == mime))
                .map(|(_, data)| data.clone())
                .ok_or_else(|| ReadError::Failed(anyhow!("no {}", mime)))?;
            if data.len() > limit {
                return Err(ReadError::TooLarge);
            }
            Ok(data)
        }

        fn offer(&mut self, selection: Selection, formats: Vec<Format>) -> Result<()> {
            self.selections.insert(selection, formats);
            Ok(())
        }

        fn clear(&mut self, selection: Selection) -> Result<()> {
            self.selections.remove(&selection);
            Ok(())
        }
    }

    const IMAGE: (&str, &[u8]) = ("image/png", b"\x89PNG data");
    const HTML: (&str, &[u8]) = ("text/html", b"<b>old</b>");
    const TEXT: (&str, &[u8]) = ("text/plain;charset=utf-8", b"old");

    #[test]
    fn every_format_is_saved_and_pseudo_targets_are_skipped() {
        let clock = Arc::new(FakeClock::new());
        let mut memory = Memory::new(&clock).with(
            Selection::Clipboard,
            &[IMAGE, ("TARGETS", b""), HTML, TEXT, HTML],
        );
        let snapshot = snapshot(&mut memory, Selection::Clipboard, clock.as_ref()).unwrap();
        let Snapshot::Formats(formats) = snapshot else {
            panic!("expected formats");
        };
        let mimes: Vec<&str> = formats.iter().map(|(mime, _)| mime.as_str()).collect();
        assert_eq!(
            mimes,
            ["image/png", "text/html", "text/plain;charset=utf-8"]
        );
        assert_eq!(formats[0].1, IMAGE.1);
    }

    #[test]
    fn an_empty_clipboard_is_a_snapshot_too() {
        let clock = Arc::new(FakeClock::new());
        let mut memory = Memory::new(&clock);
        assert_eq!(
            snapshot(&mut memory, Selection::Clipboard, clock.as_ref()),
            Ok(Snapshot::Empty)
        );
    }

    #[test]
    fn the_size_cap_counts_every_format() {
        let clock = Arc::new(FakeClock::new());
        let mut memory =
            Memory::new(&clock).with(Selection::Clipboard, &[("a", &[0; 6]), ("b", &[0; 6])]);
        let budget = Duration::from_secs(1);
        assert_eq!(
            snapshot_within(
                &mut memory,
                Selection::Clipboard,
                12,
                budget,
                clock.as_ref()
            ),
            Ok(Snapshot::Formats(vec![
                ("a".into(), vec![0; 6]),
                ("b".into(), vec![0; 6])
            ]))
        );
        assert_eq!(
            snapshot_within(
                &mut memory,
                Selection::Clipboard,
                11,
                budget,
                clock.as_ref()
            ),
            Err(NoRestore::TooLarge)
        );
    }

    #[test]
    fn the_time_cap_covers_all_reads() {
        let clock = Arc::new(FakeClock::new());
        let mut memory = Memory::new(&clock).with(Selection::Clipboard, &[IMAGE, HTML, TEXT]);
        memory.read_delay = Duration::from_millis(400);
        assert_eq!(
            snapshot(&mut memory, Selection::Clipboard, clock.as_ref()),
            Err(NoRestore::TooSlow)
        );
        assert_eq!(memory.reads, 3, "the third read passes the budget");
    }

    #[test]
    fn a_failed_read_means_no_restore() {
        let clock = Arc::new(FakeClock::new());
        let mut memory = Memory::new(&clock).with(Selection::Clipboard, &[IMAGE, TEXT]);
        memory.fail_on = Some("image/png".into());
        assert!(matches!(
            snapshot(&mut memory, Selection::Clipboard, clock.as_ref()),
            Err(NoRestore::Unreadable(why)) if why.contains("image/png")
        ));
    }

    #[test]
    fn the_transcript_carries_the_history_hint() {
        let transcript = Transcript::new("hello");
        let formats = transcript.formats();
        assert!(formats
            .iter()
            .any(|(mime, data)| mime == HISTORY_HINT_MIME && data == b"secret"));
        assert!(formats
            .iter()
            .any(|(mime, data)| mime == "text/plain;charset=utf-8" && data == b"hello"));
        assert_ne!(Transcript::new("hello").token, transcript.token);
    }

    #[test]
    fn the_still_ours_check_uses_the_marker() {
        let clock = Arc::new(FakeClock::new());
        let mut memory = Memory::new(&clock);
        let transcript = Transcript::new("hello");
        memory
            .offer(Selection::Clipboard, transcript.formats())
            .unwrap();
        assert!(holds(
            &mut memory,
            Selection::Clipboard,
            &transcript,
            clock.as_ref()
        )
        .unwrap());

        let earlier = Transcript::new("hello");
        assert!(
            !holds(&mut memory, Selection::Clipboard, &earlier, clock.as_ref()).unwrap(),
            "another paste's token is not this one"
        );

        memory
            .offer(Selection::Clipboard, text_formats("hello"))
            .unwrap();
        assert!(
            !holds(
                &mut memory,
                Selection::Clipboard,
                &transcript,
                clock.as_ref()
            )
            .unwrap(),
            "the user copied the same text"
        );
    }

    #[test]
    fn the_still_ours_check_compares_text_without_a_marker() {
        let clock = Arc::new(FakeClock::new());
        let mut memory = Memory::new(&clock);
        memory.marker = false;
        let transcript = Transcript::new("hello");
        memory
            .offer(Selection::Clipboard, text_formats("hello"))
            .unwrap();
        assert!(holds(
            &mut memory,
            Selection::Clipboard,
            &transcript,
            clock.as_ref()
        )
        .unwrap());
        memory
            .offer(Selection::Clipboard, text_formats("hello world"))
            .unwrap();
        assert!(!holds(
            &mut memory,
            Selection::Clipboard,
            &transcript,
            clock.as_ref()
        )
        .unwrap());
        memory
            .offer(Selection::Clipboard, vec![("image/png".into(), vec![1])])
            .unwrap();
        assert!(!holds(
            &mut memory,
            Selection::Clipboard,
            &transcript,
            clock.as_ref()
        )
        .unwrap());
    }

    #[test]
    fn a_paste_restores_every_format() {
        let clock = Arc::new(FakeClock::new());
        let mut memory = Memory::new(&clock).with(Selection::Clipboard, &[IMAGE, HTML]);
        let saved = snapshot(&mut memory, Selection::Clipboard, clock.as_ref());
        let transcript = Transcript::new("hello");
        memory
            .offer(Selection::Clipboard, transcript.formats())
            .unwrap();
        let outcome = restore(
            &mut memory,
            Selection::Clipboard,
            saved,
            &transcript,
            clock.as_ref(),
        );
        assert_eq!(outcome, Restore::Restored);
        assert_eq!(
            memory.current(Selection::Clipboard),
            Some(&vec![
                (IMAGE.0.to_string(), IMAGE.1.to_vec()),
                (HTML.0.to_string(), HTML.1.to_vec())
            ])
        );
    }

    #[test]
    fn an_empty_clipboard_is_cleared_again() {
        let clock = Arc::new(FakeClock::new());
        let mut memory = Memory::new(&clock);
        let saved = snapshot(&mut memory, Selection::Clipboard, clock.as_ref());
        let transcript = Transcript::new("hello");
        memory
            .offer(Selection::Clipboard, transcript.formats())
            .unwrap();
        let outcome = restore(
            &mut memory,
            Selection::Clipboard,
            saved,
            &transcript,
            clock.as_ref(),
        );
        assert_eq!(outcome, Restore::Cleared);
        assert_eq!(memory.current(Selection::Clipboard), None);
    }

    #[test]
    fn a_copy_during_the_wait_is_kept() {
        let clock = Arc::new(FakeClock::new());
        let mut memory = Memory::new(&clock).with(Selection::Clipboard, &[TEXT]);
        let saved = snapshot(&mut memory, Selection::Clipboard, clock.as_ref());
        let transcript = Transcript::new("hello");
        memory
            .offer(Selection::Clipboard, transcript.formats())
            .unwrap();
        memory
            .offer(Selection::Clipboard, text_formats("new copy"))
            .unwrap();
        let outcome = restore(
            &mut memory,
            Selection::Clipboard,
            saved,
            &transcript,
            clock.as_ref(),
        );
        assert_eq!(outcome, Restore::Replaced);
        assert_eq!(
            memory.current(Selection::Clipboard),
            Some(&text_formats("new copy"))
        );
    }

    #[test]
    fn a_failed_snapshot_leaves_the_transcript() {
        let clock = Arc::new(FakeClock::new());
        let mut memory = Memory::new(&clock);
        let transcript = Transcript::new("hello");
        memory
            .offer(Selection::Clipboard, transcript.formats())
            .unwrap();
        let outcome = restore(
            &mut memory,
            Selection::Clipboard,
            Err(NoRestore::TooLarge),
            &transcript,
            clock.as_ref(),
        );
        assert_eq!(outcome, Restore::Kept(NoRestore::TooLarge));
        assert_eq!(
            memory.current(Selection::Clipboard),
            Some(&transcript.formats())
        );
    }
}
