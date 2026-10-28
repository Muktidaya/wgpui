// SPDX-License-Identifier: Apache-2.0
// Copyright 2022 - 2025 Zed Industries, Inc.
// Adapted from the gpui-pre-macos 0.3.10 pasteboard contract.

use crate::{ClipboardEntry, ClipboardItem, ClipboardString, ExternalPaths, Image, ImageFormat};
use anyhow::{Result, ensure};
use objc2::rc::autoreleasepool;
use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
use objc2_foundation::{NSArray, NSData, NSString};
use std::path::PathBuf;
use strum::IntoEnumIterator as _;

const TEXT_HASH_TYPE: &str = "zed-text-hash";
const METADATA_TYPE: &str = "zed-metadata";

pub(super) fn read() -> Result<Option<ClipboardItem>> {
    autoreleasepool(|_| read_from(&NSPasteboard::generalPasteboard()))
}

fn read_from(pasteboard: &NSPasteboard) -> Result<Option<ClipboardItem>> {
    #[expect(
        deprecated,
        reason = "The standard GPUI clipboard contract reads this file-copy type"
    )]
    let filenames = unsafe { pasteboard.propertyListForType(objc2_app_kit::NSFilenamesPboardType) };
    if let Some(filenames) = filenames
        .as_ref()
        .and_then(|value| value.downcast_ref::<NSArray>())
    {
        let mut paths = Vec::new();
        for filename in filenames {
            if let Some(filename) = filename.downcast_ref::<NSString>() {
                paths.push(PathBuf::from(filename.to_string()));
            }
        }
        if !paths.is_empty() {
            let mut entries = vec![ClipboardEntry::ExternalPaths(ExternalPaths::new(paths))];
            if let Some(text) = read_text(pasteboard)? {
                entries.push(text);
            }
            return Ok(Some(ClipboardItem { entries }));
        }
    }
    if let Some(text) = read_text(pasteboard)? {
        return Ok(Some(ClipboardItem {
            entries: vec![text],
        }));
    }
    let Some(types) = pasteboard.types() else {
        return Ok(None);
    };
    for format in ImageFormat::iter() {
        if !types.containsObject(&image_type(format)) {
            continue;
        }
        if let Some(data) = pasteboard.dataForType(&image_type(format)) {
            let bytes = data.to_vec();
            // AppKit can advertise a synthesized image type whose conversion produced no data.
            if bytes.is_empty() {
                continue;
            }
            return Ok(Some(ClipboardItem::new_image(&Image::from_bytes(
                format, bytes,
            ))));
        }
    }
    Ok(None)
}

pub(super) fn write(item: ClipboardItem) -> Result<()> {
    autoreleasepool(|_| write_to(&NSPasteboard::generalPasteboard(), item))
}

fn write_to(pasteboard: &NSPasteboard, item: ClipboardItem) -> Result<()> {
    match item.entries.as_slice() {
        [] => {
            pasteboard.clearContents();
        }
        [ClipboardEntry::String(text)] => write_text(pasteboard, text)?,
        [ClipboardEntry::Image(image)] => {
            pasteboard.clearContents();
            ensure!(
                pasteboard.setData_forType(
                    Some(&NSData::with_bytes(image.bytes())),
                    &image_type(image.format()),
                ),
                "macOS rejected clipboard image data"
            );
        }
        [ClipboardEntry::ExternalPaths(_)] => {
            log::debug!("The standard macOS clipboard contract does not write file lists");
        }
        _ => {
            let mut combined = ClipboardString::new(String::new());
            for entry in item.entries {
                if let ClipboardEntry::String(text) = entry {
                    combined.text.push_str(&text.text);
                    if combined.metadata.is_none() {
                        combined.metadata = text.metadata;
                    }
                }
            }
            write_text(pasteboard, &combined)?;
        }
    }
    Ok(())
}

fn read_text(pasteboard: &NSPasteboard) -> Result<Option<ClipboardEntry>> {
    let Some(types) = pasteboard.types() else {
        return Ok(None);
    };
    if !types.containsObject(unsafe { NSPasteboardTypeString }) {
        return Ok(None);
    }
    let Some(data) = pasteboard.dataForType(unsafe { NSPasteboardTypeString }) else {
        return Ok(None);
    };
    let text = String::from_utf8_lossy(&data.to_vec()).into_owned();
    let metadata = pasteboard
        .dataForType(&NSString::from_str(TEXT_HASH_TYPE))
        .and_then(|data| <[u8; 8]>::try_from(data.to_vec()).ok())
        .filter(|bytes| u64::from_be_bytes(*bytes) == ClipboardString::text_hash(&text))
        .and_then(|_| pasteboard.dataForType(&NSString::from_str(METADATA_TYPE)))
        .and_then(|data| String::from_utf8(data.to_vec()).ok());
    Ok(Some(ClipboardEntry::String(ClipboardString {
        text,
        metadata,
    })))
}

