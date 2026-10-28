use crate::{
    AssetSource, DevicePixels, IsZero, RenderImage, Result, SharedString, Size, unpremultiply_rgba,
};
use image::Frame;
use resvg::tiny_skia::Pixmap;
use smallvec::SmallVec;
use std::{
    hash::Hash,
    sync::{Arc, LazyLock},
};

/// When rendering SVGs, we render them at twice the size to get a higher-quality result.
pub const SMOOTH_SVG_SCALE_FACTOR: f32 = 2.;

#[derive(Clone, PartialEq, Hash, Eq)]
#[doc(hidden)]
pub struct RenderSvgParams {
    pub(crate) path: SharedString,
    pub(crate) size: Size<DevicePixels>,
}

#[derive(Clone)]
/// A struct holding everything necessary to render SVGs.
pub struct SvgRenderer {
    asset_source: Arc<dyn AssetSource>,
    usvg_options: Arc<usvg::Options<'static>>,
}

/// The size in which to render the SVG.
pub enum SvgSize {
    /// An absolute size in device pixels.
    Size(Size<DevicePixels>),
    /// A scaling factor to apply to the size provided by the SVG.
    ScaleFactor(f32),
}

impl SvgRenderer {
    /// Creates a new SVG renderer with the provided asset source.
    pub fn new(asset_source: Arc<dyn AssetSource>) -> Self {
        static FONT_DB: LazyLock<Arc<usvg::fontdb::Database>> =
            LazyLock::new(|| Arc::new(crate::platform::system_fonts::database()));
        let default_font_resolver = usvg::FontResolver::default_font_selector();
        let font_resolver = Box::new(
            move |font: &usvg::Font, db: &mut Arc<usvg::fontdb::Database>| {
                if db.is_empty() {
                    *db = FONT_DB.clone();
                }
                default_font_resolver(font, db)
            },
        );
        let options = usvg::Options {
            font_resolver: usvg::FontResolver {
                select_font: font_resolver,
                select_fallback: usvg::FontResolver::default_fallback_selector(),
            },
            ..Default::default()
        };
        Self {
            asset_source,
            usvg_options: Arc::new(options),
        }
    }

    /// Renders the given bytes into a straight-alpha RGBA image buffer.
    ///
    /// The final argument is retained for source compatibility. Both values now
    /// produce the RGBA format required by every processed image and color glyph.
    pub fn render_single_frame(
        &self,
        bytes: &[u8],
        scale_factor: f32,
        _legacy_bgra: bool,
    ) -> Result<Arc<RenderImage>, usvg::Error> {
        self.render_pixmap(
            bytes,
            SvgSize::ScaleFactor(scale_factor * SMOOTH_SVG_SCALE_FACTOR),
        )
        .map(|pixmap| {
            let mut buffer =
                image::ImageBuffer::from_raw(pixmap.width(), pixmap.height(), pixmap.take())
                    .unwrap();

            unpremultiply_rgba(&mut buffer);

            let mut image = RenderImage::new(SmallVec::from_const([Frame::new(buffer)]));
            image.scale_factor = SMOOTH_SVG_SCALE_FACTOR;
            Arc::new(image)
        })
    }

    pub(crate) fn render_alpha_mask(
        &self,
        params: &RenderSvgParams,
        bytes: Option<&[u8]>,
    ) -> Result<Option<(Size<DevicePixels>, Vec<u8>)>> {
        anyhow::ensure!(!params.size.is_zero(), "can't render at a zero size");

        let render_pixmap = |bytes| {
            let pixmap = self.render_pixmap(bytes, SvgSize::Size(params.size))?;

            // Convert the pixmap's pixels into an alpha mask.
            let size = Size::new(
                DevicePixels(pixmap.width() as i32),
                DevicePixels(pixmap.height() as i32),
            );
            let alpha_mask = pixmap
                .pixels()
                .iter()
                .map(|p| p.alpha())
                .collect::<Vec<_>>();

            Ok(Some((size, alpha_mask)))
        };

        if let Some(bytes) = bytes {
            render_pixmap(bytes)
        } else if let Some(bytes) = self.asset_source.load(&params.path)? {
            render_pixmap(&bytes)
        } else {
            Ok(None)
        }
    }

