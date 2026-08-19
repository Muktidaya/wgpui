use crate::{
    BackgroundExecutor, Bounds, Capslock, DevicePixels, DisplayId, DummyKeyboardMapper,
    ExternalPaths, FileDropEvent, ForegroundExecutor, KeyDownEvent, KeyUpEvent, Keystroke,
    Modifiers, ModifiersChangedEvent, MouseButton, MouseDownEvent, MouseExitEvent, MouseMoveEvent,
    MouseUpEvent, OwnedMenu, Pixels, Platform, PlatformDisplay, PlatformInput,
    PlatformWindow as _, PriorityQueueReceiver, RunnableVariant, ScrollWheelEvent, Size, point, px,
    size,
    platform::{
        dispatcher::{CrossEvent, Dispatcher},
        keyboard::CrossKeyboardLayout,
        render_context::WgpuContext,
        text_system::CosmicTextSystem,
        window::CrossWindow,
    },
};
use anyhow::Result;
use collections::FxHashMap;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
    time::Instant,
};
use winit::event_loop::ActiveEventLoop;

thread_local! {
    static ACTIVE_CONTEXT: Cell<Option<(*const ActiveEventLoop, *mut AppState)>> = const { Cell::new(None) };
    static ACTIVE_PLATFORM: Cell<Option<*const CrossPlatform>> = const { Cell::new(None) };
}

// Helper to access the context
fn with_active_context<R>(f: impl FnOnce(&ActiveEventLoop, &mut AppState) -> R) -> Option<R> {
    ACTIVE_CONTEXT.with(|storage| {
        let (loop_ptr, app_ptr) = storage.get()?;
        // SAFETY: We strictly manage these pointers during winit callbacks
        unsafe { Some(f(&*loop_ptr, &mut *app_ptr)) }
    })
}

pub(crate) fn with_active_platform<R>(f: impl FnOnce(&CrossPlatform) -> R) -> Option<R> {
    ACTIVE_PLATFORM.with(|platform| {
        let platform = platform.get()?;
        unsafe { Some(f(&*platform)) }
    })
}

pub(crate) struct CrossPlatform {
    background_executor: BackgroundExecutor,
    foreground_executor: ForegroundExecutor,
    text_system: Arc<CosmicTextSystem>,
    wgpu_context: Arc<WgpuContext>,
    main_rx: PriorityQueueReceiver<RunnableVariant>,
    event_loop: Cell<Option<winit::event_loop::EventLoop<CrossEvent>>>,
    event_loop_proxy: winit::event_loop::EventLoopProxy<CrossEvent>,
    callbacks: PlatformCallbacks,
    displays: RefCell<Vec<Rc<dyn crate::PlatformDisplay>>>,
    menus: RefCell<Option<Vec<OwnedMenu>>>,
}

#[derive(Default)]
struct PlatformCallbacks {
    on_open_urls: Cell<Option<Box<dyn FnMut(Vec<String>)>>>,
    on_quit: Cell<Option<Box<dyn FnMut()>>>,
    on_reopen: Cell<Option<Box<dyn FnMut()>>>,
    on_app_menu_action: Cell<Option<Box<dyn FnMut(&dyn crate::Action)>>>,
    on_will_open_app_menu: Cell<Option<Box<dyn FnMut()>>>,
    on_validate_app_menu_command: Cell<Option<Box<dyn FnMut(&dyn crate::Action) -> bool>>>,
}

struct AppState {
    windows: FxHashMap<winit::window::WindowId, CrossWindow>,
    on_finish_launching: Cell<Option<Box<dyn 'static + FnOnce()>>>,
    main_rx: PriorityQueueReceiver<RunnableVariant>,
    current_modifiers: Modifiers,
    pressed_button: Option<MouseButton>,
    click_state: ClickState,
    hover_paths: Vec<std::path::PathBuf>,
}

struct ClickState {
    last_button: MouseButton,
    last_position: crate::Point<Pixels>,
    last_time: Option<Instant>,
    current_count: usize,
}

impl CrossPlatform {
    pub fn new() -> Result<Self> {
        let (main_tx, main_rx) = PriorityQueueReceiver::new();
        let mut event_loop =
            winit::event_loop::EventLoop::<CrossEvent>::with_user_event().build()?;
        let event_loop_proxy = event_loop.create_proxy();

        let dispatcher = Arc::new(Dispatcher::new(main_tx, event_loop_proxy.clone()));
        let background_executor = BackgroundExecutor::new(dispatcher.clone());
        let foreground_executor = ForegroundExecutor::new(dispatcher);

        Ok(Self {
            background_executor,
            foreground_executor,
            text_system: Arc::new(CosmicTextSystem::new()),
            wgpu_context: Arc::new(WgpuContext::new()?),
            main_rx,
            event_loop: Cell::new(Some(event_loop)),
            event_loop_proxy,
            callbacks: PlatformCallbacks::default(),
            displays: RefCell::new(Vec::new()),
            menus: RefCell::new(None),
        })
    }

