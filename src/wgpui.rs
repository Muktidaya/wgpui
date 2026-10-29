#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![allow(clippy::type_complexity)] // Not useful, WGPUI makes heavy use of callbacks
#![allow(clippy::collapsible_else_if)] // False positives in platform specific code
#![allow(unused_mut)] // False positives in platform specific code

// Same-package macro expansions use the library name in both library and example targets.
extern crate self as wgpui;

#[macro_use]
mod action;
mod a11y;
mod app;

mod arena;
mod asset_cache;
mod assets;
mod bounds_tree;
mod color;
/// The default colors used by WGPUI.
pub mod default_colors;
mod element;
mod elements;
mod executor;
mod gestures;
mod platform_scheduler;
pub(crate) use platform_scheduler::PlatformScheduler;
mod geometry;
mod global;
mod input;
mod inspector;
mod interactive;
mod key_dispatch;
mod keymap;
mod path_builder;
mod platform;
pub mod prelude;
/// Task timings and optional frame, input, action, and view measurements.
pub mod profiler;
mod queue;
mod scene;
/// The scheduler module provides task scheduling, execution, and timing primitives.
#[allow(missing_docs)]
pub mod scheduler;
mod shared_string;
mod shared_uri;
mod spring;
mod style;
mod styled;
mod subscription;
mod svg_renderer;
mod tab_stop;
mod taffy;
#[cfg(any(test, feature = "test-support"))]
pub mod test;
mod text_system;
mod util;
mod view;
mod window;

#[cfg(doc)]
pub mod _ownership_and_data_flow;

/// Do not touch, here be dragons for use by wgpui_macros and such.
#[doc(hidden)]
pub mod private {
    pub use anyhow;
    #[cfg(feature = "bench-support")]
    pub use criterion;
    pub use ctor;
    pub use inventory;
    pub use schemars;
    pub use serde;
    pub use serde_json;
}

mod seal {
    /// A mechanism for restricting implementations of a trait to only those in WGPUI.
    /// See: <https://predr.ag/blog/definitive-guide-to-sealed-traits-in-rust/>
    pub trait Sealed {}
}

pub use a11y::{A11yActionListener, A11ySubtreeBuilder, AriaProperties};
pub use accesskit;
pub use accesskit::Action as AccessibleAction;
pub use accesskit::{Orientation, Role, Toggled};
pub use action::*;
pub use anyhow::Result;
pub use app::*;
pub(crate) use arena::*;
pub use asset_cache::*;
pub use assets::*;
pub use color::*;
pub use default_colors::*;
pub use element::*;
pub use elements::*;
pub use executor::*;
pub use geometry::*;
pub use gestures::*;
pub use global::*;
pub use http_client;
pub use input::*;
pub use inspector::*;
pub use interactive::*;
use key_dispatch::*;
pub use keymap::*;
pub use path_builder::*;
pub use platform::*;
pub use profiler::*;
pub(crate) use queue::{PriorityQueueReceiver, PriorityQueueSender};
pub use refineable::*;
pub use scene::*;
pub use wgpui_derive::{AppContext, IntoElement, Render, VisualContext, register_action, test};
pub use wgpui_derive::{bench, ctor};
// Re-export scheduler types selectively to avoid ambiguity with executor wrappers.
// Types like Task, BackgroundExecutor, ForegroundExecutor, FallibleTask, and Priority
// are re-exported via the executor module which wraps them.
pub use scheduler::{Clock, Instant, RunnableMeta, Scheduler, SessionId, Timer};
#[cfg(any(test, feature = "test-support"))]
pub use scheduler::{SharedRng, TestScheduler, TestSchedulerConfig, Yield};
pub use shared_string::*;
pub use shared_uri::*;
pub use spring::*;
use std::{any::Any, future::Future};
pub use style::*;
pub use styled::*;
pub use subscription::*;
pub use svg_renderer::*;
pub(crate) use tab_stop::*;
use taffy::TaffyLayoutEngine;
pub use taffy::{AvailableSpace, LayoutId};
#[cfg(any(test, feature = "test-support"))]
pub use test::*;
pub use text_system::*;
#[cfg(any(test, feature = "test-support"))]
pub use util::smol_timeout;
pub use util::{FutureExt, Timeout, arc_cow::ArcCow};
pub use view::*;
pub use window::*;

/// The context trait, allows the different contexts in WGPUI to be used
/// interchangeably for certain operations.
pub trait AppContext {
    /// The result type for this context, used for async contexts that
    /// can't hold a direct reference to the application context.
    type Result<T>;

