// SPDX-License-Identifier: Apache-2.0

use crate::{
    Bounds, DevicePixels, Font, FontFeatures, FontId, FontMetrics, FontRun, FontStyle, GlyphId,
    LineLayout, Pixels, PlatformTextSystem, RenderGlyphParams, SUBPIXEL_VARIANTS_X, ShapedGlyph,
    ShapedRun, Size, point, px, size, swap_rgba_pa_to_bgra,
};
use anyhow::{Context as _, Result, anyhow, ensure};
use collections::HashMap;
use pango::{
    glib::translate::{ToGlibPtr, from_glib},
    prelude::*,
};
use std::{
    borrow::Cow,
    ffi::{CString, c_char, c_int, c_void},
    io::Write,
    ops::Range,
    os::unix::ffi::OsStrExt,
    ptr::NonNull,
    sync::mpsc,
    thread,
};

const BASE_FONT_SIZE: f64 = 16.0;
const PANGO_SCALE: f64 = pango::SCALE as f64;

#[link(name = "fontconfig")]
unsafe extern "C" {
    fn FcInitLoadConfigAndFonts() -> *mut c_void;
    fn FcConfigDestroy(config: *mut c_void);
    fn FcConfigAppFontAddFile(config: *mut c_void, file: *const u8) -> c_int;
    fn FcPatternGetBool(
        pattern: *mut c_void,
        name: *const c_char,
        index: c_int,
        value: *mut c_int,
    ) -> c_int;
}
#[link(name = "pangoft2-1.0")]
unsafe extern "C" {
    fn pango_fc_font_map_set_config(font_map: *mut pango::ffi::PangoFontMap, config: *mut c_void);
    fn pango_fc_font_map_config_changed(font_map: *mut pango::ffi::PangoFontMap);
    fn pango_fc_font_get_pattern(font: *mut pango::ffi::PangoFont) -> *mut c_void;
}
#[link(name = "pango-1.0")]
unsafe extern "C" {
    fn pango_font_get_hb_font(font: *mut pango::ffi::PangoFont) -> *mut c_void;
}
#[link(name = "harfbuzz")]
unsafe extern "C" {
    fn hb_font_get_face(font: *mut c_void) -> *mut c_void;
    fn hb_face_get_upem(face: *mut c_void) -> u32;
    fn hb_face_get_glyph_count(face: *mut c_void) -> u32;
    fn hb_font_get_scale(font: *mut c_void, x: *mut c_int, y: *mut c_int);
    fn hb_font_get_nominal_glyph(font: *mut c_void, character: u32, glyph: *mut u32) -> c_int;
    fn hb_font_get_glyph_extents(
        font: *mut c_void,
        glyph: u32,
        extents: *mut NativeExtents,
    ) -> c_int;
    fn hb_face_reference_table(face: *mut c_void, tag: u32) -> *mut c_void;
    fn hb_blob_get_data(blob: *mut c_void, length: *mut u32) -> *const u8;
    fn hb_blob_destroy(blob: *mut c_void);
}

#[repr(C)]
#[derive(Default)]
struct NativeExtents {
    x_bearing: i32,
    y_bearing: i32,
    width: i32,
    height: i32,
}

struct FontConfig(NonNull<c_void>);
impl Drop for FontConfig {
    fn drop(&mut self) {
        unsafe { FcConfigDestroy(self.0.as_ptr()) };
    }
}

struct FontTable(NonNull<c_void>);
impl FontTable {
    fn bytes(&self) -> &[u8] {
        let mut length = 0;
        let pointer = unsafe { hb_blob_get_data(self.0.as_ptr(), &mut length) };
        if pointer.is_null() || length == 0 {
            return &[];
        }
        unsafe { std::slice::from_raw_parts(pointer, length as usize) }
    }
}
impl Drop for FontTable {
    fn drop(&mut self) {
        unsafe { hb_blob_destroy(self.0.as_ptr()) };
    }
}

type Operation = Box<dyn FnOnce(&mut State) + Send>;
enum Message {
    Run(Operation),
    Exit,
}

pub(crate) struct PangoTextSystem {
    sender: mpsc::Sender<Message>,
    worker: Option<thread::JoinHandle<()>>,
}

