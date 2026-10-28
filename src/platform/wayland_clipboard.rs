// The ordinary-seat connection and pipe dispatch follow smithay-clipboard 0.7.3.
// Copyright (c) 2018 Lucas Timmins & Victor Berger. MIT license; see clipboard_licenses.txt.

use crate::{ClipboardItem, Image, ImageFormat};
use anyhow::{Result, anyhow, ensure};
use parking_lot::Mutex;
use raw_window_handle::HasDisplayHandle as _;
use sctk::reexports::{
    calloop::{EventLoop, LoopHandle, PostAction, RegistrationToken, channel},
    calloop_wayland_source::WaylandSource,
};
use sctk::{
    data_device_manager::{
        DataDeviceManagerState, WritePipe,
        data_device::{DataDevice, DataDeviceHandler},
        data_offer::{DataOfferHandler, DragOffer},
        data_source::{CopyPasteSource, DataSourceHandler},
    },
    dispatch2::Dispatch2,
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        pointer::{PointerData, PointerEvent, PointerEventKind, PointerHandler},
    },
};
use smithay_client_toolkit as sctk;
use std::{
    collections::HashMap,
    io::{Read as _, Write as _},
    os::fd::{AsRawFd as _, RawFd},
    sync::{Arc, mpsc},
    thread::JoinHandle,
    time::{Duration, Instant},
};
use strum::IntoEnumIterator as _;
use wayland_client::{
    Connection, Proxy as _, QueueHandle,
    backend::{Backend, ObjectId},
    globals::registry_queue_init,
    protocol::{
        wl_data_device::WlDataDevice,
        wl_data_device_manager::DndAction,
        wl_data_source::WlDataSource,
        wl_keyboard::{self, WlKeyboard},
        wl_pointer::WlPointer,
        wl_seat::WlSeat,
        wl_surface::WlSurface,
    },
};

const TEXT_FORMATS: [&str; 3] = ["text/plain;charset=utf-8", "UTF8_STRING", "text/plain"];
const READ_TIMEOUT: Duration = Duration::from_secs(4);
type ReadReply = mpsc::SyncSender<Result<Option<ClipboardItem>>>;

pub(super) struct WaylandClipboard {
    commands: channel::Sender<Command>,
    worker: Option<JoinHandle<()>>,
}

// The borrowed wl_display remains live until the queue and every connection clone are dropped.
struct ForeignConnection {
    connection: Connection,
    _display: winit::event_loop::OwnedDisplayHandle,
}

enum Command {
    Store(ClipboardItem),
    Load(ReadReply),
    Exit,
}

impl WaylandClipboard {
    pub(super) fn for_display(
        display: winit::event_loop::OwnedDisplayHandle,
    ) -> Result<Option<Self>> {
        let raw_window_handle::RawDisplayHandle::Wayland(handle) =
            display.display_handle()?.as_raw()
        else {
            return Ok(None);
        };
        let backend = unsafe { Backend::from_foreign_display(handle.display.as_ptr().cast()) };
        let connection = Connection::from_backend(backend);
        let foreign = ForeignConnection {
            connection,
            _display: display,
        };
        let (commands, receiver) = channel::channel();
        let (initialized, initialization) = mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("WGPUI Wayland clipboard".into())
            .spawn(move || {
                if let Err(error) = run_worker(foreign, receiver, initialized) {
                    log::error!("Wayland clipboard worker failed: {error:#}");
                }
            })?;
        let clipboard = Self {
            commands,
            worker: Some(worker),
        };
        initialization
            .recv_timeout(READ_TIMEOUT)
            .map_err(|error| {
                anyhow!("Wayland clipboard initialization did not finish: {error}")
            })??;
        Ok(Some(clipboard))
    }

    pub(super) fn write(&self, item: ClipboardItem) -> Result<()> {
        // This reports enqueue failure. Protocol ownership has no compositor acknowledgement.
        self.commands
            .send(Command::Store(item))
            .map_err(|_| anyhow!("Wayland clipboard worker stopped"))
    }

