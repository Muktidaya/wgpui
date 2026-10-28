// SPDX-License-Identifier: Apache-2.0
// Copyright 2022 - 2025 Zed Industries, Inc.
// Core Text line/glyph integration is adapted from gpui-pre-apple 0.3.10.

use crate::{
    Bounds, DevicePixels, Font, FontId, FontMetrics, FontRun, GlyphId, LineLayout, Pixels,
    PlatformTextSystem, RenderGlyphParams, SUBPIXEL_VARIANTS_X, ShapedGlyph, ShapedRun, Size,
    point, px, size, swap_rgba_pa_to_bgra,
};
use anyhow::{Context as _, Result, anyhow, ensure};
use collections::{HashMap, HashSet};
use core_foundation::{
    array::{CFArray, CFArrayRef},
    attributed_string::CFMutableAttributedString,
    base::{CFEqual, CFHash, CFRange, CFType, CFTypeRef, TCFType},
    data::{CFData, CFDataRef},
    dictionary::CFDictionary,
    number::CFNumber,
    string::{CFString, CFStringCreateWithCharacters, CFStringRef},
};
use core_graphics::{
    base::{CGGlyph, kCGImageAlphaPremultipliedLast},
    color_space::CGColorSpace,
    context::{CGContext, CGTextDrawingMode},
    geometry::{CGAffineTransform, CGPoint, CGSize},
};
use core_text::{
    font::{CTFont, CTFontRef},
    font_descriptor::{
        CTFontDescriptor, CTFontDescriptorCreateWithAttributes, CTFontDescriptorRef,
        kCTFontCascadeListAttribute, kCTFontFamilyNameAttribute, kCTFontFeatureSettingsAttribute,
        kCTFontSlantTrait, kCTFontTraitsAttribute, kCTFontWeightTrait,
    },
    line::CTLine,
    string_attributes::{kCTFontAttributeName, kCTParagraphStyleAttributeName},
};
use parking_lot::RwLock;
use std::{borrow::Cow, ffi::c_void, ptr};

const BASE_FONT_SIZE: f64 = 16.0;
type Attributes = CFDictionary<CFString, CFType>;

#[repr(C)]
struct ParagraphStyleSetting {
    specifier: u32,
    value_size: usize,
    value: *const c_void,
}

#[link(name = "CoreText", kind = "framework")]
unsafe extern "C" {
    fn CTFontCreateUIFontForLanguage(kind: u32, size: f64, language: CFStringRef) -> CTFontRef;
    fn CTFontCreateCopyWithAttributes(
        font: CTFontRef,
        size: f64,
        matrix: *const CGAffineTransform,
        descriptor: CTFontDescriptorRef,
    ) -> CTFontRef;
    fn CTParagraphStyleCreate(
        settings: *const ParagraphStyleSetting,
        setting_count: usize,
    ) -> CFTypeRef;
    fn CTFontManagerCopyAvailableFontFamilyNames() -> CFArrayRef;
    fn CTFontManagerCopyAvailablePostScriptNames() -> CFArrayRef;
    fn CTFontManagerCreateFontDescriptorsFromData(data: CFDataRef) -> CFArrayRef;
    static kCTFontOpenTypeFeatureTag: CFStringRef;
    static kCTFontOpenTypeFeatureValue: CFStringRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFLocaleCopyPreferredLanguages() -> CFArrayRef;
}

pub(crate) struct CoreTextSystem(RwLock<State>);

struct State {
    fonts: Vec<CTFont>,
    selections: HashMap<Font, FontId>,
    native_ids: HashMap<usize, Vec<FontId>>,
    memory_fonts: Vec<CTFont>,
    names: HashSet<String>,
}

impl CoreTextSystem {
    pub(super) fn system_font(monospace: bool) -> Result<CTFont> {
        let system = Self::new();
        let mut state = system.0.write();
        let requested = crate::font(if monospace {
            ".SystemMonospaceFont"
        } else {
            ".SystemUIFont"
        });
        let id = state.resolve(&requested)?;
        Ok(state.font(id)?.clone())
    }

    pub(crate) fn new() -> Self {
        let mut names = HashSet::default();
        for values in unsafe {
            [
                CTFontManagerCopyAvailableFontFamilyNames(),
                CTFontManagerCopyAvailablePostScriptNames(),
            ]
        } {
            if !values.is_null() {
                let values: CFArray<CFString> = unsafe { CFArray::wrap_under_create_rule(values) };
                names.extend(values.into_iter().map(|name| name.to_string()));
            }
        }
        Self(RwLock::new(State {
            fonts: Vec::new(),
            selections: HashMap::default(),
            native_ids: HashMap::default(),
            memory_fonts: Vec::new(),
            names,
        }))
    }
}

impl State {
    fn font(&self, id: FontId) -> Result<&CTFont> {
        self.fonts
            .get(id.0)
            .context("Unknown Core Text font identifier")
    }