impl PangoTextSystem {
    pub(crate) fn new() -> Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        // Keep all Pango/Cairo GObjects on their creating thread without asserting unsafe Send.
        let worker = thread::Builder::new()
            .name("wgpui-native-text".into())
            .spawn(move || {
                let mut state = match State::new() {
                    Ok(state) => state,
                    Err(error) => {
                        if ready_sender.send(Err(error)).is_err() {
                            log::error!("Pango initialization receiver closed");
                        }
                        return;
                    }
                };
                if ready_sender.send(Ok(())).is_err() {
                    log::error!("Pango initialization receiver closed");
                    return;
                }
                while let Ok(message) = receiver.recv() {
                    match message {
                        Message::Run(operation) => operation(&mut state),
                        Message::Exit => break,
                    }
                }
            })
            .context("Could not start the Pango text worker")?;
        let initialization = ready_receiver
            .recv()
            .context("Pango worker stopped during initialization")
            .and_then(|value| value);
        if let Err(error) = initialization {
            if worker.join().is_err() {
                log::error!("Pango worker panicked during initialization");
            }
            return Err(error);
        }
        Ok(Self {
            sender,
            worker: Some(worker),
        })
    }

    fn execute<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut State) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.sender
            .send(Message::Run(Box::new(move |state| {
                if sender.send(operation(state)).is_err() {
                    log::error!("Pango text result receiver closed");
                }
            })))
            .map_err(|_| anyhow!("Pango text worker is unavailable"))?;
        receiver
            .recv()
            .context("Pango text worker stopped before returning its result")?
    }
}
impl Drop for PangoTextSystem {
    fn drop(&mut self) {
        if self.sender.send(Message::Exit).is_err() {
            log::error!("Pango text worker stopped before shutdown");
        }
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            log::error!("Pango text worker panicked");
        }
    }
}

