//! Resolution-cap sidecars for very large images.
//!
//! When the user caps the display resolution, raster sources whose long edge
//! exceeds the cap are decoded once, downscaled with an interpolation filter
//! (nearest shrink breaks periodic tones like screentone), and published as a
//! JPEG (PNG when the source has an alpha channel or the mode is pixelated)
//! under `process_temp/scaled/`.
//! The WebView then decodes the smaller copy, keeping peak memory bounded for
//! huge images.
//!
//! Formats that must keep the original bytes are left alone: GIF (animation),
//! animated WebP and animated PNG (APNG). Anything the `image` crate cannot decode (SVG, AVIF)
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
/// Changing the downscale algorithm must not reuse older scaled copies.
/// v2: 축소 sidecar는 감지 결과와 무관하게 항상 보간 필터를 쓴다(nearest 축소는
/// 스크린톤 같은 주기 패턴을 깨뜨린다).
/// v3: 무채색 이미지의 축소를 진 선형광에서 한다(`linear_resize`). 기존 캐시가
/// 감마 공간 사본을 그대로 쓰지 않게 revision을 갱신한다.
const PIXEL_ART_FILTER_REVISION: &[u8] = crate::linear_resize::DOWNSCALE_FILTER_REVISION;
/// Lower bound rejects accidental 1px requests; upper bound keeps the cap a
/// memory optimization, not a general preview downsizer.
const MIN_MAX_SIDE: u32 = 512;
const MAX_MAX_SIDE: u32 = 8192;
/// 디코드 할당 상한. 표시·클립보드 경로와 같은 값이다. 지정하지 않으면 `image`
/// 기본값(512MiB)이 걸려, 원본으로는 열리는 이미지가 상한을 켰을 때만 실패한다.
const MAX_DECODE_ALLOC: u64 = 1024 * 1024 * 1024;

/// 표시 설정에 따라 sidecar를 만들 때 사용할 저장 정책이다.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageScalingMode {
    #[default]
    Smooth,
    Pixelated,
}

impl ImageScalingMode {
    /// 표시 설정 문자열을 정규화한다. `auto`와 알 수 없는 값은 smooth다(`auto`의
    /// 픽셀 보존은 확대 배율에서 호스트가 정한다).
    pub fn from_command(value: Option<&str>) -> Self {
        match value {
            Some("pixelated") => Self::Pixelated,
            _ => Self::Smooth,
        }
    }

    fn cache_discriminator(self) -> u8 {
        match self {
            Self::Smooth => b's',
            Self::Pixelated => b'p',
        }
    }
}

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
    ensure_scaled_sidecar_with_mode(source, max_side, ImageScalingMode::Smooth)
}

