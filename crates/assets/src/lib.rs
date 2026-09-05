/// Embed application assets for GPUI Component.
///
/// This crate embeds Lucide SVGs for `wgpui-component`'s `IconName`.
///
/// ## Usage
///
/// ```rust,no_run
/// use wgpui::*;
/// use wgpui_component_assets::Assets;
///
/// let app = Application::new().with_assets(Assets);
/// ```
///
/// ## Platform Differences
///
/// - **Native (Desktop)**: Icons are embedded in the binary using RustEmbed
/// - **WASM (Web)**: Icons are downloaded from CDN using web_sys::Request
///   - This significantly reduces WASM bundle size
///   - Icons are downloaded on-demand when first used
///   - Downloaded icons are cached in memory
#[cfg(not(target_family = "wasm"))]
mod native_assets;

#[cfg(target_family = "wasm")]
mod wasm_assets;

#[cfg(not(target_family = "wasm"))]
pub use native_assets::Assets;

#[cfg(target_family = "wasm")]
pub use wasm_assets::Assets;
