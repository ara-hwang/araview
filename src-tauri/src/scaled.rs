//! Resolution-cap sidecars for very large images.
//!
//! When the user caps the display resolution, raster sources whose long edge
//! exceeds the cap are decoded once, downscaled, and published as a JPEG
//! (PNG when the source has an alpha channel) under `process_temp/scaled/`.
//! The WebView then decodes the smaller copy, keeping peak memory bounded for
//! huge images.
//!
//! Formats that must keep the original bytes are left alone: GIF (animation)
//! and animated WebP. Anything the `image` crate cannot decode (SVG, AVIF)
//! fails the header probe and falls back to the original as well, so the cap
//! can only ever make rendering cheaper, never break it.

use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use crate::app_error::{AppError, ErrorCode};
use crate::sidecar::Rgb8;

pub(crate) const SCALED_SUBDIR: &str = "scaled";
/// Upper bound for the scaled directory; oldest files evicted past this.
pub(crate) const MAX_SCALED_BYTES: u64 = 500 * 1024 * 1024;
/// JPEG quality for the capped copy. Close to the HEIF paint sidecar (90).
const JPEG_QUALITY: u8 = 88;
/// Lower bound rejects accidental 1px requests; upper bound keeps the cap a
/// memory optimization, not a general preview downsizer.
const MIN_MAX_SIDE: u32 = 512;
const MAX_MAX_SIDE: u32 = 8192;

#[derive(Debug)]
pub struct ScaledSidecar {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
}

pub fn clamp_max_side(max_side: u32) -> u32 {
    max_side.clamp(MIN_MAX_SIDE, MAX_MAX_SIDE)
}

/// Downscaled copy path when the source is over the cap, `None` when the
/// original should be rendered as-is.
pub fn ensure_scaled_sidecar(
    source: &Path,
    max_side: u32,
) -> Result<Option<ScaledSidecar>, AppError> {
    if !source.is_file() {
        return Ok(None);
    }
    let max_side = clamp_max_side(max_side);
    let Some((format, (width, height))) = probe(source) else {
        return Ok(None);
    };
    match format {
        image::ImageFormat::Gif => return Ok(None),
        image::ImageFormat::WebP if is_animated_webp(source) => return Ok(None),
        _ => {}
    }
    // EXIF orientation 5..=8 swaps the axes but not the long edge, so the cap
    // check is orientation-invariant.
    if width <= max_side && height <= max_side {
        return Ok(None);
    }

    let identity = crate::sidecar::file_identity_hash(source, &max_side.to_le_bytes())?;
    let dir = scaled_dir()?;
    let jpeg_dest = dir.join(format!("{identity:016x}.jpg"));
    let png_dest = dir.join(format!("{identity:016x}.png"));
    for dest in [&jpeg_dest, &png_dest] {
        if dest.exists() {
            crate::process_temp::touch_cache_file(dest);
            crate::process_temp::mark_in_use(dest);
            return sidecar_info(dest);
        }
    }

    let lock_key = jpeg_dest.to_string_lossy().into_owned();
    // Protect both possible destinations before writing. A cache clear can run
    // concurrently with a large decode and must not remove a file after publish
    // but before the WebView receives its asset URL.
    crate::process_temp::mark_in_use(&jpeg_dest);
    crate::process_temp::mark_in_use(&png_dest);
    let dest = crate::sidecar::with_file_lock(&lock_key, "scaled sidecar lock", || {
        for dest in [&jpeg_dest, &png_dest] {
            if dest.exists() {
                crate::process_temp::touch_cache_file(dest);
                return Ok(dest.clone());
            }
        }
        let img = image::ImageReader::open(source)
            .map_err(|e| AppError::io("Failed to open image", e, ErrorCode::Corrupt))?
            .with_guessed_format()
            .map_err(|e| AppError::io("Failed to guess image format", e, ErrorCode::Corrupt))?
            .decode()
            .map_err(|e| AppError::image_error("Failed to decode image", e))?;
        // JPEG/TIFF EXIF orientation을 픽셀에 반영한다. sidecar JPEG에는 EXIF가
        // 없으므로 WebView는 회전을 적용하지 않고, 반환 치수도 이 기준과 맞춘다.
        let img =
            crate::orientation::apply_to_image(img, crate::orientation::read_orientation(source));
        let has_alpha = img.color().has_alpha();
        let scaled = img.resize(max_side, max_side, image::imageops::FilterType::Triangle);
        if has_alpha {
            // 투명도를 보존하려면 JPEG로 재인코딩할 수 없다.
            let tmp = crate::sidecar::tmp_path_for(&png_dest);
            scaled
                .save_with_format(&tmp, image::ImageFormat::Png)
                .map_err(|e| AppError::unknown(format!("Failed to encode scaled PNG: {e}")))?;
            crate::sidecar::publish_atomic(&tmp, &png_dest, "Failed to publish scaled sidecar")?;
            Ok(png_dest.clone())
        } else {
            let rgb = Rgb8 {
                width: scaled.width(),
                height: scaled.height(),
                bytes: scaled.to_rgb8().into_raw(),
            };
            crate::sidecar::write_rgb8_jpeg_atomic(&jpeg_dest, &rgb, JPEG_QUALITY)?;
            Ok(jpeg_dest.clone())
        }
    })?;
    // WebView가 이 경로로 asset URL을 유지하므로 축출에서 보호한다.
    crate::process_temp::mark_in_use(&dest);
    // Best effort: eviction failures must not fail image delivery.
    crate::process_temp::enforce_cap(SCALED_SUBDIR, MAX_SCALED_BYTES).ok();
    sidecar_info(&dest)
}

