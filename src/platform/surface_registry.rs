use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

/// An opaque identifier for a registered WGPU surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SurfaceId(pub(crate) u64);

#[allow(dead_code)]
struct DoubleBuffer {
    textures: [wgpu::Texture; 2],
    views: [wgpu::TextureView; 2],
    front: usize,
    /// Changes whenever resize replaces the textures while preserving the surface id.
    revision: u64,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PresentStatus {
    Idle,
    Pending,
    Closed,
}

/// Shared with retained handles so closure remains terminal after textures are removed.
pub(crate) struct SurfacePresentation {
    status: Mutex<PresentStatus>,
    consumed: Condvar,
}

impl SurfacePresentation {
    fn new(status: PresentStatus) -> Self {
        Self {
            status: Mutex::new(status),
            consumed: Condvar::new(),
        }
    }

    fn request(&self) -> Option<bool> {
        let mut status = self.status.lock().unwrap();
        if *status == PresentStatus::Closed {
            return None;
        }
        let pending = *status == PresentStatus::Pending;
        *status = PresentStatus::Pending;
        Some(pending)
    }

    fn clear(&self) {
        let mut status = self.status.lock().unwrap();
        if *status == PresentStatus::Pending {
            *status = PresentStatus::Idle;
        }
        self.consumed.notify_all();
    }

    fn close(&self) {
        *self.status.lock().unwrap() = PresentStatus::Closed;
        self.consumed.notify_all();
    }

    pub(crate) fn is_closed(&self) -> bool {
        *self.status.lock().unwrap() == PresentStatus::Closed
    }

    pub(crate) fn is_pending(&self) -> bool {
        *self.status.lock().unwrap() == PresentStatus::Pending
    }

    pub(crate) fn wait(&self) {
        let mut status = self.status.lock().unwrap();
        while *status == PresentStatus::Pending {
            status = self.consumed.wait(status).unwrap();
        }
    }
}

/// Identifies one lifetime of a native window, not a reusable native id alone.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SurfaceOwner {
    window_id: winit::window::WindowId,
    generation: u64,
}

struct RegisteredSurface<T> {
    data: T,
    owner: Option<SurfaceOwner>,
    presentation: Arc<SurfacePresentation>,
}
impl<T> std::ops::Deref for RegisteredSurface<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.data
    }
}
impl<T> std::ops::DerefMut for RegisteredSurface<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.data
    }
}

struct RegistryState<T> {
    surfaces: HashMap<SurfaceId, RegisteredSurface<T>>,
    closed_owners: HashSet<SurfaceOwner>,
    closed: bool,
    next_owner: u64,
}
impl<T> RegistryState<T> {
    fn new() -> Self {
        Self {
            surfaces: HashMap::new(),
            closed_owners: HashSet::new(),
            closed: false,
            next_owner: 1,
        }
    }
    fn register(
        &mut self,
        id: SurfaceId,
        owner: Option<SurfaceOwner>,
        create: impl FnOnce() -> T,
    ) -> bool {
        if self.closed || owner.is_some_and(|owner| self.closed_owners.contains(&owner)) {
            return false;
        }
        self.surfaces.insert(
            id,
            RegisteredSurface {
                data: create(),
                owner,
                presentation: Arc::new(SurfacePresentation::new(PresentStatus::Idle)),
            },
        );
        true
    }
    fn open_owner(&mut self, window_id: winit::window::WindowId) -> SurfaceOwner {
        let generation = self.next_owner;
        self.next_owner = generation
            .checked_add(1)
            .expect("surface owner generation exhausted");
        SurfaceOwner {
            window_id,
            generation,
        }
    }
    fn close_owner(&mut self, owner: SurfaceOwner) {
        self.closed_owners.insert(owner);
        self.surfaces.retain(|_, surface| {
            if surface.owner == Some(owner) {
                surface.presentation.close();
                false
            } else {
                true
            }
        });
    }
    fn shutdown(&mut self) {
        self.closed = true;
        for surface in self.surfaces.values() {
            surface.presentation.close();
        }
        self.surfaces.clear();
    }
}

/// Thread-safe registry of all active WGPU surfaces.
/// Maps `SurfaceId` to double-buffered texture pairs.
pub struct SurfaceRegistry {
    surfaces: Mutex<RegistryState<DoubleBuffer>>,
    next_id: AtomicU64,
}