    /// Create a new entity in the app context.
    #[expect(
        clippy::wrong_self_convention,
        reason = "`App::new` is an ubiquitous function for creating entities"
    )]
    fn new<T: 'static>(
        &mut self,
        build_entity: impl FnOnce(&mut Context<T>) -> T,
    ) -> Self::Result<Entity<T>>;

    /// Reserve a slot for a entity to be inserted later.
    /// The returned [Reservation] allows you to obtain the [EntityId] for the future entity.
    fn reserve_entity<T: 'static>(&mut self) -> Self::Result<Reservation<T>>;

    /// Insert a new entity in the app context based on a [Reservation] previously obtained from [`reserve_entity`].
    ///
    /// [`reserve_entity`]: Self::reserve_entity
    fn insert_entity<T: 'static>(
        &mut self,
        reservation: Reservation<T>,
        build_entity: impl FnOnce(&mut Context<T>) -> T,
    ) -> Self::Result<Entity<T>>;

    /// Update a entity in the app context.
    fn update_entity<T, R>(
        &mut self,
        handle: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> Self::Result<R>
    where
        T: 'static;

    /// Update a entity in the app context.
    fn as_mut<'a, T>(&'a mut self, handle: &Entity<T>) -> Self::Result<GpuiBorrow<'a, T>>
    where
        T: 'static;

    /// Read a entity from the app context.
    fn read_entity<T, R>(
        &self,
        handle: &Entity<T>,
        read: impl FnOnce(&T, &App) -> R,
    ) -> Self::Result<R>
    where
        T: 'static;

    /// Update a window for the given handle.
    fn update_window<T, F>(&mut self, window: AnyWindowHandle, f: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T;

    /// Read a window off of the application context.
    fn read_window<T, R>(
        &self,
        window: &WindowHandle<T>,
        read: impl FnOnce(Entity<T>, &App) -> R,
    ) -> Result<R>
    where
        T: 'static;

    /// Spawn a future on a background thread
    fn background_spawn<R>(&self, future: impl Future<Output = R> + Send + 'static) -> Task<R>
    where
        R: Send + 'static;

    /// Read a global from this app context
    fn read_global<G, R>(&self, callback: impl FnOnce(&G, &App) -> R) -> Self::Result<R>
    where
        G: Global;
}

/// Returned by [Context::reserve_entity] to later be passed to [Context::insert_entity].
/// Allows you to obtain the [EntityId] for a entity before it is created.
pub struct Reservation<T>(pub(crate) Slot<T>);

impl<T: 'static> Reservation<T> {
    /// Returns the [EntityId] that will be associated with the entity once it is inserted.
    pub fn entity_id(&self) -> EntityId {
        self.0.entity_id()
    }
}

/// This trait is used for the different visual contexts in WGPUI that
/// require a window to be present.
pub trait VisualContext: AppContext {
    /// Returns the handle of the window associated with this context.
    fn window_handle(&self) -> AnyWindowHandle;

    /// Update a view with the given callback
    fn update_window_entity<T: 'static, R>(
        &mut self,
        entity: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Window, &mut Context<T>) -> R,
    ) -> Self::Result<R>;

    /// Create a new entity, with access to `Window`.
    fn new_window_entity<T: 'static>(
        &mut self,
        build_entity: impl FnOnce(&mut Window, &mut Context<T>) -> T,
    ) -> Self::Result<Entity<T>>;

    /// Replace the root view of a window with a new view.
    fn replace_root_view<V>(
        &mut self,
        build_view: impl FnOnce(&mut Window, &mut Context<V>) -> V,
    ) -> Self::Result<Entity<V>>
    where
        V: 'static + Render;

    /// Focus a entity in the window, if it implements the [`Focusable`] trait.
    fn focus<V>(&mut self, entity: &Entity<V>) -> Self::Result<()>
    where
        V: Focusable;
}

/// A trait for tying together the types of a WGPUI entity and the events it can
/// emit.
pub trait EventEmitter<E: Any>: 'static {}

/// A helper trait for auto-implementing certain methods on contexts that
/// can be used interchangeably.
pub trait BorrowAppContext {
    /// Set a global value on the context.
    fn set_global<T: Global>(&mut self, global: T);
    /// Updates the global state of the given type.
    fn update_global<G, R>(&mut self, f: impl FnOnce(&mut G, &mut Self) -> R) -> R
    where
        G: Global;
    /// Updates the global state of the given type, creating a default if it didn't exist before.
    fn update_default_global<G, R>(&mut self, f: impl FnOnce(&mut G, &mut Self) -> R) -> R
    where
        G: Global + Default;
}

