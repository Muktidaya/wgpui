// SPDX-License-Identifier: Apache-2.0
// Copyright 2022 - 2025 Zed Industries, Inc.
// Adapted from the gpui-pre-windows 0.3.10 clipboard contract.

use crate::{ClipboardEntry, ClipboardItem, ClipboardString, ExternalPaths, Image, ImageFormat};
use anyhow::{Context as _, Result, anyhow, ensure};
use std::{
    cell::RefCell,
    ffi::{OsString, c_void},
    io::Cursor,
    os::windows::ffi::OsStringExt as _,
    path::PathBuf,
};
use windows::{
    Win32::{
        Foundation::{ERROR_SUCCESS, GetLastError, HANDLE, HGLOBAL, HWND, SetLastError},
        System::{
            DataExchange::{
                CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData,
                OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
            },
            Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize},
            Ole::{CF_DIB, CF_HDROP, CF_UNICODETEXT},
        },
        UI::{
            Shell::{DragQueryFileW, HDROP},
            WindowsAndMessaging::{
                CreateWindowExW, DestroyWindow, HWND_MESSAGE, WINDOW_EX_STYLE, WINDOW_STYLE,
            },
        },
    },
    core::{PCWSTR, w},
};

// Zero can indicate success for these APIs; their generated Result projections treat it as failure.
#[link(name = "kernel32")]
unsafe extern "system" {
    #[link_name = "GlobalUnlock"]
    fn global_unlock_raw(memory: *mut c_void) -> i32;
    #[link_name = "GlobalFree"]
    fn global_free_raw(memory: *mut c_void) -> *mut c_void;
}

struct OwnerWindow(HWND);

impl Drop for OwnerWindow {
    fn drop(&mut self) {
        if let Err(error) = unsafe { DestroyWindow(self.0) } {
            log::warn!("Clipboard owner window destruction failed: {error}");
        }
    }
}

thread_local! {
    static OWNER: RefCell<Option<OwnerWindow>> = const { RefCell::new(None) };
}

fn owner_window() -> Result<HWND> {
    OWNER.with(|owner| {
        let mut owner = owner.borrow_mut();
        if let Some(window) = owner.as_ref() {
            return Ok(window.0);
        }
        // A non-null owner is required after EmptyClipboard. Every format is rendered eagerly.
        let window = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("WGPUI clipboard owner"),
                WINDOW_STYLE::default(),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            )
        }?;
        *owner = Some(OwnerWindow(window));
        Ok(window)
    })
}

struct ClipboardGuard(bool);

impl ClipboardGuard {
    fn open(owner: Option<HWND>) -> Result<Self> {
        unsafe { OpenClipboard(owner) }.context("Could not open the Windows clipboard")?;
        Ok(Self(true))
    }

    fn close(mut self) -> Result<()> {
        self.0 = false;
        unsafe { CloseClipboard() }.context("Could not close the Windows clipboard")
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        if self.0 {
            if let Err(error) = unsafe { CloseClipboard() } {
                log::warn!("Clipboard close during cleanup failed: {error}");
            }
        }
    }
}

fn with_clipboard<T>(owner: Option<HWND>, operation: impl FnOnce() -> Result<T>) -> Result<T> {
    let clipboard = ClipboardGuard::open(owner)?;
    let result = operation();
    match (result, clipboard.close()) {
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(error)) => Err(error),
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(close_error)) => {
            log::warn!("Clipboard cleanup also failed: {close_error:#}");
            Err(error)
        }
    }
}

struct Formats {
    hash: u32,
    metadata: u32,
    png: u32,
    gif: u32,
    jpeg: u32,
    svg: u32,
}

impl Formats {
    fn new() -> Result<Self> {
        fn register(name: PCWSTR) -> Result<u32> {
            let format = unsafe { RegisterClipboardFormatW(name) };
            ensure!(
                format != 0,
                "Clipboard format registration failed: {}",
                std::io::Error::last_os_error()
            );
            Ok(format)
        }
        Ok(Self {
            hash: register(w!("GPUI internal text hash"))?,
            metadata: register(w!("GPUI internal metadata"))?,
            png: register(w!("PNG"))?,
            gif: register(w!("GIF"))?,
            jpeg: register(w!("JFIF"))?,
            svg: register(w!("image/svg+xml"))?,
        })
    }

    fn image_format(&self, native: u32) -> Option<ImageFormat> {
        if native == self.png {
            Some(ImageFormat::Png)
        } else if native == self.gif {
            Some(ImageFormat::Gif)
        } else if native == self.jpeg {
            Some(ImageFormat::Jpeg)
        } else if native == self.svg {
            Some(ImageFormat::Svg)
        } else {
            None
        }
    }
}

