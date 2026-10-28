use usvg::fontdb::{Database, Family, Query};

pub(crate) fn database() -> Database {
    let mut database = Database::new();
    database.load_system_fonts();
    #[cfg(target_os = "macos")]
    macos::configure(&mut database);
    #[cfg(target_os = "windows")]
    windows::configure(&mut database);
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    unix::configure(&mut database);
    ensure_available_family(&mut database, Family::SansSerif, false);
    ensure_available_family(&mut database, Family::Monospace, true);
    database
}

fn set_family(database: &mut Database, generic: Family<'_>, requested: &str) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let family = database
        .faces()
        .flat_map(|face| face.families.iter())
        .find(|(family, _)| family.eq_ignore_ascii_case(requested))
        .map(|(family, _)| family.clone())
        .with_context(|| {
            format!("OS font family {requested:?} is absent from the font database")
        })?;
    match generic {
        Family::SansSerif => database.set_sans_serif_family(family),
        Family::Monospace => database.set_monospace_family(family),
        _ => anyhow::bail!("Unsupported system font role"),
    }
    Ok(())
}

fn ensure_available_family(database: &mut Database, generic: Family<'_>, monospace: bool) {
    if database
        .query(&Query {
            families: &[generic],
            ..Query::default()
        })
        .and_then(|id| database.face(id))
        .is_some_and(|face| !monospace || face.monospaced)
    {
        return;
    }
    let family = database
        .faces()
        .find(|face| face.monospaced == monospace)
        .and_then(|face| face.families.first())
        .map(|(family, _)| family.clone());
    if let Some(family) = family {
        log::warn!(
            "The OS default {generic:?} font is unavailable; using installed family {family:?}"
        );
        if let Err(error) = set_family(database, generic, &family) {
            log::warn!("Could not select the installed font fallback: {error:#}");
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use crate::platform::macos_text_system::CoreTextSystem;
    use anyhow::{Context as _, Result};
    use core_text::font::CTFont;

    fn native_family(database: &mut Database, font: &CTFont) -> Result<String> {
        use usvg::fontdb::Source;

        // Logical system roles may have no URL; their Core Graphics backing face supplies the physical identity.
        let backing = core_text::font::new_from_CGFont(&font.copy_to_CGFont(), font.pt_size());
        let postscript = backing.postscript_name();
        let family = Some(backing.family_name());
        let named_family = database
            .faces()
            .find(|face| face.post_script_name == postscript)
            .and_then(|face| face.families.first())
            .map(|(family, _)| family.clone())
            .or_else(|| {
                family.filter(|family| {
                    database
                        .faces()
                        .any(|face| face.families.iter().any(|(name, _)| name == family))
                })
            });
        if let Some(family) = named_family {
            return Ok(family);
        }

        // Some native system names are absent from font-file names; follow the OS-selected backing file.
        let file = backing
            .url()
            .and_then(|url| url.to_path())
            .with_context(|| {
                format!("The native system font {postscript:?} has no backing file")
            })?;
        let matches_file = |face: &usvg::fontdb::FaceInfo| match &face.source {
            Source::File(path) | Source::SharedFile(path, _) => path == &file,
            Source::Binary(_) => false,
        };
        if !database.faces().any(matches_file) {
            database.load_font_file(&file).with_context(|| {
                format!("Could not load the OS-selected system font {postscript:?}")
            })?;
        }
        database
            .faces()
            .find(|face| matches_file(face))
            .and_then(|face| face.families.first())
            .map(|(family, _)| family.clone())
            .with_context(|| format!("The native system font {postscript:?} has no usable family"))
    }

    pub(super) fn families(database: &mut Database) -> (Result<String>, Result<String>) {
        (
            CoreTextSystem::system_font(false).and_then(|font| native_family(database, &font)),
            CoreTextSystem::system_font(true).and_then(|font| native_family(database, &font)),
        )
    }

    pub(super) fn configure(database: &mut Database) {
        let (ui, mono) = families(database);
        for (role, family) in [(Family::SansSerif, ui), (Family::Monospace, mono)] {
            if let Err(error) = family.and_then(|family| set_family(database, role, &family)) {
                log::warn!("Could not select the native {role:?} font: {error:#}");
            }
        }
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::*;
    use ::windows::Win32::{
        Graphics::{
            DirectWrite::{DWRITE_FACTORY_TYPE_SHARED, DWriteCreateFactory, IDWriteFactory},
            Gdi::{ANSI_FIXED_FONT, GetObjectW, GetStockObject, LOGFONTW},
        },
        UI::WindowsAndMessaging::{
            NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
            SystemParametersInfoW,
        },
    };
    use anyhow::{Context as _, Result, ensure};

    fn family(logical_font: &LOGFONTW) -> Result<String> {
        let factory: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }?;
        let interop = unsafe { factory.GetGdiInterop() }?;
        let font = unsafe { interop.CreateFontFromLOGFONT(logical_font) }?;
        let family = unsafe { font.GetFontFamily() }?;
        let names = unsafe { family.GetFamilyNames() }?;
        ensure!(
            unsafe { names.GetCount() } > 0,
            "The native font has no family name"
        );
        let length = usize::try_from(unsafe { names.GetStringLength(0) }?)?;
        let capacity = length
            .checked_add(1)
            .context("Native font family length overflow")?;
        let mut name = vec![0_u16; capacity];
        unsafe { names.GetString(0, &mut name) }?;
        Ok(String::from_utf16(
            name.get(..length)
                .context("Native font family exceeds its buffer")?,
        )?)
    }

    fn ui_family() -> Result<String> {
        let mut metrics = NONCLIENTMETRICSW {
            cbSize: u32::try_from(std::mem::size_of::<NONCLIENTMETRICSW>())?,
            ..Default::default()
        };
        unsafe {
            SystemParametersInfoW(
                SPI_GETNONCLIENTMETRICS,
                metrics.cbSize,
                Some((&raw mut metrics).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS::default(),
            )
        }?;
        family(&metrics.lfMessageFont)
    }

    fn mono_family() -> Result<String> {
        let stock = unsafe { GetStockObject(ANSI_FIXED_FONT) };
        ensure!(
            !stock.0.is_null(),
            "Could not obtain the OS fixed-pitch font"
        );
        let mut font = LOGFONTW::default();
        let size = i32::try_from(std::mem::size_of::<LOGFONTW>())?;
        let copied = unsafe { GetObjectW(stock, size, Some((&raw mut font).cast())) };
        ensure!(copied == size, "Could not read the OS fixed-pitch font");
        family(&font)
    }

    pub(super) fn configure(database: &mut Database) {
        for (role, family) in [
            (Family::SansSerif, ui_family()),
            (Family::Monospace, mono_family()),
        ] {
            if let Err(error) = family.and_then(|family| set_family(database, role, &family)) {
                log::warn!("Could not select the native {role:?} font: {error:#}");
            }
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
mod unix {
    use super::*;
    use anyhow::{Context as _, Result, ensure};
    use std::{
        ffi::{CStr, OsString, c_char, c_int, c_void},
        os::unix::ffi::OsStringExt as _,
        path::PathBuf,
        ptr::NonNull,
    };

    #[link(name = "fontconfig")]
    unsafe extern "C" {
        fn FcInit() -> c_int;
        fn FcNameParse(name: *const u8) -> *mut c_void;
        fn FcConfigSubstitute(config: *mut c_void, pattern: *mut c_void, kind: c_int) -> c_int;
        fn FcDefaultSubstitute(pattern: *mut c_void);
        fn FcFontMatch(
            config: *mut c_void,
            pattern: *mut c_void,
            result: *mut c_int,
        ) -> *mut c_void;
        fn FcPatternGetString(
            pattern: *mut c_void,
            object: *const c_char,
            index: c_int,
            value: *mut *mut u8,
        ) -> c_int;
        fn FcPatternDestroy(pattern: *mut c_void);
    }

    struct Pattern(NonNull<c_void>);
    impl Drop for Pattern {
        fn drop(&mut self) {
            unsafe { FcPatternDestroy(self.0.as_ptr()) };
        }
    }

    fn property(pattern: &Pattern, name: &CStr) -> Result<Vec<u8>> {
        let mut value = std::ptr::null_mut();
        let result =
            unsafe { FcPatternGetString(pattern.0.as_ptr(), name.as_ptr(), 0, &mut value) };
        ensure!(
            result == 0 && !value.is_null(),
            "Fontconfig did not return {name:?}"
        );
        // The matched pattern owns this string until its guard is dropped.
        Ok(unsafe { CStr::from_ptr(value.cast()) }.to_bytes().to_vec())
    }

    fn matched_family(requested: &CStr) -> Result<(String, PathBuf)> {
        ensure!(unsafe { FcInit() } != 0, "Fontconfig initialization failed");
        let pattern = Pattern(
            NonNull::new(unsafe { FcNameParse(requested.as_ptr().cast()) })
                .context("Fontconfig could not construct the system font query")?,
        );
        ensure!(
            unsafe { FcConfigSubstitute(std::ptr::null_mut(), pattern.0.as_ptr(), 0) } != 0,
            "Fontconfig could not apply the system configuration"
        );
        unsafe { FcDefaultSubstitute(pattern.0.as_ptr()) };
        let mut status = 0;
        let matched = Pattern(
            NonNull::new(unsafe {
                FcFontMatch(std::ptr::null_mut(), pattern.0.as_ptr(), &mut status)
            })
            .context("Fontconfig did not find a system font")?,
        );
        ensure!(
            status == 0,
            "Fontconfig matching failed with status {status}"
        );
        let family = String::from_utf8(property(&matched, c"family")?)?;
        let file = PathBuf::from(OsString::from_vec(property(&matched, c"file")?));
        Ok((family, file))
    }

    pub(super) fn configure(database: &mut Database) {
        for (role, name) in [
            (Family::SansSerif, c"system-ui"),
            (Family::Monospace, c"monospace"),
        ] {
            let result = (|| -> Result<()> {
                let (family, file) = matched_family(name)?;
                if !database
                    .faces()
                    .any(|face| face.families.iter().any(|(name, _)| name == &family))
                {
                    database.load_font_file(file)?;
                }
                set_family(database, role, &family)
            })();
            if let Err(error) = result {
                log::warn!("Could not select the native {role:?} font: {error:#}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_default_families_resolve_without_bundled_fonts() -> anyhow::Result<()> {
        use anyhow::Context as _;
        let database = database();
        let ui = database
            .query(&Query {
                families: &[Family::SansSerif],
                ..Query::default()
            })
            .context("The OS must provide a UI font")?;
        let mono = database
            .query(&Query {
                families: &[Family::Monospace],
                ..Query::default()
            })
            .context("The OS must provide a monospace font")?;
        assert!(database.face(ui).is_some());
        assert!(
            database
                .face(mono)
                .context("Missing monospace face")?
                .monospaced
        );
        #[cfg(target_os = "macos")]
        {
            let mut database = database;
            let (ui, mono) = macos::families(&mut database);
            assert_eq!(database.family_name(&Family::SansSerif), ui?);
            assert_eq!(database.family_name(&Family::Monospace), mono?);
        }
        Ok(())
    }
}