struct FontRecord {
    native: pango::Font,
    description: pango::FontDescription,
    physical_key: String,
    features: FontFeatures,
    color: bool,
}
struct State {
    fonts: Vec<FontRecord>,
    selections: HashMap<Font, FontId>,
    fallback_ids: HashMap<(String, FontFeatures), FontId>,
    context: pango::Context,
    font_map: pango::FontMap,
    // Native fonts/context/map must drop before the configuration and its application-font files.
    config: FontConfig,
    font_files: Vec<tempfile::NamedTempFile>,
}
impl State {
    fn new() -> Result<Self> {
        let version = cairo::Version::new();
        ensure!(
            (version.major(), version.minor()) >= (1, 18),
            "Pango text requires Cairo 1.18 or later for fractional glyph positions; loaded {version}"
        );
        let config = FontConfig(
            NonNull::new(unsafe { FcInitLoadConfigAndFonts() })
                .context("Fontconfig could not load the OS font configuration")?,
        );
        let font_map = pangocairo::FontMap::for_font_type(cairo::FontType::FontTypeFt)
            .context("Pango's native FreeType font map is unavailable")?;
        unsafe { pango_fc_font_map_set_config(font_map.to_glib_none().0, config.0.as_ptr()) };
        let context = font_map.create_context();
        context.set_round_glyph_positions(false);
        if let Some(locale) = sys_locale::get_locale() {
            context.set_language(Some(&pango::Language::from_string(&locale)));
        }
        let mut options = cairo::FontOptions::new()?;
        options.set_antialias(cairo::Antialias::Gray);
        options.set_hint_style(cairo::HintStyle::Slight);
        options.set_hint_metrics(cairo::HintMetrics::Off);
        pangocairo::functions::context_set_font_options(&context, Some(&options));
        let mut state = Self {
            fonts: Vec::new(),
            selections: HashMap::default(),
            fallback_ids: HashMap::default(),
            context,
            font_map,
            config,
            font_files: Vec::new(),
        };
        state.resolve(&crate::font(".SystemUIFont"))?;
        state.resolve(&crate::font(".SystemMonospaceFont"))?;
        Ok(state)
    }
    fn font(&self, id: FontId) -> Result<&FontRecord> {
        self.fonts
            .get(id.0)
            .context("Unknown Pango font identifier")
    }
    fn resolve(&mut self, requested: &Font) -> Result<FontId> {
        if let Some(id) = self.selections.get(requested) {
            return Ok(*id);
        }
        ensure!(requested.weight.0.is_finite(), "Font weight must be finite");
        let family = native_family(requested.family.as_ref());
        if !matches!(family, "system-ui" | "monospace") {
            ensure!(
                self.font_map
                    .list_families()
                    .iter()
                    .any(|candidate| candidate.name().eq_ignore_ascii_case(family)),
                "Font family {family:?} is not installed"
            );
        }
        let mut families = vec![family.to_owned()];
        if let Some(fallbacks) = &requested.fallbacks {
            families.extend(
                fallbacks
                    .fallback_list()
                    .iter()
                    .map(|family| native_family(family).to_owned()),
            );
        }
        let mut description = pango::FontDescription::new();
        description.set_family(&families.join(","));
        description.set_absolute_size(BASE_FONT_SIZE * PANGO_SCALE);
        description
            .set_weight(unsafe { from_glib(requested.weight.0.clamp(1.0, 1000.0).round() as i32) });
        description.set_style(match requested.style {
            FontStyle::Normal => pango::Style::Normal,
            FontStyle::Italic => pango::Style::Italic,
            FontStyle::Oblique => pango::Style::Oblique,
        });
        let native = self
            .font_map
            .load_font(&self.context, &description)
            .with_context(|| format!("Pango could not load font {family:?}"))?;
        let id = FontId(self.fonts.len());
        self.fonts.push(FontRecord {
            physical_key: physical_key(&native),
            color: is_color_font(&native),
            native,
            description,
            features: requested.features.clone(),
        });
        self.selections.insert(requested.clone(), id);
        Ok(id)
    }
    fn sized_font(&self, id: FontId, pixels: f64) -> Result<pango::Font> {
        ensure!(
            pixels.is_finite() && pixels > 0.0 && pixels * PANGO_SCALE <= i32::MAX as f64,
            "Invalid Pango font size"
        );
        // Use the realized face, so a fallback glyph cannot be reselected from the original family list.
        let mut description = self.font(id)?.native.describe();
        description.set_absolute_size(pixels * PANGO_SCALE);
        self.font_map
            .load_font(&self.context, &description)
            .context("Pango could not resize the selected font")
    }
    fn fallback_id(&mut self, native: &pango::Font, source: FontId) -> Result<FontId> {
        let physical_key = physical_key(native);
        let record = self.font(source)?;
        if record.physical_key == physical_key {
            return Ok(source);
        }
        let features = record.features.clone();
        let key = (physical_key.clone(), features.clone());
        if let Some(id) = self.fallback_ids.get(&key) {
            return Ok(*id);
        }
        let mut description = native.describe();
        description.set_absolute_size(BASE_FONT_SIZE * PANGO_SCALE);
        let native = self
            .font_map
            .load_font(&self.context, &description)
            .context("Pango fallback font disappeared")?;
        let id = FontId(self.fonts.len());
        self.fonts.push(FontRecord {
            physical_key,
            color: is_color_font(&native),
            native,
            description,
            features,
        });
        self.fallback_ids.insert(key, id);
        Ok(id)
    }
    fn add_fonts(&mut self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        let mut result = Ok(());
        for bytes in fonts {
            let add = (|| -> Result<()> {
                ensure!(!bytes.is_empty(), "Application font data is empty");
                let mut file = tempfile::NamedTempFile::new()
                    .context("Could not create an application-font file")?;
                file.write_all(&bytes)?;
                file.flush()?;
                let path = CString::new(file.path().as_os_str().as_bytes())?;
                ensure!(
                    unsafe { FcConfigAppFontAddFile(self.config.0.as_ptr(), path.as_ptr().cast()) }
                        != 0,
                    "Fontconfig rejected application font data"
                );
                self.font_files.push(file);
                Ok(())
            })();
            if let Err(error) = add {
                result = Err(error);
                break;
            }
        }
        // Registration can partially succeed; invalidate native selection even when a later file fails.
        unsafe { pango_fc_font_map_config_changed(self.font_map.to_glib_none().0) };
        self.context.changed();
        self.selections.clear();
        self.fallback_ids.clear();
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
            "Invalid Pango line font size"
        );
        ensure!(
            text.len() <= i32::MAX as usize,
            "Text exceeds Pango's byte-index limit"
        );
        if text.is_empty() || font_size.0 == 0.0 {
            return Ok(LineLayout {
                font_size,
                len: text.len(),
                ..LineLayout::default()
            });
        }
        let layout = pango::Layout::new(&self.context);
        layout.set_single_paragraph_mode(true);
        layout.set_auto_dir(true);
        layout.set_width(-1);
        layout.set_text(text);
        let attributes = pango::AttrList::new();
        let mut offset = 0_usize;
        let mut ranges = Vec::new();
        for (index, run) in font_runs.iter().enumerate() {
            let end = offset
                .checked_add(run.len)
                .context("Font run length overflow")?;
            ensure!(
                end <= text.len() && text.is_char_boundary(offset) && text.is_char_boundary(end),
                "Font run is outside UTF-8 boundaries"
            );
            if offset == end {
                continue;
            }
            let record = self.font(run.font_id)?;
            let mut description = record.description.clone();
            // Adjacent WGPUI font runs must not form a shared ligature; Pango sizes have 1/1024-pixel precision.
            description
                .set_absolute_size(f64::from(font_size.0) * PANGO_SCALE + (index % 2) as f64);
            insert_attribute(
                &attributes,
                pango::AttrFontDesc::new(&description),
                offset..end,
            )?;
            if !record.features.tag_value_list().is_empty() {
                let features = feature_string(&record.features)?;
                insert_attribute(
                    &attributes,
                    pango::AttrFontFeatures::new(&features),
                    offset..end,
                )?;
            }
            if ranges.is_empty() {
                layout.set_font_description(Some(&description));
            }
            ranges.push((offset..end, run.font_id));
            offset = end;
        }
        ensure!(
            offset == text.len(),
            "Font runs do not cover the complete line"
        );
        layout.set_attributes(Some(&attributes));
        if let Some((_, first)) = ranges.first() {
            let native = self.sized_font(*first, f64::from(font_size.0))?;
            if let Some(glyph) = nominal_glyph(&native, ' ') {
                let (_, advance) = native.glyph_extents(glyph.0);
                let stop = advance
                    .width()
                    .checked_mul(4)
                    .context("Pango tab stop overflow")?;
                if stop > 0 {
                    let mut tabs = pango::TabArray::new(2, false);
                    tabs.set_tab(0, pango::TabAlign::Left, stop);
                    tabs.set_tab(
                        1,
                        pango::TabAlign::Left,
                        stop.checked_mul(2).context("Pango tab stop overflow")?,
                    );
                    layout.set_tabs(Some(&tabs));
                }
            }
        }
        let (_, logical) = layout.extents();
        let baseline = layout.baseline();
        let mut iterator = layout.iter();
        let mut shaped: Vec<ShapedRun> = Vec::new();
        loop {
            if let Some(run) = iterator.run_readonly() {
                let item = run.item();
                let offset = usize::try_from(item.offset())?;
                let source = ranges
                    .iter()
                    .find(|(range, _)| range.contains(&offset))
                    .map(|(_, id)| *id)
                    .context("Pango returned a run outside the input font ranges")?;
                let native = item.analysis().font();
                let id = self.fallback_id(&native, source)?;
                let color = self.font(id)?.color;
                let glyphs = run.glyph_string();
                let (_, run_logical) = iterator.run_extents();
                let run_baseline = iterator.run_baseline();
                let mut advance = i64::from(run_logical.x());
                let mut values = Vec::new();
                ensure!(
                    glyphs.glyph_info().len() == glyphs.log_clusters().len(),
                    "Pango glyph cluster count mismatch"
                );
                for (glyph, cluster) in glyphs.glyph_info().iter().zip(glyphs.log_clusters()) {
                    let cluster = offset
                        .checked_add(usize::try_from(*cluster)?)
                        .context("Pango cluster index overflow")?;
                    ensure!(
                        cluster < text.len() && text.is_char_boundary(cluster),
                        "Pango returned an invalid UTF-8 cluster"
                    );
                    let geometry = glyph.geometry();
                    values.push(ShapedGlyph {
                        id: GlyphId(glyph.glyph()),
                        position: point(
                            px(
                                ((advance + i64::from(geometry.x_offset())) as f64 / PANGO_SCALE)
                                    as f32,
                            ),
                            px(((i64::from(run_baseline) - i64::from(baseline)
                                + i64::from(geometry.y_offset()))
                                as f64
                                / PANGO_SCALE) as f32),
                        ),
                        index: cluster,
                        is_emoji: color,
                    });
                    advance = advance
                        .checked_add(i64::from(geometry.width()))
                        .context("Pango glyph position overflow")?;
                }
                if let Some(previous) = shaped.last_mut().filter(|previous| previous.font_id == id)
                {
                    previous.glyphs.extend(values);
                } else {
                    shaped.push(ShapedRun {
                        font_id: id,
                        glyphs: values,
                    });
                }
            }
            if !iterator.next_run() {
                break;
            }
        }
        Ok(LineLayout {
            font_size,
            width: px(logical.width() as f32 / pango::SCALE as f32),
            ascent: px((i64::from(baseline) - i64::from(logical.y())) as f32 / pango::SCALE as f32),
            descent: px(
                (i64::from(logical.y()) + i64::from(logical.height()) - i64::from(baseline)) as f32
                    / pango::SCALE as f32,
            ),
            runs: shaped,
            len: text.len(),
        })
    }

    fn raster_bounds(&self, parameters: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        let font = self.sized_font(parameters.font_id, raster_size(parameters)?)?;
        let (ink, _) = font.glyph_extents(parameters.glyph_id.0);
        let phase_x = f64::from(parameters.subpixel_variant.x) / f64::from(SUBPIXEL_VARIANTS_X);
        let phase_y =
            f64::from(parameters.subpixel_variant.y) / f64::from(crate::SUBPIXEL_VARIANTS_Y);
        let left = (f64::from(ink.x()) / PANGO_SCALE + phase_x).floor() - 1.0;
        let top = (f64::from(ink.y()) / PANGO_SCALE + phase_y).floor() - 1.0;
        let right = ((i64::from(ink.x()) + i64::from(ink.width())) as f64 / PANGO_SCALE + phase_x)
            .ceil()
            + 1.0;
        let bottom =
            ((i64::from(ink.y()) + i64::from(ink.height())) as f64 / PANGO_SCALE + phase_y).ceil()
                + 1.0;
        ensure!(
            [left, top, right, bottom]
                .into_iter()
                .all(|value| value.is_finite()
                    && value >= i32::MIN as f64
                    && value <= i32::MAX as f64),
            "Pango glyph raster bounds exceed device coordinates"
        );
        Ok(Bounds::new(
            point(DevicePixels(left as i32), DevicePixels(top as i32)),
            size(
                DevicePixels(
                    (right as i32)
                        .checked_sub(left as i32)
                        .context("Glyph raster width overflow")?,
                ),
                DevicePixels(
                    (bottom as i32)
                        .checked_sub(top as i32)
                        .context("Glyph raster height overflow")?,
                ),
            ),
        ))
    }
    fn rasterize(
        &self,
        parameters: &RenderGlyphParams,
        bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        let font = self.sized_font(parameters.font_id, raster_size(parameters)?)?;
        let width = bounds.size.width.0;
        let height = bounds.size.height.0;
        ensure!(width > 0 && height > 0, "Glyph raster bounds are empty");
        let mut surface = cairo::ImageSurface::create(
            if parameters.is_emoji {
                cairo::Format::ARgb32
            } else {
                cairo::Format::A8
            },
            width,
            height,
        )?;
        {
            let context = cairo::Context::new(&surface)?;
            context.set_source_rgba(0.0, 0.0, 0.0, 1.0);
            context.translate(
                -f64::from(bounds.origin.x.0)
                    + f64::from(parameters.subpixel_variant.x) / f64::from(SUBPIXEL_VARIANTS_X),
                -f64::from(bounds.origin.y.0)
                    + f64::from(parameters.subpixel_variant.y)
                        / f64::from(crate::SUBPIXEL_VARIANTS_Y),
            );
            let mut glyphs = pango::GlyphString::new();
            glyphs.set_size(1);
            let glyph = glyphs
                .glyph_info_mut()
                .first_mut()
                .context("Pango did not allocate a glyph")?;
            glyph.set_glyph(parameters.glyph_id.0);
            glyph.geometry_mut().set_width(0);
            glyph.geometry_mut().set_x_offset(0);
            glyph.geometry_mut().set_y_offset(0);
            pangocairo::functions::show_glyph_string(&context, &font, &mut glyphs);
            context.status()?;
        }
        surface.flush();
        let channels = if parameters.is_emoji { 4 } else { 1 };
        let row_bytes = usize::try_from(width)?
            .checked_mul(channels)
            .context("Glyph raster stride overflow")?;
        let length = row_bytes
            .checked_mul(usize::try_from(height)?)
            .context("Glyph raster allocation overflow")?;
        let stride = usize::try_from(surface.stride())?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length)?;
        let data = surface.data()?;
        for row in data.chunks_exact(stride).take(usize::try_from(height)?) {
            bytes.extend_from_slice(
                row.get(..row_bytes)
                    .context("Cairo glyph stride is shorter than its pixels")?,
            );
        }
        ensure!(bytes.len() == length, "Cairo glyph surface is truncated");
        if parameters.is_emoji {
            for pixel in bytes.chunks_exact_mut(4) {
                let native = u32::from_ne_bytes(pixel.try_into()?);
                pixel.copy_from_slice(&[
                    (native >> 16) as u8,
                    (native >> 8) as u8,
                    native as u8,
                    (native >> 24) as u8,
                ]);
            }
            swap_rgba_pa_to_bgra(&mut bytes);
        }
        Ok((bounds.size, bytes))
    }
}

