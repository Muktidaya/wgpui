use crate::{
    AnyWindowHandle, Bounds, Capslock, Modifiers, Pixels, PlatformInputHandler, PlatformWindow,
    Point, Size, WgpuSurfaceHandle, WindowAppearance, WindowBackgroundAppearance, WindowBounds,
    platform::{
        atlas::WgpuAtlas, dispatcher::CrossEvent, render_context::WgpuContext,
        renderer::WgpuRenderer,
    },
};
use std::{
    cell::{Cell, OnceCell, RefCell},
    rc::Rc,
    sync::Arc,
};
use winit::event_loop::EventLoopProxy;

#[derive(Clone)]
pub struct CrossWindow(pub(crate) Rc<CrossWindowInner>);

pub(crate) struct CrossWindowInner {
    pub(crate) winit_window: OnceCell<Arc<winit::window::Window>>,
    pub(crate) renderer: OnceCell<RefCell<WgpuRenderer>>,
    pub(crate) wgpu_context: Arc<WgpuContext>,
    pub(crate) sprite_atlas: Arc<WgpuAtlas>,
    pub(crate) event_loop_proxy: EventLoopProxy<CrossEvent>,
    pub(crate) handle: AnyWindowHandle,
    pub(crate) hovered: Cell<bool>,
    pub(crate) accessibility_adapter: OnceCell<RefCell<accesskit_winit::Adapter>>,
    pub(crate) state: CrossWindowState,
}

#[derive(Default)]
pub(crate) struct CrossWindowState {
    pub(crate) callbacks: Callbacks,
    pub(crate) input_handler: RefCell<Option<PlatformInputHandler>>,
    pub(crate) mouse_position: Cell<Point<Pixels>>,
    pub(crate) touch_ids: RefCell<collections::FxHashMap<u64, crate::TouchId>>,
    pub(crate) next_touch_id: Cell<u64>,
    pub(crate) modifiers: Cell<Modifiers>,
    pub(crate) capslock: Cell<Capslock>,
    pub(crate) occluded: Cell<bool>,
    pub(crate) accessibility_active: Cell<bool>,
    pub(crate) text_input_configuration: RefCell<crate::TextInputConfiguration>,
}

#[derive(Default)]
pub(crate) struct Callbacks {
    pub(crate) on_request_frame: Cell<Option<Box<dyn FnMut(crate::RequestFrameOptions)>>>,
    pub(crate) on_input:
        Cell<Option<Box<dyn FnMut(crate::PlatformInput) -> crate::DispatchEventResult>>>,
    pub(crate) on_active_status_change: Cell<Option<Box<dyn FnMut(bool)>>>,
    pub(crate) on_hover_status_change: Cell<Option<Box<dyn FnMut(bool)>>>,
    pub(crate) on_resize: Cell<Option<Box<dyn FnMut(crate::Size<crate::Pixels>, f32)>>>,
    pub(crate) on_moved: Cell<Option<Box<dyn FnMut()>>>,
    pub(crate) on_visibility_change: Cell<Option<Box<dyn FnMut(bool)>>>,
    pub(crate) on_visual_viewport_changed: Cell<Option<Box<dyn FnMut()>>>,
    pub(crate) on_accessibility_event: Cell<Option<Box<dyn FnMut(accesskit_winit::WindowEvent)>>>,
    pub(crate) on_should_close: Cell<Option<Box<dyn FnMut() -> bool>>>,
    pub(crate) on_hit_test_window_control:
        Cell<Option<Box<dyn FnMut() -> Option<crate::WindowControlArea>>>>,
    pub(crate) on_close: Cell<Option<Box<dyn FnOnce()>>>,
    pub(crate) on_appearance_changed: Cell<Option<Box<dyn FnMut()>>>,
}

