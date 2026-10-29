use crate::{
    Bounds, DevicePixels, Font, FontId, FontMetrics, FontRun, GlyphId, LineLayout, Pixels,
    PlatformTextSystem, RenderGlyphParams, Size,
};
use anyhow::{Result, anyhow};
use std::{borrow::Cow, sync::Arc};

pub(crate) fn try_new() -> Result<Arc<dyn PlatformTextSystem>> {
    #[cfg(any(
        target_os = "macos",
        target_os = "windows",
        target_os = "linux",
        target_os = "freebsd"
    ))]
    {
        #[cfg(target_os = "macos")]
        let system = super::macos_text_system::CoreTextSystem::new();
        #[cfg(target_os = "windows")]
        let system = super::windows_text_system::DirectWriteTextSystem::new()?;
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        let system = super::pango_text_system::PangoTextSystem::new()?;
        Ok(Arc::new(system))
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "windows",
        target_os = "linux",
        target_os = "freebsd"
    )))]
    {
        Err(anyhow!(
            "Native text services support macOS, Windows, Linux and FreeBSD"
        ))
    }
}

pub(crate) fn new() -> Arc<dyn PlatformTextSystem> {
    try_new().unwrap_or_else(|error| {
        let reason = format!("Native text initialization failed: {error:#}");
        log::error!("{reason}");
        Arc::new(UnavailableTextSystem(reason))
    })
}

// The existing infallible service accessor must retain initialization failures for its fallible methods.
struct UnavailableTextSystem(String);
impl UnavailableTextSystem {
    fn error(&self) -> anyhow::Error {
        anyhow!("{}", self.0)
    }
}
impl PlatformTextSystem for UnavailableTextSystem {
    fn add_fonts(&self, _fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        Err(self.error())
    }
    fn all_font_names(&self) -> Vec<String> {
        Vec::new()
    }
    fn font_id(&self, _descriptor: &Font) -> Result<FontId> {
        Err(self.error())
    }
    fn font_metrics(&self, _font_id: FontId) -> FontMetrics {
        log::error!("{}", self.0);
        FontMetrics {
            units_per_em: 1000,
            ascent: 0.0,
            descent: 0.0,
            line_gap: 0.0,
            underline_position: 0.0,
            underline_thickness: 0.0,
            cap_height: 0.0,
            x_height: 0.0,
            bounding_box: Bounds::default(),
        }
    }
    fn typographic_bounds(&self, _font_id: FontId, _glyph_id: GlyphId) -> Result<Bounds<f32>> {
        Err(self.error())
    }
    fn advance(&self, _font_id: FontId, _glyph_id: GlyphId) -> Result<Size<f32>> {
        Err(self.error())
    }
    fn glyph_for_char(&self, _font_id: FontId, _character: char) -> Option<GlyphId> {
        log::error!("{}", self.0);
        None
    }
    fn glyph_raster_bounds(&self, _parameters: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        Err(self.error())
    }
    fn rasterize_glyph(
        &self,
        _parameters: &RenderGlyphParams,
        _bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        Err(self.error())
    }
    fn layout_line(&self, text: &str, font_size: Pixels, _runs: &[FontRun]) -> LineLayout {
        log::error!("{}", self.0);
        LineLayout {
            font_size,
            len: text.len(),
            ..LineLayout::default()
        }
    }
}