    pub(super) fn read(&self) -> Result<Option<ClipboardItem>> {
        let (reply, receiver) = mpsc::sync_channel(1);
        self.commands
            .send(Command::Load(reply))
            .map_err(|_| anyhow!("Wayland clipboard worker stopped"))?;
        receiver
            .recv_timeout(READ_TIMEOUT + Duration::from_secs(1))
            .map_err(|error| anyhow!("Wayland clipboard read did not finish: {error}"))?
    }
}

impl Drop for WaylandClipboard {
    fn drop(&mut self) {
        if self.commands.send(Command::Exit).is_err() {
            log::debug!("Wayland clipboard worker was already stopped");
        }
        if let Some(worker) = self.worker.take() {
            if worker.is_finished() {
                if worker.join().is_err() {
                    log::error!("Wayland clipboard worker panicked during shutdown");
                }
            } else {
                log::debug!("Wayland clipboard worker retains its display while shutting down");
            }
        }
    }
}

struct State {
    registry: RegistryState,
    seat_state: SeatState,
    manager: DataDeviceManagerState,
    seats: HashMap<ObjectId, Seat>,
    latest_seat: Option<ObjectId>,
    sources: HashMap<ObjectId, Source>,
    queue: QueueHandle<Self>,
    handle: LoopHandle<'static, Self>,
    pending_reads: Vec<PendingRead>,
    exit: bool,
}

#[derive(Default)]
struct Seat {
    keyboard: Option<WlKeyboard>,
    pointer: Option<WlPointer>,
    device: Option<DataDevice>,
    focused: bool,
    serial: Option<u32>,
}

struct KeyboardSeat(ObjectId);
struct Source {
    _source: CopyPasteSource,
    owner_mime: String,
    item: ClipboardItem,
    text: Arc<[u8]>,
}
struct PendingRead {
    token: RegistrationToken,
    deadline: Instant,
    reply: Arc<Mutex<Option<ReadReply>>>,
}

fn run_worker(
    foreign: ForeignConnection,
    commands: channel::Channel<Command>,
    initialized: mpsc::SyncSender<Result<()>>,
) -> Result<()> {
    // Leave the display guard in the argument so it outlives the queue and connection locals.
    let connection = foreign.connection;
    let setup = (|| -> Result<_> {
        let (globals, queue) = registry_queue_init::<State>(&connection)?;
        let event_loop: EventLoop<State> = EventLoop::try_new()?;
        let handle = event_loop.handle();
        let queue_handle = queue.handle();
        let seat_state = SeatState::new(&globals, &queue_handle);
        #[allow(
            clippy::mutable_key_type,
            reason = "ObjectId hashes immutable object identity, never its atomic liveness value"
        )]
        let seats = seat_state
            .seats()
            .map(|seat| (seat.id(), Seat::default()))
            .collect();
        let state = State {
            registry: RegistryState::new(&globals),
            seat_state,
            manager: DataDeviceManagerState::bind(&globals, &queue_handle)?,
            seats,
            latest_seat: None,
            sources: HashMap::new(),
            queue: queue_handle,
            handle: handle.clone(),
            pending_reads: Vec::new(),
            exit: false,
        };
        handle
            .insert_source(commands, |event, _, state| match event {
                channel::Event::Msg(Command::Store(item)) => {
                    if let Err(error) = state.store(item) {
                        log::warn!("Wayland clipboard store failed: {error:#}");
                    }
                }
                channel::Event::Msg(Command::Load(reply)) => state.load(reply),
                channel::Event::Msg(Command::Exit) | channel::Event::Closed => state.exit = true,
            })
            .map_err(|error| {
                anyhow!(
                    "Wayland clipboard channel registration failed: {}",
                    error.error
                )
            })?;
        WaylandSource::new(connection.clone(), queue).insert(handle)?;
        Ok((event_loop, state))
    })();
    let (mut event_loop, mut state) = match setup {
        Ok(value) => {
            if initialized.send(Ok(())).is_err() {
                return Ok(());
            }
            value
        }
        Err(error) => {
            if initialized.send(Err(error)).is_err() {
                log::debug!("Wayland clipboard initialization receiver was dropped");
            }
            return Ok(());
        }
    };
    let result = (|| -> Result<()> {
        while !state.exit {
            event_loop.dispatch(Some(Duration::from_millis(50)), &mut state)?;
            state.expire_reads();
            connection.flush()?;
        }
        Ok(())
    })();
    for pending in state.pending_reads.drain(..) {
        complete_read(
            &pending.reply,
            Err(anyhow!("Wayland clipboard worker stopped")),
        );
    }
    result
}

