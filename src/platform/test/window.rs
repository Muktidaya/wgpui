use crate::{
    AnyWindowHandle, AtlasKey, AtlasTextureId, AtlasTile, Bounds, DispatchEventResult, GpuSpecs,
    Pixels, PlatformAtlas, PlatformDisplay, PlatformInput, PlatformInputHandler, PlatformWindow,
    Point, PromptButton, RequestFrameOptions, Size, TestPlatform, TileId, WindowAppearance,
    WindowBackgroundAppearance, WindowBounds, WindowControlArea, WindowParams,
};
use collections::HashMap;
use parking_lot::Mutex;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{
    rc::{Rc, Weak},
    sync::{self, Arc},
};

pub(crate) struct TestWindowState {
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) handle: AnyWindowHandle,
    display: Rc<dyn PlatformDisplay>,
    pub(crate) title: Option<String>,
    pub(crate) edited: bool,
    platform: Weak<TestPlatform>,
    sprite_atlas: Arc<dyn PlatformAtlas>,
    headless_renderer: Option<Box<dyn crate::PlatformHeadlessRenderer>>,
    pub(crate) should_close_handler: Option<Box<dyn FnMut() -> bool>>,
    hit_test_window_control_callback: Option<Box<dyn FnMut() -> Option<WindowControlArea>>>,
    input_callback: Option<Box<dyn FnMut(PlatformInput) -> DispatchEventResult>>,
    active_status_change_callback: Option<Box<dyn FnMut(bool)>>,
    hover_status_change_callback: Option<Box<dyn FnMut(bool)>>,
    resize_callback: Option<Box<dyn FnMut(Size<Pixels>, f32)>>,
    moved_callback: Option<Box<dyn FnMut()>>,
    visibility_callback: Option<Box<dyn FnMut(bool)>>,
    visual_viewport: Option<Bounds<Pixels>>,
    visual_viewport_callback: Option<Box<dyn FnMut()>>,
    input_handler: Option<PlatformInputHandler>,
    is_fullscreen: bool,
    scale_factor: f32,
    is_resizable: bool,
    is_visible: bool,
    pub(crate) soft_keyboard_requested: bool,
    pub(crate) accessibility_active: bool,
    pub(crate) accessibility_tree: Option<accesskit::TreeUpdate>,
    accessibility_callback: Option<Box<dyn FnMut(accesskit_winit::WindowEvent)>>,
    pub(crate) text_input_configuration: crate::TextInputConfiguration,
}

#[derive(Clone)]
pub(crate) struct TestWindow(pub(crate) Rc<Mutex<TestWindowState>>);

impl HasWindowHandle for TestWindow {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        Err(raw_window_handle::HandleError::Unavailable)
    }
}

impl HasDisplayHandle for TestWindow {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        Err(raw_window_handle::HandleError::Unavailable)
    }
}

impl TestWindow {
    pub fn new(
        handle: AnyWindowHandle,
        params: WindowParams,
        platform: Weak<TestPlatform>,
        display: Rc<dyn PlatformDisplay>,
        headless_renderer: Option<Box<dyn crate::PlatformHeadlessRenderer>>,
    ) -> Self {
        Self(Rc::new(Mutex::new(TestWindowState {
            bounds: params.bounds,
            scale_factor: 2.0,
            is_resizable: params.is_resizable,
            is_visible: params.show,
            soft_keyboard_requested: false,
            accessibility_active: false,
            accessibility_tree: None,
            accessibility_callback: None,
            text_input_configuration: Default::default(),
            display,
            platform,
            handle,
            sprite_atlas: headless_renderer
                .as_ref()
                .map(|renderer| renderer.sprite_atlas())
                .unwrap_or_else(|| Arc::new(TestAtlas::new())),
            headless_renderer,
            title: Default::default(),
            edited: false,
            should_close_handler: None,
            hit_test_window_control_callback: None,
            input_callback: None,
            active_status_change_callback: None,
            hover_status_change_callback: None,
            resize_callback: None,
            moved_callback: None,
            visibility_callback: None,
            visual_viewport: None,
            visual_viewport_callback: None,
            input_handler: None,
            is_fullscreen: false,
        })))
    }

