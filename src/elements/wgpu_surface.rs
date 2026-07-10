use std::sync::{Arc, Mutex};

use refineable::Refineable as _;

use crate::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, Style, StyleRefinement, Styled, Window,
    platform::surface_registry::{SurfaceId, SurfaceRegistry},
};

/// Inner state shared across clones of `WgpuSurfaceHandle`.
/// When the last clone is dropped, the surface is removed from the registry.
struct WgpuSurfaceHandleInner {
    surface_id: SurfaceId,
    registry: Arc<SurfaceRegistry>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    present_trigger: Arc<dyn Fn() + Send + Sync>,
    /// Optional direct handle to the winit window.  Having an `Arc` lets
    /// us call `request_redraw()` from another thread without touching the
    /// event bus.
    winit_window: Option<Arc<winit::window::Window>>,
    size: Mutex<(u32, u32)>,
    format: wgpu::TextureFormat,
}

impl Drop for WgpuSurfaceHandleInner {
    fn drop(&mut self) {
        self.registry.remove(self.surface_id);
    }
}

/// A handle to a double-buffered WGPU surface.
///
/// External code uses this to render into the surface's back buffer using the
/// provided `wgpu::Device` and `wgpu::Queue`, then calls [`present()`](Self::present)
/// to swap buffers and trigger a window re-composite.
///
/// All rendering stays on the GPU — `swap_buffers()` is a pointer swap (no copy),
/// and the renderer samples the front buffer texture directly in the shader.
///
/// This handle is `Clone + Send + Sync`.
#[derive(Clone)]
pub struct WgpuSurfaceHandle {
    inner: Arc<WgpuSurfaceHandleInner>,
}

impl WgpuSurfaceHandle {
    /// Returns true when the WGPUI_BENCHMARK environment variable is set.
    /// In benchmark mode wgpui bypasses the compositor entirely and
    /// operations like `present()` become no-ops so the render thread can
    /// drive the GPU at full speed.
    fn benchmark_mode() -> bool {
        std::env::var("WGPUI_BENCHMARK").is_ok()
    }
    pub(crate) fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface_id: SurfaceId,
        registry: Arc<SurfaceRegistry>,
        present_trigger: Arc<dyn Fn() + Send + Sync>,
        winit_window: Option<Arc<winit::window::Window>>,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            inner: Arc::new(WgpuSurfaceHandleInner {
                surface_id,
                registry,
                device,
                queue,
                present_trigger,
                winit_window,
                size: Mutex::new((width, height)),
                format,
            }),
        }
    }

    /// The wgpu `Device` for creating GPU resources and command encoders.
    pub fn device(&self) -> &wgpu::Device {
        &self.inner.device
    }

    /// The wgpu `Queue` for submitting command buffers.
    pub fn queue(&self) -> &wgpu::Queue {
        &self.inner.queue
    }

    /// Get a `TextureView` of the back buffer for use as a render target.
    /// Render into this, then call [`present()`](Self::present).
    pub fn back_buffer_view(&self) -> Option<wgpu::TextureView> {
        self.inner.registry.back_view(self.inner.surface_id)
    }

    /// Atomically obtain the back buffer view _and_ its pixel dimensions.
    /// This avoids races where the surface is resized between separate calls
    /// to `back_buffer_view` and `.size()`.
    pub fn back_view_with_size(&self) -> Option<(wgpu::TextureView, (u32, u32))> {
        self.inner
            .registry
            .lock_and_get_back_with_size(self.inner.surface_id)
    }

    /// Swap front and back buffers (GPU pointer swap, zero copy).
    /// After this, the content you rendered into the back buffer becomes the
    /// front buffer that the renderer will composite.
    pub fn swap_buffers(&self) {
        self.inner.registry.swap_buffers(self.inner.surface_id);
    }

    /// Request the window to re-present its scene without a full layout/paint.
    /// The renderer will pick up the current front buffer texture.
    pub fn request_present(&self) {
        (self.inner.present_trigger)();
    }

    /// Convenience: swap buffers and immediately request a present.
    ///
    /// If the handle was created with a `CrossWindow` (default on WGPU
    /// platforms), this method will call `window.request_redraw()` directly
    /// from the render thread, sidestepping the `CrossEvent` event bus.  The
    /// underlying queue is still coalesced to prevent flooding.
    pub fn present(&self) {
        self.swap_buffers();
        if Self::benchmark_mode() {
            // do nothing, let producer run unrestricted
            return;
        }

        // coalesce events by setting the pending flag; only send if there
        // was not one outstanding already.
        if !self
            .inner
            .registry
            .set_present_pending(self.inner.surface_id)
        {
            if let Some(winit) = &self.inner.winit_window {
                winit.request_redraw();
            } else {
                self.request_present();
            }
        }
    }

    /// Current size in device pixels.
    pub fn size(&self) -> (u32, u32) {
        *self.inner.size.lock().unwrap()
    }

    /// Returns `true` while the compositor still has an unconsumed frame.
    pub fn is_present_pending(&self) -> bool {
        self.inner
            .registry
            .is_present_pending(self.inner.surface_id)
    }

    /// Block until the pending flag clears, yielding the thread.
    pub fn wait_for_present(&self) {
        if Self::benchmark_mode() {
            return;
        }
        while self.is_present_pending() {
            std::thread::sleep(std::time::Duration::from_micros(50));
        }
    }

    /// The texture format used by this surface's buffers.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.inner.format
    }

    /// The `SurfaceId` for this handle (used internally by the element).
    pub(crate) fn id(&self) -> SurfaceId {
        self.inner.surface_id
    }

    /// Resize the surface's double buffers. Called by the element when bounds change.
    pub(crate) fn resize(&self, width: u32, height: u32) {
        let mut size = self.inner.size.lock().unwrap();
        if size.0 == width && size.1 == height {
            return;
        }
        self.inner
            .registry
            .resize(&self.inner.device, self.inner.surface_id, width, height);
        *size = (width, height);
    }
}