    pub(crate) fn perform_menu_action(&self, action_identifier: usize) {
        #[cfg(target_os = "macos")]
        crate::platform::macos_menu::with_action(action_identifier, |action| {
            if let Some(mut callback) = self.callbacks.on_app_menu_action.take() {
                callback(action);
                self.callbacks.on_app_menu_action.set(Some(callback));
            }
        });
    }

    pub(crate) fn validate_menu_action(&self, action_identifier: usize) -> bool {
        #[cfg(target_os = "macos")]
        {
            return crate::platform::macos_menu::with_action(action_identifier, |action| {
                if let Some(mut callback) = self.callbacks.on_validate_app_menu_command.take() {
                    let is_enabled = callback(action);
                    self.callbacks
                        .on_validate_app_menu_command
                        .set(Some(callback));
                    is_enabled
                } else {
                    true
                }
            })
            .unwrap_or(false);
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = action_identifier;
            false
        }
    }

    pub(crate) fn will_open_app_menu(&self) {
        if let Some(mut callback) = self.callbacks.on_will_open_app_menu.take() {
            callback();
            self.callbacks.on_will_open_app_menu.set(Some(callback));
        }
    }
}

impl Platform for CrossPlatform {
    fn background_executor(&self) -> BackgroundExecutor {
        self.background_executor.clone()
    }

    fn foreground_executor(&self) -> ForegroundExecutor {
        self.foreground_executor.clone()
    }