fn write_text(pasteboard: &NSPasteboard, text: &ClipboardString) -> Result<()> {
    pasteboard.clearContents();
    ensure!(
        pasteboard.setData_forType(Some(&NSData::with_bytes(text.text.as_bytes())), unsafe {
            NSPasteboardTypeString
        },),
        "macOS rejected clipboard text"
    );
    if let Some(metadata) = &text.metadata {
        ensure!(
            pasteboard.setData_forType(
                Some(&NSData::with_bytes(
                    &ClipboardString::text_hash(&text.text).to_be_bytes()
                )),
                &NSString::from_str(TEXT_HASH_TYPE),
            ),
            "macOS rejected clipboard metadata hash"
        );
        ensure!(
            pasteboard.setData_forType(
                Some(&NSData::with_bytes(metadata.as_bytes())),
                &NSString::from_str(METADATA_TYPE),
            ),
            "macOS rejected clipboard metadata"
        );
    }
    Ok(())
}

fn image_type(format: ImageFormat) -> objc2::rc::Retained<NSString> {
    NSString::from_str(match format {
        ImageFormat::Png => "public.png",
        ImageFormat::Jpeg => "public.jpeg",
        ImageFormat::Webp => "org.webmproject.webp",
        ImageFormat::Gif => "com.compuserve.gif",
        ImageFormat::Svg => "public.svg-image",
        ImageFormat::Bmp => "com.microsoft.bmp",
        ImageFormat::Tiff => "public.tiff",
        ImageFormat::Pnm => "public.pbm",
        ImageFormat::Ico => "com.microsoft.ico",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pasteboard() -> objc2::rc::Retained<NSPasteboard> {
        NSPasteboard::pasteboardWithName(&NSString::from_str(&format!(
            "wgpui-clipboard-test-{}",
            uuid::Uuid::new_v4(),
        )))
    }

    #[test]
    #[ignore = "requires the native macOS pasteboard service; uses an isolated named pasteboard"]
    fn text_metadata_empty_nul_and_foreign_replacement() -> Result<()> {
        let pasteboard = pasteboard();
        for text in ["", "héllo\0世界🦀\r\n"] {
            let item =
                ClipboardItem::new_string_with_metadata(text.into(), "{\"value\":42}".into());
            write_to(&pasteboard, item.clone())?;
            assert_eq!(read_from(&pasteboard)?, Some(item));
        }
        ensure!(
            pasteboard.setData_forType(Some(&NSData::with_bytes(b"other owner")), unsafe {
                NSPasteboardTypeString
            }),
            "could not replace clipboard text"
        );
        assert_eq!(
            read_from(&pasteboard)?,
            Some(ClipboardItem::new_string("other owner".into()))
        );
        write_to(
            &pasteboard,
            ClipboardItem {
                entries: Vec::new(),
            },
        )?;
        assert_eq!(read_from(&pasteboard)?, None);
        Ok(())
    }

    #[test]
    #[ignore = "requires the native macOS pasteboard service; uses an isolated named pasteboard"]
    fn external_files_with_text_precede_images_and_preserve_paths() -> Result<()> {
        let pasteboard = pasteboard();
        let paths = ["/sample/space and 世界.png", "/sample/second.txt"];
        let filenames = paths
            .iter()
            .map(|path| NSString::from_str(path))
            .collect::<Vec<_>>();
        #[expect(
            deprecated,
            reason = "Exercise the exact standard file-copy representation"
        )]
        let wrote_files = unsafe {
            pasteboard.setPropertyList_forType(
                &NSArray::from_retained_slice(&filenames),
                objc2_app_kit::NSFilenamesPboardType,
            )
        };
        ensure!(wrote_files, "could not write native file list");
        ensure!(
            pasteboard.setData_forType(
                Some(&NSData::with_bytes(paths.join("\n").as_bytes())),
                unsafe { NSPasteboardTypeString }
            ),
            "could not write file text"
        );
        let bytes = b"original PNG representation";
        ensure!(
            pasteboard.setData_forType(
                Some(&NSData::with_bytes(bytes)),
                &image_type(ImageFormat::Png)
            ),
            "could not write image representation"
        );
        assert_eq!(
            read_from(&pasteboard)?,
            Some(ClipboardItem {
                entries: vec![
                    ClipboardEntry::ExternalPaths(ExternalPaths::new(paths.map(PathBuf::from))),
                    ClipboardEntry::String(ClipboardString::new(paths.join("\n"))),
                ],
            })
        );
        pasteboard.clearContents();
        ensure!(
            pasteboard.setData_forType(
                Some(&NSData::with_bytes(bytes)),
                &image_type(ImageFormat::Png)
            ),
            "could not write image representation"
        );
        assert_eq!(
            read_from(&pasteboard)?,
            Some(ClipboardItem::new_image(&Image::from_bytes(
                ImageFormat::Png,
                bytes.to_vec()
            ),))
        );
        Ok(())
    }

    #[test]
    #[ignore = "requires the native macOS pasteboard service; uses an isolated named pasteboard"]
    fn mixed_write_matches_the_standard_text_combination_contract() -> Result<()> {
        let pasteboard = pasteboard();
        write_to(
            &pasteboard,
            ClipboardItem {
                entries: vec![
                    ClipboardEntry::Image(Image::empty()),
                    ClipboardEntry::String(ClipboardString {
                        text: "first".into(),
                        metadata: Some("one".into()),
                    }),
                    ClipboardEntry::String(ClipboardString {
                        text: "second".into(),
                        metadata: Some("two".into()),
                    }),
                ],
            },
        )?;
        assert_eq!(
            read_from(&pasteboard)?,
            Some(ClipboardItem::new_string_with_metadata(
                "firstsecond".into(),
                "one".into(),
            ))
        );
        Ok(())
    }

    #[test]
    #[ignore = "requires the native macOS pasteboard service; uses an isolated named pasteboard"]
    fn native_images_preserve_original_data_and_read_usable_representations() -> Result<()> {
        let pasteboard = pasteboard();
        let malformed = ClipboardItem::new_image(&Image::from_bytes(
            ImageFormat::Pnm,
            vec![0, 255, 128, 17, 0, 42],
        ));
        write_to(&pasteboard, malformed.clone())?;
        assert_eq!(
            read_from(&pasteboard)?,
            Some(malformed),
            "an empty synthesized TIFF must not hide the original PNM representation"
        );

        for format in ImageFormat::iter() {
            let bytes = match format {
                ImageFormat::Svg => {
                    br#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="black"/></svg>"#.to_vec()
                }
                ImageFormat::Pnm => b"P4\n2 2\n\xc0\xc0".to_vec(),
                _ => {
                    let encoding = match format {
                        ImageFormat::Png => image::ImageFormat::Png,
                        ImageFormat::Jpeg => image::ImageFormat::Jpeg,
                        ImageFormat::Webp => image::ImageFormat::WebP,
                        ImageFormat::Gif => image::ImageFormat::Gif,
                        ImageFormat::Bmp => image::ImageFormat::Bmp,
                        ImageFormat::Tiff => image::ImageFormat::Tiff,
                        ImageFormat::Ico => image::ImageFormat::Ico,
                        _ => unreachable!("SVG and PNM have explicit fixtures"),
                    };
                    let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
                        2,
                        2,
                        image::Rgb([0, 0, 0]),
                    ));
                    let mut encoded = std::io::Cursor::new(Vec::new());
                    image.write_to(&mut encoded, encoding)?;
                    encoded.into_inner()
                }
            };
            write_to(
                &pasteboard,
                ClipboardItem::new_image(&Image::from_bytes(format, bytes.clone())),
            )?;
            assert_eq!(
                pasteboard
                    .dataForType(&image_type(format))
                    .expect("the original image type is stored")
                    .to_vec(),
                bytes,
                "native storage keeps the original {format:?} data"
            );
            let item = read_from(&pasteboard)?.expect("a native image representation is available");
            let [ClipboardEntry::Image(image)] = item.entries.as_slice() else {
                panic!("expected exactly one image for {format:?}");
            };
            assert!(
                !image.bytes().is_empty(),
                "the selected representation has bytes"
            );
            if image.format() == format {
                assert_eq!(image.bytes(), bytes.as_slice());
            } else {
                let decoded = image::load_from_memory(image.bytes())?.to_rgb8();
                assert_eq!(decoded.dimensions(), (2, 2));
                assert!(decoded.pixels().all(|pixel| pixel.0 == [0, 0, 0]));
            }
        }
        Ok(())
    }
}