impl Callbacks {
    pub(crate) fn invoke_mut<F: ?Sized>(
        &self,
        cell: &Cell<Option<Box<F>>>,
        f: impl FnOnce(&mut F),
    ) {
        if let Some(mut cb) = cell.take() {
            f(&mut cb);
            cell.set(Some(cb));
        }
    }
}

impl CrossWindow {
    pub(crate) fn new(
        wgpu_context: Arc<WgpuContext>,
        event_loop_proxy: EventLoopProxy<CrossEvent>,
        handle: AnyWindowHandle,
    ) -> Self {
        let sprite_atlas = Arc::new(WgpuAtlas::new(wgpu_context.clone()));

        Self(Rc::new(CrossWindowInner {
            winit_window: OnceCell::new(),
            wgpu_context,
            renderer: OnceCell::new(),
            sprite_atlas,
            event_loop_proxy,
            handle,
            hovered: Cell::new(false),
            accessibility_adapter: OnceCell::new(),
            state: CrossWindowState::default(),
        }))
    }

    pub(crate) fn handle(&self) -> crate::AnyWindowHandle {
        self.0.handle
    }

    pub(crate) fn set_hovered(&self, hovered: bool) {
        let was_hovered = self.0.hovered.replace(hovered);
        if was_hovered != hovered {
            self.0.state.callbacks.invoke_mut(
                &self.0.state.callbacks.on_hover_status_change,
                |callback| {
                    callback(hovered);
                },
            );
        }
    }

    pub(crate) fn initialize(
        &self,
        winit_window: winit::window::Window,
        event_loop: &winit::event_loop::ActiveEventLoop,
    ) {
        let initial_size = winit_window.inner_size();

        self.0
            .winit_window
            .set(Arc::new(winit_window))
            .expect("winit_window already initialized");

        self.0
            .accessibility_adapter
            .set(RefCell::new(
                accesskit_winit::Adapter::with_event_loop_proxy(
                    event_loop,
                    self.window(),
                    self.0.event_loop_proxy.clone(),
                ),
            ))
            .unwrap_or_else(|_| panic!("accessibility adapter already initialized"));

        if initial_size.width > 0 && initial_size.height > 0 {
            let renderer = WgpuRenderer::new(
                self.0.wgpu_context.clone(),
                self.window(),
                self.0.sprite_atlas.clone(),
                initial_size.width,
                initial_size.height,
                4,
            )
            .expect("Failed to create renderer");

            let _ = self.0.renderer.set(RefCell::new(renderer));
            self.window().request_redraw();
        }
    }

    pub(crate) fn window(&self) -> &winit::window::Window {
        self.0
            .winit_window
            .get()
            .expect("winit_window should be initialized")
    }
}

impl PlatformWindow for CrossWindow {
    fn on_visual_viewport_changed(&self, callback: Box<dyn FnMut()>) {
        self.0
            .state
            .callbacks
            .on_visual_viewport_changed
            .set(Some(callback));
    }

    fn is_a11y_active(&self) -> bool {
        self.0.state.accessibility_active.get()
    }

    fn update_a11y(&self, update: accesskit::TreeUpdate) {
        if let Some(adapter) = self.0.accessibility_adapter.get() {
            adapter.borrow_mut().update_if_active(|| update);
        }
    }

    fn on_accessibility_event(&self, callback: Box<dyn FnMut(accesskit_winit::WindowEvent)>) {
        self.0
            .state
            .callbacks
            .on_accessibility_event
            .set(Some(callback));
    }

    fn bounds(&self) -> Bounds<Pixels> {
        let scale_factor = self.window().scale_factor() as f32;
        let physical_size = self.window().inner_size();
        let origin = self
            .window()
            .outer_position()
            .map(|pos| Point {
                x: Pixels(pos.x as f32 / scale_factor),
                y: Pixels(pos.y as f32 / scale_factor),
            })
            .unwrap_or_default();

        Bounds {
            origin,
            size: Size {
                width: Pixels(physical_size.width as f32 / scale_factor),
                height: Pixels(physical_size.height as f32 / scale_factor),
            },
        }
    }