    fn text_system(&self) -> Arc<dyn crate::PlatformTextSystem> {
        self.text_system.clone()
    }

    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>) {
        let mut event_loop = self.event_loop.take().expect("App is already running");

        let mut app_state = AppState {
            windows: Default::default(),
            on_finish_launching: Cell::new(Some(on_finish_launching)),
            main_rx: self.main_rx.clone(),
            current_modifiers: Modifiers::default(),
            pressed_button: None,
            click_state: ClickState {
                last_button: MouseButton::Left,
                last_position: point(Pixels(0.0), Pixels(0.0)),
                last_time: None,
                current_count: 0,
            },
            hover_paths: Vec::new(),
        };

        ACTIVE_PLATFORM.with(|platform| platform.set(Some(self as *const CrossPlatform)));
        event_loop
            .run_app(&mut app_state)
            .expect("Failed to run App");
        ACTIVE_PLATFORM.with(|platform| platform.set(None));
    }

    fn quit(&self) {
        // NOTE(mdeand): The event loop will exit when all windows are closed and there are no
        // NOTE(mdeand): more events to process. For an explicit quit, we rely on winit's exit
        // NOTE(mdeand): mechanism via the ActiveEventLoop.
        with_active_context(|event_loop, _| {
            event_loop.exit();
        });
    }

    fn restart(&self, _binary_path: Option<std::path::PathBuf>) {
        log::warn!("restart is not implemented in WGPUI 0.3.4");
    }

    fn activate(&self, _ignoring_other_apps: bool) {}

    fn hide(&self) {
        log::warn!("hide is not implemented in WGPUI 0.3.4");
    }

    fn hide_other_apps(&self) {
        log::warn!("hide_other_apps is not implemented in WGPUI 0.3.4");
    }

    fn unhide_other_apps(&self) {
        log::warn!("unhide_other_apps is not implemented in WGPUI 0.3.4");
    }

    fn displays(&self) -> Vec<Rc<dyn crate::PlatformDisplay>> {
        self.displays.borrow().clone()
    }

    fn primary_display(&self) -> Option<Rc<dyn crate::PlatformDisplay>> {
        self.displays.borrow().first().cloned()
    }

    fn active_window(&self) -> Option<crate::AnyWindowHandle> {
        with_active_context(|_, app_state| {
            app_state
                .windows
                .values()
                .find(|window| window.window().has_focus())
                .map(|window| window.handle())
        })
        .flatten()
    }

    fn open_window(
        &self,
        handle: crate::AnyWindowHandle,
        options: crate::WindowParams,
    ) -> anyhow::Result<Box<dyn crate::PlatformWindow>> {
        let window = CrossWindow::new(
            self.wgpu_context.clone(),
            self.event_loop_proxy.clone(),
            handle,
        );

        let success = with_active_context(|event_loop, app_state| {
            let bounds = options.bounds;
            let attributes = winit::window::Window::default_attributes()
                .with_title(
                    options
                        .titlebar
                        .and_then(|t| t.title)
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "WGPUI".into()),
                )
                .with_inner_size(winit::dpi::LogicalSize::new(
                    bounds.size.width.0 as f64,
                    bounds.size.height.0 as f64,
                ));

            let winit_window = event_loop
                .create_window(attributes)
                .expect("Failed to create window");
            winit_window.set_ime_allowed(true);
            let window_id = winit_window.id();

            window.initialize(winit_window);
            app_state.windows.insert(window_id, window.clone());
            window.window().request_redraw();
        })
        .is_some();

        if !success {
            anyhow::bail!("open_window called outside of main thread event loop");
        }

        Ok(Box::new(window))
    }

    fn window_appearance(&self) -> crate::WindowAppearance {
        crate::WindowAppearance::default()
    }

    fn open_url(&self, url: &str) {
        let url = url.to_string();
        self.background_executor
            .spawn(async move {
                if let Err(error) = open::that(&url) {
                    log::warn!("open_url failed: {error}");
                }
            })
            .detach();
    }

    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>) {
        self.callbacks.on_open_urls.set(Some(callback));
    }

    fn register_url_scheme(&self, _url: &str) -> crate::Task<anyhow::Result<()>> {
        crate::Task::ready(Err(anyhow::anyhow!(
            "register_url_scheme is not implemented in WGPUI 0.3.4"
        )))
    }

    fn prompt_for_paths(
        &self,
        options: crate::PathPromptOptions,
    ) -> futures::channel::oneshot::Receiver<anyhow::Result<Option<Vec<std::path::PathBuf>>>> {
        let (sender, receiver) = futures::channel::oneshot::channel();
        let files = options.files;
        let directories = options.directories;
        let multiple = options.multiple;
        let prompt = options.prompt;
        self.foreground_executor
            .spawn(async move {
                let mut dialog = rfd::FileDialog::new();
                if let Some(prompt) = prompt {
                    dialog = dialog.set_title(prompt.to_string());
                }
                let paths = if directories && !files {
                    if multiple {
                        dialog.pick_folders()
                    } else {
                        dialog.pick_folder().map(|path| vec![path])
                    }
                } else if multiple {
                    dialog.pick_files()
                } else {
                    dialog.pick_file().map(|path| vec![path])
                };
                let _ = sender.send(Ok(paths));
            })
            .detach();
        receiver
    }

    fn prompt_for_new_path(
        &self,
        directory: &std::path::Path,
        suggested_name: Option<&str>,
    ) -> futures::channel::oneshot::Receiver<anyhow::Result<Option<std::path::PathBuf>>> {
        let (sender, receiver) = futures::channel::oneshot::channel();
        let directory = directory.to_path_buf();
        let suggested_name = suggested_name.map(str::to_string);
        self.foreground_executor
            .spawn(async move {
                let mut dialog = rfd::FileDialog::new().set_directory(&directory);
                if let Some(name) = suggested_name {
                    dialog = dialog.set_file_name(name);
                }
                let _ = sender.send(Ok(dialog.save_file()));
            })
            .detach();
        receiver
    }

    fn can_select_mixed_files_and_dirs(&self) -> bool {
        false
    }

    fn reveal_path(&self, path: &std::path::Path) {
        let path = path.to_path_buf();
        self.background_executor
            .spawn(async move {
                #[cfg(target_os = "macos")]
                {
                    if let Err(error) = smol::process::Command::new("open")
                        .arg("-R")
                        .arg(&path)
                        .status()
                        .await
                    {
                        log::warn!("reveal_path failed: {error}");
                    }
                    return;
                }
                #[cfg(not(target_os = "macos"))]
                {
                    let parent = path.parent().unwrap_or(&path);
                    if let Err(error) = open::that(parent) {
                        log::warn!("reveal_path failed: {error}");
                    }
                }
            })
            .detach();
    }

    fn open_with_system(&self, path: &std::path::Path) {
        let path = path.to_path_buf();
        self.background_executor
            .spawn(async move {
                if let Err(error) = open::that(&path) {
                    log::warn!("open_with_system failed: {error}");
                }
            })
            .detach();
    }

    fn on_quit(&self, callback: Box<dyn FnMut()>) {
        self.callbacks.on_quit.set(Some(callback));
    }

    fn on_reopen(&self, callback: Box<dyn FnMut()>) {
        self.callbacks.on_reopen.set(Some(callback));
    }

    fn set_menus(&self, menus: Vec<crate::Menu>, keymap: &crate::Keymap) {
        let owned_menus = menus
            .into_iter()
            .map(crate::Menu::owned)
            .collect::<Vec<_>>();
        self.menus.replace(Some(owned_menus.clone()));
        #[cfg(target_os = "macos")]
        crate::platform::macos_menu::install_menus(owned_menus, keymap);
    }

    fn get_menus(&self) -> Option<Vec<crate::OwnedMenu>> {
        self.menus.borrow().clone()
    }

    fn set_dock_menu(&self, _menu: Vec<crate::MenuItem>, _keymap: &crate::Keymap) {
        log::warn!("set_dock_menu is not implemented in WGPUI 0.3.4");
    }

    fn on_app_menu_action(&self, callback: Box<dyn FnMut(&dyn crate::Action)>) {
        self.callbacks.on_app_menu_action.set(Some(callback));
    }

    fn on_will_open_app_menu(&self, callback: Box<dyn FnMut()>) {
        self.callbacks.on_will_open_app_menu.set(Some(callback));
    }

    fn on_validate_app_menu_command(&self, callback: Box<dyn FnMut(&dyn crate::Action) -> bool>) {
        self.callbacks
            .on_validate_app_menu_command
            .set(Some(callback));
    }

    fn app_path(&self) -> anyhow::Result<std::path::PathBuf> {
        Ok(std::env::current_exe()?)
    }

    fn path_for_auxiliary_executable(&self, _name: &str) -> anyhow::Result<std::path::PathBuf> {
        Err(anyhow::anyhow!(
            "path_for_auxiliary_executable is not implemented in WGPUI 0.3.4"
        ))
    }

    fn set_cursor_style(&self, style: crate::CursorStyle) {
        with_active_context(|_, app_state| {
            let Some(window) = app_state
                .windows
                .values()
                .find(|window| window.window().has_focus())
            else {
                return;
            };
            let winit_window = window.window();
            winit_window.set_cursor_visible(style != crate::CursorStyle::None);
            winit_window.set_cursor(cursor_icon(style));
        });
    }

    fn should_auto_hide_scrollbars(&self) -> bool {
        false
    }

    fn write_to_clipboard(&self, item: crate::ClipboardItem) {
        let Some(text) = item.text() else {
            log::warn!("clipboard write skipped: no text entries");
            return;
        };
        match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.set_text(text)) {
            Ok(()) => {}
            Err(error) => log::warn!("write_to_clipboard failed: {error}"),
        }
    }

    fn read_from_clipboard(&self) -> Option<crate::ClipboardItem> {
        arboard::Clipboard::new()
            .ok()?
            .get_text()
            .ok()
            .map(crate::ClipboardItem::new_string)
    }

    fn write_credentials(
        &self,
        _url: &str,
        _username: &str,
        _password: &[u8],
    ) -> crate::Task<anyhow::Result<()>> {
        crate::Task::ready(Err(anyhow::anyhow!(
            "write_credentials is not implemented in WGPUI 0.3.4"
        )))
    }

    fn read_credentials(
        &self,
        _url: &str,
    ) -> crate::Task<anyhow::Result<Option<(String, Vec<u8>)>>> {
        crate::Task::ready(Err(anyhow::anyhow!(
            "read_credentials is not implemented in WGPUI 0.3.4"
        )))
    }

    fn delete_credentials(&self, _url: &str) -> crate::Task<anyhow::Result<()>> {
        crate::Task::ready(Err(anyhow::anyhow!(
            "delete_credentials is not implemented in WGPUI 0.3.4"
        )))
    }

    fn keyboard_layout(&self) -> Box<dyn crate::PlatformKeyboardLayout> {
        Box::new(CrossKeyboardLayout)
    }

    fn keyboard_mapper(&self) -> Rc<dyn crate::PlatformKeyboardMapper> {
        Rc::new(DummyKeyboardMapper)
    }

    fn on_keyboard_layout_change(&self, _callback: Box<dyn FnMut()>) {
        // Keyboard layout change notifications are not wired in 0.3.4.
    }
}