    fn render_pixmap(&self, bytes: &[u8], size: SvgSize) -> Result<Pixmap, usvg::Error> {
        let tree = usvg::Tree::from_data(bytes, &self.usvg_options)?;
        let svg_size = tree.size();
        let scale = match size {
            SvgSize::Size(size) => size.width.0 as f32 / svg_size.width(),
            SvgSize::ScaleFactor(scale) => scale,
        };

        // Render the SVG to a pixmap with the specified width and height.
        let mut pixmap = resvg::tiny_skia::Pixmap::new(
            (svg_size.width() * scale) as u32,
            (svg_size.height() * scale) as u32,
        )
        .ok_or(usvg::Error::InvalidSize)?;

        let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);

        resvg::render(&tree, transform, &mut pixmap.as_mut());

        Ok(pixmap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context as _;

    const HALF_RED: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="2"><rect width="2" height="2" fill="red" opacity="0.5"/></svg>"#;

    #[test]
    fn svg_alpha_mask_preserves_scaled_size_and_transparency() -> anyhow::Result<()> {
        let renderer = SvgRenderer::new(Arc::new(()));
        let requested = Size::new(DevicePixels(8), DevicePixels(4));
        let (size, alpha) = renderer
            .render_alpha_mask(
                &RenderSvgParams {
                    path: "inline.svg".into(),
                    size: requested,
                },
                Some(HALF_RED),
            )?
            .context("inline SVG must produce an alpha mask")?;
        assert_eq!(size, requested);
        assert_eq!(alpha.len(), 32);
        for row in alpha.chunks_exact(8) {
            assert_eq!(row, &[128, 128, 128, 128, 0, 0, 0, 0]);
        }
        Ok(())
    }

    #[test]
    fn svg_single_frame_preserves_supersampling_and_straight_rgba() -> anyhow::Result<()> {
        let renderer = SvgRenderer::new(Arc::new(()));
        let rgba = renderer.render_single_frame(HALF_RED, 1.0, false)?;
        let legacy = renderer.render_single_frame(HALF_RED, 1.0, true)?;
        assert_eq!(rgba.frame_count(), 1);
        assert_eq!(rgba.size(0), Size::new(DevicePixels(8), DevicePixels(4)));
        assert_eq!(legacy.size(0), rgba.size(0));
        assert_eq!(rgba.scale_factor, SMOOTH_SVG_SCALE_FACTOR);
        let rgba = rgba.as_bytes(0).context("SVG frame must contain pixels")?;
        let legacy = legacy
            .as_bytes(0)
            .context("SVG frame must contain pixels")?;
        assert_eq!(rgba, legacy);
        for pixel in rgba.chunks_exact(4) {
            if pixel[3] == 0 {
                assert_eq!(pixel, &[0, 0, 0, 0]);
            } else {
                // Unpremultiplication passes through quantized 8-bit alpha.
                assert!(pixel[0].abs_diff(255) <= 1);
                assert_eq!(&pixel[1..], &[0, 0, 128]);
            }
        }
        Ok(())
    }

    #[test]
    fn svg_text_uses_system_fonts_and_preserves_clipping() -> anyhow::Result<()> {
        let mut options = usvg::Options::default();
        *options.fontdb_mut() = crate::platform::system_fonts::database();
        let renderer = SvgRenderer {
            asset_source: Arc::new(()),
            usvg_options: Arc::new(options),
        };
        let pixmap = renderer.render_pixmap(
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="32"><defs><clipPath id="clip"><rect width="16" height="32"/></clipPath></defs><text x="2" y="24" font-family="sans-serif" font-size="24" clip-path="url(#clip)">Ag</text></svg>"#,
            SvgSize::ScaleFactor(1.0),
        )?;
        assert!(pixmap.pixels().iter().any(|pixel| pixel.alpha() > 0));
        for row in pixmap.pixels().chunks_exact(64) {
            assert!(row.iter().skip(16).all(|pixel| pixel.alpha() == 0));
        }
        Ok(())
    }

    #[test]
    fn svg_invalid_input_and_missing_assets_remain_fallible() -> anyhow::Result<()> {
        let renderer = SvgRenderer::new(Arc::new(()));
        assert!(
            renderer
                .render_single_frame(b"invalid SVG", 1.0, false)
                .is_err()
        );
        assert!(renderer.render_single_frame(HALF_RED, 0.0, false).is_err());
        let mut params = RenderSvgParams {
            path: "missing.svg".into(),
            size: Size::new(DevicePixels(8), DevicePixels(4)),
        };
        assert!(renderer.render_alpha_mask(&params, None)?.is_none());
        params.size.width = DevicePixels(0);
        assert!(renderer.render_alpha_mask(&params, Some(HALF_RED)).is_err());
        Ok(())
    }
}