impl State {
    fn store(&mut self, item: ClipboardItem) -> Result<()> {
        let seat = self
            .latest_seat
            .as_ref()
            .and_then(|id| self.seats.get(id))
            .ok_or_else(|| anyhow!("No Wayland clipboard seat has received focus"))?;
        ensure!(
            seat.focused,
            "Wayland clipboard writes require keyboard focus"
        );
        let serial = seat
            .serial
            .ok_or_else(|| anyhow!("Wayland clipboard ownership requires a recent input serial"))?;
        let device = seat
            .device
            .as_ref()
            .ok_or_else(|| anyhow!("Wayland clipboard data device is unavailable"))?;
        let owner_mime = format!("pid/{}/wgpui-{}", std::process::id(), uuid::Uuid::new_v4());
        let mut formats = TEXT_FORMATS
            .iter()
            .map(|format| (*format).to_owned())
            .collect::<Vec<_>>();
        formats.push(owner_mime.clone());
        let source = self.manager.create_copy_paste_source(&self.queue, formats);
        source.set_selection(device, serial);
        let text = Arc::from(item.text().unwrap_or_default().into_bytes());
        self.sources.insert(
            source.inner().id(),
            Source {
                _source: source,
                owner_mime,
                item,
                text,
            },
        );
        // set_selection has no acceptance reply; this acknowledges worker processing only.
        Ok(())
    }

    fn load(&mut self, reply: ReadReply) {
        let reply = Arc::new(Mutex::new(Some(reply)));
        if let Err(error) = self.begin_load(reply.clone()) {
            complete_read(&reply, Err(error));
        }
    }

    fn begin_load(&mut self, reply: Arc<Mutex<Option<ReadReply>>>) -> Result<()> {
        let seat = self
            .latest_seat
            .as_ref()
            .and_then(|id| self.seats.get(id))
            .ok_or_else(|| anyhow!("No Wayland clipboard seat has received focus"))?;
        ensure!(
            seat.focused,
            "Wayland clipboard reads require keyboard focus"
        );
        let Some(offer) = seat
            .device
            .as_ref()
            .and_then(|device| device.data().selection_offer())
        else {
            complete_read(&reply, Ok(None));
            return Ok(());
        };
        let formats = offer.with_mime_types(|formats| formats.to_vec());
        for source in self.sources.values() {
            if formats.contains(&source.owner_mime) {
                complete_read(&reply, Ok(Some(source.item.clone())));
                return Ok(());
            }
        }
        let selected = ["text/plain;charset=utf-8", "UTF8_STRING"]
            .into_iter()
            .find(|format| formats.iter().any(|candidate| candidate == format))
            .map(|format| (format.to_owned(), None))
            .or_else(|| {
                ImageFormat::iter().find_map(|format| {
                    formats
                        .iter()
                        .any(|candidate| candidate == format.mime_type())
                        .then(|| (format.mime_type().to_owned(), Some(format)))
                })
            });
        let Some((mime, image_format)) = selected else {
            complete_read(&reply, Ok(None));
            return Ok(());
        };
        let pipe = offer.receive(mime)?;
        set_nonblocking(pipe.as_raw_fd())?;
        let mut bytes = Vec::new();
        let callback_reply = reply.clone();
        let token = self
            .handle
            .insert_source(pipe, move |_, file, _| {
                let file = unsafe { file.get_mut() };
                let mut buffer = [0_u8; 8192];
                loop {
                    match file.read(&mut buffer) {
                        Ok(0) => {
                            let bytes = std::mem::take(&mut bytes);
                            let result = match image_format {
                                Some(format) => Ok(Some(ClipboardItem::new_image(
                                    &Image::from_bytes(format, bytes),
                                ))),
                                None => String::from_utf8(bytes)
                                    .map(|text| {
                                        Some(ClipboardItem::new_string(text.replace("\r\n", "\n")))
                                    })
                                    .map_err(anyhow::Error::from),
                            };
                            complete_read(&callback_reply, result);
                            return PostAction::Remove;
                        }
                        Ok(count) => {
                            if let Err(error) = bytes.try_reserve(count) {
                                complete_read(&callback_reply, Err(error.into()));
                                return PostAction::Remove;
                            }
                            bytes.extend_from_slice(&buffer[..count]);
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            return PostAction::Continue;
                        }
                        Err(error) => {
                            complete_read(&callback_reply, Err(error.into()));
                            return PostAction::Remove;
                        }
                    }
                }
            })
            .map_err(|error| {
                anyhow!(
                    "Wayland clipboard read registration failed: {}",
                    error.error
                )
            })?;
        self.pending_reads.push(PendingRead {
            token,
            deadline: Instant::now() + READ_TIMEOUT,
            reply,
        });
        Ok(())
    }