impl AppState {
    fn set_active_context(&mut self, event_loop: &ActiveEventLoop) {
        ACTIVE_CONTEXT.with(|s| s.set(Some((event_loop as *const _, self as *mut _))));
    }

    fn clear_active_context(&self) {
        ACTIVE_CONTEXT.with(|s| s.set(None));
    }

    fn drain_main_queue(&mut self) {
        while let Ok(Some(runnable)) = self.main_rx.try_pop() {
            runnable.run();
        }
    }
}

impl winit::application::ApplicationHandler<CrossEvent> for AppState {
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: winit::event::StartCause) {}

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: CrossEvent) {
        self.set_active_context(event_loop);

        match event {
            CrossEvent::WakeUp => {
                self.drain_main_queue();
                for window in self.windows.values() {
                    window.window().request_redraw();
                }
            }
            CrossEvent::SurfacePresent(window_id) => {
                if let Some(window) = self.windows.get(&window_id) {
                    window.window().request_redraw();
                }
            }
        }

        self.clear_active_context();
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        _event: winit::event::DeviceEvent,
    ) {
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.set_active_context(event_loop);

        self.drain_main_queue();
        refresh_displays(event_loop);

        // Do NOT unconditionally request_redraw() here. Rendering is driven
        // by three sources:
        //   1. CrossEvent::WakeUp — sent by BackgroundExecutor::wake() from
        //      external threads (e.g. a tokio watcher task) when new data
        //      arrives. The WakeUp handler calls request_redraw().
        //   2. CrossEvent::SurfacePresent — fired after each GPU present; the
        //      handler calls request_redraw() to chain the next frame when
        //      the window still has content to show.
        //   3. OS events (resize, focus, etc.) which set dirty state via
        //      cx.notify(), causing the dispatcher to send WakeUp.
        //
        // With no unconditional redraw, the event loop genuinely sleeps when
        // idle, dropping CPU usage to ~0%.
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);

        self.clear_active_context();
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {}

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {}

    fn memory_warning(&mut self, _event_loop: &ActiveEventLoop) {}

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.set_active_context(event_loop);

        if let Some(on_finish_launching) = self.on_finish_launching.take() {
            on_finish_launching();
        }
        refresh_displays(event_loop);

        self.clear_active_context();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        self.set_active_context(event_loop);

        let Some(window) = self.windows.get(&window_id) else {
            return;
        };

        match event {
            winit::event::WindowEvent::Resized(physical_size) => {
                if physical_size.width == 0 || physical_size.height == 0 {
                    return;
                }

                let scale_factor = window.scale_factor();

                if let Some(renderer) = window.0.renderer.get() {
                    renderer.borrow_mut().update_drawable_size(Size {
                        width: DevicePixels(physical_size.width as i32),
                        height: DevicePixels(physical_size.height as i32),
                    });
                }
                let size = crate::Size {
                    width: crate::Pixels(physical_size.width as f32 / scale_factor),
                    height: crate::Pixels(physical_size.height as f32 / scale_factor),
                };

                window
                    .0
                    .state
                    .callbacks
                    .invoke_mut(&window.0.state.callbacks.on_resize, |cb| {
                        cb(size, scale_factor);
                    });
            }

            winit::event::WindowEvent::Moved(_) => {
                window
                    .0
                    .state
                    .callbacks
                    .invoke_mut(&window.0.state.callbacks.on_moved, |cb| {
                        cb();
                    });
            }

            winit::event::WindowEvent::Focused(active) => {
                window
                    .0
                    .state
                    .callbacks
                    .invoke_mut(&window.0.state.callbacks.on_active_status_change, |cb| {
                        cb(active)
                    });
            }

            winit::event::WindowEvent::ThemeChanged(_) => {
                window
                    .0
                    .state
                    .callbacks
                    .invoke_mut(&window.0.state.callbacks.on_appearance_changed, |cb| cb());
            }

            winit::event::WindowEvent::CloseRequested => {
                let should_close = window
                    .0
                    .state
                    .callbacks
                    .on_should_close
                    .take()
                    .map(|mut cb| {
                        let result = cb();
                        window.0.state.callbacks.on_should_close.set(Some(cb));
                        result
                    })
                    .unwrap_or(true);

                if should_close {
                    if let Some(cb) = window.0.state.callbacks.on_close.take() {
                        cb();
                    }
                    self.windows.remove(&window_id);
                }
            }

            winit::event::WindowEvent::RedrawRequested => {
                let physical_size = window.window().inner_size();
                if physical_size.width == 0 || physical_size.height == 0 {
                    return;
                }

                window.0.state.callbacks.invoke_mut(
                    &window.0.state.callbacks.on_request_frame,
                    |cb| {
                        cb(crate::RequestFrameOptions {
                            force_render: false,
                            // Only present after an actual draw; don't keep the
                            // GPU busy re-presenting the same frame every tick.
                            require_presentation: false,
                        });
                    },
                );
                // Do NOT fall through to the unconditional request_redraw() at
                // the end of window_event — RedrawRequested must not chain
                // itself or the event loop never sleeps under ControlFlow::Wait.
                self.clear_active_context();
                return;
            }

            winit::event::WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        logical_key,
                        state,
                        text,
                        repeat,
                        ..
                    },
                ..
            } => {
                let modifiers = self.current_modifiers;

                if let Some(keystroke) = winit_key_to_keystroke(&logical_key, modifiers, &text) {
                    let platform_event = match state {
                        winit::event::ElementState::Pressed => {
                            PlatformInput::KeyDown(KeyDownEvent {
                                keystroke,
                                is_held: repeat,
                                prefer_character_input: false,
                            })
                        }
                        winit::event::ElementState::Released => {
                            PlatformInput::KeyUp(KeyUpEvent { keystroke })
                        }
                    };

                    window
                        .0
                        .state
                        .callbacks
                        .invoke_mut(&window.0.state.callbacks.on_input, |cb| {
                            cb(platform_event.clone());
                        });
                }
            }

            winit::event::WindowEvent::ModifiersChanged(new_modifiers) => {
                let modifiers = winit_modifiers_to_wgpui(new_modifiers.state());
                self.current_modifiers = modifiers;

                window.0.state.modifiers.set(modifiers);

                let platform_event = PlatformInput::ModifiersChanged(ModifiersChangedEvent {
                    modifiers,
                    capslock: Capslock::default(),
                });

                window
                    .0
                    .state
                    .callbacks
                    .invoke_mut(&window.0.state.callbacks.on_input, |cb| {
                        cb(platform_event.clone());
                    });
            }

            winit::event::WindowEvent::CursorMoved { position, .. } => {
                let scale_factor = window.scale_factor();
                let position = point(
                    Pixels(position.x as f32 / scale_factor),
                    Pixels(position.y as f32 / scale_factor),
                );

                window.0.state.mouse_position.set(position);
                window.set_hovered(true);

                let platform_event = PlatformInput::MouseMove(MouseMoveEvent {
                    position,
                    pressed_button: self.pressed_button,
                    modifiers: self.current_modifiers,
                });

                window
                    .0
                    .state
                    .callbacks
                    .invoke_mut(&window.0.state.callbacks.on_input, |cb| {
                        cb(platform_event.clone());
                    });
            }

            winit::event::WindowEvent::CursorLeft { .. } => {
                window.set_hovered(false);
                let position = window.0.state.mouse_position.get();
                let platform_event = PlatformInput::MouseExited(MouseExitEvent {
                    position,
                    pressed_button: self.pressed_button,
                    modifiers: self.current_modifiers,
                });

                window
                    .0
                    .state
                    .callbacks
                    .invoke_mut(&window.0.state.callbacks.on_input, |cb| {
                        cb(platform_event.clone());
                    });
            }

            winit::event::WindowEvent::MouseInput { state, button, .. } => {
                let position = window.0.state.mouse_position.get();
                let mouse_button = winit_mouse_button_to_wgpui(button);
                let modifiers = self.current_modifiers;

                match state {
                    winit::event::ElementState::Pressed => {
                        self.pressed_button = Some(mouse_button);

                        let click_count =
                            self.click_state
                                .update(mouse_button, position, Instant::now());

                        let platform_event = PlatformInput::MouseDown(MouseDownEvent {
                            button: mouse_button,
                            position,
                            modifiers,
                            click_count,
                            first_mouse: false,
                        });

                        window.0.state.callbacks.invoke_mut(
                            &window.0.state.callbacks.on_input,
                            |cb| {
                                cb(platform_event.clone());
                            },
                        );
                    }
                    winit::event::ElementState::Released => {
                        self.pressed_button = None;

                        let platform_event = PlatformInput::MouseUp(MouseUpEvent {
                            button: mouse_button,
                            position,
                            modifiers,
                            click_count: self.click_state.current_count,
                        });

                        window.0.state.callbacks.invoke_mut(
                            &window.0.state.callbacks.on_input,
                            |cb| {
                                cb(platform_event.clone());
                            },
                        );
                    }
                }
            }

            winit::event::WindowEvent::MouseWheel { delta, phase, .. } => {
                let position = window.0.state.mouse_position.get();
                let modifiers = self.current_modifiers;

                let scroll_delta = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => {
                        crate::ScrollDelta::Lines(point(x, y))
                    }
                    winit::event::MouseScrollDelta::PixelDelta(delta) => {
                        let scale_factor = window.scale_factor();
                        crate::ScrollDelta::Pixels(point(
                            Pixels(delta.x as f32 / scale_factor),
                            Pixels(delta.y as f32 / scale_factor),
                        ))
                    }
                };

                let touch_phase = match phase {
                    winit::event::TouchPhase::Started => crate::TouchPhase::Started,
                    winit::event::TouchPhase::Moved => crate::TouchPhase::Moved,
                    winit::event::TouchPhase::Ended | winit::event::TouchPhase::Cancelled => {
                        crate::TouchPhase::Ended
                    }
                };

                let platform_event = PlatformInput::ScrollWheel(ScrollWheelEvent {
                    position,
                    delta: scroll_delta,
                    modifiers,
                    touch_phase,
                });

                window
                    .0
                    .state
                    .callbacks
                    .invoke_mut(&window.0.state.callbacks.on_input, |cb| {
                        cb(platform_event.clone());
                    });
            }

            winit::event::WindowEvent::Ime(ime) => {
                handle_ime(&window, ime);
            }

            winit::event::WindowEvent::DroppedFile(path) => {
                self.hover_paths.push(path);
                let position = window.0.state.mouse_position.get();
                let paths = ExternalPaths(self.hover_paths.drain(..).collect());
                dispatch_input(
                    &window,
                    PlatformInput::FileDrop(FileDropEvent::Entered { position, paths }),
                );
                dispatch_input(
                    &window,
                    PlatformInput::FileDrop(FileDropEvent::Submit { position }),
                );
            }

            winit::event::WindowEvent::HoveredFile(path) => {
                let position = window.0.state.mouse_position.get();
                let first = self.hover_paths.is_empty();
                self.hover_paths.push(path);
                if first {
                    let paths = ExternalPaths(self.hover_paths.iter().cloned().collect());
                    dispatch_input(
                        &window,
                        PlatformInput::FileDrop(FileDropEvent::Entered { position, paths }),
                    );
                } else {
                    dispatch_input(
                        &window,
                        PlatformInput::FileDrop(FileDropEvent::Pending { position }),
                    );
                }
            }

            winit::event::WindowEvent::HoveredFileCancelled => {
                self.hover_paths.clear();
                dispatch_input(&window, PlatformInput::FileDrop(FileDropEvent::Exited));
            }

            _ => (),
        }

        // Any window event may dirty the window via cx.notify().
        // Under ControlFlow::Wait, redraws only happen when explicitly
        // requested, so we must request one here. The on_request_frame
        // handler checks invalidator.is_dirty() before doing real work,
        // so this is a no-op when nothing actually changed.
        if let Some(window) = self.windows.get(&window_id) {
            window.window().request_redraw();
        }

        self.clear_active_context();
    }
}