impl SurfaceRegistry {
    pub fn new() -> Self {
        Self {
            surfaces: Mutex::new(RegistryState::new()),
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
        owner: Option<SurfaceOwner>,
    ) -> Option<SurfaceId> {
        let id = SurfaceId(self.next_id.fetch_add(1, Ordering::Relaxed));
        self.surfaces
            .lock()
            .unwrap()
            .register(id, owner, || {
                Self::create_double_buffer(device, width, height, format)
            })
            .then_some(id)
    }

    /// Begin a new native window lifetime, including a legitimately reused window id.
    pub(crate) fn open_owner(&self, owner: winit::window::WindowId) -> SurfaceOwner {
        self.surfaces.lock().unwrap().open_owner(owner)
    }

    pub(crate) fn close_owner(&self, owner: SurfaceOwner) {
        self.surfaces.lock().unwrap().close_owner(owner);
    }

    pub(crate) fn shutdown(&self) {
        self.surfaces.lock().unwrap().shutdown();
    }

    pub(crate) fn presentation(&self, id: SurfaceId) -> Arc<SurfacePresentation> {
        self.surfaces
            .lock()
            .unwrap()
            .surfaces
            .get(&id)
            .map(|surface| surface.presentation.clone())
            .unwrap_or_else(|| Arc::new(SurfacePresentation::new(PresentStatus::Closed)))
    }

    /// Swap front and back buffers (pointer swap, no GPU work).
    pub fn swap_buffers(&self, id: SurfaceId) {
        if let Some(db) = self.surfaces.lock().unwrap().surfaces.get_mut(&id) {
            db.front = 1 - db.front;
        }
    }

    /// Resize both buffers, creating new textures.
    pub fn resize(&self, device: &wgpu::Device, id: SurfaceId, width: u32, height: u32) {
        let mut state = self.surfaces.lock().unwrap();
        let surfaces = &mut state.surfaces;
        if let Some(db) = surfaces.get_mut(&id) {
            if db.width == width && db.height == height {
                return;
            }
            let mut new_db = Self::create_double_buffer(device, width, height, db.format);
            new_db.revision = db.revision.wrapping_add(1);
            db.data = new_db;
        }
    }

    /// Get the front buffer's `TextureView` (what the renderer reads from).
    #[allow(dead_code)]
    pub fn front_view(&self, id: SurfaceId) -> Option<wgpu::TextureView> {
        // clone an already-created view instead of making a new one every frame.
        let state = self.surfaces.lock().unwrap();
        let surfaces = &state.surfaces;
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
        let state = self.surfaces.lock().unwrap();
        let surfaces = &state.surfaces;
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
        let state = self.surfaces.lock().unwrap();
        let surfaces = &state.surfaces;
        surfaces.get(&id).map(|db| {
            let back = 1 - db.front;
            (db.views[back].clone(), (db.width, db.height))
        })
    }

    /// Atomically snapshot the texture views needed to bind a surface.
    ///
    /// A surface keeps the same id when it is resized, but both texture views are
    /// replaced. The revision lets renderer-side caches distinguish those texture
    /// generations and rebuild bind groups instead of sampling stale views.
    pub fn binding_snapshot(&self, id: SurfaceId) -> Option<(usize, u64, [wgpu::TextureView; 2])> {
        let state = self.surfaces.lock().unwrap();
        let surfaces = &state.surfaces;
        surfaces.get(&id).map(|db| {
            (
                db.front,
                db.revision,
                [db.views[0].clone(), db.views[1].clone()],
            )
        })
    }

    /// Get the current size of a surface.
    #[allow(dead_code)]
    pub fn size(&self, id: SurfaceId) -> Option<(u32, u32)> {
        let state = self.surfaces.lock().unwrap();
        let surfaces = &state.surfaces;
        surfaces.get(&id).map(|db| (db.width, db.height))
    }

    /// Get the texture format for a surface.
    #[allow(dead_code)]
    pub fn format(&self, id: SurfaceId) -> Option<wgpu::TextureFormat> {
        let state = self.surfaces.lock().unwrap();
        let surfaces = &state.surfaces;
        surfaces.get(&id).map(|db| db.format)
    }

    /// Remove a surface and release any retained handle's presentation wait.
    pub fn remove(&self, id: SurfaceId) {
        if let Some(surface) = self.surfaces.lock().unwrap().surfaces.remove(&id) {
            surface.presentation.close();
        }
    }

    /// Return whether an earlier request is pending, or this surface is closed.
    /// Closed/missing surfaces never schedule another compositor callback.
    pub fn set_present_pending(&self, id: SurfaceId) -> bool {
        self.presentation(id).request().unwrap_or(true)
    }

    pub fn is_present_pending(&self, id: SurfaceId) -> bool {
        self.presentation(id).is_pending()
    }

    pub fn has_present_pending_for(&self, owner: winit::window::WindowId) -> bool {
        self.surfaces
            .lock()
            .unwrap()
            .surfaces
            .values()
            .any(|surface| {
                surface.owner.is_some_and(|token| token.window_id == owner)
                    && surface.presentation.is_pending()
            })
    }

    /// Consume a frame without reopening a terminally closed presentation state.
    pub fn clear_present_pending(&self, id: SurfaceId) {
        self.presentation(id).clear();
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
            revision: 0,
            width: w,
            height: h,
            format,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{Barrier, mpsc},
        time::Duration,
    };

    #[test]
    fn missing_surface_never_schedules_a_pending_frame() {
        let registry = SurfaceRegistry::new();
        assert!(registry.set_present_pending(SurfaceId(99)));
        assert!(!registry.is_present_pending(SurfaceId(99)));
        assert!(registry.back_view(SurfaceId(99)).is_none());
    }

    #[test]
    fn retained_presentation_wait_finishes_and_later_requests_stay_closed() {
        let mut registry = RegistryState::new();
        let owner = registry.open_owner(winit::window::WindowId::from(1));
        assert!(registry.register(SurfaceId(1), Some(owner), || ()));
        let retained = registry.surfaces[&SurfaceId(1)].presentation.clone();
        assert_eq!(retained.request(), Some(false));
        let waiter = retained.clone();
        let (finished, result) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            waiter.wait();
            finished.send(()).unwrap();
        });
        registry.close_owner(owner);
        result
            .recv_timeout(Duration::from_secs(2))
            .expect("close must release a retained presentation wait");
        thread.join().unwrap();
        for _ in 0..100 {
            retained.clear();
            assert_eq!(
                retained.request(),
                None,
                "present after close cannot re-arm pending"
            );
            assert!(!retained.is_pending());
            retained.wait();
        }
        assert!(!registry.register(SurfaceId(2), Some(owner), || panic!(
            "late creation must not allocate"
        )));
        assert!(
            registry.surfaces.get_mut(&SurfaceId(1)).is_none(),
            "resize cannot resurrect a closed surface"
        );
    }

    #[test]
    fn closing_one_owner_preserves_live_presentations_and_reused_window_ids() {
        let mut registry = RegistryState::new();
        let owner = registry.open_owner(winit::window::WindowId::from(1));
        let other = registry.open_owner(winit::window::WindowId::from(2));
        assert!(registry.register(SurfaceId(1), Some(owner), || ()));
        assert!(registry.register(SurfaceId(2), Some(other), || ()));
        let old = registry.surfaces[&SurfaceId(1)].presentation.clone();
        let live = registry.surfaces[&SurfaceId(2)].presentation.clone();
        assert_eq!(live.request(), Some(false));
        assert_eq!(live.request(), Some(true));
        registry.close_owner(owner);
        assert!(old.is_closed());
        assert!(live.is_pending());
        live.clear();
        live.wait();
        assert_eq!(live.request(), Some(false));
        let replacement = registry.open_owner(owner.window_id);
        assert!(!registry.register(SurfaceId(4), Some(owner), || panic!(
            "stale owner registration"
        )));
        assert!(registry.register(SurfaceId(3), Some(replacement), || ()));
        registry.close_owner(owner);
        assert!(!registry.surfaces[&SurfaceId(3)].presentation.is_closed());
        assert_eq!(
            registry.surfaces[&SurfaceId(3)].presentation.request(),
            Some(false)
        );
        assert_eq!(
            old.request(),
            None,
            "a new window lifetime cannot reopen old handles"
        );
    }

    #[test]
    fn registration_racing_close_cannot_resurrect_owner_surfaces() {
        let (finished, result) = mpsc::channel();
        let coordinator = std::thread::spawn(move || {
            let state = Arc::new(Mutex::new(RegistryState::new()));
            let owner = state
                .lock()
                .unwrap()
                .open_owner(winit::window::WindowId::from(1));
            let start = Arc::new(Barrier::new(2));
            let registration = state.clone();
            let ready = start.clone();
            let producer = std::thread::spawn(move || {
                ready.wait();
                let mut handles = Vec::new();
                for id in 1..1001 {
                    let mut state = registration.lock().unwrap();
                    if state.register(SurfaceId(id), Some(owner), || ()) {
                        handles.push(state.surfaces[&SurfaceId(id)].presentation.clone());
                    }
                }
                handles
            });
            start.wait();
            state.lock().unwrap().close_owner(owner);
            let handles = producer.join().unwrap();
            assert!(handles.iter().all(|handle| handle.is_closed()));
            let mut state = state.lock().unwrap();
            assert!(state.surfaces.is_empty());
            assert!(!state.register(SurfaceId(1002), Some(owner), || panic!(
                "closed owner allocation"
            )));
            finished.send(()).unwrap();
        });
        result
            .recv_timeout(Duration::from_secs(3))
            .expect("registration/close must finish without deadlock");
        coordinator.join().unwrap();
    }

    #[test]
    fn shutdown_closes_all_owners_and_rejects_all_later_registration() {
        let mut registry = RegistryState::new();
        let owner = registry.open_owner(winit::window::WindowId::from(1));
        assert!(registry.register(SurfaceId(1), Some(owner), || ()));
        assert!(registry.register(SurfaceId(2), None, || ()));
        let handles = registry
            .surfaces
            .values()
            .map(|surface| surface.presentation.clone())
            .collect::<Vec<_>>();
        for handle in &handles {
            assert_eq!(handle.request(), Some(false));
        }
        registry.shutdown();
        let replacement = registry.open_owner(owner.window_id);
        assert!(
            !registry.register(SurfaceId(5), Some(replacement), || panic!(
                "shutdown new owner allocation"
            ))
        );
        for handle in handles {
            assert!(handle.is_closed());
            assert_eq!(handle.request(), None);
            handle.wait();
        }
        assert!(!registry.register(SurfaceId(3), Some(owner), || panic!("shutdown allocation")));
        assert!(!registry.register(SurfaceId(4), None, || panic!(
            "shutdown headless allocation"
        )));
    }
}