    pub fn simulate_scale_factor_change(&mut self, scale_factor: f32) {
        assert!(scale_factor.is_finite() && scale_factor > 0.0);
        self.0.lock().scale_factor = scale_factor;
        let size = self.0.lock().bounds.size;
        self.simulate_resize(size);
    }

    pub fn simulate_resize(&mut self, size: Size<Pixels>) {
        let scale_factor = self.scale_factor();
        let mut lock = self.0.lock();
        let Some(mut callback) = lock.resize_callback.take() else {
            return;
        };
        lock.bounds.size = size;
        drop(lock);
        callback(size, scale_factor);
        self.0.lock().resize_callback = Some(callback);
    }

    pub(crate) fn simulate_active_status_change(&self, active: bool) {
        let mut lock = self.0.lock();
        let Some(mut callback) = lock.active_status_change_callback.take() else {
            return;
        };
        drop(lock);
        callback(active);
        self.0.lock().active_status_change_callback = Some(callback);
    }

    pub fn simulate_visibility_change(&mut self, visible: bool) {
        let mut state = self.0.lock();
        state.is_visible = visible;
        let callback = state.visibility_callback.take();
        drop(state);
        if let Some(mut callback) = callback {
            callback(visible);
            self.0.lock().visibility_callback = Some(callback);
        }
    }

    pub fn simulate_visual_viewport_change(&self, bounds: Bounds<Pixels>) {
        let callback = {
            let mut state = self.0.lock();
            state.visual_viewport = Some(bounds);
            state.visual_viewport_callback.take()
        };
        if let Some(mut callback) = callback {
            callback();
            self.0.lock().visual_viewport_callback = Some(callback);
        }
    }

    pub fn simulate_accessibility_event(&mut self, event: accesskit_winit::WindowEvent) {
        let mut state = self.0.lock();
        match &event {
            accesskit_winit::WindowEvent::InitialTreeRequested => state.accessibility_active = true,
            accesskit_winit::WindowEvent::AccessibilityDeactivated => {
                state.accessibility_active = false
            }
            accesskit_winit::WindowEvent::ActionRequested(_) => {}
        }
        let callback = state.accessibility_callback.take();
        drop(state);
        if let Some(mut callback) = callback {
            callback(event);
            self.0.lock().accessibility_callback = Some(callback);
        }
    }

    pub fn simulate_input(&mut self, event: PlatformInput) -> bool {
        let mut lock = self.0.lock();
        let Some(mut callback) = lock.input_callback.take() else {
            return false;
        };
        drop(lock);
        let result = callback(event);
        self.0.lock().input_callback = Some(callback);
        !result.propagate
    }

    pub(crate) fn simulate_ime_event(&mut self, event: crate::ImeEvent) -> anyhow::Result<()> {
        let mut handler = self
            .take_input_handler()
            .ok_or_else(|| anyhow::anyhow!("the window has no focused text input handler"))?;
        let result = handler.handle_ime_event(event);
        let mut state = self.0.lock();
        if state.input_handler.is_none() {
            state.input_handler = Some(handler);
        }
        result
    }
}

impl PlatformWindow for TestWindow {
    fn visual_viewport_bounds(&self) -> Bounds<Pixels> {
        let state = self.0.lock();
        state
            .visual_viewport
            .unwrap_or_else(|| Bounds::new(Point::default(), state.bounds.size))
    }

    fn on_visual_viewport_changed(&self, callback: Box<dyn FnMut()>) {
        self.0.lock().visual_viewport_callback = Some(callback);
    }

