// SPDX-License-Identifier: Apache-2.0
// Copyright 2022 - 2025 Zed Industries, Inc.
// DirectWrite font/layout integration is adapted from gpui-pre-windows 0.3.10.

use crate::{
    Bounds, DevicePixels, Font, FontFeatures, FontId, FontMetrics, FontRun, FontStyle, GlyphId,
    LineLayout, Pixels, PlatformTextSystem, RenderGlyphParams, ShapedGlyph, ShapedRun, Size, point,
    px, size,
};
use anyhow::{Context as _, Result, ensure};
use collections::HashMap;
use parking_lot::Mutex;
use std::{borrow::Cow, ffi::c_void};
use windows::Win32::{
    Foundation::*,
    Globalization::{
        C2_LEFTTORIGHT, C2_RIGHTTOLEFT, CT_CTYPE2, GetStringTypeW, GetUserDefaultLocaleName,
    },
    Graphics::{DirectWrite::*, Gdi::*},
    UI::WindowsAndMessaging::*,
};
use windows_core::{BOOL, HSTRING, IUnknown, Interface, PCWSTR, Ref, implement};

pub(crate) struct DirectWriteTextSystem(Mutex<State>);

struct FontRecord {
    face: IDWriteFontFace3,
    family: HSTRING,
    collection: IDWriteFontCollection1,
    typography: IDWriteTypography,
    fallback_families: Vec<(HSTRING, IDWriteFontCollection1)>,
}

struct FontRegistry {
    collection: Option<IDWriteFontCollection1>,
    builder: Option<IDWriteFontSetBuilder1>,
    loader: IDWriteInMemoryFontFileLoader,
    factory: IDWriteFactory5,
}
impl FontRegistry {
    fn new(factory: &IDWriteFactory5) -> Result<Self> {
        let loader = unsafe { factory.CreateInMemoryFontFileLoader() }?;
        unsafe { factory.RegisterFontFileLoader(&loader) }?;
        let mut registry = Self {
            collection: None,
            builder: None,
            loader,
            factory: factory.clone(),
        };
        registry.builder = Some(unsafe { factory.CreateFontSetBuilder() }?);
        registry.refresh()?;
        Ok(registry)
    }
    fn builder(&self) -> Result<&IDWriteFontSetBuilder1> {
        self.builder
            .as_ref()
            .context("DirectWrite application font builder is unavailable")
    }
    fn collection(&self) -> Result<&IDWriteFontCollection1> {
        self.collection
            .as_ref()
            .context("DirectWrite application font collection is unavailable")
    }
    fn refresh(&mut self) -> Result<()> {
        let set = unsafe { self.builder()?.CreateFontSet() }?;
        self.collection = Some(unsafe { self.factory.CreateFontCollectionFromFontSet(&set) }?);
        Ok(())
    }
}
impl Drop for FontRegistry {
    fn drop(&mut self) {
        drop(self.collection.take());
        drop(self.builder.take());
        if let Err(error) = unsafe { self.factory.UnregisterFontFileLoader(&self.loader) } {
            log::error!("Could not unregister the native application font loader: {error}");
        }
    }
}

struct State {
    // Release faces and their collection references before unregistering the application font loader.
    fonts: Vec<FontRecord>,
    selections: HashMap<Font, FontId>,
    physical_faces: HashMap<usize, FontId>,
    system_fonts: IDWriteFontCollection1,
    locale: HSTRING,
    ui_family: String,
    mono_family: String,
    color_rasterizer: Option<rasterizer::ColorRasterizer>,
    registry: FontRegistry,
    factory: IDWriteFactory5,
}

impl DirectWriteTextSystem {
    pub(crate) fn new() -> Result<Self> {
        let factory: IDWriteFactory5 = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }
            .context("Creating the native DirectWrite factory")?;
        let mut system_fonts = None;
        unsafe { factory.GetSystemFontCollection(false, &mut system_fonts, true) }
            .context("Reading the native DirectWrite system font collection")?;
        let system_fonts =
            system_fonts.context("DirectWrite returned no system font collection")?;
        let mut locale = [0_u16; 85];
        let count = unsafe { GetUserDefaultLocaleName(&mut locale) };
        ensure!(count > 0, "Windows did not return the user font locale");
        let locale = locale
            .get(..usize::try_from(count - 1)?)
            .context("Windows locale exceeded its buffer")?;
        let locale = HSTRING::from_wide(locale);
        let ui_family =
            system_ui_family(&factory, &locale).context("Resolving the Windows system UI font")?;
        let mono_family = system_monospace_family(&factory, &locale)
            .context("Resolving the Windows system monospace font")?;
        let registry =
            FontRegistry::new(&factory).context("Creating the native application font registry")?;
        Ok(Self(Mutex::new(State {
            fonts: Vec::new(),
            selections: HashMap::default(),
            physical_faces: HashMap::default(),
            system_fonts,
            locale,
            ui_family,
            mono_family,
            color_rasterizer: None,
            registry,
            factory,
        })))
    }
}