    fn register(&mut self, font: CTFont) -> FontId {
        let font = font.clone_with_font_size(BASE_FONT_SIZE);
        let hash = unsafe { CFHash(font.as_CFTypeRef()) };
        if let Some(ids) = self.native_ids.get(&hash) {
            for id in ids {
                if let Some(existing) = self.fonts.get(id.0)
                    && unsafe { CFEqual(existing.as_CFTypeRef(), font.as_CFTypeRef()) != 0 }
                {
                    return *id;
                }
            }
        }
        let id = FontId(self.fonts.len());
        self.fonts.push(font);
        self.native_ids.entry(hash).or_default().push(id);
        id
    }

    fn resolve(&mut self, requested: &Font) -> Result<FontId> {
        if let Some(id) = self.selections.get(requested) {
            return Ok(*id);
        }
        ensure!(requested.weight.0.is_finite(), "Font weight must be finite");
        let base = match requested.family.as_ref() {
            ".SystemUIFont" | ".ZedSans" | "Zed Plex Sans" => {
                let raw = unsafe { CTFontCreateUIFontForLanguage(2, BASE_FONT_SIZE, ptr::null()) };
                ensure!(!raw.is_null(), "Core Text could not create the OS UI font");
                unsafe { CTFont::wrap_under_create_rule(raw) }
            }
            ".SystemMonospaceFont" | ".ZedMono" | "Zed Plex Mono" => {
                let font =
                    objc2_app_kit::NSFont::monospacedSystemFontOfSize_weight(BASE_FONT_SIZE, 0.0);
                core_text::font::new_from_name(&font.fontName().to_string(), BASE_FONT_SIZE)
                    .map_err(|()| anyhow!("Core Text could not create the OS monospace font"))?
            }
            family => {
                if let Some(font) = self
                    .memory_fonts
                    .iter()
                    .find(|font| font.family_name() == family || font.postscript_name() == family)
                {
                    font.clone()
                } else {
                    ensure!(
                        self.names.contains(family),
                        "Font family {family:?} is not installed"
                    );
                    core_text::font::new_from_name(family, BASE_FONT_SIZE)
                        .map_err(|()| anyhow!("Core Text could not create font {family:?}"))?
                }
            }
        };
        let font = configure_font(&base, requested)?;
        let id = self.register(font);
        self.selections.insert(requested.clone(), id);
        Ok(id)
    }