impl<C> BorrowAppContext for C
where
    C: std::borrow::BorrowMut<App>,
{
    fn set_global<G: Global>(&mut self, global: G) {
        self.borrow_mut().set_global(global)
    }

    #[track_caller]
    fn update_global<G, R>(&mut self, f: impl FnOnce(&mut G, &mut Self) -> R) -> R
    where
        G: Global,
    {
        let mut global = self.borrow_mut().lease_global::<G>();
        let result = f(&mut global, self);
        self.borrow_mut().end_global_lease(global);
        result
    }

    fn update_default_global<G, R>(&mut self, f: impl FnOnce(&mut G, &mut Self) -> R) -> R
    where
        G: Global + Default,
    {
        self.borrow_mut().default_global::<G>();
        self.update_global(f)
    }
}

/// A flatten equivalent for anyhow `Result`s.
pub trait Flatten<T> {
    /// Convert this type into a simple `Result<T>`.
    fn flatten(self) -> Result<T>;
}

impl<T> Flatten<T> for Result<Result<T>> {
    fn flatten(self) -> Result<T> {
        self?
    }
}

impl<T> Flatten<T> for Result<T> {
    fn flatten(self) -> Result<T> {
        self
    }
}

/// Information about the GPU WGPUI is running on.
#[derive(Default, Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct GpuSpecs {
    /// Whether the GPU is really a fake (like `llvmpipe`) running on the CPU.
    pub is_software_emulated: bool,
    /// The name of the device, as reported by Vulkan.
    pub device_name: String,
    /// The name of the driver, as reported by Vulkan.
    pub driver_name: String,
    /// Further information about the driver, as reported by Vulkan.
    pub driver_info: String,
}

/// Defines a group of UI benchmark functions.
#[cfg(feature = "bench-support")]
#[macro_export]
macro_rules! bench_group {
    ($name:ident, $($benchmark:path),+ $(,)?) => {
        fn $name(criterion: &mut $crate::private::criterion::Criterion) {
            $($benchmark(criterion);)+
        }
    };
}

/// Runs benchmark groups with Criterion's command-line configuration.
#[cfg(feature = "bench-support")]
#[macro_export]
macro_rules! bench_main {
    ($($group:path),+ $(,)?) => {
        fn main() {
            let mut criterion = $crate::private::criterion::Criterion::default().configure_from_args();
            $($group(&mut criterion);)+
            criterion.final_summary();
        }
    };
}

/// One complete-scene compositor frame that was captured on request.
///
/// This is the frame the compositor presented — WGPUI chrome and any hosted GPU surface
/// content together — not a separate rendering of one embedded surface. Rows are tightly
/// packed (`width * 4` bytes each) and the channel order follows
/// [`format`](Self::format); the readback padding `wgpu` requires has already been removed,
/// so the bytes can be consumed row by row without a stride.
#[derive(Clone, Debug)]
pub struct FrameCapture {
    /// Width of the captured frame, in physical pixels.
    pub width: u32,
    /// Height of the captured frame, in physical pixels.
    pub height: u32,
    /// The format the captured pixels are in.
    pub format: wgpu::TextureFormat,
    /// Tightly packed pixels, `width * height * 4` bytes long.
    pub bytes: Vec<u8>,
}

/// Why a requested frame capture produced no frame.
#[derive(Debug)]
pub enum FrameCaptureError {
    /// The native surface cannot be copied on this adapter.
    UnsupportedUsage,
    /// A previously submitted frame capture is still being read back.
    Busy,
    /// The frame to capture had a zero width or height.
    InvalidDimensions,
    /// The compositor's surface format is not one this readback can copy.
    UnsupportedFormat(wgpu::TextureFormat),
    /// The readback buffer could not be mapped after its copy completed.
    BufferMap,
    /// Waiting for the readback copy to complete failed.
    DevicePoll(wgpu::PollError),
}

impl std::fmt::Display for FrameCaptureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedUsage => {
                formatter.write_str("surface does not support frame capture copies")
            }
            Self::Busy => formatter.write_str("a frame capture is already in flight"),
            Self::InvalidDimensions => {
                formatter.write_str("frame capture dimensions must be non-zero")
            }
            Self::UnsupportedFormat(format) => {
                write!(formatter, "unsupported frame capture format: {format:?}")
            }
            Self::BufferMap => formatter.write_str("frame capture buffer mapping failed"),
            Self::DevicePoll(error) => write!(formatter, "frame capture GPU poll failed: {error}"),
        }
    }
}

impl std::error::Error for FrameCaptureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DevicePoll(error) => Some(error),
            Self::InvalidDimensions
            | Self::UnsupportedFormat(_)
            | Self::BufferMap
            | Self::UnsupportedUsage
            | Self::Busy => None,
        }
    }
}