const DOUBLE_CLICK_THRESHOLD_MS: u128 = 500;
const DOUBLE_CLICK_DISTANCE: f32 = 5.0;

impl ClickState {
    fn update(
        &mut self,
        button: MouseButton,
        position: crate::Point<Pixels>,
        now: Instant,
    ) -> usize {
        let is_same_button = self.last_button == button;
        let is_within_time = self
            .last_time
            .map(|t| now.duration_since(t).as_millis() < DOUBLE_CLICK_THRESHOLD_MS)
            .unwrap_or(false);
        let distance = ((position.x - self.last_position.x).0.powi(2)
            + (position.y - self.last_position.y).0.powi(2))
        .sqrt();
        let is_within_distance = distance < DOUBLE_CLICK_DISTANCE;

        if is_same_button && is_within_time && is_within_distance {
            self.current_count += 1;
        } else {
            self.current_count = 1;
        }

        self.last_button = button;
        self.last_position = position;
        self.last_time = Some(now);

        self.current_count
    }
}

fn winit_modifiers_to_wgpui(modifiers: winit::keyboard::ModifiersState) -> Modifiers {
    Modifiers {
        control: modifiers.control_key(),
        alt: modifiers.alt_key(),
        shift: modifiers.shift_key(),
        platform: modifiers.super_key(),
        function: false,
    }
}