    fn is_a11y_active(&self) -> bool {
        self.0.lock().accessibility_active
    }
    fn update_a11y(&self, update: accesskit::TreeUpdate) {
        self.0.lock().accessibility_tree = Some(update);
    }
    fn on_accessibility_event(&self, callback: Box<dyn FnMut(accesskit_winit::WindowEvent)>) {
        self.0.lock().accessibility_callback = Some(callback);
    }

    fn bounds(&self) -> Bounds<Pixels> {
        self.0.lock().bounds
    }

    fn window_bounds(&self) -> WindowBounds {
        WindowBounds::Windowed(self.bounds())
    }

    fn is_maximized(&self) -> bool {
        false
    }

    fn content_size(&self) -> Size<Pixels> {
        self.bounds().size
    }

    fn resize(&mut self, size: Size<Pixels>) {
        let mut lock = self.0.lock();
        lock.bounds.size = size;
    }

    fn scale_factor(&self) -> f32 {
        self.0.lock().scale_factor
    }

    fn is_resizable(&self) -> bool {
        self.0.lock().is_resizable
    }

    fn is_visible(&self) -> bool {
        self.0.lock().is_visible
    }

    fn set_text_input_configuration(&self, configuration: &crate::TextInputConfiguration) {
        self.0.lock().text_input_configuration = configuration.clone();
    }

    fn show_soft_keyboard(&self) {
        self.0.lock().soft_keyboard_requested = true;
    }

    fn hide_soft_keyboard(&self) {
        self.0.lock().soft_keyboard_requested = false;
    }

    fn appearance(&self) -> WindowAppearance {
        WindowAppearance::Light
    }

    fn display(&self) -> Option<std::rc::Rc<dyn crate::PlatformDisplay>> {
        Some(self.0.lock().display.clone())
    }

    fn mouse_position(&self) -> Point<Pixels> {
        Point::default()
    }

    fn modifiers(&self) -> crate::Modifiers {
        crate::Modifiers::default()
    }

    fn capslock(&self) -> crate::Capslock {
        crate::Capslock::default()
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        self.0.lock().input_handler = Some(input_handler);
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.0.lock().input_handler.take()
    }

    fn prompt(
        &self,
        _level: crate::PromptLevel,
        msg: &str,
        detail: Option<&str>,
        answers: &[PromptButton],
    ) -> Option<futures::channel::oneshot::Receiver<usize>> {
        Some(
            self.0
                .lock()
                .platform
                .upgrade()
                .expect("platform dropped")
                .prompt(msg, detail, answers),
        )
    }

    fn activate(&self) {
        self.0
            .lock()
            .platform
            .upgrade()
            .unwrap()
            .set_active_window(Some(self.clone()))
    }

    fn is_active(&self) -> bool {
        false
    }

    fn is_hovered(&self) -> bool {
        false
    }

    fn set_title(&mut self, title: &str) {
        self.0.lock().title = Some(title.to_owned());
    }

    fn set_app_id(&mut self, _app_id: &str) {}

    fn set_background_appearance(&self, _background: WindowBackgroundAppearance) {}

    fn set_edited(&mut self, edited: bool) {
        self.0.lock().edited = edited;
    }

    fn show_character_palette(&self) {
        unimplemented!()
    }

    fn minimize(&self) {
        unimplemented!()
    }

    fn zoom(&self) {
        unimplemented!()
    }

    fn toggle_fullscreen(&self) {
        let mut lock = self.0.lock();
        lock.is_fullscreen = !lock.is_fullscreen;
    }

    fn is_fullscreen(&self) -> bool {
        self.0.lock().is_fullscreen
    }

    fn on_request_frame(&self, _callback: Box<dyn FnMut(RequestFrameOptions)>) {}