pub(super) fn write(item: ClipboardItem) -> Result<()> {
    let formats = Formats::new()?;
    with_clipboard(Some(owner_window()?), || {
        unsafe { EmptyClipboard() }.context("Could not clear the Windows clipboard")?;
        for entry in item.entries {
            match entry {
                ClipboardEntry::String(text) => write_text(&text, &formats)?,
                ClipboardEntry::Image(image) => write_image(&image, &formats)?,
                ClipboardEntry::ExternalPaths(_) => {
                    log::debug!(
                        "The standard Windows clipboard contract does not write file lists"
                    );
                }
            }
        }
        Ok(())
    })
}

pub(super) fn read() -> Result<Option<ClipboardItem>> {
    let formats = Formats::new()?;
    with_clipboard(None, || {
        let mut available = Vec::new();
        let mut previous = 0;
        loop {
            unsafe { SetLastError(ERROR_SUCCESS) };
            let current = unsafe { EnumClipboardFormats(previous) };
            if current == 0 {
                let error = unsafe { GetLastError() };
                ensure!(
                    error == ERROR_SUCCESS,
                    "Clipboard format enumeration failed with Win32 error {}",
                    error.0
                );
                break;
            }
            available.push(current);
            previous = current;
        }
        let mut entries = Vec::new();
        let mut have_text = false;
        let mut have_image = false;
        let mut have_files = false;
        let mut first_error = None;
        for native in available.iter().copied() {
            let result = if native == CF_UNICODETEXT.0 as u32 && !have_text {
                Some(read_text(&formats, &available).map(|entry| (entry, 0)))
            } else if !have_image
                && (native == CF_DIB.0 as u32 || formats.image_format(native).is_some())
            {
                Some(read_image(native, &formats).map(|entry| (entry, 1)))
            } else if native == CF_HDROP.0 as u32 && !have_files {
                Some(read_files().map(|entry| (entry, 2)))
            } else {
                None
            };
            if let Some(result) = result {
                match result {
                    Ok((entry, kind)) => {
                        match kind {
                            0 => have_text = true,
                            1 => have_image = true,
                            _ => have_files = true,
                        }
                        entries.push(entry);
                    }
                    Err(error) => {
                        log::warn!("Could not read clipboard format {native}: {error:#}");
                        if first_error.is_none() {
                            first_error = Some(error);
                        }
                    }
                }
            }
        }
        if !entries.is_empty() {
            return Ok(Some(ClipboardItem { entries }));
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        Ok(None)
    })
}

fn write_text(text: &ClipboardString, formats: &Formats) -> Result<()> {
    let normalized = text.text.replace('\0', " ");
    set_bytes(CF_UNICODETEXT.0 as u32, &wide_bytes(&normalized))?;
    if let Some(metadata) = &text.metadata {
        set_bytes(
            formats.hash,
            &ClipboardString::text_hash(&normalized).to_ne_bytes(),
        )?;
        set_bytes(formats.metadata, &wide_bytes(metadata))?;
    }
    Ok(())
}

fn wide_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .chain(Some(0))
        .flat_map(u16::to_ne_bytes)
        .collect()
}

fn read_wide(format: u32) -> Result<String> {
    let bytes = read_bytes(format)?;
    ensure!(
        bytes.len() % 2 == 0,
        "Clipboard UTF-16 data has an odd byte count"
    );
    let words = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_ne_bytes([pair[0], pair[1]]))
        .take_while(|word| *word != 0)
        .collect::<Vec<_>>();
    Ok(String::from_utf16_lossy(&words))
}

fn read_text(formats: &Formats, available: &[u32]) -> Result<ClipboardEntry> {
    let text = read_wide(CF_UNICODETEXT.0 as u32)?;
    let metadata = match read_metadata(&text, formats, available) {
        Ok(metadata) => metadata,
        Err(error) => {
            log::warn!("Clipboard text metadata could not be read: {error:#}");
            None
        }
    };
    Ok(ClipboardEntry::String(ClipboardString { text, metadata }))
}

fn read_metadata(text: &str, formats: &Formats, available: &[u32]) -> Result<Option<String>> {
    if !available.contains(&formats.hash) || !available.contains(&formats.metadata) {
        return Ok(None);
    }
    let hash = read_bytes(formats.hash)?;
    let matches = hash
        .get(..8)
        .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
        .is_some_and(|bytes| u64::from_ne_bytes(bytes) == ClipboardString::text_hash(text));
    if matches {
        Ok(Some(read_wide(formats.metadata)?))
    } else {
        Ok(None)
    }
}