fn winit_mouse_button_to_wgpui(button: winit::event::MouseButton) -> MouseButton {
    match button {
        winit::event::MouseButton::Left => MouseButton::Left,
        winit::event::MouseButton::Right => MouseButton::Right,
        winit::event::MouseButton::Middle => MouseButton::Middle,
        winit::event::MouseButton::Back => MouseButton::Navigate(crate::NavigationDirection::Back),
        winit::event::MouseButton::Forward => {
            MouseButton::Navigate(crate::NavigationDirection::Forward)
        }
        winit::event::MouseButton::Other(_) => MouseButton::Left,
    }
}

fn winit_key_to_keystroke(
    logical_key: &winit::keyboard::Key,
    modifiers: Modifiers,
    text: &Option<winit::keyboard::SmolStr>,
) -> Option<Keystroke> {
    use winit::keyboard::Key as WKey;
    use winit::keyboard::NamedKey;

    let (key, key_char) = match logical_key {
        WKey::Named(named) => {
            let key_name = match named {
                NamedKey::Backspace => "backspace",
                NamedKey::Tab => "tab",
                NamedKey::Enter => "enter",
                NamedKey::Escape => "escape",
                NamedKey::Space => "space",
                NamedKey::ArrowLeft => "left",
                NamedKey::ArrowRight => "right",
                NamedKey::ArrowUp => "up",
                NamedKey::ArrowDown => "down",
                NamedKey::Home => "home",
                NamedKey::End => "end",
                NamedKey::PageUp => "pageup",
                NamedKey::PageDown => "pagedown",
                NamedKey::Insert => "insert",
                NamedKey::Delete => "delete",
                NamedKey::F1 => "f1",
                NamedKey::F2 => "f2",
                NamedKey::F3 => "f3",
                NamedKey::F4 => "f4",
                NamedKey::F5 => "f5",
                NamedKey::F6 => "f6",
                NamedKey::F7 => "f7",
                NamedKey::F8 => "f8",
                NamedKey::F9 => "f9",
                NamedKey::F10 => "f10",
                NamedKey::F11 => "f11",
                NamedKey::F12 => "f12",
                NamedKey::BrowserBack => "back",
                NamedKey::BrowserForward => "forward",
                // Modifier-only keys don't produce keystrokes by themselves
                NamedKey::Shift
                | NamedKey::Control
                | NamedKey::Alt
                | NamedKey::Super
                | NamedKey::Meta => return None,
                _ => return None,
            };
            (key_name.to_string(), None)
        }
        WKey::Character(ch) => {
            let key = ch.to_lowercase();
            let key_char = text.as_ref().map(|t| t.to_string()).or_else(|| {
                if !modifiers.control
                    && !modifiers.platform
                    && !modifiers.function
                    && !modifiers.alt
                {
                    if modifiers.shift {
                        Some(ch.to_uppercase())
                    } else {
                        Some(ch.to_string())
                    }
                } else {
                    None
                }
            });
            (key, key_char)
        }
        WKey::Unidentified(_) | WKey::Dead(_) => return None,
    };

    Some(Keystroke {
        modifiers,
        key,
        key_char,
    })
}