fn probe(source: &Path) -> Option<(image::ImageFormat, (u32, u32))> {
    let reader = image::ImageReader::open(source)
        .ok()?
        .with_guessed_format()
        .ok()?;
    let format = reader.format()?;
    let dimensions = reader.into_dimensions().ok()?;
    Some((format, dimensions))
}

/// 움직이는 WebP는 첫 프레임으로 굳지 않게 원본을 그대로 쓴다.
fn is_animated_webp(source: &Path) -> bool {
    let Ok(file) = fs::File::open(source) else {
        return true;
    };
    match image::codecs::webp::WebPDecoder::new(BufReader::new(file)) {
        Ok(decoder) => decoder.has_animation(),
        // 헤더를 읽지 못하면 원본 유지가 안전하다.
        Err(_) => true,
    }
}

fn sidecar_info(path: &Path) -> Result<Option<ScaledSidecar>, AppError> {
    let (width, height) = image::image_dimensions(path)
        .map_err(|e| AppError::corrupt(format!("Failed to read scaled sidecar: {e}")))?;
    Ok(Some(ScaledSidecar {
        path: path.to_path_buf(),
        width,
        height,
    }))
}

fn scaled_dir() -> Result<PathBuf, AppError> {
    let dir = crate::process_temp::process_temp_dir()?.join(SCALED_SUBDIR);
    fs::create_dir_all(&dir)
        .map_err(|e| AppError::io("Failed to create scaled dir", e, ErrorCode::Unknown))?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_png(path: &Path, width: u32, height: u32) {
        let img =
            image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(width, height, |x, y| {
                image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
            }));
        img.save(path).expect("write png fixture");
    }

    #[test]
    fn within_limit_keeps_original() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("small.png");
        write_png(&source, 200, 100);

        assert!(ensure_scaled_sidecar(&source, 256).unwrap().is_none());
        // 경계값(긴 변 == cap)은 원본 유지
        assert!(ensure_scaled_sidecar(&source, 200).unwrap().is_none());
    }

    #[test]
    fn over_limit_produces_reusable_jpeg() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("wide.png");
        write_png(&source, 2000, 1000);

        let first = ensure_scaled_sidecar(&source, 512)
            .unwrap()
            .expect("scaled sidecar");
        assert_eq!(first.path.extension().and_then(|e| e.to_str()), Some("jpg"));
        assert_eq!((first.width, first.height), (512, 256));
        let bytes = fs::read(&first.path).unwrap();
        assert_eq!(&bytes[..2], &[0xFF, 0xD8]);

        let second = ensure_scaled_sidecar(&source, 512)
            .unwrap()
            .expect("reuse sidecar");
        assert_eq!(first.path, second.path);
    }

    #[test]
    fn alpha_sources_keep_transparency_as_png() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("logo.png");
        let img = image::RgbaImage::from_fn(1000, 500, |x, _| {
            if x < 500 {
                image::Rgba([255, 0, 0, 0])
            } else {
                image::Rgba([0, 0, 255, 255])
            }
        });
        image::DynamicImage::ImageRgba8(img).save(&source).unwrap();

        let scaled = ensure_scaled_sidecar(&source, 512)
            .unwrap()
            .expect("scaled sidecar");
        assert_eq!(
            scaled.path.extension().and_then(|e| e.to_str()),
            Some("png")
        );
        assert_eq!((scaled.width, scaled.height), (512, 256));
        // 축소 후에도 왼쪽 투명, 오른쪽 불투명이 유지된다.
        let out = image::open(&scaled.path).unwrap().to_rgba8();
        assert_eq!(out.get_pixel(0, 0).0[3], 0);
        assert_eq!(out.get_pixel(out.width() - 1, 0).0[3], 255);
    }

    #[test]
    fn exif_orientation_is_applied_before_scaling() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("phone.jpg");
        // 저장 2048x1024 + orientation 6(시계 90도) → 표시 1024x2048, 상한 512 → 256x512
        crate::orientation::write_test_jpeg_with_orientation(&source, 2048, 1024, 6);

        let scaled = ensure_scaled_sidecar(&source, 512)
            .unwrap()
            .expect("scaled sidecar");
        assert_eq!((scaled.width, scaled.height), (256, 512));
    }

    #[test]
    fn gif_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("anim.gif");
        let img = image::DynamicImage::ImageRgb8(image::RgbImage::new(2000, 1000));
        img.save_with_format(&source, image::ImageFormat::Gif)
            .unwrap();

        assert!(ensure_scaled_sidecar(&source, 512).unwrap().is_none());
    }

    #[test]
    fn static_webp_is_scaled() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("photo.webp");
        write_png(&source, 2000, 1000);
        let webp = dir.path().join("real.webp");
        image::open(&source)
            .unwrap()
            .save_with_format(&webp, image::ImageFormat::WebP)
            .unwrap();

        assert!(!is_animated_webp(&webp));
        let scaled = ensure_scaled_sidecar(&webp, 512)
            .unwrap()
            .expect("scaled sidecar");
        assert_eq!((scaled.width, scaled.height), (512, 256));
    }

    #[test]
    fn undecodable_source_falls_back_to_original() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("broken.png");
        fs::write(&source, b"not really a png").unwrap();
        assert!(ensure_scaled_sidecar(&source, 512).unwrap().is_none());
        assert!(ensure_scaled_sidecar(Path::new("missing.png"), 512)
            .unwrap()
            .is_none());
    }

    #[test]
    fn max_side_is_clamped() {
        assert_eq!(clamp_max_side(1), MIN_MAX_SIDE);
        assert_eq!(clamp_max_side(100_000), MAX_MAX_SIDE);
        assert_eq!(clamp_max_side(3840), 3840);
    }
}