    fn expire_reads(&mut self) {
        let now = Instant::now();
        self.pending_reads.retain(|pending| {
            if pending.reply.lock().is_none() {
                return false;
            }
            if now >= pending.deadline {
                self.handle.remove(pending.token);
                complete_read(
                    &pending.reply,
                    Err(anyhow!("Wayland clipboard read timed out")),
                );
                return false;
            }
            true
        });
    }
}

fn complete_read(reply: &Mutex<Option<ReadReply>>, result: Result<Option<ClipboardItem>>) {
    if let Some(reply) = reply.lock().take() {
        if reply.send(result).is_err() {
            log::debug!("Wayland clipboard read receiver was dropped");
        }
    }
}

impl SeatHandler for State {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, seat: WlSeat) {
        self.seats.entry(seat.id()).or_default();
    }
    fn new_capability(
        &mut self,
        _: &Connection,
        queue: &QueueHandle<Self>,
        seat: WlSeat,
        capability: Capability,
    ) {
        let state = self.seats.entry(seat.id()).or_default();
        match capability {
            Capability::Keyboard => {
                state.keyboard = Some(seat.get_keyboard(queue, KeyboardSeat(seat.id())));
                state.device = Some(self.manager.get_data_device(queue, &seat));
            }
            Capability::Pointer => match self.seat_state.get_pointer(queue, &seat) {
                Ok(pointer) => state.pointer = Some(pointer),
                Err(error) => {
                    log::warn!("Wayland clipboard pointer initialization failed: {error}")
                }
            },
            _ => {}
        }
    }
    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        seat: WlSeat,
        capability: Capability,
    ) {
        let Some(state) = self.seats.get_mut(&seat.id()) else {
            return;
        };
        match capability {
            Capability::Keyboard => {
                state.device = None;
                state.focused = false;
                state.serial = None;
                if let Some(keyboard) = state.keyboard.take() {
                    if keyboard.version() >= 3 {
                        keyboard.release();
                    }
                }
            }
            Capability::Pointer => {
                if let Some(pointer) = state.pointer.take() {
                    if pointer.version() >= 3 {
                        pointer.release();
                    }
                }
            }
            _ => {}
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, seat: WlSeat) {
        self.seats.remove(&seat.id());
        if self.latest_seat.as_ref() == Some(&seat.id()) {
            self.latest_seat = None;
        }
    }
}