    fn is_resizable(&self) -> bool {
        self.window().is_resizable()
    }

    fn is_visible(&self) -> bool {
        self.window().is_visible().unwrap_or(true) && !self.0.state.occluded.get()
    }

    fn set_text_input_configuration(&self, configuration: &crate::TextInputConfiguration) {
        // Winit exposes IME purpose, but no per-field capitalization or enter-key hint.
        // Preserve those preferences for backends that can apply richer native sessions.
        self.0
            .state
            .text_input_configuration
            .replace(configuration.clone());
        self.window()
            .set_ime_purpose(winit::window::ImePurpose::Normal);
    }

    fn show_soft_keyboard(&self) {
        self.window().set_ime_allowed(true);
    }

    fn hide_soft_keyboard(&self) {
        self.window().set_ime_allowed(false);
    }

    fn is_maximized(&self) -> bool {
        self.window().is_maximized()
    }

    fn window_bounds(&self) -> crate::WindowBounds {
        let bounds = self.bounds();

        if let Some(_fullscreen) = self.window().fullscreen() {
            return WindowBounds::Fullscreen(bounds);
        }

        if self.window().is_maximized() {
            return WindowBounds::Maximized(bounds);
        }

        WindowBounds::Windowed(bounds)
    }

    fn content_size(&self) -> crate::Size<crate::Pixels> {
        let scale_factor = self.window().scale_factor() as f32;
        let physical_size = self.window().inner_size();

        crate::Size {
            width: Pixels(physical_size.width as f32 / scale_factor),
            height: Pixels(physical_size.height as f32 / scale_factor),
        }
    }

    fn resize(&mut self, size: crate::Size<crate::Pixels>) {
        let _ =
            self.window()
                .request_inner_size(winit::dpi::Size::Logical(winit::dpi::LogicalSize {
                    width: size.width.0 as f64,
                    height: size.height.0 as f64,
                }));
    }

    fn scale_factor(&self) -> f32 {
        self.window().scale_factor() as f32
    }

    fn appearance(&self) -> crate::WindowAppearance {
        match self.window().theme() {
            Some(winit::window::Theme::Light) => WindowAppearance::Light,
            Some(winit::window::Theme::Dark) => WindowAppearance::Dark,
            None => WindowAppearance::default(),
        }
    }

    fn display(&self) -> Option<std::rc::Rc<dyn crate::PlatformDisplay>> {
        let monitor = self.window().current_monitor()?;
        Some(crate::platform::platform::display_for_winit_monitor(
            &monitor,
        ))
    }

    fn mouse_position(&self) -> Point<Pixels> {
        self.0.state.mouse_position.get()
    }

    fn modifiers(&self) -> Modifiers {
        self.0.state.modifiers.get()
    }

    fn capslock(&self) -> Capslock {
        self.0.state.capslock.get()
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        self.0
            .state
            .input_handler
            .borrow_mut()
            .replace(input_handler);
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.0.state.input_handler.borrow_mut().take()
    }

    fn prompt(
        &self,
        _level: crate::PromptLevel,
        _msg: &str,
        _detail: Option<&str>,
        _answers: &[crate::PromptButton],
    ) -> Option<futures::channel::oneshot::Receiver<usize>> {
        None
    }

    fn activate(&self) {
        self.window().focus_window();
    }

    fn is_active(&self) -> bool {
        self.window().has_focus()
    }

    fn is_hovered(&self) -> bool {
        self.0.hovered.get()
    }

    fn set_title(&mut self, title: &str) {
        self.window().set_title(title);
    }

    fn set_background_appearance(&self, _background_appearance: WindowBackgroundAppearance) {
        // Window chrome transparency is not implemented by the WGPUI window backend.
    }

    fn minimize(&self) {
        self.window().set_minimized(true);
    }

    fn zoom(&self) {
        self.window().set_maximized(!self.window().is_maximized());
    }

