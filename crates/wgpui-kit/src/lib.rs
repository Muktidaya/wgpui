//! WGPUI Kit: one dependency for native wgpu/winit applications.
//!
//! The framework, Base, styled components, and assets are available through
//! this façade. Browser integrations and upstream frame profiling are not ported.
//!
//! | Path            | Crate             | Feature          |
//! | --------------- | ----------------- | ---------------- |
//! | `wgpui_kit::*`   | `gpui`            | always           |
//! | [`platform`]    | `gpui_platform`   | always           |
//! | [`base`]        | `gpui-base`       | always           |
//! | [`component`]   | `gpui-component`  | `component` (on) |
//! | [`assets`]      | `gpui-kit-assets` | `assets` (on)    |
//!
//! [`application`] opens the platform and [`init`] initializes the enabled
//! layers:
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
//!         cx.spawn(async move |cx| {
//!             cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Hello))
//!                 .expect("failed to open window");
//!         })
//!         .detach();
//!     });
//! }
//! ```
//!
//! See [`component`] for the same program with the styled component library.

/// Defines unit actions without requiring consumers to depend on GPUI under the
/// crate name `gpui`.
///
/// GPUI's original macro spells its derive as `gpui::Action`, which does not
/// resolve when GPUI is consumed solely through this facade.
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
pub use ::gpui;

/// UI integration testing: render real components in headless windows, dispatch
/// pointer and keyboard events, and assert state, focus, layout and callbacks.
/// Run tests with `#[wgpui_kit::test]`; use this module to interact with their UI.
#[cfg(feature = "test-support")]
pub mod test;

pub use ::gpui_base as base;
pub use ::gpui_platform as platform;

/// The styled component library.
///
/// ```no_run
/// use wgpui_kit::component::button::*;
/// use wgpui_kit::component::Root;
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
///         cx.spawn(async move |cx| {
///             cx.open_window(WindowOptions::default(), |window, cx| {
///                 let view = cx.new(|_| Hello);
///                 cx.new(|cx| Root::new(view, window, cx))
///             })
///             .expect("failed to open window");
///         })
///         .detach();
///     });
/// }
/// ```
#[cfg(feature = "component")]
pub use ::gpui_component as component;
#[cfg(feature = "assets")]
pub use ::gpui_kit_assets as assets;

pub use ::gpui_platform::application;

/// Initializes every enabled layer. Call it once, before using anything else.
///
/// With the `component` feature (on by default) this is
/// `gpui_component::init`, which also initializes `gpui-base`; otherwise it
/// is `gpui_base::init`.
pub fn init(cx: &mut App) {
    #[cfg(feature = "component")]
    gpui_component::init(cx);
    #[cfg(not(feature = "component"))]
    gpui_base::init(cx);
}

/// Fluent UI test observation, inert unless `test-support` is enabled.
pub use gpui_base::TestSupportExt;