/// 표시 정책을 반영한 축소 sidecar를 만든다.
pub fn ensure_scaled_sidecar_with_mode(
    source: &Path,
    max_side: u32,
    mode: ImageScalingMode,
) -> Result<Option<ScaledSidecar>, AppError> {
    if !source.is_file() {
        return Ok(None);
    }
    let max_side = clamp_max_side(max_side);
    let Some((format, (width, height))) = probe(source) else {
        return Ok(None);
    };
    if format == image::ImageFormat::Gif {
        return Ok(None);
    }
    // EXIF orientation 5..=8 swaps the axes but not the long edge, so the cap
    // check is orientation-invariant.
    if width <= max_side && height <= max_side {
        return Ok(None);
    }

    let mut identity_extra = max_side.to_le_bytes().to_vec();
    identity_extra.push(mode.cache_discriminator());
    identity_extra.extend_from_slice(PIXEL_ART_FILTER_REVISION);
    identity_extra.extend_from_slice(crate::orientation::cache_scope_tag(source));
    let identity = crate::sidecar::file_identity_hash(source, &identity_extra)?;
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
    // 캐시 미스에서만 원본을 다시 열어 움직이는 WebP/PNG를 가린다. 히트에서는
    // 위 `exists`로 끝나 probe 1회로 끝난다.
    match format {
        image::ImageFormat::WebP if is_animated_webp(source) => return Ok(None),
        image::ImageFormat::Png if is_animated_png(source) => return Ok(None),
        _ => {}
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
        let mut reader = image::ImageReader::open(source)
            .map_err(|e| AppError::io("Failed to open image", e, ErrorCode::Corrupt))?
            .with_guessed_format()
            .map_err(|e| AppError::io("Failed to guess image format", e, ErrorCode::Corrupt))?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(MAX_DECODE_ALLOC);
        reader.limits(limits);
        let img = reader
            .decode()
            .map_err(|e| AppError::image_error("Failed to decode image", e))?;
        // JPEG/TIFF EXIF orientation을 픽셀에 반영한다. sidecar에는 EXIF가 없으므로
        // 표시는 회전을 다시 적용하지 않고, 반환 치수도 이 기준과 맞춘다. 원본 보기가
        // 회전하지 않는 PNG/WebP의 태그는 무시해야 상한을 켰을 때만 돌아가지 않는다.
        let img = if crate::orientation::applies_to(source) {
            crate::orientation::apply_to_image(img, crate::orientation::read_orientation(source))
        } else {
            img
        };
        let has_alpha = img.color().has_alpha();
        let is_pixel_art = matches!(mode, ImageScalingMode::Pixelated);
        // 목표 치수는 image 크레이트 resize와 같은 규칙(min 비율 × round)으로
        // 정한다. 알파는 프리멀티 처리가 필요해 감마 공간 Triangle 축소를 쓰고,
        // 알파가 없으면 표시용 공용 축소(무채색은 선형광)를 쓴다.
        let (scaled_w, scaled_h) =
            crate::linear_resize::fit_inside(img.width(), img.height(), max_side, max_side);
        let scaled = if has_alpha {
            img.resize_exact(scaled_w, scaled_h, image::imageops::FilterType::Triangle)
        } else {
            let rgb = Rgb8 {
                width: img.width(),
                height: img.height(),
                bytes: img.into_rgb8().into_raw(),
            };
            let small = crate::linear_resize::resize_rgb8(rgb, scaled_w, scaled_h);
            image::DynamicImage::ImageRgb8(
                image::RgbImage::from_raw(scaled_w, scaled_h, small.bytes)
                    .expect("공용 축소는 목표 치수의 RGB8 버퍼를 돌려준다"),
            )
        };
        if has_alpha || is_pixel_art {
            // JPEG는 픽셀 경계에 다시 아티팩트를 만들 수 있으므로 픽셀 보존 모드는
            // 알파 여부와 무관하게 lossless PNG sidecar로 보존한다.
            crate::sidecar::write_cache_atomic(
                &png_dest,
                "Failed to publish scaled sidecar",
                |tmp| {
                    scaled
                        .save_with_format(tmp, image::ImageFormat::Png)
                        .map_err(|e| AppError::unknown(format!("Failed to encode scaled PNG: {e}")))
                },
            )?;
            // Best effort: eviction failures must not fail image delivery.
            crate::process_temp::note_published(&png_dest, MAX_SCALED_BYTES);
            Ok(png_dest.clone())
        } else {
            let rgb = Rgb8 {
                width: scaled.width(),
                height: scaled.height(),
                bytes: scaled.to_rgb8().into_raw(),
            };
            crate::sidecar::write_rgb8_jpeg_atomic(&jpeg_dest, &rgb, JPEG_QUALITY)?;
            crate::process_temp::note_published(&jpeg_dest, MAX_SCALED_BYTES);
            Ok(jpeg_dest.clone())
        }
    })?;
    // WebView가 이 경로로 asset URL을 유지하므로 축출에서 보호한다.
    crate::process_temp::mark_in_use(&dest);
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