/// Create a `WgpuSurface` element from an existing handle.
pub fn wgpu_surface(handle: WgpuSurfaceHandle) -> WgpuSurface {
    WgpuSurface {
        handle,
        style: StyleRefinement::default(),
        on_resize: None,
    }
}

/// An element that displays content rendered externally via WGPU.
///
/// On the WGPU platform, the renderer composites the surface's front buffer
/// texture directly (GPU → GPU, no copies). On other platforms this renders
/// as a fallback colored box.
pub struct WgpuSurface {
    handle: WgpuSurfaceHandle,
    style: StyleRefinement,
    on_resize: Option<Box<dyn Fn(u32, u32, &WgpuSurfaceHandle) + 'static>>,
}

impl WgpuSurface {
    /// Register a callback invoked when the element's layout bounds change.
    /// The surface textures are automatically resized; use this to recreate
    /// any external resources that depend on the size.
    pub fn on_resize(mut self, callback: impl Fn(u32, u32, &WgpuSurfaceHandle) + 'static) -> Self {
        self.on_resize = Some(Box::new(callback));
        self
    }
}

impl Element for WgpuSurface {
    type RequestLayoutState = Style;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.refine(&self.style);
        let layout_id = window.request_layout(style.clone(), [], cx);
        (layout_id, style)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        // Compute pixel size accounting for scale factor
        let scale = window.scale_factor();
        let pixel_w = (bounds.size.width.0 * scale).round() as u32;
        let pixel_h = (bounds.size.height.0 * scale).round() as u32;

        let (cur_w, cur_h) = self.handle.size();
        if pixel_w != cur_w || pixel_h != cur_h {
            self.handle.resize(pixel_w, pixel_h);
            if let Some(cb) = &self.on_resize {
                cb(pixel_w, pixel_h, &self.handle);
            }
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        style: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        style.paint(bounds, window, cx, |window, _cx| {
            window.paint_wgpu_surface(bounds, self.handle.id());
        });
    }
}

impl IntoElement for WgpuSurface {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Styled for WgpuSurface {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