impl State {
    fn font(&self, id: FontId) -> Result<&FontRecord> {
        self.fonts
            .get(id.0)
            .context("Unknown DirectWrite font identifier")
    }
    fn family<'a>(&'a self, family: &'a str) -> &'a str {
        match family {
            ".SystemUIFont" | ".ZedSans" | "Zed Plex Sans" => &self.ui_family,
            ".SystemMonospaceFont" | ".ZedMono" | "Zed Plex Mono" => &self.mono_family,
            family => family,
        }
    }
    fn find_family(
        &self,
        name: &str,
    ) -> Result<Option<(IDWriteFontFamily, IDWriteFontCollection1)>> {
        let name = HSTRING::from(name);
        for collection in [self.registry.collection()?, &self.system_fonts] {
            let mut index = 0;
            let mut exists = false.into();
            unsafe { collection.FindFamilyName(&name, &mut index, &mut exists) }?;
            if exists.as_bool() {
                return Ok(Some((
                    unsafe { collection.GetFontFamily(index) }?.into(),
                    collection.clone(),
                )));
            }
        }
        Ok(None)
    }
    fn resolve(&mut self, requested: &Font) -> Result<FontId> {
        if let Some(id) = self.selections.get(requested) {
            return Ok(*id);
        }
        ensure!(requested.weight.0.is_finite(), "Font weight must be finite");
        let family = self.family(requested.family.as_ref()).to_owned();
        let (native_family, _collection) = self
            .find_family(&family)?
            .with_context(|| format!("Font family {family:?} is not installed"))?;
        let native = unsafe {
            native_family.GetFirstMatchingFont(
                DWRITE_FONT_WEIGHT(requested.weight.0.clamp(1.0, 1000.0).round() as i32),
                DWRITE_FONT_STRETCH_NORMAL,
                native_style(requested.style),
            )
        }?;
        let face: IDWriteFontFace3 = unsafe { native.CreateFontFace() }?.cast()?;
        let typography = unsafe { self.factory.CreateTypography() }?;
        apply_features(&typography, &requested.features)?;
        let mut fallback_families = Vec::new();
        if let Some(families) = &requested.fallbacks {
            for family in families.fallback_list() {
                let family = self.family(family);
                if let Some((_, collection)) = self.find_family(family)? {
                    fallback_families.push((HSTRING::from(family), collection));
                }
            }
        }
        let id = FontId(self.fonts.len());
        let alias = HSTRING::from(format!("WGPUI-Selection-{}", id.0));
        let aliases = [
            DWRITE_FONT_PROPERTY_ID_FAMILY_NAME,
            DWRITE_FONT_PROPERTY_ID_TYPOGRAPHIC_FAMILY_NAME,
            DWRITE_FONT_PROPERTY_ID_WIN32_FAMILY_NAME,
        ]
        .map(|property_id| DWRITE_FONT_PROPERTY {
            propertyId: property_id,
            propertyValue: PCWSTR(alias.as_ptr()),
            localeName: PCWSTR::null(),
        });
        // Private family aliases let one native fallback object distinguish each font run's fallback policy.
        let builder = unsafe { self.factory.CreateFontSetBuilder() }?;
        let reference = unsafe { face.GetFontFaceReference() }?;
        unsafe { builder.AddFontFaceReference(&reference, &aliases) }?;
        let set = unsafe { builder.CreateFontSet() }?;
        let collection = unsafe { self.factory.CreateFontCollectionFromFontSet(&set) }?;
        let identity = face.cast::<IUnknown>()?.as_raw().addr();
        self.fonts.push(FontRecord {
            face,
            family: alias,
            collection,
            typography,
            fallback_families,
        });
        self.physical_faces.entry(identity).or_insert(id);
        self.selections.insert(requested.clone(), id);
        Ok(id)
    }
    fn register_fallback(&mut self, face: IDWriteFontFace3) -> Result<FontId> {
        let identity = face.cast::<IUnknown>()?.as_raw().addr();
        if let Some(id) = self.physical_faces.get(&identity) {
            return Ok(*id);
        }
        let native = match unsafe { self.system_fonts.GetFontFromFontFace(&face) } {
            Ok(font) => (font, self.system_fonts.clone()),
            Err(_) => (
                unsafe { self.registry.collection()?.GetFontFromFontFace(&face) }?,
                self.registry.collection()?.clone(),
            ),
        };
        let family = localized_name(
            &unsafe { native.0.GetFontFamily()?.GetFamilyNames() }?,
            &self.locale,
        )?;
        let typography = unsafe { self.factory.CreateTypography() }?;
        let id = FontId(self.fonts.len());
        self.fonts.push(FontRecord {
            face,
            family: HSTRING::from(family),
            collection: native.1,
            typography,
            fallback_families: Vec::new(),
        });
        self.physical_faces.insert(identity, id);
        Ok(id)
    }
    fn add_fonts(&mut self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        self.selections.clear();
        let mut result = Ok(());
        for bytes in fonts {
            let add = (|| -> Result<()> {
                ensure!(!bytes.is_empty(), "Application font data is empty");
                // With no owner object DirectWrite copies these bytes before the caller releases them.
                let file = unsafe {
                    self.registry.loader.CreateInMemoryFontFileReference(
                        &self.factory,
                        bytes.as_ptr().cast(),
                        u32::try_from(bytes.len())?,
                        None,
                    )
                }?;
                unsafe { self.registry.builder()?.AddFontFile(&file) }?;
                Ok(())
            })();
            if let Err(error) = add {
                result = Err(error);
                break;
            }
        }
        self.registry.refresh()?;
        result
    }
    fn shape(
        &mut self,
        text: &str,
        font_size: Pixels,
        font_runs: &[FontRun],
    ) -> Result<LineLayout> {
        ensure!(
            font_size.0.is_finite() && font_size.0 >= 0.0,
            "Invalid DirectWrite line font size"
        );
        if text.is_empty() || font_size.0 == 0.0 {
            return Ok(LineLayout {
                font_size,
                len: text.len(),
                ..LineLayout::default()
            });
        }
        let mut utf16_to_utf8 = Vec::new();
        for (offset, character) in text.char_indices() {
            utf16_to_utf8.extend(std::iter::repeat_n(offset, character.len_utf16()));
        }
        utf16_to_utf8.push(text.len());
        let utf16 = text.encode_utf16().collect::<Vec<_>>();
        u32::try_from(utf16.len())?;
        let first = font_runs
            .first()
            .context("Font runs do not cover the line")?;
        let record = self.font(first.font_id)?;
        let format: IDWriteTextFormat1 = unsafe {
            self.factory.CreateTextFormat(
                &record.family,
                &record.collection,
                record.face.GetWeight(),
                record.face.GetStyle(),
                DWRITE_FONT_STRETCH_NORMAL,
                font_size.0,
                &self.locale,
            )
        }?
        .cast()?;
        let builder = unsafe { self.factory.CreateFontFallbackBuilder() }?;
        let mut seen = std::collections::HashSet::new();
        for run in font_runs {
            if !seen.insert(run.font_id) {
                continue;
            }
            let record = self.font(run.font_id)?;
            for (family, collection) in &record.fallback_families {
                unsafe {
                    builder.AddMapping(
                        &[DWRITE_UNICODE_RANGE {
                            first: 0,
                            last: 0x10ffff,
                        }],
                        &[family.as_ptr()],
                        collection,
                        None,
                        &record.family,
                        1.0,
                    )
                }?;
            }
        }
        unsafe {
            builder.AddMappings(&self.factory.GetSystemFontFallback()?)?;
            format.SetFontFallback(&builder.CreateFontFallback()?)?;
        }
        let mut directions = vec![0_u16; utf16.len()];
        unsafe { GetStringTypeW(CT_CTYPE2, &utf16, directions.as_mut_ptr()) }?;
        let direction = paragraph_reading_direction(&utf16, &directions);
        unsafe { format.SetReadingDirection(direction) }?;
        unsafe {
            format.SetTextAlignment(if direction == DWRITE_READING_DIRECTION_RIGHT_TO_LEFT {
                DWRITE_TEXT_ALIGNMENT_TRAILING
            } else {
                DWRITE_TEXT_ALIGNMENT_LEADING
            })
        }?;
        let space = native_glyph(&record.face, ' ').context("OS font has no tab advance")?;
        let space = design_metrics(&record.face, space)?.advanceWidth;
        let metrics = native_metrics(&record.face);
        ensure!(
            metrics.units_per_em > 0,
            "DirectWrite font has no design units"
        );
        unsafe {
            format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            format.SetIncrementalTabStop(
                space as f32 / metrics.units_per_em as f32 * font_size.0 * 4.0,
            )?;
        }
        let layout: IDWriteTextLayout2 = unsafe {
            self.factory
                .CreateTextLayout(&utf16, &format, f32::MAX, f32::MAX)
        }?
        .cast()?;
        let mut byte_offset = 0_usize;
        let mut wide_offset = 0_u32;
        for (index, run) in font_runs.iter().enumerate() {
            let end = byte_offset
                .checked_add(run.len)
                .context("Font run length overflow")?;
            let run_text = text
                .get(byte_offset..end)
                .context("Font run is outside UTF-8 boundaries")?;
            let wide_length = u32::try_from(run_text.encode_utf16().count())?;
            let range = DWRITE_TEXT_RANGE {
                startPosition: wide_offset,
                length: wide_length,
            };
            let record = self.font(run.font_id)?;
            if wide_length > 0 {
                unsafe {
                    layout.SetFontCollection(&record.collection, range)?;
                    layout.SetFontFamilyName(&record.family, range)?;
                    layout.SetFontWeight(record.face.GetWeight(), range)?;
                    layout.SetFontStyle(record.face.GetStyle(), range)?;
                    layout.SetTypography(&record.typography, range)?;
                    // A native font-size boundary prevents ligatures across separate WGPUI font runs.
                    layout.SetFontSize(
                        if index % 2 == 0 {
                            font_size.0
                        } else {
                            font_size.0.next_up()
                        },
                        range,
                    )?;
                }
            }
            byte_offset = end;
            wide_offset = wide_offset
                .checked_add(wide_length)
                .context("UTF-16 font range overflow")?;
        }
        ensure!(
            byte_offset == text.len(),
            "Font runs do not cover the complete line"
        );
        let mut lines = [DWRITE_LINE_METRICS::default()];
        let mut count = 0;
        unsafe { layout.GetLineMetrics(Some(&mut lines), &mut count) }?;
        ensure!(count == 1, "DirectWrite returned more than one line");
        let line = lines
            .first()
            .context("DirectWrite returned no line metrics")?;
        let baseline = line.baseline;
        let ascent = line.baseline;
        let descent = line.height - line.baseline;
        let mut text_metrics = DWRITE_TEXT_METRICS1::default();
        unsafe { layout.GetMetrics(&mut text_metrics) }?;
        let renderer: IDWriteTextRenderer = TextRenderer.into();
        let mut context = DrawingContext {
            state: self,
            text,
            utf16_to_utf8: &utf16_to_utf8,
            baseline,
            runs: Vec::new(),
        };
        unsafe {
            layout.Draw(
                Some((&mut context as *mut DrawingContext).cast()),
                &renderer,
                0.0,
                0.0,
            )
        }?;
        for run in &mut context.runs {
            run.glyphs
                .sort_by(|left, right| left.position.x.0.total_cmp(&right.position.x.0));
        }
        context.runs.sort_by(|left, right| {
            let x = |run: &ShapedRun| run.glyphs.first().map_or(0.0, |glyph| glyph.position.x.0);
            x(left).total_cmp(&x(right))
        });
        Ok(LineLayout {
            font_size,
            width: px(text_metrics.Base.widthIncludingTrailingWhitespace),
            ascent: px(ascent),
            descent: px(descent),
            runs: context.runs,
            len: text.len(),
        })
    }
}

fn paragraph_reading_direction(text: &[u16], directions: &[u16]) -> DWRITE_READING_DIRECTION {
    let mut isolate_depth = 0_usize;
    for (&character, &direction) in text.iter().zip(directions) {
        // UAX #9 P2 excludes isolate contents from the surrounding paragraph's direction.
        match character {
            0x2066..=0x2068 => {
                isolate_depth += 1;
                continue;
            }
            0x2069 => {
                isolate_depth = isolate_depth.saturating_sub(1);
                continue;
            }
            _ if isolate_depth > 0 => continue,
            _ => {}
        }
        match u32::from(direction) {
            C2_LEFTTORIGHT => return DWRITE_READING_DIRECTION_LEFT_TO_RIGHT,
            C2_RIGHTTOLEFT => return DWRITE_READING_DIRECTION_RIGHT_TO_LEFT,
            _ => {}
        }
    }
    DWRITE_READING_DIRECTION_LEFT_TO_RIGHT
}

fn native_style(style: FontStyle) -> DWRITE_FONT_STYLE {
    match style {
        FontStyle::Normal => DWRITE_FONT_STYLE_NORMAL,
        FontStyle::Italic => DWRITE_FONT_STYLE_ITALIC,
        FontStyle::Oblique => DWRITE_FONT_STYLE_OBLIQUE,
    }
}
fn localized_name(names: &IDWriteLocalizedStrings, locale: &HSTRING) -> Result<String> {
    ensure!(
        unsafe { names.GetCount() } > 0,
        "Native font has no localized name"
    );
    let mut index = 0;
    let mut exists = false.into();
    unsafe { names.FindLocaleName(locale, &mut index, &mut exists) }?;
    if !exists.as_bool() {
        index = 0;
    }
    let length = usize::try_from(unsafe { names.GetStringLength(index) }?)?
        .checked_add(1)
        .context("Font name buffer overflow")?;
    let mut buffer = vec![0_u16; length];
    unsafe { names.GetString(index, &mut buffer) }?;
    String::from_utf16(
        buffer
            .get(..length - 1)
            .context("Font name exceeds its buffer")?,
    )
    .context("Native font name is invalid UTF-16")
}
fn font_family(native: &IDWriteFont, locale: &HSTRING) -> Result<String> {
    localized_name(
        &unsafe { native.GetFontFamily()?.GetFamilyNames() }?,
        locale,
    )
}

fn valid_gdi_object(object: HGDIOBJ) -> bool {
    !object.0.is_null() && object.0 as isize != -1
}