fn write_image(image: &Image, formats: &Formats) -> Result<()> {
    let native = match image.format() {
        ImageFormat::Png => Some(formats.png),
        ImageFormat::Gif => Some(formats.gif),
        ImageFormat::Jpeg => Some(formats.jpeg),
        ImageFormat::Svg => Some(formats.svg),
        _ => None,
    };
    if let Some(native) = native {
        set_bytes(native, image.bytes())?;
    }
    if image.format() == ImageFormat::Svg || native == Some(formats.png) {
        return Ok(());
    }
    let encoding = match image.format() {
        ImageFormat::Jpeg => image::ImageFormat::Jpeg,
        ImageFormat::Webp => image::ImageFormat::WebP,
        ImageFormat::Gif => image::ImageFormat::Gif,
        ImageFormat::Bmp => image::ImageFormat::Bmp,
        ImageFormat::Tiff => image::ImageFormat::Tiff,
        unsupported => {
            return Err(anyhow!(
                "No standard Windows clipboard PNG conversion for {unsupported:?}"
            ));
        }
    };
    let conversion = (|| -> Result<Vec<u8>> {
        let decoded = image::load_from_memory_with_format(image.bytes(), encoding)?;
        let mut output = Cursor::new(Vec::new());
        decoded.write_to(&mut output, image::ImageFormat::Png)?;
        Ok(output.into_inner())
    })();
    match conversion {
        Ok(bytes) => set_bytes(formats.png, &bytes),
        Err(error) if native.is_some() => {
            log::warn!(
                "Clipboard retained the original image, but PNG conversion failed: {error:#}"
            );
            Ok(())
        }
        Err(error) => Err(error.context("Could not convert the clipboard image to PNG")),
    }
}

fn read_image(native: u32, formats: &Formats) -> Result<ClipboardEntry> {
    let bytes = read_bytes(native)?;
    let (format, bytes) = if native == CF_DIB.0 as u32 {
        (ImageFormat::Bmp, dib_to_bmp(&bytes)?)
    } else {
        (
            formats
                .image_format(native)
                .context("Unsupported native image format")?,
            bytes,
        )
    };
    Ok(ClipboardEntry::Image(Image::from_bytes(format, bytes)))
}

fn read_files() -> Result<ClipboardEntry> {
    let handle = unsafe { GetClipboardData(CF_HDROP.0 as u32) }?;
    let drop_handle = HDROP(handle.0);
    let count = unsafe { DragQueryFileW(drop_handle, u32::MAX, None) };
    let mut paths = Vec::new();
    for index in 0..count {
        let length = unsafe { DragQueryFileW(drop_handle, index, None) };
        ensure!(length > 0, "Clipboard file path is empty or unreadable");
        let capacity = usize::try_from(length)?
            .checked_add(1)
            .context("Clipboard file name length overflow")?;
        ensure!(
            u32::try_from(capacity).is_ok(),
            "Clipboard file name exceeds the native length limit"
        );
        let mut buffer = Vec::new();
        buffer.try_reserve_exact(capacity)?;
        buffer.resize(capacity, 0);
        let copied = unsafe { DragQueryFileW(drop_handle, index, Some(&mut buffer)) };
        ensure!(
            copied == length,
            "Clipboard file name changed while reading"
        );
        paths.push(PathBuf::from(OsString::from_wide(
            buffer
                .get(..usize::try_from(copied)?)
                .context("Clipboard file name exceeds its buffer")?,
        )));
    }
    Ok(ClipboardEntry::ExternalPaths(ExternalPaths::new(paths)))
}

struct Allocation(Option<HGLOBAL>);

impl Drop for Allocation {
    fn drop(&mut self) {
        if let Some(memory) = self.0.take() {
            if !unsafe { global_free_raw(memory.0) }.is_null() {
                log::warn!(
                    "Clipboard memory release failed: {}",
                    std::io::Error::last_os_error()
                );
            }
        }
    }
}

struct LockedGlobal {
    handle: HGLOBAL,
    pointer: *mut c_void,
    size: usize,
    locked: bool,
}

impl LockedGlobal {
    fn new(handle: HGLOBAL) -> Result<Self> {
        let size = unsafe { GlobalSize(handle) };
        ensure!(
            size <= isize::MAX as usize,
            "Clipboard allocation exceeds Rust's slice size limit"
        );
        let pointer = unsafe { GlobalLock(handle) };
        ensure!(
            !pointer.is_null(),
            "Could not lock clipboard memory: {}",
            std::io::Error::last_os_error()
        );
        Ok(Self {
            handle,
            pointer,
            size,
            locked: true,
        })
    }