fn refresh_displays(event_loop: &ActiveEventLoop) {
    with_active_platform(|platform| {
        let displays = event_loop
            .available_monitors()
            .enumerate()
            .map(|(index, monitor)| {
                Rc::new(WinitDisplay::from_monitor(index as u32, &monitor))
                    as Rc<dyn PlatformDisplay>
            })
            .collect();
        *platform.displays.borrow_mut() = displays;
    });
}

fn dispatch_input(window: &CrossWindow, event: PlatformInput) {
    window
        .0
        .state
        .callbacks
        .invoke_mut(&window.0.state.callbacks.on_input, |callback| {
            callback(event.clone());
        });
}

fn handle_ime(window: &CrossWindow, ime: winit::event::Ime) {
    let mut input_handler = window.0.state.input_handler.borrow_mut();
    let Some(handler) = input_handler.as_mut() else {
        return;
    };
    match ime {
        winit::event::Ime::Enabled => {}
        winit::event::Ime::Preedit(text, cursor) => {
            if text.is_empty() {
                handler.unmark_text();
            } else {
                let selected = cursor.map(|(start, end)| {
                    byte_offset_to_utf16(&text, start)..byte_offset_to_utf16(&text, end)
                });
                handler.replace_and_mark_text_in_range(None, &text, selected);
            }
        }
        winit::event::Ime::Commit(text) => {
            handler.replace_text_in_range(None, &text);
            handler.unmark_text();
        }
        winit::event::Ime::Disabled => {
            handler.unmark_text();
        }
    }
}

