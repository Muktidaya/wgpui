use crate::{ClipboardItem, Image, ImageFormat};
use anyhow::{Context as _, Result, anyhow, bail, ensure};
use parking_lot::{Condvar, Mutex};
use std::{
    os::fd::AsRawFd as _,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use strum::IntoEnumIterator as _;
use x11rb::{
    COPY_DEPTH_FROM_PARENT, COPY_FROM_PARENT, NONE,
    connection::{Connection as _, RequestConnection as _},
    protocol::{
        Event,
        xproto::{
            Atom, AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, PropMode, Property,
            SELECTION_NOTIFY_EVENT, SelectionNotifyEvent, SelectionRequestEvent, WindowClass,
        },
    },
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
};

x11rb::atom_manager! {
    Atoms: AtomCookies {
        CLIPBOARD, CLIPBOARD_MANAGER, SAVE_TARGETS, TARGETS, INCR, UTF8_STRING,
        TEXT_UTF8: b"text/plain;charset=utf-8",
        TEXT_UTF8_UPPER: b"text/plain;charset=UTF-8",
        STRING, TEXT, TEXT_PLAIN: b"text/plain",
        WGPUI_SELECTION,
    }
}

const READ_TIMEOUT: Duration = Duration::from_secs(4);

pub(super) struct X11Clipboard {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}

struct Shared {
    connection: RustConnection,
    window: u32,
    atoms: Atoms,
    item: Mutex<Option<ClipboardItem>>,
    stopped: AtomicBool,
    handover: Mutex<Handover>,
    handover_done: Condvar,
}

#[derive(Default)]
struct Handover {
    active: bool,
    written: bool,
    notified: bool,
}

impl X11Clipboard {
    pub(super) fn new() -> Result<Self> {
        let (connection, window) = open_connection()?;
        let atoms = Atoms::new(&connection)?.reply()?;
        let shared = Arc::new(Shared {
            connection,
            window,
            atoms,
            item: Mutex::new(None),
            stopped: AtomicBool::new(false),
            handover: Mutex::new(Handover::default()),
            handover_done: Condvar::new(),
        });
        let worker = std::thread::Builder::new()
            .name("WGPUI X11 clipboard".into())
            .spawn({
                let shared = shared.clone();
                move || {
                    if let Err(error) = serve_selection(&shared) {
                        log::error!("X11 clipboard worker failed: {error:#}");
                    }
                    shared.stopped.store(true, Ordering::Release);
                    shared.handover_done.notify_all();
                }
            })?;
        Ok(Self {
            shared,
            worker: Some(worker),
        })
    }

    pub(super) fn write(&self, item: ClipboardItem) -> Result<()> {
        ensure!(
            !self.shared.stopped.load(Ordering::Acquire),
            "X11 clipboard worker stopped"
        );
        let text = item.text().unwrap_or_default();
        ensure!(
            text.len()
                <= self
                    .shared
                    .connection
                    .maximum_request_bytes()
                    .saturating_sub(64),
            "X11 clipboard text exceeds the server's single-property transfer limit"
        );
        let mut current = self.shared.item.lock();
        self.shared
            .connection
            .set_selection_owner(
                self.shared.window,
                self.shared.atoms.CLIPBOARD,
                x11rb::CURRENT_TIME,
            )?
            .check()?;
        let owner = self
            .shared
            .connection
            .get_selection_owner(self.shared.atoms.CLIPBOARD)?
            .reply()?
            .owner;
        ensure!(
            owner == self.shared.window,
            "X11 refused clipboard ownership"
        );
        *current = Some(item);
        Ok(())
    }

    pub(super) fn read(&self) -> Result<Option<ClipboardItem>> {
        let current = self.shared.item.lock();
        let owner = self
            .shared
            .connection
            .get_selection_owner(self.shared.atoms.CLIPBOARD)?
            .reply()?
            .owner;
        if owner == self.shared.window {
            ensure!(
                !self.shared.stopped.load(Ordering::Acquire),
                "X11 clipboard worker stopped"
            );
            return Ok(current.clone());
        }
        drop(current);
        if owner == NONE {
            return Ok(None);
        }
        read_external()
    }
}

impl Drop for X11Clipboard {
    fn drop(&mut self) {
        // The selection owner must remain live while a clipboard manager fetches data before exit.
        shutdown(&self.shared, self.worker.take());
    }
}

fn shutdown(shared: &Shared, worker: Option<JoinHandle<()>>) {
    if let Err(error) = hand_to_manager(shared) {
        log::warn!("X11 clipboard manager handover failed: {error:#}");
    }
    let stopped = shared
        .connection
        .destroy_window(shared.window)
        .map(|cookie| cookie.check())
        .map_err(anyhow::Error::from)
        .and_then(|result| result.map_err(anyhow::Error::from));
    match stopped {
        Ok(()) => {
            if let Some(worker) = worker {
                if worker.join().is_err() {
                    log::error!("X11 clipboard worker panicked during shutdown");
                }
            }
        }
        Err(error) => log::error!("X11 clipboard shutdown failed: {error:#}"),
    }
}

fn hand_to_manager(shared: &Shared) -> Result<()> {
    if shared
        .connection
        .get_selection_owner(shared.atoms.CLIPBOARD)?
        .reply()?
        .owner
        != shared.window
        || shared
            .connection
            .get_selection_owner(shared.atoms.CLIPBOARD_MANAGER)?
            .reply()?
            .owner
            == NONE
    {
        return Ok(());
    }
    let mut handover = shared.handover.lock();
    *handover = Handover {
        active: true,
        ..Handover::default()
    };
    shared
        .connection
        .convert_selection(
            shared.window,
            shared.atoms.CLIPBOARD_MANAGER,
            shared.atoms.SAVE_TARGETS,
            NONE,
            x11rb::CURRENT_TIME,
        )?
        .check()?;
    shared.connection.flush()?;
    let deadline = Instant::now() + READ_TIMEOUT;
    while !(handover.written && handover.notified) {
        ensure!(
            !shared.stopped.load(Ordering::Acquire),
            "X11 clipboard worker stopped during handover"
        );
        ensure!(
            Instant::now() < deadline,
            "X11 clipboard manager handover timed out"
        );
        shared.handover_done.wait_until(&mut handover, deadline);
    }
    Ok(())
}

fn open_connection() -> Result<(RustConnection, u32)> {
    let (connection, screen_index) = RustConnection::connect(None)?;
    let screen = connection
        .setup()
        .roots
        .get(screen_index)
        .ok_or_else(|| anyhow!("X11 did not return the requested screen"))?;
    let window = connection.generate_id()?;
    connection
        .create_window(
            COPY_DEPTH_FROM_PARENT,
            window,
            screen.root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            COPY_FROM_PARENT,
            &CreateWindowAux::new()
                .event_mask(EventMask::PROPERTY_CHANGE | EventMask::STRUCTURE_NOTIFY),
        )?
        .check()?;
    Ok((connection, window))
}

fn serve_selection(shared: &Shared) -> Result<()> {
    loop {
        match shared.connection.wait_for_event()? {
            Event::DestroyNotify(event) if event.window == shared.window => return Ok(()),
            Event::SelectionClear(event) if event.selection == shared.atoms.CLIPBOARD => {
                let mut current = shared.item.lock();
                // A queued loss event can arrive after a later copy regained ownership.
                if shared
                    .connection
                    .get_selection_owner(shared.atoms.CLIPBOARD)?
                    .reply()?
                    .owner
                    != shared.window
                {
                    *current = None;
                }
            }
            Event::SelectionRequest(event) if event.selection == shared.atoms.CLIPBOARD => {
                if let Err(error) = reply_to_request(shared, event) {
                    log::warn!("X11 clipboard selection request failed: {error:#}");
                }
            }
            Event::SelectionNotify(event) if event.selection == shared.atoms.CLIPBOARD_MANAGER => {
                let mut handover = shared.handover.lock();
                handover.notified = true;
                shared.handover_done.notify_all();
            }
            _ => {}
        }
    }
}

fn reply_to_request(shared: &Shared, event: SelectionRequestEvent) -> Result<()> {
    let property = if event.property == NONE {
        event.target
    } else {
        event.property
    };
    let formats = [
        shared.atoms.UTF8_STRING,
        shared.atoms.TEXT_UTF8,
        shared.atoms.TEXT_UTF8_UPPER,
    ];
    let write_result = (|| -> Result<bool> {
        let current = shared.item.lock();
        let Some(item) = current.as_ref() else {
            return Ok(false);
        };
        if event.target == shared.atoms.TARGETS {
            let mut targets = formats.to_vec();
            targets.push(shared.atoms.TARGETS);
            targets.push(shared.atoms.SAVE_TARGETS);
            shared
                .connection
                .change_property32(
                    PropMode::REPLACE,
                    event.requestor,
                    property,
                    AtomEnum::ATOM,
                    &targets,
                )?
                .check()?;
            return Ok(true);
        }
        if formats.contains(&event.target) {
            let text = item.text().unwrap_or_default();
            shared
                .connection
                .change_property8(
                    PropMode::REPLACE,
                    event.requestor,
                    property,
                    event.target,
                    text.as_bytes(),
                )?
                .check()?;
            return Ok(true);
        }
        Ok(false)
    })();
    let success = match write_result {
        Ok(success) => success,
        Err(error) => {
            log::warn!("X11 clipboard conversion failed: {error:#}");
            false
        }
    };
    shared
        .connection
        .send_event(
            false,
            event.requestor,
            EventMask::NO_EVENT,
            SelectionNotifyEvent {
                response_type: SELECTION_NOTIFY_EVENT,
                sequence: 0,
                time: event.time,
                requestor: event.requestor,
                selection: event.selection,
                target: event.target,
                property: if success { property } else { NONE },
            },
        )?
        .check()?;
    shared.connection.flush()?;
    if success && event.target != shared.atoms.TARGETS {
        let mut handover = shared.handover.lock();
        if handover.active {
            handover.written = true;
            shared.handover_done.notify_all();
        }
    }
    Ok(())
}

fn read_external() -> Result<Option<ClipboardItem>> {
    let (connection, window) = open_connection()?;
    let atoms = Atoms::new(&connection)?.reply()?;
    let owner = connection
        .get_selection_owner(atoms.CLIPBOARD)?
        .reply()?
        .owner;
    if owner == NONE {
        return Ok(None);
    }
    let targets = request(&connection, window, &atoms, atoms.TARGETS)?;
    let Some(targets) = targets else {
        return Ok(None);
    };
    ensure!(
        targets.len() % 4 == 0,
        "X11 TARGETS payload has an invalid size"
    );
    let targets = targets
        .chunks_exact(4)
        .map(|bytes| u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        .collect::<Vec<_>>();
    for format in ImageFormat::iter() {
        let target = connection
            .intern_atom(false, format.mime_type().as_bytes())?
            .reply()?
            .atom;
        if targets.contains(&target) {
            if let Some(bytes) = request(&connection, window, &atoms, target)? {
                ensure!(
                    connection
                        .get_selection_owner(atoms.CLIPBOARD)?
                        .reply()?
                        .owner
                        == owner,
                    "X11 clipboard ownership changed while reading an image"
                );
                return Ok(Some(ClipboardItem::new_image(&Image::from_bytes(
                    format, bytes,
                ))));
            }
        }
    }
    for target in [
        atoms.UTF8_STRING,
        atoms.TEXT_UTF8,
        atoms.TEXT_UTF8_UPPER,
        atoms.STRING,
        atoms.TEXT,
        atoms.TEXT_PLAIN,
    ] {
        if !targets.contains(&target) {
            continue;
        }
        if let Some(bytes) = request(&connection, window, &atoms, target)? {
            let text = if target == atoms.STRING {
                bytes.into_iter().map(char::from).collect()
            } else {
                String::from_utf8(bytes).context("X11 clipboard text is not UTF-8")?
            };
            ensure!(
                connection
                    .get_selection_owner(atoms.CLIPBOARD)?
                    .reply()?
                    .owner
                    == owner,
                "X11 clipboard ownership changed while reading text"
            );
            return Ok(Some(ClipboardItem::new_string(text)));
        }
    }
    Ok(None)
}

fn request(
    connection: &RustConnection,
    window: u32,
    atoms: &Atoms,
    target: Atom,
) -> Result<Option<Vec<u8>>> {
    connection
        .delete_property(window, atoms.WGPUI_SELECTION)?
        .check()?;
    connection
        .convert_selection(
            window,
            atoms.CLIPBOARD,
            target,
            atoms.WGPUI_SELECTION,
            x11rb::CURRENT_TIME,
        )?
        .check()?;
    connection.flush()?;
    let deadline = Instant::now() + READ_TIMEOUT;
    let mut incremental = false;
    let mut bytes = Vec::new();
    loop {
        ensure!(Instant::now() < deadline, "X11 clipboard read timed out");
        let event = match connection.poll_for_event()? {
            Some(event) => event,
            None => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                ensure!(!remaining.is_zero(), "X11 clipboard read timed out");
                let mut descriptor = libc::pollfd {
                    fd: connection.stream().as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                };
                let timeout = i32::try_from(remaining.as_millis())
                    .unwrap_or(i32::MAX)
                    .max(1);
                let status = unsafe { libc::poll(&mut descriptor, 1, timeout) };
                if status < 0 {
                    let error = std::io::Error::last_os_error();
                    if error.kind() == std::io::ErrorKind::Interrupted {
                        continue;
                    }
                    return Err(error.into());
                }
                continue;
            }
        };
        match event {
            Event::SelectionNotify(event)
                if event.requestor == window && event.target == target =>
            {
                if event.property == NONE {
                    return Ok(None);
                }
                let reply = connection
                    .get_property(
                        true,
                        window,
                        atoms.WGPUI_SELECTION,
                        AtomEnum::ANY,
                        0,
                        u32::MAX,
                    )?
                    .reply()?;
                if reply.type_ == atoms.INCR {
                    incremental = true;
                    connection
                        .delete_property(window, atoms.WGPUI_SELECTION)?
                        .check()?;
                    connection.flush()?;
                } else {
                    return Ok(Some(reply.value));
                }
            }
            Event::PropertyNotify(event)
                if incremental
                    && event.window == window
                    && event.atom == atoms.WGPUI_SELECTION
                    && event.state == Property::NEW_VALUE =>
            {
                let reply = connection
                    .get_property(
                        true,
                        window,
                        atoms.WGPUI_SELECTION,
                        AtomEnum::ANY,
                        0,
                        u32::MAX,
                    )?
                    .reply()?;
                if reply.value.is_empty() {
                    return Ok(Some(bytes));
                }
                bytes.try_reserve(reply.value.len())?;
                bytes.extend_from_slice(&reply.value);
                connection.flush()?;
            }
            Event::Error(error) => bail!("X11 clipboard protocol error: {error:?}"),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClipboardEntry, ClipboardString};

    #[test]
    #[ignore = "requires an X11 display and selection exchange"]
    fn external_ownership_invalidates_full_cached_payload_even_with_identical_text() -> Result<()> {
        let first = X11Clipboard::new()?;
        let second = X11Clipboard::new()?;
        let item = ClipboardItem {
            entries: vec![
                ClipboardEntry::Image(Image::empty()),
                ClipboardEntry::String(ClipboardString {
                    text: "same text".into(),
                    metadata: Some("private selection metadata".into()),
                }),
            ],
        };
        first.write(item.clone())?;
        assert_eq!(first.read()?, Some(item));
        second.write(ClipboardItem::new_string("same text".into()))?;
        assert_eq!(
            first.read()?,
            Some(ClipboardItem::new_string("same text".into()))
        );
        first.write(ClipboardItem::new_string(String::new()))?;
        assert_eq!(
            second.read()?,
            Some(ClipboardItem::new_string(String::new()))
        );
        Ok(())
    }
}