    fn bytes(&self) -> &[u8] {
        // GlobalLock keeps this allocation stable until unlock or Drop.
        unsafe { std::slice::from_raw_parts(self.pointer.cast(), self.size) }
    }

    fn unlock(mut self) -> Result<()> {
        self.locked = false;
        unlock(self.handle)
    }
}

impl Drop for LockedGlobal {
    fn drop(&mut self) {
        if self.locked {
            if let Err(error) = unlock(self.handle) {
                log::warn!("Clipboard memory unlock during cleanup failed: {error:#}");
            }
        }
    }
}

fn unlock(memory: HGLOBAL) -> Result<()> {
    unsafe { SetLastError(ERROR_SUCCESS) };
    let still_locked = unsafe { global_unlock_raw(memory.0) };
    let error = unsafe { GetLastError() };
    ensure!(
        still_locked != 0 || error == ERROR_SUCCESS,
        "Clipboard memory unlock failed with Win32 error {}",
        error.0
    );
    Ok(())
}

fn set_bytes(format: u32, bytes: &[u8]) -> Result<()> {
    ensure!(!bytes.is_empty(), "Clipboard format {format} has no bytes");
    let mut allocation = Allocation(Some(unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len()) }?));
    let handle = allocation.0.context("Clipboard allocation missing")?;
    let locked = LockedGlobal::new(handle)?;
    ensure!(
        locked.size >= bytes.len(),
        "Clipboard allocation is smaller than requested"
    );
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), locked.pointer.cast(), bytes.len()) };
    locked.unlock()?;
    unsafe { SetClipboardData(format, Some(HANDLE(handle.0))) }?;
    // SetClipboardData now owns the allocation; only pre-transfer failures free it here.
    allocation.0 = None;
    Ok(())
}

fn read_bytes(format: u32) -> Result<Vec<u8>> {
    let handle = HGLOBAL(unsafe { GetClipboardData(format) }?.0);
    let locked = LockedGlobal::new(handle)?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(locked.size)?;
    bytes.extend_from_slice(locked.bytes());
    locked.unlock()?;
    Ok(bytes)
}