fn byte_offset_to_utf16(text: &str, byte: usize) -> usize {
    text.get(..byte.min(text.len()))
        .map(|prefix| prefix.encode_utf16().count())
        .unwrap_or_else(|| text.encode_utf16().count())
}

fn cursor_icon(style: crate::CursorStyle) -> winit::window::CursorIcon {
    use crate::CursorStyle;
    use winit::window::CursorIcon;
    match style {
        CursorStyle::Arrow => CursorIcon::Default,
        CursorStyle::IBeam => CursorIcon::Text,
        CursorStyle::Crosshair => CursorIcon::Crosshair,
        CursorStyle::ClosedHand => CursorIcon::Grabbing,
        CursorStyle::OpenHand => CursorIcon::Grab,
        CursorStyle::PointingHand => CursorIcon::Pointer,
        CursorStyle::ResizeLeft => CursorIcon::WResize,
        CursorStyle::ResizeRight => CursorIcon::EResize,
        CursorStyle::ResizeLeftRight => CursorIcon::EwResize,
        CursorStyle::ResizeUp => CursorIcon::NResize,
        CursorStyle::ResizeDown => CursorIcon::SResize,
        CursorStyle::ResizeUpDown => CursorIcon::NsResize,
        CursorStyle::ResizeUpLeftDownRight => CursorIcon::NeswResize,
        CursorStyle::ResizeUpRightDownLeft => CursorIcon::NwseResize,
        CursorStyle::ResizeColumn => CursorIcon::ColResize,
        CursorStyle::ResizeRow => CursorIcon::RowResize,
        CursorStyle::IBeamCursorForVerticalLayout => CursorIcon::VerticalText,
        CursorStyle::OperationNotAllowed => CursorIcon::NotAllowed,
        CursorStyle::DragLink => CursorIcon::Alias,
        CursorStyle::DragCopy => CursorIcon::Copy,
        CursorStyle::ContextualMenu => CursorIcon::ContextMenu,
        CursorStyle::None => CursorIcon::Default,
    }
}

#[derive(Debug)]
struct WinitDisplay {
    id: DisplayId,
    uuid: uuid::Uuid,
    bounds: Bounds<Pixels>,
}

impl WinitDisplay {
    fn from_monitor(index: u32, monitor: &winit::monitor::MonitorHandle) -> Self {
        let scale = monitor.scale_factor() as f32;
        let position = monitor.position();
        let physical = monitor.size();
        let origin = point(
            px(position.x as f32 / scale),
            px(position.y as f32 / scale),
        );
        let bounds_size = size(
            px(physical.width as f32 / scale),
            px(physical.height as f32 / scale),
        );
        let name = monitor
            .name()
            .unwrap_or_else(|| "unnamed-display".into());
        let uuid = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, name.as_bytes());
        Self {
            id: DisplayId(index),
            uuid,
            bounds: Bounds::new(origin, bounds_size),
        }
    }
}

pub(crate) fn display_for_winit_monitor(
    monitor: &winit::monitor::MonitorHandle,
) -> Rc<dyn PlatformDisplay> {
    let name = monitor
        .name()
        .unwrap_or_else(|| "unnamed-display".into());
    let uuid = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, name.as_bytes());
    with_active_platform(|platform| {
        platform
            .displays
            .borrow()
            .iter()
            .find(|display| display.uuid().ok() == Some(uuid))
            .cloned()
    })
    .flatten()
    .unwrap_or_else(|| Rc::new(WinitDisplay::from_monitor(0, monitor)))
}

impl PlatformDisplay for WinitDisplay {
    fn id(&self) -> DisplayId {
        self.id
    }

    fn uuid(&self) -> anyhow::Result<uuid::Uuid> {
        Ok(self.uuid)
    }

    fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }
}