    fn on_input(&self, callback: Box<dyn FnMut(crate::PlatformInput) -> DispatchEventResult>) {
        self.0.lock().input_callback = Some(callback)
    }

    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.0.lock().active_status_change_callback = Some(callback)
    }

    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.0.lock().hover_status_change_callback = Some(callback)
    }

    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        self.0.lock().resize_callback = Some(callback)
    }

    fn on_visibility_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.0.lock().visibility_callback = Some(callback);
    }

    fn on_moved(&self, callback: Box<dyn FnMut()>) {
        self.0.lock().moved_callback = Some(callback)
    }

    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>) {
        self.0.lock().should_close_handler = Some(callback);
    }

    fn on_close(&self, _callback: Box<dyn FnOnce()>) {}

    fn on_hit_test_window_control(&self, callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
        self.0.lock().hit_test_window_control_callback = Some(callback);
    }

    fn on_appearance_changed(&self, _callback: Box<dyn FnMut()>) {}

    fn draw(&self, scene: &crate::Scene) {
        let mut state = self.0.lock();
        let size = crate::size(
            crate::DevicePixels((state.bounds.size.width.0 * state.scale_factor).ceil() as i32),
            crate::DevicePixels((state.bounds.size.height.0 * state.scale_factor).ceil() as i32),
        );
        if let Some(renderer) = state.headless_renderer.as_mut() {
            if let Err(error) = renderer.render_scene(scene, size) {
                log::error!("headless frame submission failed: {error:#}");
            }
        }
    }

    fn render_to_image(&self, scene: &crate::Scene) -> anyhow::Result<image::RgbaImage> {
        let mut state = self.0.lock();
        let size = crate::size(
            crate::DevicePixels((state.bounds.size.width.0 * state.scale_factor).ceil() as i32),
            crate::DevicePixels((state.bounds.size.height.0 * state.scale_factor).ceil() as i32),
        );
        let renderer = state.headless_renderer.as_mut().ok_or_else(|| {
            anyhow::anyhow!("screenshot capture requires a real headless renderer")
        })?;
        renderer.render_scene_to_image(scene, size)
    }

    fn sprite_atlas(&self) -> sync::Arc<dyn crate::PlatformAtlas> {
        self.0.lock().sprite_atlas.clone()
    }

    fn as_test(&mut self) -> Option<&mut TestWindow> {
        Some(self)
    }

    fn show_window_menu(&self, _position: Point<Pixels>) {
        unimplemented!()
    }

    fn start_window_move(&self) {
        unimplemented!()
    }

    fn update_ime_position(&self, _bounds: Bounds<Pixels>) {}

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        None
    }
}

pub(crate) struct TestAtlasState {
    next_id: u32,
    tiles: HashMap<AtlasKey, AtlasTile>,
}

pub(crate) struct TestAtlas(Mutex<TestAtlasState>);

impl TestAtlas {
    pub fn new() -> Self {
        TestAtlas(Mutex::new(TestAtlasState {
            next_id: 0,
            tiles: HashMap::default(),
        }))
    }
}

impl PlatformAtlas for TestAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &crate::AtlasKey,
        build: &mut dyn FnMut() -> anyhow::Result<
            Option<(Size<crate::DevicePixels>, std::borrow::Cow<'a, [u8]>)>,
        >,
    ) -> anyhow::Result<Option<crate::AtlasTile>> {
        let mut state = self.0.lock();
        if let Some(tile) = state.tiles.get(key) {
            return Ok(Some(tile.clone()));
        }
        drop(state);

        let Some((size, _)) = build()? else {
            return Ok(None);
        };

        let mut state = self.0.lock();
        state.next_id += 1;
        let texture_id = state.next_id;
        state.next_id += 1;
        let tile_id = state.next_id;

        state.tiles.insert(
            key.clone(),
            crate::AtlasTile {
                texture_id: AtlasTextureId {
                    index: texture_id,
                    kind: crate::AtlasTextureKind::Monochrome,
                },
                tile_id: TileId(tile_id),
                padding: 0,
                bounds: crate::Bounds {
                    origin: Point::default(),
                    size,
                },
            },
        );

        Ok(Some(state.tiles[key].clone()))
    }

    fn remove(&self, key: &AtlasKey) {
        let mut state = self.0.lock();
        state.tiles.remove(key);
    }
}
