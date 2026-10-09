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
/// Changing the pixel-art downscale heuristic must not reuse older scaled copies.
/// v2: 축소 sidecar는 감지 결과와 무관하게 항상 보간 필터를 쓴다(nearest 축소는
/// 스크린톤 같은 주기 패턴을 깨뜨린다).
const PIXEL_ART_FILTER_REVISION: &[u8] = b"pixel-art-filter-v2";
/// Lower bound rejects accidental 1px requests; upper bound keeps the cap a
/// memory optimization, not a general preview downsizer.
const MIN_MAX_SIDE: u32 = 512;
const MAX_MAX_SIDE: u32 = 8192;

/// 표시 설정에 따라 sidecar를 만들 때 사용할 보간 정책이다.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageScalingMode {
    #[default]
    Smooth,
    Pixelated,
    Auto,
}

impl ImageScalingMode {
    /// Tauri command의 문자열 인자를 정규화한다. 알 수 없는 값은 안전한 smooth다.
    pub fn from_command(value: Option<&str>, auto_detect: bool) -> Self {
        match value {
            Some("pixelated") => Self::Pixelated,
            Some("auto") if auto_detect => Self::Auto,
            _ => Self::Smooth,
        }
    }

    fn cache_discriminator(self) -> u8 {
        match self {
            Self::Smooth => b's',
            Self::Pixelated => b'p',
            Self::Auto => b'a',
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
    match format {
        image::ImageFormat::Gif => return Ok(None),
        image::ImageFormat::WebP if is_animated_webp(source) => return Ok(None),
        image::ImageFormat::Png if is_animated_png(source) => return Ok(None),
        _ => {}
    }
    // EXIF orientation 5..=8 swaps the axes but not the long edge, so the cap
    // check is orientation-invariant.
    if width <= max_side && height <= max_side {
        return Ok(None);
    }

    let mut identity_extra = max_side.to_le_bytes().to_vec();
    identity_extra.push(mode.cache_discriminator());
    identity_extra.extend_from_slice(PIXEL_ART_FILTER_REVISION);
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
        // JPEG EXIF orientation을 픽셀에 반영한다. sidecar JPEG에는 EXIF가
        // 없으므로 WebView는 회전을 적용하지 않고, 반환 치수도 이 기준과 맞춘다.
        let img =
            crate::orientation::apply_to_image(img, crate::orientation::read_orientation(source));
        let has_alpha = img.color().has_alpha();
        // 축소(sidecar 생성)는 감지 결과와 무관하게 항상 보간 필터를 쓴다.
        // Nearest 축소는 스크린톤 같은 주기 패턴을 계단·무아레로 깨뜨린다. 픽셀
        // 아트 감지는 확대 표시 힌트로만 쓰인다(스무딩 여부는 프론트가 배율로 결정).
        // Auto 모드에서만 감지를 돌려 분석 결과를 후속 IPC 캐시에 남긴다.
        let detection = match mode {
            ImageScalingMode::Smooth | ImageScalingMode::Pixelated => None,
            ImageScalingMode::Auto => Some(crate::pixel_art::detect_decoded_image(&img)),
        };
        let is_pixel_art = matches!(mode, ImageScalingMode::Pixelated);
        let filter = image::imageops::FilterType::Triangle;
        let scaled = img.resize(max_side, max_side, filter);
        if has_alpha || is_pixel_art {
            // JPEG는 픽셀 경계에 다시 아티팩트를 만들 수 있으므로 픽셀 보존 모드는
            // 알파 여부와 무관하게 lossless PNG sidecar로 보존한다.
            let tmp = crate::sidecar::tmp_path_for(&png_dest);
            scaled
                .save_with_format(&tmp, image::ImageFormat::Png)
                .map_err(|e| AppError::unknown(format!("Failed to encode scaled PNG: {e}")))?;
            crate::sidecar::publish_atomic(&tmp, &png_dest, "Failed to publish scaled sidecar")?;
            if let Some(detection) = &detection {
                crate::pixel_art::cache_detection(&png_dest, detection);
            }
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
            if let Some(detection) = &detection {
                crate::pixel_art::cache_detection(&jpeg_dest, detection);
            }
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