    fn toggle_fullscreen(&self) {
        self.window()
            .set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
    }

    fn is_fullscreen(&self) -> bool {
        self.window().fullscreen().is_some()
    }

    fn on_request_frame(&self, callback: Box<dyn FnMut(crate::RequestFrameOptions)>) {
        self.0.state.callbacks.on_request_frame.set(Some(callback));
    }

    fn on_input(
        &self,
        callback: Box<dyn FnMut(crate::PlatformInput) -> crate::DispatchEventResult>,
    ) {
        self.0.state.callbacks.on_input.set(Some(callback));
    }

    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.0
            .state
            .callbacks
            .on_active_status_change
            .set(Some(callback));
    }

    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.0
            .state
            .callbacks
            .on_hover_status_change
            .set(Some(callback));
    }

    fn on_resize(&self, callback: Box<dyn FnMut(crate::Size<crate::Pixels>, f32)>) {
        self.0.state.callbacks.on_resize.set(Some(callback));
    }

    fn on_visibility_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.0
            .state
            .callbacks
            .on_visibility_change
            .set(Some(callback));
    }

    fn on_moved(&self, callback: Box<dyn FnMut()>) {
        self.0.state.callbacks.on_moved.set(Some(callback));
    }

    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>) {
        self.0.state.callbacks.on_should_close.set(Some(callback));
    }

    fn on_hit_test_window_control(
        &self,
        callback: Box<dyn FnMut() -> Option<crate::WindowControlArea>>,
    ) {
        self.0
            .state
            .callbacks
            .on_hit_test_window_control
            .set(Some(callback));
    }

    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        self.0.state.callbacks.on_close.set(Some(callback));
    }

    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>) {
        self.0
            .state
            .callbacks
            .on_appearance_changed
            .set(Some(callback));
    }

    fn draw(&self, scene: &crate::Scene) {
        if let Some(renderer) = self.0.renderer.get() {
            renderer.borrow().draw(scene);
        }
    }

    fn create_wgpu_surface(
        &self,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Option<WgpuSurfaceHandle> {
        let ctx = &self.0.wgpu_context;
        let registry = ctx.surface_registry.clone();
        let surface_id = registry.create(&ctx.device, width, height, format);

        // Build the present trigger: sends a CrossEvent to wake the event loop
        // and request a redraw for this window.
        let proxy = self.0.event_loop_proxy.clone();
        let window_id = self.0.winit_window.get().map(|w| w.id());
        let present_trigger: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            if let Some(wid) = window_id {
                let _ = proxy.send_event(CrossEvent::SurfacePresent(wid));
            }
        });

        // capture winit window Arc so handle can request redraw directly
        let winit_arc = self.0.winit_window.get().cloned();
        Some(WgpuSurfaceHandle::new(
            ctx.device.clone(),
            ctx.queue.clone(),
            surface_id,
            registry,
            present_trigger,
            winit_arc,
            width,
            height,
            format,
        ))
    }

    fn sprite_atlas(&self) -> std::sync::Arc<dyn crate::PlatformAtlas> {
        self.0.sprite_atlas.clone()
    }

    fn gpu_specs(&self) -> Option<crate::GpuSpecs> {
        self.0
            .renderer
            .get()
            .map(|renderer| renderer.borrow().gpu_specs())
    }

    fn update_ime_position(&self, bounds: crate::Bounds<crate::Pixels>) {
        let window = self.window();
        window.set_ime_cursor_area(
            winit::dpi::LogicalPosition::new(bounds.origin.x.0 as f64, bounds.origin.y.0 as f64),
            winit::dpi::LogicalSize::new(bounds.size.width.0 as f64, bounds.size.height.0 as f64),
        );
    }
}

impl raw_window_handle::HasDisplayHandle for CrossWindow {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        self.window().display_handle()
    }
}

impl raw_window_handle::HasWindowHandle for CrossWindow {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        self.window().window_handle()
    }
}