impl Dispatch2<WlKeyboard, State> for KeyboardSeat {
    fn event(
        &self,
        state: &mut State,
        _: &WlKeyboard,
        event: wl_keyboard::Event,
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        let Some(seat) = state.seats.get_mut(&self.0) else {
            return;
        };
        match event {
            wl_keyboard::Event::Key { serial, .. }
            | wl_keyboard::Event::Modifiers { serial, .. } => {
                seat.serial = Some(serial);
                state.latest_seat = Some(self.0.clone());
            }
            wl_keyboard::Event::Enter { serial, .. } => {
                seat.serial = Some(serial);
                seat.focused = true;
                state.latest_seat = Some(self.0.clone());
            }
            wl_keyboard::Event::Leave { .. } => {
                seat.focused = false;
                seat.serial = None;
            }
            _ => {}
        }
    }
}

impl PointerHandler for State {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        pointer: &WlPointer,
        events: &[PointerEvent],
    ) {
        let Some(data) = pointer.data::<PointerData<()>>() else {
            return;
        };
        let seat_id = data.seat().id();
        let Some(seat) = self.seats.get_mut(&seat_id) else {
            return;
        };
        for event in events {
            if let PointerEventKind::Press { serial, .. }
            | PointerEventKind::Release { serial, .. } = event.kind
            {
                seat.serial = Some(serial);
                self.latest_seat = Some(seat_id.clone());
            }
        }
    }
}

impl DataDeviceHandler for State {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlDataDevice,
        _: f64,
        _: f64,
        _: &WlSurface,
    ) {
    }
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}
    fn motion(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice, _: f64, _: f64) {}
    fn drop_performed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}
    fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}
}

impl DataSourceHandler for State {
    fn send_request(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        source: &WlDataSource,
        mime: String,
        pipe: WritePipe,
    ) {
        if !TEXT_FORMATS.contains(&mime.as_str()) {
            return;
        }
        let Some(source) = self.sources.get(&source.id()) else {
            return;
        };
        let bytes = source.text.clone();
        if let Err(error) = set_nonblocking(pipe.as_raw_fd()) {
            log::warn!("Wayland clipboard send setup failed: {error}");
            return;
        }
        let mut written = 0;
        if let Err(error) = self.handle.insert_source(pipe, move |_, file, _| {
            let file = unsafe { file.get_mut() };
            while written < bytes.len() {
                match file.write(&bytes[written..]) {
                    Ok(0) => {
                        log::warn!("Wayland clipboard pipe accepted zero bytes");
                        return PostAction::Remove;
                    }
                    Ok(count) => written += count,
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        return PostAction::Continue;
                    }
                    Err(error) => {
                        log::warn!("Wayland clipboard send failed: {error}");
                        return PostAction::Remove;
                    }
                }
            }
            PostAction::Remove
        }) {
            log::warn!(
                "Wayland clipboard send registration failed: {}",
                error.error
            );
        }
    }
    fn cancelled(&mut self, _: &Connection, _: &QueueHandle<Self>, source: &WlDataSource) {
        self.sources.remove(&source.id());
    }
    fn accept_mime(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlDataSource,
        _: Option<String>,
    ) {
    }
    fn dnd_dropped(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}
    fn dnd_finished(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}
    fn action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: DndAction) {}
}

impl DataOfferHandler for State {
    fn source_actions(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &mut DragOffer,
        _: DndAction,
    ) {
    }
    fn selected_action(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &mut DragOffer,
        _: DndAction,
    ) {
    }
}

impl ProvidesRegistryState for State {
    registry_handlers![SeatState];
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
}

sctk::delegate_dispatch2!(State);
sctk::delegate_registry!(State);

impl Drop for Seat {
    fn drop(&mut self) {
        if let Some(keyboard) = self.keyboard.take() {
            if keyboard.version() >= 3 {
                keyboard.release();
            }
        }
        if let Some(pointer) = self.pointer.take() {
            if pointer.version() >= 3 {
                pointer.release();
            }
        }
    }
}

fn set_nonblocking(descriptor: RawFd) -> std::io::Result<()> {
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFL) };
    if flags < 0 {
        return Err(std::io::Error::last_os_error());
    }
    if unsafe { libc::fcntl(descriptor, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}