struct MappedSystemFont {
    context: HDC,
    font: HFONT,
    previous: HGDIOBJ,
}

impl Drop for MappedSystemFont {
    fn drop(&mut self) {
        if !self.context.0.is_null() {
            if valid_gdi_object(self.previous)
                && !valid_gdi_object(unsafe { SelectObject(self.context, self.previous) })
            {
                log::warn!("Could not restore the native system-font device context");
            }
            if !unsafe { DeleteDC(self.context) }.as_bool() {
                log::warn!("Could not release the native system-font device context");
            }
        }
        if !self.font.0.is_null() && !unsafe { DeleteObject(self.font.into()) }.as_bool() {
            log::warn!("Could not release the mapped native system font");
        }
    }
}

fn outline_system_font(logical: &LOGFONTW, monospace: bool) -> Result<LOGFONTW> {
    let mut mapping = MappedSystemFont {
        context: unsafe { CreateCompatibleDC(None) },
        font: HFONT::default(),
        previous: HGDIOBJ::default(),
    };
    ensure!(
        !mapping.context.0.is_null(),
        "Could not create the system-font device context"
    );
    let mut physical = *logical;
    // GDI stock faces can be bitmap-only; retain the OS mapping while requesting an outline face.
    physical.lfOutPrecision = OUT_TT_ONLY_PRECIS;
    if monospace {
        physical.lfPitchAndFamily = FIXED_PITCH.0 | FF_MODERN.0;
    }
    mapping.font = unsafe { CreateFontIndirectW(&physical) };
    ensure!(
        !mapping.font.0.is_null(),
        "Windows could not map its system font to an outline font"
    );
    mapping.previous = unsafe { SelectObject(mapping.context, mapping.font.into()) };
    ensure!(
        valid_gdi_object(mapping.previous),
        "Could not select the mapped system font"
    );
    let mut metrics = TEXTMETRICW::default();
    ensure!(
        unsafe { GetTextMetricsW(mapping.context, &mut metrics) }.as_bool(),
        "Could not read the mapped system-font metrics"
    );
    ensure!(
        metrics.tmPitchAndFamily.contains(TMPF_TRUETYPE),
        "Windows did not map its system font to a TrueType face"
    );
    if monospace {
        // Despite its name, TMPF_FIXED_PITCH is set for variable-pitch physical fonts.
        ensure!(
            !metrics.tmPitchAndFamily.contains(TMPF_FIXED_PITCH),
            "Windows mapped its monospace role to a proportional font"
        );
    }
    let mut face = [0_u16; 32];
    ensure!(
        unsafe { GetTextFaceW(mapping.context, Some(&mut face)) } > 0,
        "Windows did not return the mapped system-font face"
    );
    let length = face
        .iter()
        .position(|character| *character == 0)
        .context("Mapped system-font face exceeds LOGFONT capacity")?;
    ensure!(length > 0, "Mapped system-font face is empty");
    physical.lfFaceName = face;
    Ok(physical)
}

fn native_system_font(
    factory: &IDWriteFactory,
    logical: &LOGFONTW,
    monospace: bool,
) -> Result<IDWriteFont> {
    let role = if monospace { "monospace" } else { "UI" };
    let length = logical
        .lfFaceName
        .iter()
        .position(|character| *character == 0)
        .context("Windows system font exceeds LOGFONT capacity")?;
    let requested = String::from_utf16(&logical.lfFaceName[..length])?;
    let interop = unsafe { factory.GetGdiInterop() }
        .with_context(|| format!("Creating GDI interop for the {role} font"))?;
    let native = match unsafe { interop.CreateFontFromLOGFONT(logical) } {
        Ok(native) => native,
        Err(error) if error.code() == DWRITE_E_NOFONT => {
            let physical = outline_system_font(logical, monospace).with_context(|| {
                format!("Realizing the OS {role} font {requested:?} as an outline face")
            })?;
            unsafe { interop.CreateFontFromLOGFONT(&physical) }.with_context(|| {
                format!("Resolving the realized OS {role} font for {requested:?}")
            })?
        }
        Err(error) => {
            return Err(error)
                .with_context(|| format!("Resolving the OS {role} LOGFONT {requested:?}"));
        }
    };
    if monospace {
        ensure!(
            unsafe { native.cast::<IDWriteFont1>()?.IsMonospacedFont() }.as_bool(),
            "The native Windows monospace role resolved to a proportional face"
        );
    }
    Ok(native)
}

pub(crate) fn system_ui_font(factory: &IDWriteFactory) -> Result<IDWriteFont> {
    let mut metrics = NONCLIENTMETRICSW {
        cbSize: u32::try_from(std::mem::size_of::<NONCLIENTMETRICSW>())?,
        ..Default::default()
    };
    unsafe {
        SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS,
            metrics.cbSize,
            Some((&mut metrics as *mut NONCLIENTMETRICSW).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS::default(),
        )
    }
    .context("Reading the OS UI font descriptor")?;
    native_system_font(factory, &metrics.lfMessageFont, false)
}

pub(crate) fn system_monospace_font(factory: &IDWriteFactory) -> Result<IDWriteFont> {
    let stock = unsafe { GetStockObject(ANSI_FIXED_FONT) };
    ensure!(
        !stock.0.is_null(),
        "Windows did not return its fixed-pitch system font"
    );
    let mut logical = LOGFONTW::default();
    let bytes = i32::try_from(std::mem::size_of::<LOGFONTW>())?;
    ensure!(
        unsafe { GetObjectW(stock, bytes, Some((&mut logical as *mut LOGFONTW).cast())) } == bytes,
        "Windows did not return its fixed-pitch font descriptor"
    );
    native_system_font(factory, &logical, true)
}

fn system_ui_family(factory: &IDWriteFactory5, locale: &HSTRING) -> Result<String> {
    font_family(&system_ui_font(factory)?, locale)
}

fn system_monospace_family(factory: &IDWriteFactory5, locale: &HSTRING) -> Result<String> {
    font_family(&system_monospace_font(factory)?, locale)
}
fn apply_features(typography: &IDWriteTypography, features: &FontFeatures) -> Result<()> {
    for (tag, value) in features.tag_value_list() {
        ensure!(
            tag.len() == 4 && tag.is_ascii(),
            "OpenType tags must contain four ASCII bytes"
        );
        let tag: [u8; 4] = tag.as_bytes().try_into()?;
        unsafe {
            typography.AddFontFeature(DWRITE_FONT_FEATURE {
                nameTag: DWRITE_FONT_FEATURE_TAG(u32::from_le_bytes(tag)),
                parameter: *value,
            })
        }?;
    }
    Ok(())
}
fn native_glyph(face: &IDWriteFontFace3, character: char) -> Option<GlyphId> {
    let character = u32::from(character);
    let mut glyph = 0;
    match unsafe { face.GetGlyphIndices(&character, 1, &mut glyph) } {
        Ok(()) if glyph != 0 => Some(GlyphId(u32::from(glyph))),
        Ok(()) => None,
        Err(error) => {
            log::error!("DirectWrite glyph lookup failed: {error}");
            None
        }
    }
}
fn design_metrics(face: &IDWriteFontFace3, glyph: GlyphId) -> Result<DWRITE_GLYPH_METRICS> {
    let glyph = u16::try_from(glyph.0)?;
    let mut metrics = DWRITE_GLYPH_METRICS::default();
    unsafe { face.GetDesignGlyphMetrics(&glyph, 1, &mut metrics, false) }?;
    Ok(metrics)
}
fn native_metrics(face: &IDWriteFontFace3) -> FontMetrics {
    let mut metrics = DWRITE_FONT_METRICS1::default();
    unsafe { face.GetMetrics(&mut metrics) };
    FontMetrics {
        units_per_em: u32::from(metrics.Base.designUnitsPerEm),
        ascent: f32::from(metrics.Base.ascent),
        descent: -f32::from(metrics.Base.descent),
        line_gap: f32::from(metrics.Base.lineGap),
        underline_position: f32::from(metrics.Base.underlinePosition),
        underline_thickness: f32::from(metrics.Base.underlineThickness),
        cap_height: f32::from(metrics.Base.capHeight),
        x_height: f32::from(metrics.Base.xHeight),
        bounding_box: Bounds::new(
            point(
                f32::from(metrics.glyphBoxLeft),
                f32::from(metrics.glyphBoxBottom),
            ),
            size(
                (i32::from(metrics.glyphBoxRight) - i32::from(metrics.glyphBoxLeft)) as f32,
                (i32::from(metrics.glyphBoxTop) - i32::from(metrics.glyphBoxBottom)) as f32,
            ),
        ),
    }
}

