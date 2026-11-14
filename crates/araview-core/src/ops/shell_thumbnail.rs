//! Explorer용 RGB 축소판. 앱의 영구 캐시를 사용하거나 원본을 수정하지 않는다.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use image::{ImageDecoder as _, ImageReader, Limits};

use crate::app_error::AppError;
use crate::sidecar::{downscale_rgb8, Rgb8};

/// PSD, HEIC/HEIF/AVIF, EXR, RAW 및 CBZ/ZIP의 표지를 디코드한다.
/// 호출자는 COM 경계에서 panic을 잡고, I/O를 UI 스레드 밖에서 수행한다.
pub fn decode_shell_thumbnail(source: &Path, max_side: u32) -> Result<Rgb8, AppError> {
    let max_side = max_side.clamp(16, 2560);
    if crate::image::is_archive_file(source) {
        let entries = crate::archive_index::get_archive_entries(source)?;
        let info = crate::comic_info::read_comic_info(source).ok().flatten();
        let cover = info
            .as_ref()
            .and_then(|info| info.pages.as_ref())
            .and_then(|pages| {
                pages.iter().find_map(|page| {
                    page.page_type
                        .as_deref()
                        .filter(|kind| kind.eq_ignore_ascii_case("FrontCover"))
                        .and_then(|_| entries.images.get(page.image as usize))
                })
            })
            .or_else(|| entries.images.first())
            .ok_or_else(|| AppError::not_found("No images in archive"))?;
        let temp = tempfile::tempdir().map_err(|e| AppError::unknown(e.to_string()))?;
        let extracted = crate::archive::extract_archive_image_with_protection(
            source,
            cover,
            temp.path(),
            false,
        )?;
        return decode_image(&extracted, max_side);
    }
    decode_image(source, max_side)
}

fn decode_image(source: &Path, max_side: u32) -> Result<Rgb8, AppError> {
    let rgb = if let Some(decoder) = crate::transcode::decoder_for(source) {
        crate::transcode::decode_rgb8(source, decoder)?
    } else if crate::image::resolve_mime(source) == Some("image/svg+xml") {
        let image = crate::svg_raster::rasterize(source, max_side)?;
        Rgb8 {
            width: image.width(),
            height: image.height(),
            bytes: image.into_raw(),
        }
    } else {
        let file = File::open(source).map_err(|e| AppError::corrupt(e.to_string()))?;
        let mut reader = ImageReader::new(BufReader::new(file))
            .with_guessed_format()
            .map_err(|e| AppError::corrupt(e.to_string()))?;
        let mut limits = Limits::default();
        limits.max_alloc = Some(1024 * 1024 * 1024);
        reader.limits(limits);
        let decoder = reader
            .into_decoder()
            .map_err(|e| AppError::image_error("Failed to decode thumbnail", e))?;
        let (width, height) = decoder.dimensions();
        if u64::from(width) * u64::from(height) > 100_000_000 {
            return Err(AppError::too_large(
                "Thumbnail image dimensions are too large",
            ));
        }
        // DynamicImage reads only the first frame of animated comic pages.
        let image = image::DynamicImage::from_decoder(decoder)
            .map_err(|e| AppError::image_error("Failed to decode thumbnail", e))?;
        let image = if crate::orientation::applies_to(source) {
            crate::orientation::apply_to_image(image, crate::orientation::read_orientation(source))
        } else {
            image
        };
        crate::raster_sidecar::flatten_to_rgb8(image)
    };
    Ok(downscale_rgb8(rgb, max_side))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn sample(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples")
            .join(name)
    }

    #[test]
    fn shell_formats_decode_without_upscaling() {
        for name in [
            "sample.psd",
            "sample.heic",
            "sample.heif",
            "sample.avif",
            "sample-animated.avif",
            "sample.exr",
            "sample.dng",
            "sample.cr3",
            "sample.raf",
            "sample.cbz",
            "sample.zip",
        ] {
            let rgb = decode_shell_thumbnail(&sample(name), 128).unwrap();
            assert!(rgb.width > 0 && rgb.height > 0, "{name}");
            assert!(rgb.width <= 128 && rgb.height <= 128, "{name}");
            assert_eq!(
                rgb.bytes.len(),
                rgb.width as usize * rgb.height as usize * 3
            );
        }
    }

    #[test]
    fn archive_cover_uses_metadata_then_natural_order() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("comic.cbz");
        for (xml, expected) in [
            (None, [0, 255, 0]),
            (Some("<ComicInfo><Pages><Page Image=\"1\" Type=\"FrontCover\"/></Pages></ComicInfo>"), [255, 0, 0]),
            (Some("<ComicInfo><Pages><Page Image=\"99\" Type=\"FrontCover\"/></Pages></ComicInfo>"), [0, 255, 0]),
            (Some("broken xml"), [0, 255, 0]),
        ] {
            let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
            for (name, color) in [("10.png", [255, 0, 0]), ("2.png", [0, 255, 0]), (".hidden.png", [0, 0, 255])] {
                let image = image::RgbImage::from_pixel(4, 2, image::Rgb(color));
                let mut png = std::io::Cursor::new(Vec::new());
                image.write_to(&mut png, image::ImageFormat::Png).unwrap();
                zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
                zip.write_all(png.get_ref()).unwrap();
            }
            if let Some(xml) = xml {
                zip.start_file("ComicInfo.xml", zip::write::SimpleFileOptions::default()).unwrap();
                zip.write_all(xml.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
            let rgb = decode_shell_thumbnail(&path, 128).unwrap();
            assert_eq!((rgb.width, rgb.height), (4, 2));
            assert_eq!(&rgb.bytes[..3], &expected);
        }
    }

    #[test]
    fn invalid_inputs_fail() {
        let temp = tempfile::tempdir().unwrap();
        for ext in ["psd", "heic", "avif", "exr", "dng", "cbz"] {
            let path = temp.path().join(format!("bad.{ext}"));
            std::fs::write(&path, b"invalid").unwrap();
            assert!(decode_shell_thumbnail(&path, 128).is_err(), "{ext}");
        }
    }
}