fn dib_to_bmp(dib: &[u8]) -> Result<Vec<u8>> {
    fn word(bytes: &[u8], offset: usize) -> Result<u16> {
        let end = offset.checked_add(2).context("DIB word offset overflow")?;
        Ok(u16::from_le_bytes(
            bytes
                .get(offset..end)
                .context("Truncated DIB word")?
                .try_into()?,
        ))
    }
    fn number(bytes: &[u8], offset: usize) -> Result<u32> {
        let end = offset.checked_add(4).context("DIB field offset overflow")?;
        Ok(u32::from_le_bytes(
            bytes
                .get(offset..end)
                .context("Truncated DIB field")?
                .try_into()?,
        ))
    }
    ensure!(dib.len() >= 40, "DIB lacks a BITMAPINFOHEADER");
    let header_size = number(dib, 0)?;
    ensure!(
        header_size >= 40 && usize::try_from(header_size)? <= dib.len(),
        "Invalid DIB header size"
    );
    let bit_count = word(dib, 14)?;
    let compression = number(dib, 16)?;
    let colors = number(dib, 32)?;
    let palette = if bit_count <= 8 {
        let count = if colors == 0 {
            1_u32
                .checked_shl(u32::from(bit_count))
                .context("Invalid DIB bit depth")?
        } else {
            colors
        };
        count.checked_mul(4).context("DIB palette overflow")?
    } else {
        colors.checked_mul(4).context("DIB palette overflow")?
    };
    let masks = if header_size == 40 {
        match compression {
            3 => 12,
            6 => 16,
            _ => 0,
        }
    } else {
        0
    };
    let pixel_offset = 14_u32
        .checked_add(header_size)
        .and_then(|offset| offset.checked_add(palette))
        .and_then(|offset| offset.checked_add(masks))
        .context("DIB pixel offset overflow")?;
    let size = u32::try_from(dib.len())?
        .checked_add(14)
        .context("BMP file size overflow")?;
    ensure!(
        pixel_offset <= size,
        "DIB palette/masks extend beyond the allocation"
    );
    let mut bmp = Vec::new();
    bmp.try_reserve_exact(usize::try_from(size)?)?;
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&size.to_le_bytes());
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&pixel_offset.to_le_bytes());
    bmp.extend_from_slice(dib);
    Ok(bmp)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bitmap_header(
        length: usize,
        bit_count: u16,
        compression: u32,
        colors: u32,
    ) -> Result<Vec<u8>> {
        let mut header = vec![0; length];
        fn field(header: &mut [u8], offset: usize, bytes: &[u8]) -> Result<()> {
            let end = offset
                .checked_add(bytes.len())
                .context("Test bitmap field overflow")?;
            header
                .get_mut(offset..end)
                .context("Test bitmap header is too small")?
                .copy_from_slice(bytes);
            Ok(())
        }
        field(&mut header, 0, &u32::try_from(length)?.to_le_bytes())?;
        field(&mut header, 4, &2_i32.to_le_bytes())?;
        field(&mut header, 8, &1_i32.to_le_bytes())?;
        field(&mut header, 12, &1_u16.to_le_bytes())?;
        field(&mut header, 14, &bit_count.to_le_bytes())?;
        field(&mut header, 16, &compression.to_le_bytes())?;
        field(&mut header, 20, &4_u32.to_le_bytes())?;
        field(&mut header, 32, &colors.to_le_bytes())?;
        Ok(header)
    }

    fn decoded_pixels(bitmap: &[u8]) -> Result<image::RgbImage> {
        Ok(image::load_from_memory_with_format(bitmap, image::ImageFormat::Bmp)?.to_rgb8())
    }

    #[test]
    fn dib_conversion_preserves_encoded_pixels() -> Result<()> {
        let expected = image::RgbImage::from_fn(2, 1, |column, _| {
            if column == 0 {
                image::Rgb([255, 0, 0])
            } else {
                image::Rgb([0, 255, 0])
            }
        });
        let mut encoded = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(expected.clone())
            .write_to(&mut encoded, image::ImageFormat::Bmp)?;
        let original = encoded.into_inner();
        let dib = original.get(14..).context("Encoded bitmap has no DIB")?;
        let converted = dib_to_bmp(dib)?;
        assert_eq!(decoded_pixels(&converted)?, expected);
        Ok(())
    }

    #[test]
    fn dib_conversion_preserves_indexed_palette() -> Result<()> {
        let mut dib = bitmap_header(40, 1, 0, 2)?;
        dib.extend_from_slice(&[0, 0, 0, 0, 255, 255, 255, 0]);
        dib.extend_from_slice(&[0x40, 0, 0, 0]);
        let expected = image::RgbImage::from_fn(2, 1, |column, _| {
            if column == 0 {
                image::Rgb([0, 0, 0])
            } else {
                image::Rgb([255, 255, 255])
            }
        });
        assert_eq!(decoded_pixels(&dib_to_bmp(&dib)?)?, expected);
        Ok(())
    }

    #[test]
    fn dib_conversion_preserves_external_and_embedded_color_masks() -> Result<()> {
        let expected = image::RgbImage::from_fn(2, 1, |column, _| {
            if column == 0 {
                image::Rgb([255, 0, 0])
            } else {
                image::Rgb([0, 255, 0])
            }
        });
        let masks = [0x0000_f800_u32, 0x0000_07e0, 0x0000_001f]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        for header_size in [40, 108] {
            let mut dib = bitmap_header(header_size, 16, 3, 0)?;
            if header_size == 40 {
                dib.extend_from_slice(&masks);
            } else {
                dib.get_mut(40..52)
                    .context("Missing embedded bitmap masks")?
                    .copy_from_slice(&masks);
            }
            dib.extend_from_slice(&[0, 248, 224, 7]);
            assert_eq!(
                decoded_pixels(&dib_to_bmp(&dib)?)?,
                expected,
                "header size {header_size}"
            );
        }
        Ok(())
    }

    #[test]
    fn dib_conversion_rejects_truncated_and_overflowing_headers() -> Result<()> {
        for length in 0..40 {
            assert!(
                dib_to_bmp(&vec![0; length]).is_err(),
                "header length {length}"
            );
        }
        let mut oversized_header = bitmap_header(40, 24, 0, 0)?;
        oversized_header
            .get_mut(..4)
            .context("Missing header size")?
            .copy_from_slice(&124_u32.to_le_bytes());
        assert!(dib_to_bmp(&oversized_header).is_err());
        assert!(dib_to_bmp(&bitmap_header(40, 8, 0, u32::MAX)?).is_err());
        assert!(dib_to_bmp(&bitmap_header(40, 8, 0, 0)?).is_err());
        assert!(dib_to_bmp(&bitmap_header(40, 16, 3, 0)?).is_err());
        Ok(())
    }
}