impl PlatformTextSystem for DirectWriteTextSystem {
    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        self.0.lock().add_fonts(fonts)
    }
    fn all_font_names(&self) -> Vec<String> {
        let state = self.0.lock();
        let result = (|| -> Result<Vec<String>> {
            let mut names = Vec::new();
            for collection in [&state.system_fonts, state.registry.collection()?] {
                for index in 0..unsafe { collection.GetFontFamilyCount() } {
                    let family = unsafe { collection.GetFontFamily(index)?.GetFamilyNames() }?;
                    names.push(localized_name(&family, &state.locale)?);
                }
            }
            names.extend([
                ".SystemUIFont".to_owned(),
                ".SystemMonospaceFont".to_owned(),
            ]);
            names.sort();
            names.dedup();
            Ok(names)
        })();
        result.unwrap_or_else(|error| {
            log::error!("DirectWrite font enumeration failed: {error:#}");
            Vec::new()
        })
    }
    fn font_id(&self, descriptor: &Font) -> Result<FontId> {
        self.0.lock().resolve(descriptor)
    }
    fn font_metrics(&self, id: FontId) -> FontMetrics {
        match self.0.lock().font(id) {
            Ok(record) => native_metrics(&record.face),
            Err(error) => {
                log::error!("DirectWrite font metrics failed: {error:#}");
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
        let metrics = design_metrics(&self.0.lock().font(id)?.face, glyph)?;
        let width = i64::from(metrics.advanceWidth)
            - i64::from(metrics.leftSideBearing)
            - i64::from(metrics.rightSideBearing);
        let height = i64::from(metrics.advanceHeight)
            - i64::from(metrics.topSideBearing)
            - i64::from(metrics.bottomSideBearing);
        Ok(Bounds::new(
            point(
                metrics.leftSideBearing as f32,
                (i64::from(metrics.verticalOriginY) + i64::from(metrics.bottomSideBearing)
                    - i64::from(metrics.advanceHeight)) as f32,
            ),
            size(width as f32, height as f32),
        ))
    }
    fn advance(&self, id: FontId, glyph: GlyphId) -> Result<Size<f32>> {
        Ok(size(
            design_metrics(&self.0.lock().font(id)?.face, glyph)?.advanceWidth as f32,
            0.0,
        ))
    }
    fn glyph_for_char(&self, id: FontId, character: char) -> Option<GlyphId> {
        match self.0.lock().font(id) {
            Ok(record) => native_glyph(&record.face, character),
            Err(error) => {
                log::error!("DirectWrite glyph lookup failed: {error:#}");
                None
            }
        }
    }
    fn glyph_raster_bounds(&self, parameters: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        let state = self.0.lock();
        rasterizer::native_raster_bounds(
            &state.factory,
            &state.font(parameters.font_id)?.face,
            parameters,
        )
    }
    fn rasterize_glyph(
        &self,
        parameters: &RenderGlyphParams,
        bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        let mut state = self.0.lock();
        let face = state.font(parameters.font_id)?.face.clone();
        if parameters.is_emoji {
            if state.color_rasterizer.is_none() {
                state.color_rasterizer = Some(rasterizer::ColorRasterizer::new()?);
            }
            state
                .color_rasterizer
                .as_ref()
                .context("Native color glyph rasterizer is unavailable")?
                .rasterize(&state.factory, &face, parameters, bounds)
        } else {
            rasterizer::rasterize_monochrome(&state.factory, &face, parameters, bounds)
        }
    }
    fn layout_line(&self, text: &str, font_size: Pixels, runs: &[FontRun]) -> LineLayout {
        self.0
            .lock()
            .shape(text, font_size, runs)
            .unwrap_or_else(|error| {
                log::error!("DirectWrite text shaping failed: {error:#}");
                LineLayout {
                    font_size,
                    len: text.len(),
                    ..LineLayout::default()
                }
            })
    }
}

struct DrawingContext<'a> {
    state: &'a mut State,
    text: &'a str,
    utf16_to_utf8: &'a [usize],
    baseline: f32,
    runs: Vec<ShapedRun>,
}
#[implement(IDWriteTextRenderer)]
struct TextRenderer;

impl IDWritePixelSnapping_Impl for TextRenderer_Impl {
    fn IsPixelSnappingDisabled(&self, _context: *const c_void) -> windows::core::Result<BOOL> {
        Ok(true.into())
    }
    fn GetCurrentTransform(
        &self,
        _context: *const c_void,
        transform: *mut DWRITE_MATRIX,
    ) -> windows::core::Result<()> {
        let transform = unsafe { transform.as_mut() }.ok_or_else(|| {
            windows::core::Error::new(E_INVALIDARG, "Null DirectWrite transform output")
        })?;
        *transform = DWRITE_MATRIX {
            m11: 1.0,
            m22: 1.0,
            ..Default::default()
        };
        Ok(())
    }
    fn GetPixelsPerDip(&self, _context: *const c_void) -> windows::core::Result<f32> {
        Ok(1.0)
    }
}

