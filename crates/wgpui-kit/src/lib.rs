//! WGPUI Kit: one dependency for native wgpu/winit applications.
//!
//! The framework, Base, styled components, and assets are available through
//! this façade. Kit behavior runs on WGPUI’s wgpu and winit backend.
//!
//! | Path            | Crate             | Feature          |
//! | --------------- | ----------------- | ---------------- |
//! | `wgpui_kit::*`   | `wgpui`           | always           |
//! | [`platform`]    | `wgpui-platform`  | always           |
//! | [`base`]        | `wgpui-base`      | always           |
//! | [`component`]   | `wgpui-component` | `component` (on) |
//! | [`assets`]      | `wgpui-kit-assets`| `assets` (on)    |
//!
//! [`application`] opens the platform and [`init`] initializes the enabled
//! layers. [`open_window`] installs Base's root lifecycle around the content:
//!
//! ```no_run
//! use wgpui_kit::*;
//!
//! actions!(hello, [Quit]);
//!
//! struct Hello;
//!
//! impl Render for Hello {
//!     fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
//!         div().child("Hello, World!")
//!     }
//! }
//!
//! fn main() {
//!     wgpui_kit::application().run(|cx| {
//!         wgpui_kit::init(cx);
//!         wgpui_kit::open_window(WindowOptions::default(), cx, |_, cx| {
//!             cx.new(|_| Hello)
//!         })
//!         .expect("failed to open window");
//!     });
//! }
//! ```
//!
//! See [`component`] for the same program with the styled component library.

/// Defines unit actions through Kit's reexported action derive.
///
/// The expansion resolves Kit through this facade, including renamed
/// dependencies, without requiring a separate framework dependency.
#[macro_export]
macro_rules! actions {
    ($namespace:path, [ $( $(#[$attr:meta])* $name:ident),* $(,)? ]) => {
        $(
            #[derive(
                ::std::clone::Clone,
                ::std::cmp::PartialEq,
                ::std::default::Default,
                ::std::fmt::Debug,
                $crate::Action
            )]
            #[action(namespace = $namespace)]
            $(#[$attr])*
            pub struct $name;
        )*
    };
    ([ $( $(#[$attr:meta])* $name:ident),* $(,)? ]) => {
        $(
            #[derive(
                ::std::clone::Clone,
                ::std::cmp::PartialEq,
                ::std::default::Default,
                ::std::fmt::Debug,
                $crate::Action
            )]
            $(#[$attr])*
            pub struct $name;
        )*
    };
}

extern crate self as wgpui_kit;

pub use ::gpui::*;

#[doc(hidden)]
pub use crate as gpui;

#[doc(hidden)]
pub const GPUI_FAST: bool = false;

/// UI integration testing: render real components in headless windows, dispatch
/// pointer and keyboard events, and assert state, focus, layout and callbacks.
/// Run tests with `#[wgpui_kit::test]`; use this module to interact with their UI.
#[cfg(feature = "test-support")]
pub mod test;

pub use ::gpui_base as base;
pub use ::gpui_platform as platform;
pub use gpui_base::is_mobile;

/// The styled component library.
///
/// ```no_run
/// use wgpui_kit::component::button::*;
/// use wgpui_kit::*;
///
/// struct Hello;
///
/// impl Render for Hello {
///     fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
///         div().child(Button::new("ok").primary().label("Let's Go!"))
///     }
/// }
///
/// fn main() {
///     wgpui_kit::application().run(|cx| {
///         wgpui_kit::init(cx);
///         wgpui_kit::open_window(WindowOptions::default(), cx, |_, cx| {
///             cx.new(|_| Hello)
///         })
///         .expect("failed to open window");
///     });
/// }
/// ```
#[cfg(feature = "component")]
pub use ::gpui_component as component;
#[cfg(feature = "assets")]
pub use ::gpui_kit_assets as assets;

pub use ::gpui_platform::application;

/// Opens application content inside Base's root lifecycle and plugin host.
pub fn open_window<V: Render>(
    options: WindowOptions,
    cx: &mut App,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> Result<(AnyWindowHandle, Entity<V>)> {
    let mut built = None;
    let window = cx.open_window(options, |window, cx| {
        let view = build(window, cx);
        built = Some(view.clone());
        cx.new(|cx| base::Root::new(view, window, cx))
    })?;
    let content = built
        .ok_or_else(|| private::anyhow::Error::msg("window did not build application content"))?;
    Ok((window.into(), content))
}

/// Initializes every enabled layer. Call it once, before using anything else.
///
/// With the `component` feature (on by default) this is
/// `component::init`, which also initializes Base; otherwise it
/// is `base::init`.
pub fn init(cx: &mut App) {
    #[cfg(feature = "component")]
    gpui_component::init(cx);
    #[cfg(not(feature = "component"))]
    gpui_base::init(cx);
}

/// Fluent UI test observation, inert unless `test-support` is enabled.
pub use gpui_base::TestSupportExt;
