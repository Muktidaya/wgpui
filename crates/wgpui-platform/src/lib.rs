//! Native platform entry for wgpui-component.
//!
//! Upstream gpui-component examples call `gpui_platform::application()`. This
//! crate is aliased as `gpui_platform` in the workspace so vendored source
//! keeps compiling while routing through wgpui.

pub use wgpui::Application;

/// Returns a wgpui application ready for `Application::run`.
pub fn application() -> Application {
    Application::new()
}