    fn shape(
        &mut self,
        text: &str,
        font_size: Pixels,
        font_runs: &[FontRun],
    ) -> Result<LineLayout> {
        ensure!(
            font_size.0.is_finite() && font_size.0 >= 0.0,
            "Font size must be finite and nonnegative"
        );
        if font_size.0 == 0.0 {
            return Ok(LineLayout {
                font_size,
                width: px(0.0),
                ascent: px(0.0),
                descent: px(0.0),
                runs: Vec::new(),
                len: text.len(),
            });
        }
        let mut utf16_to_utf8 = Vec::new();
        for (offset, character) in text.char_indices() {
            utf16_to_utf8.extend(std::iter::repeat_n(offset, character.len_utf16()));
        }
        utf16_to_utf8.push(text.len());
        let characters = text.encode_utf16().collect::<Vec<_>>();
        // Construct from UTF-16 so a leading BOM cannot disappear and shift the byte mapping.
        let raw = unsafe {
            CFStringCreateWithCharacters(
                ptr::null(),
                characters.as_ptr(),
                isize::try_from(characters.len())?,
            )
        };
        ensure!(
            !raw.is_null(),
            "Core Foundation could not construct the line"
        );
        let content = unsafe { CFString::wrap_under_create_rule(raw) };
        let mut string = CFMutableAttributedString::new();
        string.replace_str(&content, CFRange::init(0, 0));
        ensure!(
            usize::try_from(string.char_len())? == characters.len(),
            "Core Foundation changed the line's UTF-16 length"
        );
        let mut byte_offset = 0_usize;
        let mut ascent = 0.0_f32;
        let mut descent = 0.0_f32;
        let mut separate_run = true;
        for run in font_runs {
            let end = byte_offset
                .checked_add(run.len)
                .context("Font run length overflow")?;
            ensure!(
                end <= text.len() && text.is_char_boundary(end),
                "Font run ends outside a UTF-8 boundary"
            );
            let start_utf16 = utf16_to_utf8.partition_point(|offset| *offset < byte_offset);
            let end_utf16 = utf16_to_utf8.partition_point(|offset| *offset < end);
            let font = self.font(run.font_id)?;
            ascent = ascent.max((font.ascent() / font.pt_size() * f64::from(font_size.0)) as f32);
            descent =
                descent.max((font.descent() / font.pt_size() * f64::from(font_size.0)) as f32);
            let size = if separate_run {
                font_size.0.next_up()
            } else {
                font_size.0
            };
            if end_utf16 > start_utf16 {
                // Distinct sizes prevent Core Text from joining ligatures across explicit font runs.
                unsafe {
                    string.set_attribute(
                        CFRange::init(
                            isize::try_from(start_utf16)?,
                            isize::try_from(end_utf16 - start_utf16)?,
                        ),
                        kCTFontAttributeName,
                        &font.clone_with_font_size(f64::from(size)),
                    );
                }
            }
            separate_run = !separate_run;
            byte_offset = end;
        }
        ensure!(byte_offset == text.len(), "Font runs do not cover the line");
        if text.contains('\t') {
            let run = font_runs.first().context("Tab line has no font")?;
            let font = self.font(run.font_id)?;
            let character = u16::from(b' ');
            let mut glyph = 0;
            unsafe { font.get_glyphs_for_characters(&character, &mut glyph, 1) };
            ensure!(glyph != 0, "Tab font has no space glyph");
            let mut advance = CGSize::new(0.0, 0.0);
            unsafe { font.get_advances_for_glyphs(0, &glyph, &mut advance, 1) };
            let interval = advance.width / font.pt_size() * f64::from(font_size.0) * 4.0;
            ensure!(
                interval.is_finite() && interval > 0.0,
                "Tab interval is invalid"
            );
            // Remove the native 28-point stops so editor tabs follow the four-cell interval.
            let tab_stops = CFArray::<CFType>::from_CFTypes(&[]);
            let tab_stops_ref = tab_stops.as_concrete_TypeRef();
            let settings = [
                ParagraphStyleSetting {
                    specifier: 4,
                    value_size: std::mem::size_of::<CFArrayRef>(),
                    value: (&tab_stops_ref as *const CFArrayRef).cast(),
                },
                ParagraphStyleSetting {
                    specifier: 5,
                    value_size: std::mem::size_of::<f64>(),
                    value: (&interval as *const f64).cast(),
                },
            ];
            let raw = unsafe { CTParagraphStyleCreate(settings.as_ptr(), settings.len()) };
            ensure!(!raw.is_null(), "Core Text could not configure editor tabs");
            let paragraph = unsafe { CFType::wrap_under_create_rule(raw) };
            unsafe {
                string.set_attribute(
                    CFRange::init(0, string.char_len()),
                    kCTParagraphStyleAttributeName,
                    &paragraph,
                );
            }
        }
        let line = CTLine::new_with_attributed_string(string.as_concrete_TypeRef());
        let mut shaped = Vec::<ShapedRun>::new();
        for run in line.glyph_runs().into_iter() {
            let attributes = run
                .attributes()
                .context("Core Text run has no attributes")?;
            let key = unsafe { CFString::wrap_under_get_rule(kCTFontAttributeName) };
            let value = attributes.find(&key).context("Core Text run has no font")?;
            let font = value
                .downcast::<CTFont>()
                .context("Core Text run font has an unexpected type")?;
            let color = is_color_font(&font);
            let id = self.register(font);
            let glyphs = run.glyphs();
            let positions = run.positions();
            let indices = run.string_indices();
            ensure!(
                glyphs.len() == positions.len() && glyphs.len() == indices.len(),
                "Core Text run buffers disagree"
            );
            let mut values = Vec::with_capacity(glyphs.len());
            for ((glyph, position), index) in
                glyphs.iter().zip(positions.iter()).zip(indices.iter())
            {
                let index = usize::try_from(*index)?;
                let index = *utf16_to_utf8
                    .get(index)
                    .context("Core Text glyph index exceeds its line")?;
                values.push(ShapedGlyph {
                    id: GlyphId(u32::from(*glyph)),
                    position: point(px(position.x as f32), px(-position.y as f32)),
                    index,
                    is_emoji: color,
                });
            }
            if let Some(previous) = shaped.last_mut().filter(|previous| previous.font_id == id) {
                previous.glyphs.extend(values);
            } else {
                shaped.push(ShapedRun {
                    font_id: id,
                    glyphs: values,
                });
            }
        }
        Ok(LineLayout {
            font_size,
            width: px(line.get_typographic_bounds().width as f32),
            ascent: px(ascent),
            descent: px(descent),
            runs: shaped,
            len: text.len(),
        })
    }
}

