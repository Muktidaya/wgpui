use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// An opaque identifier for a registered WGPU surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SurfaceId(pub(crate) u64);

#[allow(dead_code)]
struct DoubleBuffer {
    textures: [wgpu::Texture; 2],
    views: [wgpu::TextureView; 2],
    front: usize,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    // true when a present event has been fired but not yet consumed by
    // the renderer.  We coalesce multiple calls to `present()` so the
    // application doesn't flood the event loop at thousands of FPS.
    present_pending: std::sync::atomic::AtomicBool,
}

/// Thread-safe registry of all active WGPU surfaces.
/// Maps `SurfaceId` to double-buffered texture pairs.
pub struct SurfaceRegistry {
    surfaces: Mutex<HashMap<SurfaceId, DoubleBuffer>>,
    next_id: AtomicU64,
}

impl SurfaceRegistry {
    pub fn new() -> Self {
        Self {
            surfaces: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
        }
    }

    /// Create a new double-buffered surface. Returns its `SurfaceId`.
    pub fn create(
        &self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> SurfaceId {
        let id = SurfaceId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let db = Self::create_double_buffer(device, width, height, format);
        self.surfaces.lock().unwrap().insert(id, db);
        id
    }

    /// Swap front and back buffers (pointer swap, no GPU work).
    pub fn swap_buffers(&self, id: SurfaceId) {
        if let Some(db) = self.surfaces.lock().unwrap().get_mut(&id) {
            db.front = 1 - db.front;
        }
    }

    /// Resize both buffers, creating new textures.
    pub fn resize(&self, device: &wgpu::Device, id: SurfaceId, width: u32, height: u32) {
        let mut surfaces = self.surfaces.lock().unwrap();
        if let Some(db) = surfaces.get_mut(&id) {
            if db.width == width && db.height == height {
                return;
            }
            let new_db = Self::create_double_buffer(device, width, height, db.format);
            *db = new_db;
        }
    }

    /// Get the front buffer's `TextureView` (what the renderer reads from).
    #[allow(dead_code)]
    pub fn front_view(&self, id: SurfaceId) -> Option<wgpu::TextureView> {
        // clone an already-created view instead of making a new one every frame.
        let surfaces = self.surfaces.lock().unwrap();
        surfaces.get(&id).map(|db| db.views[db.front].clone())
    }

    /// Get the back buffer's `Texture` (what external code renders into).
    #[allow(dead_code)]
    pub fn back_texture(&self, _id: SurfaceId) -> Option<wgpu::Texture> {
        // wgpu::Texture is internally Arc'd, so we can't just hand it out.
        // Instead we'll provide a view via back_view().
        None
    }

    /// Get the back buffer's `TextureView` for use as a render target.
    pub fn back_view(&self, id: SurfaceId) -> Option<wgpu::TextureView> {
        let surfaces = self.surfaces.lock().unwrap();
        surfaces.get(&id).map(|db| {
            let back = 1 - db.front;
            db.views[back].clone()
        })
    }

    /// Atomically retrieve both the back view and the corresponding texture
    /// dimensions. This is useful when a caller needs to create auxiliary
    /// resources (e.g. a depth buffer) that must exactly match the view's size.
    pub fn lock_and_get_back_with_size(
        &self,
        id: SurfaceId,
    ) -> Option<(wgpu::TextureView, (u32, u32))> {
        let surfaces = self.surfaces.lock().unwrap();
        surfaces.get(&id).map(|db| {
            let back = 1 - db.front;
            (db.views[back].clone(), (db.width, db.height))
        })
    }

    /// Get the current front buffer index (0 or 1).
    pub fn front_index(&self, id: SurfaceId) -> Option<usize> {
        let surfaces = self.surfaces.lock().unwrap();
        surfaces.get(&id).map(|db| db.front)
    }

    /// Access the view at the given index (0 or 1).
    pub fn view_at(&self, id: SurfaceId, idx: usize) -> Option<wgpu::TextureView> {
        let surfaces = self.surfaces.lock().unwrap();
        surfaces.get(&id).and_then(|db| db.views.get(idx).cloned())
    }

    /// Get the current size of a surface.
    #[allow(dead_code)]
    pub fn size(&self, id: SurfaceId) -> Option<(u32, u32)> {
        let surfaces = self.surfaces.lock().unwrap();
        surfaces.get(&id).map(|db| (db.width, db.height))
    }

    /// Get the texture format for a surface.
    #[allow(dead_code)]
    pub fn format(&self, id: SurfaceId) -> Option<wgpu::TextureFormat> {
        let surfaces = self.surfaces.lock().unwrap();
        surfaces.get(&id).map(|db| db.format)
    }

    /// Remove a surface from the registry.
    pub fn remove(&self, id: SurfaceId) {
        self.surfaces.lock().unwrap().remove(&id);
    }

    /// Set the "present pending" flag for a surface, returning previous value.
    /// When `present()` is called by external code we use this to avoid
    /// sending duplicate events while one is already queued.
    pub fn set_present_pending(&self, id: SurfaceId) -> bool {
        if let Some(db) = self.surfaces.lock().unwrap().get(&id) {
            db.present_pending
                .swap(true, std::sync::atomic::Ordering::Relaxed)
        } else {
            false
        }
    }

    /// Query whether a present is still pending (not yet consumed).
    pub fn is_present_pending(&self, id: SurfaceId) -> bool {
        if let Some(db) = self.surfaces.lock().unwrap().get(&id) {
            db.present_pending
                .load(std::sync::atomic::Ordering::Relaxed)
        } else {
            false
        }
    }

    /// Clear the pending flag, normally invoked when the renderer consumes
    /// the next frame (in `paint_wgpu_surface`).
    pub fn clear_present_pending(&self, id: SurfaceId) {
        if let Some(db) = self.surfaces.lock().unwrap().get(&id) {
            db.present_pending
                .store(false, std::sync::atomic::Ordering::Relaxed);
        }
    }

    fn create_double_buffer(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> DoubleBuffer {
        let w = width.max(1);
        let h = height.max(1);

        let create_texture = |label: &str| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
        };

        let tex0 = create_texture("surface_buffer_0");
        let tex1 = create_texture("surface_buffer_1");
        let view0 = tex0.create_view(&wgpu::TextureViewDescriptor::default());
        let view1 = tex1.create_view(&wgpu::TextureViewDescriptor::default());

        DoubleBuffer {
            textures: [tex0, tex1],
            views: [view0, view1],
            front: 0,
            width: w,
            height: h,
            format,
            present_pending: std::sync::atomic::AtomicBool::new(false),
        }
    }
}