fn native_family(family: &str) -> &str {
    match family {
        ".SystemUIFont" | ".ZedSans" | "Zed Plex Sans" => "system-ui",
        ".SystemMonospaceFont" | ".ZedMono" | "Zed Plex Mono" => "monospace",
        family => family,
    }
}
fn physical_key(font: &pango::Font) -> String {
    let mut description = font.describe();
    description.unset_fields(pango::FontMask::SIZE);
    description.to_string()
}
fn is_color_font(font: &pango::Font) -> bool {
    let pattern = unsafe { pango_fc_font_get_pattern(font.to_glib_none().0) };
    let mut value = 0;
    !pattern.is_null()
        && unsafe { FcPatternGetBool(pattern, c"color".as_ptr(), 0, &mut value) } == 0
        && value != 0
}
fn native_hb_font(font: &pango::Font) -> Result<NonNull<c_void>> {
    NonNull::new(unsafe { pango_font_get_hb_font(font.to_glib_none().0) })
        .context("Pango font has no native shaping font")
}
fn nominal_glyph(font: &pango::Font, character: char) -> Option<GlyphId> {
    let native = native_hb_font(font).ok()?;
    let mut glyph = 0;
    (unsafe { hb_font_get_nominal_glyph(native.as_ptr(), u32::from(character), &mut glyph) } != 0)
        .then_some(GlyphId(glyph))
}
fn font_units(font: &pango::Font) -> Result<(u32, f32)> {
    let native = native_hb_font(font)?;
    let face = NonNull::new(unsafe { hb_font_get_face(native.as_ptr()) })
        .context("Pango font has no native font face")?;
    let units = unsafe { hb_face_get_upem(face.as_ptr()) };
    let mut x_scale = 0;
    let mut y_scale = 0;
    unsafe { hb_font_get_scale(native.as_ptr(), &mut x_scale, &mut y_scale) };
    ensure!(
        units > 0 && x_scale > 0 && y_scale > 0 && x_scale == y_scale,
        "Pango font has invalid native units"
    );
    Ok((units, units as f32 / x_scale as f32))
}
fn native_font_bounds(font: &pango::Font, scale: f32) -> Result<Bounds<f32>> {
    let native = native_hb_font(font)?;
    let face = NonNull::new(unsafe { hb_font_get_face(native.as_ptr()) })
        .context("Pango font has no native face")?;
    if let Some(blob) = NonNull::new(unsafe {
        hb_face_reference_table(face.as_ptr(), u32::from_be_bytes(*b"head"))
    }) {
        let blob = FontTable(blob);
        let data = blob.bytes();
        if data.len() >= 44 {
            let read = |offset| -> Result<f32> {
                let bytes = data
                    .get(offset..offset + 2)
                    .context("Native font head table is truncated")?;
                Ok(i16::from_be_bytes(bytes.try_into()?) as f32)
            };
            let left = read(36)?;
            let bottom = read(38)?;
            let right = read(40)?;
            let top = read(42)?;
            ensure!(
                right >= left && top >= bottom,
                "Native font head table has inverted bounds"
            );
            return Ok(Bounds::new(
                point(left, bottom),
                size(right - left, top - bottom),
            ));
        }
    }
    // Bitmap fonts need not have an OpenType head table; use the native face's complete glyph extents.
    let mut limits: Option<(i32, i32, i32, i32)> = None;
    for glyph in 0..unsafe { hb_face_get_glyph_count(face.as_ptr()) } {
        let mut extents = NativeExtents::default();
        if unsafe { hb_font_get_glyph_extents(native.as_ptr(), glyph, &mut extents) } != 0 {
            let right = extents
                .x_bearing
                .checked_add(extents.width)
                .context("Native glyph bounds overflow")?;
            let bottom = extents
                .y_bearing
                .checked_add(extents.height)
                .context("Native glyph bounds overflow")?;
            let next = (
                extents.x_bearing.min(right),
                bottom.min(extents.y_bearing),
                extents.x_bearing.max(right),
                bottom.max(extents.y_bearing),
            );
            limits = Some(match limits {
                None => next,
                Some(current) => (
                    current.0.min(next.0),
                    current.1.min(next.1),
                    current.2.max(next.2),
                    current.3.max(next.3),
                ),
            });
        }
    }
    let (left, bottom, right, top) = limits.context("Native font has no measurable glyphs")?;
    Ok(Bounds::new(
        point(left as f32 * scale, bottom as f32 * scale),
        size(
            (i64::from(right) - i64::from(left)) as f32 * scale,
            (i64::from(top) - i64::from(bottom)) as f32 * scale,
        ),
    ))
}
fn native_metrics(font: &pango::Font) -> Result<FontMetrics> {
    let (units_per_em, scale) = font_units(font)?;
    let metrics = font.metrics(None);
    let height = |character| -> f32 {
        nominal_glyph(font, character)
            .map(|glyph| font.glyph_extents(glyph.0).0)
            .map_or(0.0, |ink| -(ink.y() as f32) * scale)
    };
    Ok(FontMetrics {
        units_per_em,
        ascent: metrics.ascent() as f32 * scale,
        descent: -(metrics.descent() as f32) * scale,
        line_gap: (i64::from(metrics.height())
            - i64::from(metrics.ascent())
            - i64::from(metrics.descent())) as f32
            * scale,
        underline_position: metrics.underline_position() as f32 * scale,
        underline_thickness: metrics.underline_thickness() as f32 * scale,
        cap_height: height('H'),
        x_height: height('x'),
        bounding_box: native_font_bounds(font, scale)?,
    })
}
fn empty_metrics() -> FontMetrics {
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
fn feature_string(features: &FontFeatures) -> Result<String> {
    let mut values = Vec::new();
    for (tag, value) in features.tag_value_list() {
        ensure!(
            tag.len() == 4 && tag.is_ascii(),
            "OpenType tags must contain four ASCII bytes"
        );
        values.push(format!("{tag}={value}"));
    }
    Ok(values.join(","))
}
fn insert_attribute(
    attributes: &pango::AttrList,
    attribute: impl Into<pango::Attribute>,
    range: Range<usize>,
) -> Result<()> {
    let mut attribute = attribute.into();
    attribute.set_start_index(u32::try_from(range.start)?);
    attribute.set_end_index(u32::try_from(range.end)?);
    attributes.insert(attribute);
    Ok(())
}
fn raster_size(parameters: &RenderGlyphParams) -> Result<f64> {
    ensure!(
        parameters.font_size.0.is_finite()
            && parameters.font_size.0 > 0.0
            && parameters.scale_factor.is_finite()
            && parameters.scale_factor > 0.0,
        "Invalid Pango glyph raster size"
    );
    Ok(f64::from(parameters.font_size.0) * f64::from(parameters.scale_factor))
}

impl PlatformTextSystem for PangoTextSystem {
    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        self.execute(move |state| state.add_fonts(fonts))
    }
    fn all_font_names(&self) -> Vec<String> {
        self.execute(|state| {
            let mut names = state
                .font_map
                .list_families()
                .iter()
                .map(|family| family.name().to_string())
                .collect::<Vec<_>>();
            names.extend([
                ".SystemUIFont".to_owned(),
                ".SystemMonospaceFont".to_owned(),
            ]);
            names.sort();
            names.dedup();
            Ok(names)
        })
        .unwrap_or_else(|error| {
            log::error!("Pango font enumeration failed: {error:#}");
            Vec::new()
        })
    }
    fn font_id(&self, requested: &Font) -> Result<FontId> {
        let requested = requested.clone();
        self.execute(move |state| state.resolve(&requested))
    }
    fn font_metrics(&self, id: FontId) -> FontMetrics {
        self.execute(move |state| native_metrics(&state.font(id)?.native))
            .unwrap_or_else(|error| {
                log::error!("Pango font metrics failed: {error:#}");
                empty_metrics()
            })
    }
    fn typographic_bounds(&self, id: FontId, glyph: GlyphId) -> Result<Bounds<f32>> {
        self.execute(move |state| {
            let font = &state.font(id)?.native;
            let (_, scale) = font_units(font)?;
            let (ink, _) = font.glyph_extents(glyph.0);
            Ok(Bounds::new(
                point(
                    ink.x() as f32 * scale,
                    -((i64::from(ink.y()) + i64::from(ink.height())) as f32) * scale,
                ),
                size(ink.width() as f32 * scale, ink.height() as f32 * scale),
            ))
        })
    }
    fn advance(&self, id: FontId, glyph: GlyphId) -> Result<Size<f32>> {
        self.execute(move |state| {
            let font = &state.font(id)?.native;
            let (_, scale) = font_units(font)?;
            Ok(size(
                font.glyph_extents(glyph.0).1.width() as f32 * scale,
                0.0,
            ))
        })
    }
    fn glyph_for_char(&self, id: FontId, character: char) -> Option<GlyphId> {
        self.execute(move |state| Ok(nominal_glyph(&state.font(id)?.native, character)))
            .unwrap_or_else(|error| {
                log::error!("Pango glyph lookup failed: {error:#}");
                None
            })
    }
    fn glyph_raster_bounds(&self, parameters: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        let parameters = parameters.clone();
        self.execute(move |state| state.raster_bounds(&parameters))
    }
    fn rasterize_glyph(
        &self,
        parameters: &RenderGlyphParams,
        bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        let parameters = parameters.clone();
        self.execute(move |state| state.rasterize(&parameters, bounds))
    }
    fn layout_line(&self, text: &str, font_size: Pixels, runs: &[FontRun]) -> LineLayout {
        let text = text.to_owned();
        let runs = runs.to_vec();
        let length = text.len();
        self.execute(move |state| state.shape(&text, font_size, &runs))
            .unwrap_or_else(|error| {
                log::error!("Pango text shaping failed: {error:#}");
                LineLayout {
                    font_size,
                    len: length,
                    ..LineLayout::default()
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FontWeight, font};
    use std::sync::Arc;

    #[test]
    fn native_defaults_match_fontconfig_and_monospace_advances() -> Result<()> {
        let system = PangoTextSystem::new()?;
        let ui = system.font_id(&font(".SystemUIFont"))?;
        let mono = system.font_id(&font(".SystemMonospaceFont"))?;
        let database = super::super::system_fonts::database();
        for (id, role) in [
            (ui, usvg::fontdb::Family::SansSerif),
            (mono, usvg::fontdb::Family::Monospace),
        ] {
            let expected = database
                .query(&usvg::fontdb::Query {
                    families: &[role],
                    ..Default::default()
                })
                .and_then(|id| database.face(id))
                .context("OS default font is unavailable")?;
            let actual = system
                .execute(move |state| {
                    Ok(state
                        .font(id)?
                        .native
                        .describe()
                        .family()
                        .map(|family| family.to_string()))
                })?
                .context("Pango selected a font without a family")?;
            assert!(
                expected
                    .families
                    .iter()
                    .any(|(family, _)| family.eq_ignore_ascii_case(&actual)),
                "Pango family {actual:?} differs from Fontconfig OS default {:?}",
                expected.families
            );
            assert!(system.font_metrics(id).units_per_em > 0);
        }
        let narrow = system
            .glyph_for_char(mono, 'i')
            .context("OS monospace font has no i")?;
        let wide = system
            .glyph_for_char(mono, 'W')
            .context("OS monospace font has no W")?;
        assert!(
            (system.advance(mono, narrow)?.width - system.advance(mono, wide)?.width).abs() < 0.01
        );
        Ok(())
    }

    #[test]
    fn native_shaping_preserves_utf8_bom_fallback_and_rtl() -> Result<()> {
        let system = PangoTextSystem::new()?;
        let id = system.font_id(&font(".SystemUIFont"))?;
        let text = "\u{feff}A😀e\u{301}العربية漢字";
        let line = system.execute(move |state| {
            state.shape(
                text,
                px(18.0),
                &[FontRun {
                    len: text.len(),
                    font_id: id,
                }],
            )
        })?;
        assert!(line.width.0 > 0.0);
        assert_eq!(line.len, text.len());
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
            "Native acceptance requires an installed color emoji font"
        );
        let text = "مرحبا";
        let line = system.execute(move |state| {
            state.shape(
                text,
                px(18.0),
                &[FontRun {
                    len: text.len(),
                    font_id: id,
                }],
            )
        })?;
        let glyphs = line
            .runs
            .iter()
            .flat_map(|run| &run.glyphs)
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
        assert!(
            glyphs
                .windows(2)
                .all(|glyphs| glyphs[0].position.x <= glyphs[1].position.x)
        );
        Ok(())
    }

    #[test]
    fn native_rasterization_preserves_weight_scale_phase_and_color() -> Result<()> {
        let system = PangoTextSystem::new()?;
        let mut rasters = Vec::new();
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
            assert_ne!(phases.get(0), phases.get(1));
            rasters.push(phases);
        }
        assert_ne!(rasters.get(0), rasters.get(1));
        let id = system.font_id(&font(".SystemUIFont"))?;
        let text = "😀";
        let line = system.execute(move |state| {
            state.shape(
                text,
                px(24.0),
                &[FontRun {
                    len: text.len(),
                    font_id: id,
                }],
            )
        })?;
        let (font_id, glyph) = line
            .runs
            .iter()
            .find_map(|run| {
                run.glyphs
                    .iter()
                    .find(|glyph| glyph.is_emoji)
                    .map(|glyph| (run.font_id, glyph))
            })
            .context("Native color emoji fallback is missing")?;
        let parameters = RenderGlyphParams {
            font_id,
            glyph_id: glyph.id,
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
    fn native_text_worker_serializes_two_calling_threads() -> Result<()> {
        let system = Arc::new(PangoTextSystem::new()?);
        let mut callers = Vec::new();
        for _ in 0..2 {
            let system = system.clone();
            callers.push(thread::spawn(move || -> Result<(FontId, Pixels)> {
                let id = system.font_id(&font(".SystemUIFont"))?;
                let text = "Concurrent العربية";
                let line = system.execute(move |state| {
                    state.shape(
                        text,
                        px(18.0),
                        &[FontRun {
                            len: text.len(),
                            font_id: id,
                        }],
                    )
                })?;
                Ok((id, line.width))
            }));
        }
        let mut results = Vec::new();
        for caller in callers {
            results.push(
                caller
                    .join()
                    .map_err(|_| anyhow!("Pango caller panicked"))??,
            );
        }
        assert_eq!(results.get(0), results.get(1));
        Ok(())
    }

    #[test]
    fn native_tabs_and_font_boundaries_preserve_editor_positions() -> Result<()> {
        let system = PangoTextSystem::new()?;
        let id = system.font_id(&font(".SystemMonospaceFont"))?;
        let text = "A\tB\tC\tD";
        let line = system.execute(move |state| {
            state.shape(
                text,
                px(18.0),
                &[FontRun {
                    len: text.len(),
                    font_id: id,
                }],
            )
        })?;
        let space = system
            .glyph_for_char(id, ' ')
            .context("OS monospace font has no space")?;
        let metrics = system.font_metrics(id);
        let space = system.advance(id, space)?.width / metrics.units_per_em as f32 * 18.0;
        for (character, cell) in [(2, 4.0), (4, 8.0), (6, 12.0)] {
            let glyph = line
                .runs
                .iter()
                .flat_map(|run| &run.glyphs)
                .find(|glyph| glyph.index == character)
                .context("Tab following glyph is missing")?;
            assert!((glyph.position.x.0 - space * cell).abs() < 0.02);
        }
        let text = "ffi";
        let line = system.execute(move |state| {
            state.shape(
                text,
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
            )
        })?;
        assert!(
            line.runs
                .iter()
                .flat_map(|run| &run.glyphs)
                .any(|glyph| glyph.index == 1),
            "A ligature crossed an explicit font boundary"
        );
        Ok(())
    }

    #[test]
    fn native_application_font_registration_preserves_existing_ids() -> Result<()> {
        let system = PangoTextSystem::new()?;
        let id = system.font_id(&font(".SystemMonospaceFont"))?;
        let bytes = system.execute(move |state| {
            unsafe extern "C" {
                fn FcPatternGetString(
                    pattern: *mut c_void,
                    name: *const c_char,
                    index: c_int,
                    value: *mut *mut u8,
                ) -> c_int;
            }
            let pattern =
                unsafe { pango_fc_font_get_pattern(state.font(id)?.native.to_glib_none().0) };
            ensure!(!pattern.is_null(), "Native font has no Fontconfig pattern");
            let mut filename = std::ptr::null_mut();
            ensure!(
                unsafe { FcPatternGetString(pattern, c"file".as_ptr(), 0, &mut filename) } == 0
                    && !filename.is_null(),
                "Native test font has no file"
            );
            let filename = unsafe { std::ffi::CStr::from_ptr(filename.cast()) };
            let path = std::path::Path::new(std::ffi::OsStr::from_bytes(filename.to_bytes()));
            std::fs::read(path)
                .context("Could not read installed OS font for application-font registration")
        })?;
        let glyph = system
            .glyph_for_char(id, 'M')
            .context("Native test font has no M")?;
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
}
