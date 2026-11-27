//! The keymap of the Wayland seat, for a sandbox that cannot read the
//! layout settings. The compositor sends the keymap to each client right
//! after `get_keyboard`, before any surface has the keyboard focus.

use crate::text::keymap::KeyTable;
use crate::text::xkb;
use anyhow::{anyhow, bail, Context};
use std::fs::File;
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileExt;
use std::sync::Arc;
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{wl_keyboard, wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum};

/// A full XKB keymap is about 64 KiB, so a larger size is an error.
const MAX_KEYMAP_BYTES: u32 = 16 << 20;

/// The key table of the seat's keymap, kept until the keymap changes.
#[derive(Default)]
pub struct SeatKeymap {
    last: Option<(String, Result<Arc<KeyTable>, String>)>,
}

impl SeatKeymap {
    pub fn new() -> Self {
        Self::default()
    }

    /// The table for the keymap that the compositor sends now, or why
    /// there is none.
    pub fn table(&mut self) -> Result<Arc<KeyTable>, String> {
        let text = read().map_err(|e| format!("{:#}", e))?;
        self.table_for(text)
    }

    fn table_for(&mut self, text: String) -> Result<Arc<KeyTable>, String> {
        if let Some((known, table)) = &self.last {
            if *known == text {
                return table.clone();
            }
        }
        let table = xkb::build_from_text(&text)
            .map(Arc::new)
            .map_err(|e| format!("{:#}", e));
        self.last = Some((text, table.clone()));
        table
    }
}

/// The XKB keymap text of the first seat, which must have a keyboard.
fn read() -> anyhow::Result<String> {
    let connection = Connection::connect_to_env().context("no Wayland display")?;
    let (globals, mut queue) =
        registry_queue_init::<Reader>(&connection).context("no Wayland registry")?;
    let handle = queue.handle();
    // Version 1 has get_keyboard and the keymap event.
    let seat: wl_seat::WlSeat = globals
        .bind(&handle, 1..=1, ())
        .context("no Wayland seat")?;
    let mut reader = Reader::default();
    queue
        .roundtrip(&mut reader)
        .context("the Wayland seat did not answer")?;
    // get_keyboard on a seat that never had a keyboard is a protocol error.
    if !reader.keyboard {
        bail!("the Wayland seat has no keyboard");
    }
    seat.get_keyboard(&handle, ());
    queue
        .roundtrip(&mut reader)
        .context("the Wayland keyboard did not answer")?;
    reader
        .keymap
        .unwrap_or_else(|| Err(anyhow!("the compositor sent no keymap")))
}

#[derive(Default)]
struct Reader {
    keyboard: bool,
    keymap: Option<anyhow::Result<String>>,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Reader {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for Reader {
    fn event(
        state: &mut Self,
        _: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities { capabilities } = event {
            state.keyboard = wl_seat::Capability::from_bits_truncate(capabilities.into())
                .contains(wl_seat::Capability::Keyboard);
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for Reader {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_keyboard::Event::Keymap { format, fd, size } = event {
            state.keymap = Some(match format {
                WEnum::Value(wl_keyboard::KeymapFormat::XkbV1) => keymap_text(fd, size),
                _ => Err(anyhow!("the compositor sent no XKB keymap")),
            });
        }
    }
}

/// The NUL-terminated keymap at the start of the file. The read leaves the
/// file offset as it is, because the compositor can give the same open
/// file to each client.
fn keymap_text(fd: OwnedFd, size: u32) -> anyhow::Result<String> {
    if size > MAX_KEYMAP_BYTES {
        bail!("the compositor sent a keymap of {} bytes", size);
    }
    let mut bytes = vec![0; usize::try_from(size)?];
    File::from(fd)
        .read_exact_at(&mut bytes, 0)
        .context("cannot read the keymap")?;
    if let Some(end) = bytes.iter().position(|&byte| byte == 0) {
        bytes.truncate(end);
    }
    String::from_utf8(bytes).context("the keymap is not UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::xkb::tests::{small_keymap, SMALL_TWO_LAYOUTS, SMALL_US};
    use std::io::{Seek, Write};

    fn keymap_file(bytes: &[u8]) -> (File, OwnedFd) {
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(bytes).unwrap();
        file.rewind().unwrap();
        let fd = OwnedFd::from(file.try_clone().unwrap());
        (file, fd)
    }

    #[test]
    fn the_keymap_ends_at_its_nul_and_leaves_the_offset() {
        let text = b"xkb_keymap { };\0";
        let (mut file, fd) = keymap_file(text);
        assert_eq!(keymap_text(fd, 16).unwrap(), "xkb_keymap { };");
        assert_eq!(
            file.stream_position().unwrap(),
            0,
            "another client reads the same open file"
        );
        let (_, fd) = keymap_file(b"no nul");
        assert_eq!(keymap_text(fd, 6).unwrap(), "no nul");
    }

    #[test]
    fn a_short_or_huge_keymap_is_an_error() {
        let (_, fd) = keymap_file(b"xkb_keymap\0");
        assert!(keymap_text(fd, 64).is_err(), "the file is shorter");
        let (_, fd) = keymap_file(b"xkb_keymap\0");
        let error = keymap_text(fd, MAX_KEYMAP_BYTES + 1).unwrap_err();
        assert!(error.to_string().contains("bytes"), "{error}");
        let (_, fd) = keymap_file(b"\xff\xfe\0");
        assert!(keymap_text(fd, 3).is_err(), "not UTF-8");
    }

    #[test]
    #[ignore = "needs a Wayland session"]
    fn the_compositor_sends_its_keymap() {
        let text = read().expect("the compositor sends a keymap");
        assert!(text.contains("xkb_symbols"), "{text}");
        match xkb::build_from_text(&text) {
            Ok(table) => assert!(table.taps(' ').is_some(), "every layout has a space"),
            Err(e) => assert!(e.to_string().contains("layouts"), "{e:#}"),
        }
    }

    #[test]
    fn a_keymap_compiles_once_until_it_changes() {
        if !xkb::available() {
            eprintln!("Skipping the seat keymap test: libxkbcommon does not load");
            return;
        }
        let mut seat = SeatKeymap::new();
        let us = small_keymap(SMALL_US);
        let first = seat.table_for(us.clone()).unwrap();
        let again = seat.table_for(us).unwrap();
        assert!(Arc::ptr_eq(&first, &again), "the same keymap");
        let two = small_keymap(SMALL_TWO_LAYOUTS);
        let error = seat.table_for(two.clone()).unwrap_err();
        assert!(error.contains("2 layouts"), "{error}");
        assert_eq!(seat.table_for(two), Err(error), "a failure is kept too");
        let one = small_keymap("key <AC01> { [ q, Q ] };");
        let changed = seat.table_for(one).unwrap();
        assert_eq!(changed.taps('q').map(<[_]>::len), Some(1));
        assert_eq!(changed.taps('a'), None, "the new keymap");
    }
}