impl IDWriteTextRenderer_Impl for TextRenderer_Impl {
    fn DrawGlyphRun(
        &self,
        context: *const c_void,
        x: f32,
        y: f32,
        _mode: DWRITE_MEASURING_MODE,
        run: *const DWRITE_GLYPH_RUN,
        description: *const DWRITE_GLYPH_RUN_DESCRIPTION,
        _effect: Ref<IUnknown>,
    ) -> windows::core::Result<()> {
        let drawing = (|| -> Result<()> {
            let context = unsafe { context.cast_mut().cast::<DrawingContext>().as_mut() }
                .context("Null DirectWrite drawing context")?;
            let run = unsafe { run.as_ref() }.context("Null DirectWrite glyph run")?;
            let description =
                unsafe { description.as_ref() }.context("Null DirectWrite glyph description")?;
            let count = usize::try_from(run.glyphCount)?;
            if count == 0 {
                return Ok(());
            }
            let face: IDWriteFontFace3 = run
                .fontFace
                .as_ref()
                .context("DirectWrite run has no font face")?
                .cast()?;
            let id = context.state.register_fallback(face.clone())?;
            let glyphs = unsafe { native_slice(run.glyphIndices, count) }?;
            let advances = unsafe { native_slice(run.glyphAdvances, count) }?;
            let offsets = unsafe { native_slice(run.glyphOffsets, count) }?;
            let clusters = unsafe {
                native_slice(
                    description.clusterMap,
                    usize::try_from(description.stringLength)?,
                )
            }?;
            let mut cluster_starts = std::collections::BTreeMap::new();
            for (index, glyph) in clusters.iter().enumerate() {
                let glyph = usize::from(*glyph);
                ensure!(
                    glyph < count,
                    "DirectWrite cluster is outside its glyph run"
                );
                cluster_starts.entry(glyph).or_insert(index);
            }
            let rtl = run.bidiLevel % 2 != 0;
            let mut pen = x;
            let mut values = Vec::new();
            for (index, glyph) in glyphs.iter().enumerate() {
                let logical = cluster_starts
                    .range(..=index)
                    .next_back()
                    .map(|(_, value)| *value)
                    .context("DirectWrite glyph has no logical cluster")?;
                let wide_index = usize::try_from(description.textPosition)?
                    .checked_add(logical)
                    .context("DirectWrite cluster index overflow")?;
                let index_in_text = *context
                    .utf16_to_utf8
                    .get(wide_index)
                    .context("DirectWrite UTF-16 cluster is outside its text")?;
                ensure!(
                    index_in_text < context.text.len()
                        && context.text.is_char_boundary(index_in_text),
                    "DirectWrite returned an invalid UTF-8 cluster"
                );
                let advance = *advances
                    .get(index)
                    .context("DirectWrite glyph advance is missing")?;
                let offset = offsets
                    .get(index)
                    .context("DirectWrite glyph offset is missing")?;
                ensure!(
                    advance.is_finite()
                        && offset.advanceOffset.is_finite()
                        && offset.ascenderOffset.is_finite(),
                    "DirectWrite returned a nonfinite glyph position"
                );
                let position = if rtl {
                    pen -= advance;
                    pen - offset.advanceOffset
                } else {
                    let position = pen + offset.advanceOffset;
                    pen += advance;
                    position
                };
                values.push(ShapedGlyph {
                    id: GlyphId(u32::from(*glyph)),
                    position: point(
                        px(position),
                        px(y - context.baseline - offset.ascenderOffset),
                    ),
                    index: index_in_text,
                    is_emoji: rasterizer::native_color_glyph(
                        &context.state.factory,
                        &face,
                        *glyph,
                        run.fontEmSize,
                    )?,
                });
            }
            context.runs.push(ShapedRun {
                font_id: id,
                glyphs: values,
            });
            Ok(())
        })();
        drawing.map_err(|error| windows::core::Error::new(E_FAIL, error.to_string()))
    }
    fn DrawUnderline(
        &self,
        _context: *const c_void,
        _x: f32,
        _y: f32,
        _underline: *const DWRITE_UNDERLINE,
        _effect: Ref<IUnknown>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn DrawStrikethrough(
        &self,
        _context: *const c_void,
        _x: f32,
        _y: f32,
        _strike: *const DWRITE_STRIKETHROUGH,
        _effect: Ref<IUnknown>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn DrawInlineObject(
        &self,
        _context: *const c_void,
        _x: f32,
        _y: f32,
        _object: Ref<IDWriteInlineObject>,
        _sideways: BOOL,
        _rtl: BOOL,
        _effect: Ref<IUnknown>,
    ) -> windows::core::Result<()> {
        Err(windows::core::Error::new(
            E_NOTIMPL,
            "Inline text objects are not part of WGPUI font runs",
        ))
    }
}
unsafe fn native_slice<'a, T>(pointer: *const T, length: usize) -> Result<&'a [T]> {
    if length == 0 {
        return Ok(&[]);
    }
    ensure!(
        !pointer.is_null() && length <= isize::MAX as usize / std::mem::size_of::<T>(),
        "DirectWrite returned an invalid native array"
    );
    Ok(unsafe { std::slice::from_raw_parts(pointer, length) })
}

mod rasterizer {
    // SPDX-License-Identifier: Apache-2.0

    use crate::{
        Bounds, DevicePixels, RenderGlyphParams, SUBPIXEL_VARIANTS_X, SUBPIXEL_VARIANTS_Y, Size,
        point, size,
    };
    use anyhow::{Context as _, Result, ensure};
    use std::mem::ManuallyDrop;
    use windows::{
        Win32::{
            Foundation::{DWRITE_E_NOCOLOR, HMODULE},
            Graphics::{
                Direct2D::{Common::*, *},
                Direct3D::D3D_DRIVER_TYPE_WARP,
                Direct3D11::*,
                DirectWrite::*,
                Dxgi::{Common::*, IDXGIDevice},
            },
        },
        core::Interface,
    };
    use windows_numerics::{Matrix3x2, Vector2};

    const COLOR_GLYPH_FORMATS: DWRITE_GLYPH_IMAGE_FORMATS = DWRITE_GLYPH_IMAGE_FORMATS(
        DWRITE_GLYPH_IMAGE_FORMATS_COLR.0
            | DWRITE_GLYPH_IMAGE_FORMATS_SVG.0
            | DWRITE_GLYPH_IMAGE_FORMATS_PNG.0
            | DWRITE_GLYPH_IMAGE_FORMATS_JPEG.0
            | DWRITE_GLYPH_IMAGE_FORMATS_TIFF.0
            | DWRITE_GLYPH_IMAGE_FORMATS_PREMULTIPLIED_B8G8R8A8.0,
    );

    struct GlyphRun(DWRITE_GLYPH_RUN);
    impl Drop for GlyphRun {
        fn drop(&mut self) {
            unsafe { ManuallyDrop::drop(&mut self.0.fontFace) };
        }
    }

    fn with_native_glyph_run<T>(
        face: &IDWriteFontFace3,
        glyph: u16,
        font_em_size: f32,
        callback: impl FnOnce(&DWRITE_GLYPH_RUN) -> Result<T>,
    ) -> Result<T> {
        ensure!(
            font_em_size.is_finite() && font_em_size > 0.0,
            "Invalid DirectWrite glyph size"
        );
        let glyphs = [glyph];
        let advances = [0.0];
        let offsets = [DWRITE_GLYPH_OFFSET::default()];
        let run = GlyphRun(DWRITE_GLYPH_RUN {
            fontFace: ManuallyDrop::new(Some(face.cast()?)),
            fontEmSize: font_em_size,
            glyphCount: 1,
            glyphIndices: glyphs.as_ptr(),
            glyphAdvances: advances.as_ptr(),
            glyphOffsets: offsets.as_ptr(),
            isSideways: false.into(),
            bidiLevel: 0,
        });
        callback(&run.0)
    }

    pub(super) fn native_color_glyph(
        factory: &IDWriteFactory5,
        face: &IDWriteFontFace3,
        glyph: u16,
        font_em_size: f32,
    ) -> Result<bool> {
        with_native_glyph_run(face, glyph, font_em_size, |run| {
            let factory: IDWriteFactory4 = factory.cast()?;
            // Image-format metadata can omit color glyphs that DirectWrite can translate.
            match unsafe {
                factory.TranslateColorGlyphRun(
                    Vector2::default(),
                    run,
                    None,
                    COLOR_GLYPH_FORMATS,
                    DWRITE_MEASURING_MODE_NATURAL,
                    None,
                    0,
                )
            } {
                Ok(colors) => Ok(unsafe { colors.MoveNext() }
                    .context("Reading native glyph color layers")?
                    .as_bool()),
                Err(error) if error.code() == DWRITE_E_NOCOLOR => {
                    // Paint trees use newer rendering APIs; retain their existing positive recognition.
                    let face: IDWriteFontFace4 = face.cast()?;
                    let formats = unsafe { face.GetGlyphImageFormats(glyph, 1, u32::MAX) }
                        .context("Reading native glyph paint-tree formats")?;
                    Ok((formats & DWRITE_GLYPH_IMAGE_FORMATS_COLR_PAINT_TREE).0 != 0)
                }
                Err(error) => Err(error).context("Translating native glyph color layers"),
            }
        })
    }

    fn with_glyph_run<T>(
        face: &IDWriteFontFace3,
        parameters: &RenderGlyphParams,
        callback: impl FnOnce(&DWRITE_GLYPH_RUN, Vector2) -> Result<T>,
    ) -> Result<T> {
        ensure!(
            parameters.font_size.0.is_finite()
                && parameters.font_size.0 > 0.0
                && parameters.scale_factor.is_finite()
                && parameters.scale_factor > 0.0,
            "Invalid DirectWrite raster size"
        );
        let pixels = parameters.font_size.0 * parameters.scale_factor;
        ensure!(
            pixels.is_finite() && pixels > 0.0,
            "DirectWrite raster size overflow"
        );
        with_native_glyph_run(face, u16::try_from(parameters.glyph_id.0)?, pixels, |run| {
            callback(
                run,
                Vector2 {
                    X: f32::from(parameters.subpixel_variant.x) / f32::from(SUBPIXEL_VARIANTS_X),
                    Y: f32::from(parameters.subpixel_variant.y) / f32::from(SUBPIXEL_VARIANTS_Y),
                },
            )
        })
    }

    fn glyph_analysis(
        factory: &IDWriteFactory5,
        run: &DWRITE_GLYPH_RUN,
        origin: Vector2,
    ) -> Result<IDWriteGlyphRunAnalysis> {
        Ok(unsafe {
            factory.CreateGlyphRunAnalysis(
                run,
                None,
                DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC,
                DWRITE_MEASURING_MODE_NATURAL,
                DWRITE_GRID_FIT_MODE_ENABLED,
                DWRITE_TEXT_ANTIALIAS_MODE_GRAYSCALE,
                origin.X,
                origin.Y,
            )
        }?)
    }

    pub(super) fn native_raster_bounds(
        factory: &IDWriteFactory5,
        face: &IDWriteFontFace3,
        parameters: &RenderGlyphParams,
    ) -> Result<Bounds<DevicePixels>> {
        if !parameters.is_emoji {
            return with_glyph_run(face, parameters, |run, origin| {
                let analysis = glyph_analysis(factory, run, origin)?;
                let bounds = unsafe { analysis.GetAlphaTextureBounds(DWRITE_TEXTURE_ALIASED_1x1) }?;
                let width = bounds
                    .right
                    .checked_sub(bounds.left)
                    .context("DirectWrite raster width overflow")?;
                let height = bounds
                    .bottom
                    .checked_sub(bounds.top)
                    .context("DirectWrite raster height overflow")?;
                ensure!(
                    width >= 0 && height >= 0,
                    "DirectWrite returned inverted raster bounds"
                );
                Ok(Bounds::new(
                    point(DevicePixels(bounds.left), DevicePixels(bounds.top)),
                    size(DevicePixels(width), DevicePixels(height)),
                ))
            });
        }
        with_glyph_run(face, parameters, |run, origin| {
            let mut metrics = DWRITE_FONT_METRICS1::default();
            unsafe { face.GetMetrics(&mut metrics) };
            ensure!(
                metrics.Base.designUnitsPerEm > 0,
                "DirectWrite font has no design units"
            );
            let scale = f64::from(run.fontEmSize) / f64::from(metrics.Base.designUnitsPerEm);
            // A color glyph's paint can exceed its monochrome outline; retain the native face's full ink bounds.
            let left =
                (f64::from(metrics.glyphBoxLeft) * scale + f64::from(origin.X)).floor() - 1.0;
            let top = (-f64::from(metrics.glyphBoxTop) * scale + f64::from(origin.Y)).floor() - 1.0;
            let right =
                (f64::from(metrics.glyphBoxRight) * scale + f64::from(origin.X)).ceil() + 1.0;
            let bottom =
                (-f64::from(metrics.glyphBoxBottom) * scale + f64::from(origin.Y)).ceil() + 1.0;
            ensure!(
                [left, top, right, bottom]
                    .into_iter()
                    .all(|value| value.is_finite()
                        && value >= i32::MIN as f64
                        && value <= i32::MAX as f64),
                "DirectWrite color raster bounds exceed device coordinates"
            );
            let width = (right as i32)
                .checked_sub(left as i32)
                .context("DirectWrite color raster width overflow")?;
            let height = (bottom as i32)
                .checked_sub(top as i32)
                .context("DirectWrite color raster height overflow")?;
            ensure!(
                width > 0 && height > 0,
                "DirectWrite color font has empty native bounds"
            );
            Ok(Bounds::new(
                point(DevicePixels(left as i32), DevicePixels(top as i32)),
                size(DevicePixels(width), DevicePixels(height)),
            ))
        })
    }

    pub(super) fn rasterize_monochrome(
        factory: &IDWriteFactory5,
        face: &IDWriteFontFace3,
        parameters: &RenderGlyphParams,
        bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        let width = usize::try_from(bounds.size.width.0)?;
        let height = usize::try_from(bounds.size.height.0)?;
        let length = width
            .checked_mul(height)
            .context("DirectWrite glyph allocation overflow")?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length)?;
        bytes.resize(length, 0);
        if length > 0 {
            let right = bounds
                .origin
                .x
                .0
                .checked_add(bounds.size.width.0)
                .context("DirectWrite raster right overflow")?;
            let bottom = bounds
                .origin
                .y
                .0
                .checked_add(bounds.size.height.0)
                .context("DirectWrite raster bottom overflow")?;
            let rectangle = windows::Win32::Foundation::RECT {
                left: bounds.origin.x.0,
                top: bounds.origin.y.0,
                right,
                bottom,
            };
            with_glyph_run(face, parameters, |run, origin| {
                let analysis = glyph_analysis(factory, run, origin)?;
                unsafe {
                    analysis.CreateAlphaTexture(DWRITE_TEXTURE_ALIASED_1x1, &rectangle, &mut bytes)
                }?;
                Ok(())
            })?;
        }
        Ok((bounds.size, bytes))
    }

    struct ComApartment(std::marker::PhantomData<std::rc::Rc<()>>);

    impl ComApartment {
        fn new() -> Result<Self> {
            unsafe {
                windows::Win32::System::Com::CoInitializeEx(
                    None,
                    windows::Win32::System::Com::COINIT_MULTITHREADED,
                )
                .ok()?;
            }
            Ok(Self(std::marker::PhantomData))
        }
    }

    impl Drop for ComApartment {
        fn drop(&mut self) {
            unsafe { windows::Win32::System::Com::CoUninitialize() };
        }
    }

    enum ColorMessage {
        Rasterize {
            factory: IDWriteFactory5,
            face: IDWriteFontFace3,
            parameters: RenderGlyphParams,
            bounds: Bounds<DevicePixels>,
            reply: std::sync::mpsc::SyncSender<Result<(Size<DevicePixels>, Vec<u8>)>>,
        },
        Exit,
    }

    pub(super) struct ColorRasterizer {
        sender: std::sync::mpsc::Sender<ColorMessage>,
        worker: Option<std::thread::JoinHandle<()>>,
    }

    impl ColorRasterizer {
        pub(super) fn new() -> Result<Self> {
            let (sender, receiver) = std::sync::mpsc::channel();
            let (ready_sender, ready_receiver) = std::sync::mpsc::sync_channel(1);
            // Native color decoders use COM; keep initialization, device objects and teardown on one owned thread.
            let worker = std::thread::Builder::new()
                .name("wgpui-native-color-text".into())
                .spawn(move || {
                    let apartment = match ComApartment::new() {
                        Ok(apartment) => apartment,
                        Err(error) => {
                            if ready_sender.send(Err(error)).is_err() {
                                log::error!("Native color initialization receiver closed");
                            }
                            return;
                        }
                    };
                    let renderer = match NativeColorRasterizer::new() {
                        Ok(renderer) => renderer,
                        Err(error) => {
                            if ready_sender.send(Err(error)).is_err() {
                                log::error!("Native color initialization receiver closed");
                            }
                            return;
                        }
                    };
                    if ready_sender.send(Ok(())).is_err() {
                        log::error!("Native color initialization receiver closed");
                        return;
                    }
                    while let Ok(message) = receiver.recv() {
                        match message {
                            ColorMessage::Rasterize {
                                factory,
                                face,
                                parameters,
                                bounds,
                                reply,
                            } => {
                                let result =
                                    renderer.rasterize(&factory, &face, &parameters, bounds);
                                if reply.send(result).is_err() {
                                    log::error!("Native color glyph result receiver closed");
                                }
                            }
                            ColorMessage::Exit => break,
                        }
                    }
                    drop(renderer);
                    drop(apartment);
                })
                .context("Could not start the native color glyph worker")?;
            let initialization = ready_receiver
                .recv()
                .context("Native color worker stopped during initialization")
                .and_then(|result| result);
            if let Err(error) = initialization {
                if worker.join().is_err() {
                    log::error!("Native color worker panicked during initialization");
                }
                return Err(error);
            }
            Ok(Self {
                sender,
                worker: Some(worker),
            })
        }

        pub(super) fn rasterize(
            &self,
            factory: &IDWriteFactory5,
            face: &IDWriteFontFace3,
            parameters: &RenderGlyphParams,
            bounds: Bounds<DevicePixels>,
        ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
            let (reply, receiver) = std::sync::mpsc::sync_channel(1);
            self.sender
                .send(ColorMessage::Rasterize {
                    factory: factory.clone(),
                    face: face.clone(),
                    parameters: parameters.clone(),
                    bounds,
                    reply,
                })
                .map_err(|_| anyhow::anyhow!("Native color glyph worker is unavailable"))?;
            receiver
                .recv()
                .context("Native color worker stopped before returning its glyph")?
        }
    }

    impl Drop for ColorRasterizer {
        fn drop(&mut self) {
            if self.sender.send(ColorMessage::Exit).is_err() {
                log::error!("Native color glyph worker stopped before shutdown");
            }
            if let Some(worker) = self.worker.take()
                && worker.join().is_err()
            {
                log::error!("Native color glyph worker panicked");
            }
        }
    }

    struct NativeColorRasterizer {
        _device: ID3D11Device,
        context: ID2D1DeviceContext4,
    }

    impl NativeColorRasterizer {
        pub(super) fn new() -> Result<Self> {
            let mut device = None;
            unsafe {
                D3D11CreateDevice(
                    None,
                    D3D_DRIVER_TYPE_WARP,
                    HMODULE::default(),
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    None,
                    D3D11_SDK_VERSION,
                    Some(&mut device),
                    None,
                    None,
                )
            }?;
            let device = device.context("Windows did not create the native glyph raster device")?;
            let dxgi: IDXGIDevice = device.cast()?;
            let properties = D2D1_CREATION_PROPERTIES {
                threadingMode: D2D1_THREADING_MODE_MULTI_THREADED,
                debugLevel: D2D1_DEBUG_LEVEL_NONE,
                options: D2D1_DEVICE_CONTEXT_OPTIONS_NONE,
            };
            let native = unsafe { D2D1CreateDevice(&dxgi, Some(&properties)) }?;
            let context: ID2D1DeviceContext4 =
                unsafe { native.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE) }?.cast()?;
            unsafe {
                context.SetDpi(96.0, 96.0);
                context.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
                context.SetTransform(&Matrix3x2::identity());
            }
            Ok(Self {
                _device: device,
                context,
            })
        }

        pub(super) fn rasterize(
            &self,
            factory: &IDWriteFactory5,
            face: &IDWriteFontFace3,
            parameters: &RenderGlyphParams,
            bounds: Bounds<DevicePixels>,
        ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
            let width = u32::try_from(bounds.size.width.0)?;
            let height = u32::try_from(bounds.size.height.0)?;
            ensure!(width > 0 && height > 0, "DirectWrite color raster is empty");
            let properties = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
                colorContext: ManuallyDrop::new(None),
            };
            let bitmap = unsafe {
                self.context
                    .CreateBitmap(D2D_SIZE_U { width, height }, None, 0, &properties)
            }?;
            let brush = unsafe {
                self.context.CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                    None,
                )
            }?;
            unsafe {
                self.context.SetTarget(&bitmap);
                self.context.BeginDraw();
                self.context.Clear(Some(&D2D1_COLOR_F {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.0,
                }));
            }
            let drawing = with_glyph_run(face, parameters, |run, phase| {
                let origin = Vector2 {
                    X: phase.X - bounds.origin.x.0 as f32,
                    Y: phase.Y - bounds.origin.y.0 as f32,
                };
                if let Ok(modern) = self.context.cast::<ID2D1DeviceContext7>() {
                    unsafe {
                        modern.DrawGlyphRunWithColorSupport(
                            origin,
                            run,
                            None,
                            &brush,
                            None,
                            0,
                            DWRITE_MEASURING_MODE_NATURAL,
                            D2D1_COLOR_BITMAP_GLYPH_SNAP_OPTION_DISABLE,
                        );
                    }
                    return Ok(());
                }
                let factory4: IDWriteFactory4 = factory.cast()?;
                let formats = COLOR_GLYPH_FORMATS
                    | DWRITE_GLYPH_IMAGE_FORMATS_TRUETYPE
                    | DWRITE_GLYPH_IMAGE_FORMATS_CFF;
                let colors = match unsafe {
                    factory4.TranslateColorGlyphRun(
                        origin,
                        run,
                        None,
                        formats,
                        DWRITE_MEASURING_MODE_NATURAL,
                        None,
                        0,
                    )
                } {
                    Ok(colors) => colors,
                    Err(error) if error.code() == DWRITE_E_NOCOLOR => {
                        unsafe {
                            self.context.DrawGlyphRun(
                                origin,
                                run,
                                None,
                                &brush,
                                DWRITE_MEASURING_MODE_NATURAL,
                            )
                        };
                        return Ok(());
                    }
                    Err(error) => return Err(error.into()),
                };
                while unsafe { colors.MoveNext() }?.as_bool() {
                    let pointer = unsafe { colors.GetCurrentRun() }?;
                    let color = unsafe { pointer.as_ref() }
                        .context("DirectWrite returned a null color run")?;
                    let origin = Vector2 {
                        X: color.Base.baselineOriginX,
                        Y: color.Base.baselineOriginY,
                    };
                    match color.glyphImageFormat {
                        DWRITE_GLYPH_IMAGE_FORMATS_PNG
                        | DWRITE_GLYPH_IMAGE_FORMATS_JPEG
                        | DWRITE_GLYPH_IMAGE_FORMATS_TIFF
                        | DWRITE_GLYPH_IMAGE_FORMATS_PREMULTIPLIED_B8G8R8A8 => unsafe {
                            self.context.DrawColorBitmapGlyphRun(
                                color.glyphImageFormat,
                                origin,
                                &color.Base.glyphRun,
                                color.measuringMode,
                                D2D1_COLOR_BITMAP_GLYPH_SNAP_OPTION_DISABLE,
                            );
                        },
                        DWRITE_GLYPH_IMAGE_FORMATS_SVG => unsafe {
                            self.context.DrawSvgGlyphRun(
                                origin,
                                &color.Base.glyphRun,
                                &brush,
                                None,
                                0,
                                color.measuringMode,
                            );
                        },
                        DWRITE_GLYPH_IMAGE_FORMATS_COLR
                        | DWRITE_GLYPH_IMAGE_FORMATS_TRUETYPE
                        | DWRITE_GLYPH_IMAGE_FORMATS_CFF => {
                            let run_color = if color.Base.paletteIndex == u16::MAX {
                                D2D1_COLOR_F {
                                    r: 0.0,
                                    g: 0.0,
                                    b: 0.0,
                                    a: 1.0,
                                }
                            } else {
                                D2D1_COLOR_F {
                                    r: color.Base.runColor.r,
                                    g: color.Base.runColor.g,
                                    b: color.Base.runColor.b,
                                    a: color.Base.runColor.a,
                                }
                            };
                            unsafe {
                                brush.SetColor(&run_color);
                                self.context.DrawGlyphRun(
                                    origin,
                                    &color.Base.glyphRun,
                                    color
                                        .Base
                                        .glyphRunDescription
                                        .as_ref()
                                        .map(|value| value as *const _),
                                    &brush,
                                    color.measuringMode,
                                );
                            }
                        }
                        format => anyhow::bail!("Unsupported native color glyph format {format:?}"),
                    }
                }
                Ok(())
            });
            let finish = unsafe { self.context.EndDraw(None, None) };
            unsafe { self.context.SetTarget(None::<&ID2D1Image>) };
            match (drawing, finish) {
                (Err(error), Err(finish)) => {
                    log::error!("Direct2D glyph draw also failed during cleanup: {finish}");
                    return Err(error);
                }
                (Err(error), Ok(())) => return Err(error),
                (Ok(()), Err(error)) => return Err(error.into()),
                (Ok(()), Ok(())) => {}
            }
            let mut properties = properties;
            properties.bitmapOptions =
                D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW;
            let staging = unsafe {
                self.context
                    .CreateBitmap(D2D_SIZE_U { width, height }, None, 0, &properties)
            }?;
            unsafe { staging.CopyFromBitmap(None, &bitmap, None) }?;
            let mapping = BitmapMapping {
                rectangle: unsafe { staging.Map(D2D1_MAP_OPTIONS_READ) }?,
                bitmap: &staging,
                active: true,
            };
            let stride = usize::try_from(mapping.rectangle.pitch)?;
            let row_bytes = usize::try_from(width)?
                .checked_mul(4)
                .context("Color glyph row stride overflow")?;
            let length = row_bytes
                .checked_mul(usize::try_from(height)?)
                .context("Color glyph allocation overflow")?;
            let mapped_length = stride
                .checked_mul(usize::try_from(height)?)
                .context("Mapped color glyph length overflow")?;
            ensure!(
                stride >= row_bytes
                    && !mapping.rectangle.bits.is_null()
                    && mapped_length <= isize::MAX as usize,
                "Direct2D returned invalid mapped glyph rows"
            );
            let data = unsafe { std::slice::from_raw_parts(mapping.rectangle.bits, mapped_length) };
            let mut bytes = Vec::new();
            bytes.try_reserve_exact(length)?;
            for row in data.chunks_exact(stride).take(usize::try_from(height)?) {
                bytes.extend_from_slice(
                    row.get(..row_bytes)
                        .context("Mapped glyph row is truncated")?,
                );
            }
            mapping.finish()?;
            ensure!(bytes.len() == length, "Mapped glyph bitmap is truncated");
            crate::unpremultiply_bgra_to_rgba(&mut bytes);
            Ok((bounds.size, bytes))
        }
    }

    struct BitmapMapping<'a> {
        bitmap: &'a ID2D1Bitmap1,
        rectangle: D2D1_MAPPED_RECT,
        active: bool,
    }
    impl BitmapMapping<'_> {
        fn finish(mut self) -> Result<()> {
            self.active = false;
            unsafe { self.bitmap.Unmap() }?;
            Ok(())
        }
    }
    impl Drop for BitmapMapping<'_> {
        fn drop(&mut self) {
            if self.active
                && let Err(error) = unsafe { self.bitmap.Unmap() }
            {
                log::error!("Could not release the native glyph bitmap mapping: {error}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FontFallbacks, FontWeight, font};
    use std::sync::Arc;

    fn shape(system: &DirectWriteTextSystem, id: FontId, text: &str) -> Result<LineLayout> {
        system.0.lock().shape(
            text,
            px(18.0),
            &[FontRun {
                len: text.len(),
                font_id: id,
            }],
        )
    }

    fn glyph_summary(system: &DirectWriteTextSystem, line: &LineLayout) -> Result<Vec<String>> {
        let state = system.0.lock();
        line.runs
            .iter()
            .map(|run| {
                let face = &state.font(run.font_id)?.face;
                let family = localized_name(&unsafe { face.GetFamilyNames() }?, &state.locale)?;
                let color_font = unsafe { face.IsColorFont() }.as_bool();
                let face: IDWriteFontFace4 = face.cast()?;
                let glyphs = run
                    .glyphs
                    .iter()
                    .map(|glyph| {
                        let formats = unsafe {
                            face.GetGlyphImageFormats(u16::try_from(glyph.id.0)?, 1, u32::MAX)
                        }?;
                        Ok(format!(
                            "byte={} glyph={} color={} formats={:#x}",
                            glyph.index, glyph.id.0, glyph.is_emoji, formats.0
                        ))
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(format!("{family:?} color_font={color_font}: {glyphs:?}"))
            })
            .collect()
    }

    #[test]
    fn missing_logical_system_face_maps_to_an_installed_monospace_outline() -> Result<()> {
        let factory: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }?;
        let mut logical = LOGFONTW {
            lfPitchAndFamily: FIXED_PITCH.0 | FF_MODERN.0,
            ..Default::default()
        };
        let missing = "WGPUI-System-Mono-Fixture"
            .encode_utf16()
            .collect::<Vec<_>>();
        logical
            .lfFaceName
            .get_mut(..missing.len())
            .context("Fixture family exceeds LOGFONT capacity")?
            .copy_from_slice(&missing);
        let native = native_system_font(&factory, &logical, true)?;
        assert!(unsafe { native.cast::<IDWriteFont1>()?.IsMonospacedFont() }.as_bool());
        let face = unsafe { native.CreateFontFace() }?;
        let mut glyphs = [0_u16; 2];
        unsafe {
            face.GetGlyphIndices(
                [u32::from('i'), u32::from('W')].as_ptr(),
                2,
                glyphs.as_mut_ptr(),
            )
        }?;
        assert!(glyphs.iter().all(|glyph| *glyph != 0));
        let mut advances = [0_i32; 2];
        unsafe {
            face.cast::<IDWriteFontFace1>()?.GetDesignGlyphAdvances(
                2,
                glyphs.as_ptr(),
                advances.as_mut_ptr(),
                false,
            )
        }?;
        assert_eq!(advances[0], advances[1]);
        Ok(())
    }

    #[test]
    fn native_defaults_resolve_os_faces_and_monospace_advances() -> Result<()> {
        let system = DirectWriteTextSystem::new()?;
        let ui = system.font_id(&font(".SystemUIFont"))?;
        let mono = system.font_id(&font(".SystemMonospaceFont"))?;
        let state = system.0.lock();
        for (id, expected) in [(ui, &state.ui_family), (mono, &state.mono_family)] {
            let names = unsafe { state.font(id)?.face.GetFamilyNames() }?;
            assert_eq!(&localized_name(&names, &state.locale)?, expected);
            assert!(native_metrics(&state.font(id)?.face).units_per_em > 0);
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
        assert!(
            !system
                .all_font_names()
                .iter()
                .any(|name| name.starts_with("WGPUI-Selection-"))
        );
        Ok(())
    }

    #[test]
    fn native_shaping_preserves_utf8_bom_fallback_and_rtl_positions() -> Result<()> {
        let system = DirectWriteTextSystem::new()?;
        let id = system.font_id(&font(".SystemUIFont"))?;
        let text = "\u{feff}A😀e\u{301}العربية漢字";
        let line = shape(&system, id, text)?;
        assert_eq!(line.len, text.len());
        assert!(line.width.0 > 0.0);
        let glyphs = line
            .runs
            .iter()
            .flat_map(|run| &run.glyphs)
            .collect::<Vec<_>>();
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
                .any(|glyph| glyph.index == emoji && glyph.is_emoji),
            "Native emoji classification: {:?}",
            glyph_summary(&system, &line)?
        );
        assert!(
            glyphs
                .iter()
                .any(|glyph| glyph.index == 3 && !glyph.is_emoji)
        );
        let line = shape(&system, id, "مرحبا")?;
        let glyphs = line
            .runs
            .iter()
            .flat_map(|run| &run.glyphs)
            .collect::<Vec<_>>();
        let first = glyphs
            .iter()
            .find(|glyph| glyph.index == 0)
            .context("First RTL cluster is missing")?;
        let last = glyphs
            .iter()
            .max_by_key(|glyph| glyph.index)
            .context("Last RTL cluster is missing")?;
        assert!(first.position.x > last.position.x);
        assert!(
            glyphs
                .windows(2)
                .all(|pair| pair[0].position.x <= pair[1].position.x)
        );
        Ok(())
    }

    #[test]
    fn native_rasterization_preserves_weight_scale_phase_and_color() -> Result<()> {
        let system = DirectWriteTextSystem::new()?;
        let mut weights = Vec::new();
        for requested in [
            font(".SystemUIFont"),
            Font {
                weight: FontWeight::BOLD,
                ..font(".SystemUIFont")
            },
        ] {
            let id = system.font_id(&requested)?;
            let glyph = system
                .glyph_for_char(id, 'M')
                .context("OS UI font has no M")?;
            let parameters = RenderGlyphParams {
                font_id: id,
                glyph_id: glyph,
                font_size: px(18.0),
                scale_factor: 1.0,
                subpixel_variant: point(0, 0),
                is_emoji: false,
            };
            let mut phases = Vec::new();
            for (scale, phase) in [(1.0, 0), (1.0, 1), (1.5, 1), (2.0, 3)] {
                let parameters = RenderGlyphParams {
                    scale_factor: scale,
                    subpixel_variant: point(phase, 0),
                    ..parameters.clone()
                };
                let bounds = system.glyph_raster_bounds(&parameters)?;
                let (dimensions, bytes) = system.rasterize_glyph(&parameters, bounds)?;
                assert_eq!(
                    bytes.len(),
                    usize::try_from(dimensions.width.0)? * usize::try_from(dimensions.height.0)?
                );
                assert!(bytes.iter().any(|value| *value > 0));
                phases.push((dimensions, bytes));
            }
            assert_ne!(phases.first(), phases.get(1));
            weights.push(phases);
        }
        assert_ne!(weights.first(), weights.get(1));
        let ui = system.font_id(&font(".SystemUIFont"))?;
        let line = shape(&system, ui, "😀")?;
        let (id, glyph) = line
            .runs
            .iter()
            .find_map(|run| {
                run.glyphs
                    .iter()
                    .find(|glyph| glyph.is_emoji)
                    .map(|glyph| (run.font_id, glyph.id))
            })
            .with_context(|| {
                format!(
                    "Native color emoji fallback is missing: {:?}",
                    glyph_summary(&system, &line)
                )
            })?;
        let parameters = RenderGlyphParams {
            font_id: id,
            glyph_id: glyph,
            font_size: px(24.0),
            scale_factor: 1.0,
            subpixel_variant: point(0, 0),
            is_emoji: true,
        };
        let bounds = system.glyph_raster_bounds(&parameters)?;
        let (dimensions, bytes) = system.rasterize_glyph(&parameters, bounds)?;
        assert_eq!(
            bytes.len(),
            usize::try_from(dimensions.width.0)? * usize::try_from(dimensions.height.0)? * 4
        );
        assert!(
            bytes
                .chunks_exact(4)
                .any(|pixel| pixel[3] > 0 && (pixel[0] > 0 || pixel[1] > 0 || pixel[2] > 0))
        );
        Ok(())
    }

    #[test]
    fn native_font_and_line_access_is_safe_across_two_calling_threads() -> Result<()> {
        let system = Arc::new(DirectWriteTextSystem::new()?);
        let mut callers = Vec::new();
        for _ in 0..2 {
            let system = system.clone();
            callers.push(std::thread::spawn(move || -> Result<(FontId, Pixels)> {
                let id = system.font_id(&font(".SystemUIFont"))?;
                Ok((id, shape(&system, id, "Concurrent العربية")?.width))
            }));
        }
        let mut results = Vec::new();
        for caller in callers {
            results.push(
                caller
                    .join()
                    .map_err(|_| anyhow::anyhow!("Native DirectWrite caller panicked"))??,
            );
        }
        assert_eq!(results.first(), results.get(1));
        Ok(())
    }

    #[test]
    fn native_tabs_and_font_boundaries_preserve_editor_positions() -> Result<()> {
        let system = DirectWriteTextSystem::new()?;
        let id = system.font_id(&font(".SystemMonospaceFont"))?;
        let line = shape(&system, id, "A\tB\tC\tD")?;
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
                .context("A glyph following a tab is missing")?;
            assert!((glyph.position.x.0 - advance * cell).abs() < 0.02);
        }
        let line = system.0.lock().shape(
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
                .any(|glyph| glyph.index == 1)
        );
        Ok(())
    }

    fn native_font_data(face: &IDWriteFontFace3) -> Result<Vec<u8>> {
        let mut count = 0;
        unsafe { face.GetFiles(&mut count, None) }?;
        ensure!(count > 0, "Native font has no font file");
        let mut files = vec![None; usize::try_from(count)?];
        unsafe { face.GetFiles(&mut count, Some(files.as_mut_ptr())) }?;
        let file = files
            .into_iter()
            .flatten()
            .next()
            .context("Native font file is missing")?;
        let mut key = std::ptr::null_mut();
        let mut key_length = 0;
        unsafe { file.GetReferenceKey(&mut key, &mut key_length) }?;
        ensure!(
            !key.is_null() && key_length > 0,
            "Native font file has no reference key"
        );
        let loader = unsafe { file.GetLoader() }?;
        let stream = unsafe { loader.CreateStreamFromKey(key, key_length) }?;
        let length = usize::try_from(unsafe { stream.GetFileSize() }?)?;
        ensure!(
            length > 0 && length <= isize::MAX as usize,
            "Native font file has invalid size"
        );
        let mut data = std::ptr::null_mut();
        let mut context = std::ptr::null_mut();
        unsafe { stream.ReadFileFragment(&mut data, 0, u64::try_from(length)?, &mut context) }?;
        struct Fragment<'a> {
            stream: &'a IDWriteFontFileStream,
            context: *mut c_void,
        }
        impl Drop for Fragment<'_> {
            fn drop(&mut self) {
                unsafe { self.stream.ReleaseFileFragment(self.context) };
            }
        }
        let _fragment = Fragment {
            stream: &stream,
            context,
        };
        ensure!(!data.is_null(), "Native font file returned a null fragment");
        let data = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), length) };
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length)?;
        bytes.extend_from_slice(data);
        Ok(bytes)
    }

    #[test]
    fn native_application_font_registration_preserves_existing_ids() -> Result<()> {
        let system = DirectWriteTextSystem::new()?;
        let id = system.font_id(&font(".SystemMonospaceFont"))?;
        let bytes = native_font_data(&system.0.lock().font(id)?.face)?;
        let glyph = system
            .glyph_for_char(id, 'M')
            .context("OS monospace font has no M")?;
        let advance = system.advance(id, glyph)?;
        system.add_fonts(vec![Cow::Owned(bytes)])?;
        assert_eq!(system.advance(id, glyph)?, advance);
        let replacement = system.font_id(&font(".SystemMonospaceFont"))?;
        assert_eq!(system.advance(replacement, glyph)?, advance);
        assert!(
            system
                .add_fonts(vec![Cow::Borrowed(b"invalid-font-fixture")])
                .is_err()
        );
        assert_eq!(system.advance(id, glyph)?, advance);
        Ok(())
    }

    #[test]
    fn distinct_fallback_policies_remain_scoped_to_their_font_runs() -> Result<()> {
        let system = DirectWriteTextSystem::new()?;
        let mut state = system.0.lock();
        let base = font(".SystemMonospaceFont");
        let base_id = state.resolve(&base)?;
        let face = state.font(base_id)?.face.clone();
        let mut fixture = None;
        for character in ['😀', '𝔄', '𝐀', '⚙', '☀', '☺'] {
            if native_glyph(&face, character).is_some() {
                continue;
            }
            let mut families: Vec<(String, String)> = Vec::new();
            for family in [
                "Segoe UI Emoji",
                "Segoe UI Symbol",
                "Segoe UI",
                "Cambria Math",
                "Arial",
                "Times New Roman",
            ] {
                if state.find_family(family)?.is_none() {
                    continue;
                }
                let id = state.resolve(&font(family))?;
                let face = &state.font(id)?.face;
                if native_glyph(face, character).is_none() {
                    continue;
                }
                let physical = localized_name(&unsafe { face.GetFamilyNames() }?, &state.locale)?;
                if families.iter().any(|(_, name)| name == &physical) {
                    continue;
                }
                families.push((family.to_owned(), physical));
                if families.len() == 2 {
                    break;
                }
            }
            if families.len() == 2 {
                fixture = Some((character, families));
                break;
            }
        }
        let (character, families) = fixture.context(
            "Native fallback acceptance needs two distinct installed OS fonts for a shared glyph",
        )?;
        let mut ids = Vec::new();
        for (family, _) in &families {
            ids.push(state.resolve(&Font {
                fallbacks: Some(FontFallbacks::from_fonts(vec![family.clone()])),
                ..base.clone()
            })?);
        }
        let first = *ids
            .first()
            .context("First fallback fixture font is missing")?;
        let second = *ids
            .get(1)
            .context("Second fallback fixture font is missing")?;
        let text = character.to_string().repeat(2);
        let length = character.len_utf8();
        let line = state.shape(
            &text,
            px(18.0),
            &[
                FontRun {
                    len: length,
                    font_id: first,
                },
                FontRun {
                    len: length,
                    font_id: second,
                },
            ],
        )?;
        for (index, (_, expected)) in families.iter().enumerate() {
            let run = line
                .runs
                .iter()
                .find(|run| run.glyphs.iter().any(|glyph| glyph.index == index * length))
                .context("Native fallback run is missing")?;
            let actual = localized_name(
                &unsafe { state.font(run.font_id)?.face.GetFamilyNames() }?,
                &state.locale,
            )?;
            assert_eq!(&actual, expected);
        }
        Ok(())
    }
    #[test]
    fn native_paragraph_direction_ignores_directional_isolate_contents() -> Result<()> {
        for (text, expected) in [
            (
                "\u{2067}אב\u{2069} A",
                DWRITE_READING_DIRECTION_LEFT_TO_RIGHT,
            ),
            (
                "\u{2066}Latin\u{2069} אב",
                DWRITE_READING_DIRECTION_RIGHT_TO_LEFT,
            ),
            (
                "\u{2067}אב\u{2066}Latin\u{2069}\u{2069} A",
                DWRITE_READING_DIRECTION_LEFT_TO_RIGHT,
            ),
            ("\u{2067}אב", DWRITE_READING_DIRECTION_LEFT_TO_RIGHT),
            ("\u{2069}אב", DWRITE_READING_DIRECTION_RIGHT_TO_LEFT),
            (" 123 ", DWRITE_READING_DIRECTION_LEFT_TO_RIGHT),
        ] {
            let utf16 = text.encode_utf16().collect::<Vec<_>>();
            let mut directions = vec![0_u16; utf16.len()];
            unsafe { GetStringTypeW(CT_CTYPE2, &utf16, directions.as_mut_ptr()) }?;
            assert_eq!(
                paragraph_reading_direction(&utf16, &directions),
                expected,
                "{text:?}"
            );
        }
        Ok(())
    }
}