fn configure_font(base: &CTFont, requested: &Font) -> Result<CTFont> {
    let traits = unsafe {
        Attributes::from_CFType_pairs(&[
            (
                CFString::wrap_under_get_rule(kCTFontWeightTrait),
                CFNumber::from(normalized_weight(requested.weight.0)).as_CFType(),
            ),
            (
                CFString::wrap_under_get_rule(kCTFontSlantTrait),
                CFNumber::from(if requested.style == crate::FontStyle::Normal {
                    0.0
                } else {
                    0.2
                })
                .as_CFType(),
            ),
        ])
    };
    let mut attributes = vec![(
        unsafe { CFString::wrap_under_get_rule(kCTFontTraitsAttribute) },
        traits.as_CFType(),
    )];
    let mut features = Vec::new();
    for (tag, value) in requested.features.tag_value_list() {
        ensure!(
            tag.len() == 4 && tag.is_ascii(),
            "OpenType tags must contain four ASCII bytes"
        );
        features.push(unsafe {
            Attributes::from_CFType_pairs(&[
                (
                    CFString::wrap_under_get_rule(kCTFontOpenTypeFeatureTag),
                    CFString::new(tag).as_CFType(),
                ),
                (
                    CFString::wrap_under_get_rule(kCTFontOpenTypeFeatureValue),
                    CFNumber::from(i64::from(*value)).as_CFType(),
                ),
            ])
        });
    }
    attributes.push((
        unsafe { CFString::wrap_under_get_rule(kCTFontFeatureSettingsAttribute) },
        CFArray::from_CFTypes(&features).as_CFType(),
    ));
    if let Some(fallbacks) = &requested.fallbacks {
        let mut descriptors = Vec::new();
        for family in fallbacks.fallback_list() {
            let values = unsafe {
                Attributes::from_CFType_pairs(&[
                    (
                        CFString::wrap_under_get_rule(kCTFontFamilyNameAttribute),
                        CFString::new(family).as_CFType(),
                    ),
                    (
                        CFString::wrap_under_get_rule(kCTFontTraitsAttribute),
                        traits.as_CFType(),
                    ),
                ])
            };
            descriptors.push(descriptor(&values)?);
        }
        let raw = unsafe { CFLocaleCopyPreferredLanguages() };
        let languages = if raw.is_null() {
            CFArray::<CFString>::from_CFTypes(&[])
        } else {
            unsafe { CFArray::<CFString>::wrap_under_create_rule(raw) }
        };
        let system = core_text::font::cascade_list_for_languages(base, &languages);
        descriptors.extend(system.into_iter().map(|descriptor| (*descriptor).clone()));
        attributes.push((
            unsafe { CFString::wrap_under_get_rule(kCTFontCascadeListAttribute) },
            CFArray::from_CFTypes(&descriptors).as_CFType(),
        ));
    }
    let descriptor = descriptor(&Attributes::from_CFType_pairs(&attributes))?;
    let raw = unsafe {
        CTFontCreateCopyWithAttributes(
            base.as_concrete_TypeRef(),
            BASE_FONT_SIZE,
            ptr::null(),
            descriptor.as_concrete_TypeRef(),
        )
    };
    ensure!(
        !raw.is_null(),
        "Core Text could not apply the font configuration"
    );
    Ok(unsafe { CTFont::wrap_under_create_rule(raw) })
}

fn descriptor(attributes: &Attributes) -> Result<CTFontDescriptor> {
    let raw = unsafe { CTFontDescriptorCreateWithAttributes(attributes.as_concrete_TypeRef()) };
    ensure!(
        !raw.is_null(),
        "Core Text could not create a font descriptor"
    );
    Ok(unsafe { CTFontDescriptor::wrap_under_create_rule(raw) })
}

fn normalized_weight(weight: f32) -> f64 {
    let values = [-0.8, -0.6, -0.4, 0.0, 0.23, 0.3, 0.4, 0.56, 0.62];
    let position = ((weight.clamp(100.0, 900.0) - 100.0) / 100.0) as f64;
    let lower = position.floor() as usize;
    let upper = (lower + 1).min(values.len() - 1);
    values[lower] + (values[upper] - values[lower]) * position.fract()
}

fn is_color_font(font: &CTFont) -> bool {
    [*b"sbix", *b"COLR", *b"CBDT", *b"SVG "]
        .into_iter()
        .any(|tag| font.has_table(u32::from_be_bytes(tag)))
}

fn metrics(font: &CTFont) -> FontMetrics {
    let bounds = font.bounding_box();
    let scale = f64::from(font.units_per_em()) / font.pt_size();
    FontMetrics {
        units_per_em: font.units_per_em(),
        ascent: (font.ascent() * scale) as f32,
        descent: (font.descent() * scale) as f32,
        line_gap: (font.leading() * scale) as f32,
        underline_position: (font.underline_position() * scale) as f32,
        underline_thickness: (font.underline_thickness() * scale) as f32,
        cap_height: (font.cap_height() * scale) as f32,
        x_height: (font.x_height() * scale) as f32,
        bounding_box: Bounds::new(
            point(
                (bounds.origin.x * scale) as f32,
                (bounds.origin.y * scale) as f32,
            ),
            size(
                (bounds.size.width * scale) as f32,
                (bounds.size.height * scale) as f32,
            ),
        ),
    }
}