/// APNG(`acTL` 청크가 있는 PNG)도 같은 이유로 원본을 쓴다. 확장자가 `.png`여도 해당한다.
fn is_animated_png(source: &Path) -> bool {
    // 헤더를 읽지 못하면 원본 유지가 안전하다.
    crate::sniff::png_is_animated(source).unwrap_or(true)
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
    use crate::orientation::test_images::{write_jpeg, write_png};

    /// 해상도 상한 사본도 표시와 같은 기준으로만 회전한다(PNG 태그는 무시).
    #[test]
    fn png_orientation_tag_does_not_rotate_the_scaled_copy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wide.png");
        write_png(&path, 1200, 600, 6);

        let scaled = ensure_scaled_sidecar(&path, 512).unwrap().unwrap();

        assert_eq!((scaled.width, scaled.height), (512, 256));
    }

    #[test]
    fn jpeg_orientation_tag_rotates_the_scaled_copy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wide.jpg");
        write_jpeg(&path, 1200, 600, 6);

        let scaled = ensure_scaled_sidecar(&path, 512).unwrap().unwrap();

        assert_eq!((scaled.width, scaled.height), (256, 512));
    }

    fn write_gradient_png(path: &Path, width: u32, height: u32) {
        let img =
            image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(width, height, |x, y| {
                image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
            }));
        img.save(path).expect("write png fixture");
    }

    /// 스크린톤처럼 주기 패턴인 소스를 픽셀 아트로 감지하지 않고 보간 축소한다.
    fn write_periodic_png(path: &Path) {
        let img = image::RgbImage::from_fn(2000, 1000, |x, y| {
            if (x / 8 + y / 8) % 2 == 0 {
                image::Rgb([240, 30, 30])
            } else {
                image::Rgb([20, 40, 220])
            }
        });
        image::DynamicImage::ImageRgb8(img).save(path).unwrap();
    }

    #[test]
    fn within_limit_keeps_original() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("small.png");
        write_gradient_png(&source, 200, 100);

        assert!(ensure_scaled_sidecar(&source, 256).unwrap().is_none());
        // 경계값(긴 변 == cap)은 원본 유지
        assert!(ensure_scaled_sidecar(&source, 512).unwrap().is_none());
    }

    #[test]
    fn over_limit_produces_reusable_jpeg() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("wide.png");
        write_gradient_png(&source, 2000, 1000);

        let first = ensure_scaled_sidecar(&source, 512)
            .unwrap()
            .expect("scaled sidecar");
        assert_eq!(first.path.extension().and_then(|e| e.to_str()), Some("jpg"));
        assert_eq!((first.width, first.height), (512, 256));
        let bytes = fs::read(&first.path).unwrap();
        assert_eq!(&bytes[..2], &[0xff, 0xd8]);

        let second = ensure_scaled_sidecar(&source, 512)
            .unwrap()
            .expect("reuse sidecar");
        assert_eq!(first.path, second.path);
    }

    #[test]
    fn pixel_art_uses_lossless_png_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("pixel-art.png");
        write_periodic_png(&source);

        let scaled = ensure_scaled_sidecar_with_mode(&source, 512, ImageScalingMode::Pixelated)
            .unwrap()
            .expect("scaled sidecar");
        assert_eq!(
            scaled.path.extension().and_then(|e| e.to_str()),
            Some("png")
        );
        assert_eq!(
            &fs::read(&scaled.path).unwrap()[..8],
            &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]
        );
    }

    #[test]
    fn smooth_mode_scales_screen_tone_with_interpolation() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("tone.png");
        write_periodic_png(&source);

        let scaled = ensure_scaled_sidecar_with_mode(&source, 512, ImageScalingMode::Smooth)
            .unwrap()
            .expect("scaled sidecar");
        // 축소 sidecar는 감지 결과와 무관하게 보간 필터를 쓰고 JPEG로 저장된다.
        // Nearest 축소는 주기 패턴을 계단·무아레로 깨뜨린다.
        assert_eq!(
            scaled.path.extension().and_then(|e| e.to_str()),
            Some("jpg")
        );
        assert_eq!((scaled.width, scaled.height), (512, 256));
    }

    #[test]
    fn smooth_mode_keeps_jpeg_for_pixel_art_source() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("pixel-art-smooth.png");
        write_periodic_png(&source);

        let scaled = ensure_scaled_sidecar_with_mode(&source, 512, ImageScalingMode::Smooth)
            .unwrap()
            .expect("scaled sidecar");
        assert_eq!(
            scaled.path.extension().and_then(|e| e.to_str()),
            Some("jpg")
        );
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
        write_jpeg(&source, 2048, 1024, 6);

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
    fn animated_png_is_left_alone() {
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/sample.apng");
        assert!(is_animated_png(&sample));
        // 확장자가 .png여도 내용(acTL)으로 판별한다.
        let dir = tempfile::tempdir().unwrap();
        let renamed = dir.path().join("anim.png");
        fs::copy(&sample, &renamed).unwrap();
        assert!(ensure_scaled_sidecar(&renamed, 512).unwrap().is_none());

        let still = dir.path().join("still.png");
        write_gradient_png(&still, 2000, 1000);
        assert!(!is_animated_png(&still));
    }

    #[test]
    fn static_webp_is_scaled() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("photo.png");
        write_gradient_png(&source, 2000, 1000);
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
    fn command_mode_defaults_to_safe_smooth() {
        assert_eq!(
            ImageScalingMode::from_command(Some("auto")),
            ImageScalingMode::Smooth
        );
        assert_eq!(
            ImageScalingMode::from_command(Some("pixelated")),
            ImageScalingMode::Pixelated
        );
        assert_eq!(
            ImageScalingMode::from_command(Some("unknown")),
            ImageScalingMode::Smooth
        );
        assert_eq!(
            ImageScalingMode::from_command(None),
            ImageScalingMode::Smooth
        );
    }

    #[test]
    fn max_side_is_clamped() {
        assert_eq!(clamp_max_side(1), MIN_MAX_SIDE);
        assert_eq!(clamp_max_side(100_000), MAX_MAX_SIDE);
        assert_eq!(clamp_max_side(3840), 3840);
    }
}
