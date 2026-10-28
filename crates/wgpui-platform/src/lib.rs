//! Native platform entry for wgpui-component.
//!
//! Upstream gpui-component examples call `wgpui_platform::application()`. This
//! crate is aliased as `gpui_platform` in the workspace so vendored source
//! keeps compiling while routing through wgpui.

pub use wgpui::Application;

/// Returns a wgpui application ready for `Application::run`.
pub fn application() -> Application {
    Application::new()
}

/// Creates WGPUI's real offscreen renderer when an adapter is available.
#[cfg(any(feature = "test-support", feature = "bench-support"))]
pub use wgpui::current_headless_renderer;

/// Platform services usable without constructing a native event loop.
pub struct PlatformServices;

impl PlatformServices {
    /// Returns the production font discovery and text shaping implementation.
    pub fn text_system(&self) -> std::sync::Arc<dyn wgpui::PlatformTextSystem> {
        wgpui::current_platform_text_system()
    }

    /// Creates native text services and reports font-service initialization failures.
    pub fn try_text_system(&self) -> wgpui::Result<std::sync::Arc<dyn wgpui::PlatformTextSystem>> {
        wgpui::try_current_platform_text_system()
    }
}

/// Returns platform services for test or native application setup.
/// Window creation remains owned by Application.
pub fn current_platform(_headless: bool) -> PlatformServices {
    PlatformServices
}