impl PlatformTextSystem for CoreTextSystem {
    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        let mut state = self.0.write();
        state.selections.clear();
        for bytes in fonts {
            let data = CFData::from_buffer(&bytes);
            let raw =
                unsafe { CTFontManagerCreateFontDescriptorsFromData(data.as_concrete_TypeRef()) };
            ensure!(
                !raw.is_null(),
                "Core Text could not read the supplied font data"
            );
            let descriptors: CFArray<CTFontDescriptor> =
                unsafe { CFArray::wrap_under_create_rule(raw) };
            ensure!(
                !descriptors.is_empty(),
                "The supplied data contains no fonts"
            );
            for descriptor in descriptors.into_iter() {
                let font = core_text::font::new_from_descriptor(&descriptor, BASE_FONT_SIZE);
                state.names.insert(font.family_name());
                state.names.insert(font.postscript_name());
                state.memory_fonts.push(font);
            }
        }
        Ok(())
    }

    fn all_font_names(&self) -> Vec<String> {
        let mut names = self.0.read().names.iter().cloned().collect::<Vec<_>>();
        names.sort();
        names
    }

    fn font_id(&self, font: &Font) -> Result<FontId> {
        self.0.write().resolve(font)
    }

    fn font_metrics(&self, id: FontId) -> FontMetrics {
        let state = self.0.read();
        match state.font(id) {
            Ok(font) => metrics(font),
            Err(error) => {
                log::error!("Core Text metrics failed: {error:#}");
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
        }
    }

    fn typographic_bounds(&self, id: FontId, glyph: GlyphId) -> Result<Bounds<f32>> {
        let state = self.0.read();
        let glyph = CGGlyph::try_from(glyph.0)?;
        let font = state.font(id)?;
        let bounds = font.get_bounding_rects_for_glyphs(0, &[glyph]);
        let scale = f64::from(font.units_per_em()) / font.pt_size();
        Ok(Bounds::new(
            point(
                (bounds.origin.x * scale) as f32,
                (bounds.origin.y * scale) as f32,
            ),
            size(
                (bounds.size.width * scale) as f32,
                (bounds.size.height * scale) as f32,
            ),
        ))
    }

    fn advance(&self, id: FontId, glyph: GlyphId) -> Result<Size<f32>> {
        let state = self.0.read();
        let glyph = CGGlyph::try_from(glyph.0)?;
        let mut advance = CGSize::new(0.0, 0.0);
        let font = state.font(id)?;
        unsafe { font.get_advances_for_glyphs(0, &glyph, &mut advance, 1) };
        let scale = f64::from(font.units_per_em()) / font.pt_size();
        Ok(size(
            (advance.width * scale) as f32,
            (advance.height * scale) as f32,
        ))
    }

    fn glyph_for_char(&self, id: FontId, character: char) -> Option<GlyphId> {
        let state = self.0.read();
        let font = state.font(id).ok()?;
        let mut characters = [0_u16; 2];
        let characters = character.encode_utf16(&mut characters);
        let mut glyphs = [0_u16; 2];
        unsafe {
            font.get_glyphs_for_characters(
                characters.as_ptr(),
                glyphs.as_mut_ptr(),
                characters.len() as isize,
            )
        };
        glyphs
            .into_iter()
            .find(|glyph| *glyph != 0)
            .map(|glyph| GlyphId(u32::from(glyph)))
    }

    fn glyph_raster_bounds(&self, params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        ensure!(
            params.font_size.0.is_finite()
                && params.font_size.0 > 0.0
                && params.scale_factor.is_finite()
                && params.scale_factor > 0.0,
            "Invalid glyph raster size"
        );
        let state = self.0.read();
        let font = state
            .font(params.font_id)?
            .clone_with_font_size(f64::from(params.font_size.0 * params.scale_factor));
        let glyph = CGGlyph::try_from(params.glyph_id.0)?;
        let bounds = font.get_bounding_rects_for_glyphs(0, &[glyph]);
        let left = bounds.origin.x.floor() - 1.0;
        let top = (-(bounds.origin.y + bounds.size.height)).floor() - 1.0;
        let right = (bounds.origin.x + bounds.size.width).ceil() + 1.0;
        let bottom = (-bounds.origin.y).ceil() + 1.0;
        ensure!(
            [left, top, right, bottom]
                .into_iter()
                .all(|value| value.is_finite()
                    && value >= f64::from(i32::MIN)
                    && value <= f64::from(i32::MAX)),
            "Glyph raster bounds exceed device coordinates"
        );
        let width = (right as i32)
            .checked_sub(left as i32)
            .context("Glyph raster width overflow")?;
        let height = (bottom as i32)
            .checked_sub(top as i32)
            .context("Glyph raster height overflow")?;
        Ok(Bounds::new(
            point(DevicePixels(left as i32), DevicePixels(top as i32)),
            size(DevicePixels(width), DevicePixels(height)),
        ))
    }

    fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
        bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        let width = usize::try_from(bounds.size.width.0)?;
        let height = usize::try_from(bounds.size.height.0)?;
        ensure!(width > 0 && height > 0, "Glyph raster bounds are empty");
        let channels = if params.is_emoji { 4 } else { 1 };
        let stride = width
            .checked_mul(channels)
            .context("Glyph raster stride overflow")?;
        let length = stride
            .checked_mul(height)
            .context("Glyph raster allocation overflow")?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length)?;
        bytes.resize(length, 0);
        {
            let state = self.0.read();
            let font = state
                .font(params.font_id)?
                .clone_with_font_size(f64::from(params.font_size.0));
            let color_space = if params.is_emoji {
                CGColorSpace::create_device_rgb()
            } else {
                CGColorSpace::create_device_gray()
            };
            let context = CGContext::create_bitmap_context(
                Some(bytes.as_mut_ptr().cast()),
                width,
                height,
                8,
                stride,
                &color_space,
                if params.is_emoji {
                    kCGImageAlphaPremultipliedLast
                } else {
                    7
                },
            );
            // Core Graphics uses an upward baseline; atlas rows run downward from these bounds.
            context.translate(
                -f64::from(bounds.origin.x.0),
                f64::from(bounds.origin.y.0) + height as f64,
            );
            context.scale(
                f64::from(params.scale_factor),
                f64::from(params.scale_factor),
            );
            context.set_text_drawing_mode(CGTextDrawingMode::CGTextFill);
            context.set_allows_antialiasing(true);
            context.set_should_antialias(true);
            context.set_allows_font_subpixel_positioning(true);
            context.set_should_subpixel_position_fonts(true);
            context.set_allows_font_subpixel_quantization(false);
            context.set_should_subpixel_quantize_fonts(false);
            context.set_gray_fill_color(0.0, 1.0);
            let offset = params.subpixel_variant.map(|value| {
                f64::from(value) / f64::from(SUBPIXEL_VARIANTS_X) / f64::from(params.scale_factor)
            });
            font.draw_glyphs(
                &[CGGlyph::try_from(params.glyph_id.0)?],
                &[CGPoint::new(offset.x, offset.y)],
                context,
            );
        }
        if params.is_emoji {
            for pixel in bytes.chunks_exact_mut(4) {
                swap_rgba_pa_to_bgra(pixel);
            }
        }
        Ok((bounds.size, bytes))
    }

    fn layout_line(&self, text: &str, size: Pixels, runs: &[FontRun]) -> LineLayout {
        match self.0.write().shape(text, size, runs) {
            Ok(line) => line,
            Err(error) => {
                log::error!("Core Text shaping failed: {error:#}");
                LineLayout {
                    font_size: size,
                    width: px(0.0),
                    ascent: px(0.0),
                    descent: px(0.0),
                    runs: Vec::new(),
                    len: text.len(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FontWeight, font};
    use std::sync::Arc;

    #[test]
    fn native_defaults_preserve_system_faces_and_monospace_advances() -> Result<()> {
        let system = CoreTextSystem::new();
        let ui = system.font_id(&font(".SystemUIFont"))?;
        let mono = system.font_id(&font(".SystemMonospaceFont"))?;
        let state = system.0.read();
        let native_fonts = [
            (ui, objc2_app_kit::NSFont::systemFontOfSize(BASE_FONT_SIZE)),
            (
                mono,
                objc2_app_kit::NSFont::monospacedSystemFontOfSize_weight(BASE_FONT_SIZE, 0.0),
            ),
        ];
        for (id, expected) in native_fonts {
            // NSFont is toll-free bridged; its symbolic name can differ after Core Text realizes it.
            let expected = unsafe {
                CTFont::wrap_under_get_rule(objc2::rc::Retained::as_ptr(&expected).cast())
            };
            let actual = state.font(id)?;
            for tag in [*b"head", *b"hhea", *b"maxp", *b"cmap", *b"hmtx"] {
                let tag = u32::from_be_bytes(tag);
                let expected_table = expected
                    .get_font_table(tag)
                    .context("OS system font is missing its face identity table")?;
                let actual_table = actual
                    .get_font_table(tag)
                    .context("Selected system font is missing its face identity table")?;
                assert_eq!(actual_table.bytes(), expected_table.bytes());
            }
            assert_eq!(actual.units_per_em(), expected.units_per_em());
            assert!((actual.ascent() - expected.ascent()).abs() < 0.001);
            assert!((actual.descent() - expected.descent()).abs() < 0.001);
            assert!((actual.cap_height() - expected.cap_height()).abs() < 0.001);
            let characters = (0x20_u16..0x7f).collect::<Vec<_>>();
            let mut actual_glyphs = vec![0_u16; characters.len()];
            let mut expected_glyphs = vec![0_u16; characters.len()];
            unsafe {
                actual.get_glyphs_for_characters(
                    characters.as_ptr(),
                    actual_glyphs.as_mut_ptr(),
                    characters.len() as isize,
                );
                expected.get_glyphs_for_characters(
                    characters.as_ptr(),
                    expected_glyphs.as_mut_ptr(),
                    characters.len() as isize,
                );
            }
            assert_eq!(actual_glyphs, expected_glyphs);
            let mut actual_advances = vec![CGSize::new(0.0, 0.0); characters.len()];
            let mut expected_advances = vec![CGSize::new(0.0, 0.0); characters.len()];
            unsafe {
                actual.get_advances_for_glyphs(
                    0,
                    actual_glyphs.as_ptr(),
                    actual_advances.as_mut_ptr(),
                    characters.len() as isize,
                );
                expected.get_advances_for_glyphs(
                    0,
                    expected_glyphs.as_ptr(),
                    expected_advances.as_mut_ptr(),
                    characters.len() as isize,
                );
            }
            for (actual, expected) in actual_advances.iter().zip(expected_advances) {
                assert!((actual.width - expected.width).abs() < 0.001);
                assert!((actual.height - expected.height).abs() < 0.001);
            }
        }
        drop(state);
        let narrow = system
            .glyph_for_char(mono, 'i')
            .context("OS monospace font has no i")?;
        let wide = system
            .glyph_for_char(mono, 'W')
            .context("OS monospace font has no W")?;
        assert!(
            (system.advance(mono, narrow)?.width - system.advance(mono, wide)?.width).abs() < 0.01
        );
        assert!(system.font_metrics(ui).units_per_em > 0);
        Ok(())
    }

    #[test]
    fn native_shaping_preserves_utf8_clusters_bom_fallback_and_rtl_positions() -> Result<()> {
        let system = CoreTextSystem::new();
        let id = system.font_id(&font(".SystemUIFont"))?;
        let text = "\u{feff}A😀e\u{301}العربية漢字";
        let line = system.0.write().shape(
            text,
            px(18.0),
            &[FontRun {
                len: text.len(),
                font_id: id,
            }],
        )?;
        assert!(line.width.0 > 0.0);
        assert_eq!(line.len, text.len());
        let glyphs = line
            .runs
            .iter()
            .flat_map(|run| run.glyphs.iter())
            .collect::<Vec<_>>();
        assert!(!glyphs.is_empty());
        assert!(
            glyphs
                .iter()
                .all(|glyph| text.is_char_boundary(glyph.index))
        );
        assert!(glyphs.iter().any(|glyph| glyph.index == 3));
        let emoji = text.find('😀').context("Emoji fixture is missing")?;
        assert!(
            glyphs
                .iter()
                .any(|glyph| glyph.index == emoji && glyph.is_emoji)
        );
        let rtl = "مرحبا";
        let line = system.0.write().shape(
            rtl,
            px(18.0),
            &[FontRun {
                len: rtl.len(),
                font_id: id,
            }],
        )?;
        let glyphs = line
            .runs
            .iter()
            .flat_map(|run| run.glyphs.iter())
            .collect::<Vec<_>>();
        let first = glyphs
            .iter()
            .find(|glyph| glyph.index == 0)
            .context("Missing first RTL cluster")?;
        let last = glyphs
            .iter()
            .max_by_key(|glyph| glyph.index)
            .context("Missing last RTL cluster")?;
        assert!(first.position.x > last.position.x);
        Ok(())
    }

    #[test]
    fn native_rasterization_preserves_weight_scale_phase_and_color() -> Result<()> {
        let system = CoreTextSystem::new();
        let regular = font(".SystemUIFont");
        let bold = Font {
            weight: FontWeight::BOLD,
            ..regular.clone()
        };
        let mut rasters = Vec::new();
        for descriptor in [regular, bold] {
            let id = system.font_id(&descriptor)?;
            let glyph = system
                .glyph_for_char(id, 'M')
                .context("OS UI font has no M")?;
            let params = RenderGlyphParams {
                font_id: id,
                glyph_id: glyph,
                font_size: px(18.0),
                subpixel_variant: point(0, 0),
                scale_factor: 1.0,
                is_emoji: false,
            };
            let bounds = system.glyph_raster_bounds(&params)?;
            let (dimensions, bytes) = system.rasterize_glyph(&params, bounds)?;
            assert_eq!(
                bytes.len(),
                usize::try_from(dimensions.width.0)? * usize::try_from(dimensions.height.0)?
            );
            assert!(bytes.iter().any(|value| *value > 0));
            let phase_params = RenderGlyphParams {
                subpixel_variant: point(1, 0),
                ..params.clone()
            };
            let phase_bounds = system.glyph_raster_bounds(&phase_params)?;
            let phase_raster = system.rasterize_glyph(&phase_params, phase_bounds)?;
            assert_ne!(
                (&dimensions, &bytes),
                (&phase_raster.0, &phase_raster.1),
                "Fractional glyph positioning must affect coverage at the same scale"
            );
            rasters.push((dimensions, bytes));
            for (scale, phase) in [(1.5, 1), (2.0, 3)] {
                let params = RenderGlyphParams {
                    scale_factor: scale,
                    subpixel_variant: point(phase, 0),
                    ..params.clone()
                };
                let bounds = system.glyph_raster_bounds(&params)?;
                let (dimensions, bytes) = system.rasterize_glyph(&params, bounds)?;
                assert_eq!(
                    bytes.len(),
                    usize::try_from(dimensions.width.0)? * usize::try_from(dimensions.height.0)?
                );
                assert!(bytes.iter().any(|value| *value > 0));
            }
        }
        assert_ne!(
            rasters.first(),
            rasters.last(),
            "Native normal and bold glyph coverage must differ"
        );
        let id = system.font_id(&font(".SystemUIFont"))?;
        let text = "😀";
        let line = system.0.write().shape(
            text,
            px(32.0),
            &[FontRun {
                len: text.len(),
                font_id: id,
            }],
        )?;
        let (id, glyph) = line
            .runs
            .iter()
            .find_map(|run| {
                run.glyphs
                    .iter()
                    .find(|glyph| glyph.is_emoji)
                    .map(|glyph| (run.font_id, glyph.id))
            })
            .context("Native emoji fallback was not selected")?;
        let params = RenderGlyphParams {
            font_id: id,
            glyph_id: glyph,
            font_size: px(32.0),
            subpixel_variant: point(0, 0),
            scale_factor: 1.0,
            is_emoji: true,
        };
        let bounds = system.glyph_raster_bounds(&params)?;
        let (dimensions, bytes) = system.rasterize_glyph(&params, bounds)?;
        assert_eq!(
            bytes.len(),
            usize::try_from(dimensions.width.0)? * usize::try_from(dimensions.height.0)? * 4
        );
        assert!(
            bytes
                .chunks_exact(4)
                .any(|pixel| pixel[3] > 0 && pixel[..3].iter().any(|channel| *channel > 0))
        );
        Ok(())
    }

    #[test]
    fn native_font_and_line_access_is_safe_across_calling_threads() -> Result<()> {
        let system = Arc::new(CoreTextSystem::new());
        let mut workers = Vec::new();
        for _ in 0..2 {
            let system = system.clone();
            workers.push(std::thread::spawn(move || -> Result<_> {
                let id = system.font_id(&font(".SystemUIFont"))?;
                let text = "Native text 漢字 😀";
                let line = system.0.write().shape(
                    text,
                    px(16.0),
                    &[FontRun {
                        len: text.len(),
                        font_id: id,
                    }],
                )?;
                ensure!(line.width.0 > 0.0, "Native threaded line is empty");
                Ok((id, line.width))
            }));
        }
        let values = workers
            .into_iter()
            .map(|worker| {
                worker
                    .join()
                    .map_err(|_| anyhow!("Native text worker panicked"))?
            })
            .collect::<Result<Vec<_>>>()?;
        assert_eq!(values.first(), values.last());
        Ok(())
    }

    #[test]
    fn native_tabs_and_font_boundaries_preserve_editor_positions() -> Result<()> {
        let system = CoreTextSystem::new();
        let id = system.font_id(&font(".SystemMonospaceFont"))?;
        let text = "A\tB\tC\tD";
        let line = system.0.write().shape(
            text,
            px(18.0),
            &[FontRun {
                len: text.len(),
                font_id: id,
            }],
        )?;
        let space = system
            .glyph_for_char(id, ' ')
            .context("OS monospace font has no space")?;
        let metrics = system.font_metrics(id);
        let advance = system.advance(id, space)?.width / metrics.units_per_em as f32 * 18.0;
        for (index, cell) in [(2, 4.0), (4, 8.0), (6, 12.0)] {
            let glyph = line
                .runs
                .iter()
                .flat_map(|run| &run.glyphs)
                .find(|glyph| glyph.index == index)
                .context("Glyph following a tab is missing")?;
            assert!(
                (glyph.position.x.0 - advance * cell).abs() < 0.02,
                "Tab placed byte {index} at {}, expected {}",
                glyph.position.x.0,
                advance * cell
            );
        }
        let line = system.0.write().shape(
            "ffi",
            px(18.0),
            &[
                FontRun {
                    len: 1,
                    font_id: id,
                },
                FontRun {
                    len: 2,
                    font_id: id,
                },
            ],
        )?;
        assert!(
            line.runs
                .iter()
                .flat_map(|run| &run.glyphs)
                .any(|glyph| glyph.index == 1),
            "A ligature crossed an explicit font run boundary"
        );
        Ok(())
    }

    #[test]
    fn native_application_font_registration_preserves_existing_ids() -> Result<()> {
        let system = CoreTextSystem::new();
        let id = system.font_id(&font(".SystemUIFont"))?;
        let native = system.0.read().font(id)?.clone();
        let backing = core_text::font::new_from_CGFont(&native.copy_to_CGFont(), BASE_FONT_SIZE);
        let file = backing
            .url()
            .and_then(|url| url.to_path())
            .context("OS-selected fixture font has no backing file")?;
        let bytes = std::fs::read(file)
            .context("Could not read the installed OS font registration fixture")?;
        let glyph = system
            .glyph_for_char(id, 'M')
            .context("OS UI font has no M")?;
        let advance = system.advance(id, glyph)?;
        system.add_fonts(vec![Cow::Owned(bytes.clone())])?;
        assert_eq!(system.advance(id, glyph)?, advance);
        let registered = system.font_id(&font(backing.family_name()))?;
        assert!(system.font_metrics(registered).units_per_em > 0);
        let line = system.0.write().shape(
            "Memory العربية",
            px(18.0),
            &[FontRun {
                len: "Memory العربية".len(),
                font_id: registered,
            }],
        )?;
        assert!(line.width.0 > 0.0 && !line.runs.is_empty());
        assert!(
            system
                .add_fonts(vec![
                    Cow::Owned(bytes),
                    Cow::Borrowed(b"invalid-font-fixture")
                ])
                .is_err()
        );
        assert_eq!(system.advance(id, glyph)?, advance);
        let replacement = system.font_id(&font(backing.family_name()))?;
        assert!(system.glyph_for_char(replacement, 'M').is_some());
        Ok(())
    }
}
