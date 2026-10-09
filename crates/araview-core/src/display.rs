//! 렌더 바이트를 화면에 그릴 RGBA 프레임으로 디코드한다. 웹뷰가 없는 호스트
//! (GPUI)가 쓴다. 판별과 회전 기준은 클립보드 복사([`crate::clipboard_png`])와
//! 같아서 표시와 복사가 같은 내용을 만든다.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use image::{AnimationDecoder as _, ImageDecoder as _};

use crate::app_error::{AppError, ErrorCode};

/// 디코드 한 번에 허용하는 픽셀 수와 할당 상한. 클립보드 경로와 같은 값이다.
const MAX_DECODE_PIXELS: u64 = 100_000_000;
const MAX_DECODE_ALLOC: u64 = 1024 * 1024 * 1024;
/// 애니메이션 전체 프레임이 차지할 수 있는 RGBA 바이트 상한. 넘으면 첫 프레임만 쓴다.
const MAX_ANIMATION_BYTES: u64 = 512 * 1024 * 1024;
/// 프레임 지연이 0이거나 너무 짧을 때 브라우저가 쓰는 기본값과 같은 100ms.
const DEFAULT_FRAME_DELAY_MS: u32 = 100;
const MIN_FRAME_DELAY_MS: u32 = 20;

pub struct DisplayFrame {
    pub image: image::RgbaImage,
    pub delay_ms: u32,
}

pub struct DisplayImage {
    pub frames: Vec<DisplayFrame>,
    /// 유한 반복 횟수. `None`이면 무한 반복이다.
    pub loop_count: Option<u32>,
    /// 확대해도 다시 래스터화할 수 있는 벡터 원본인지.
    pub is_vector: bool,
}

impl DisplayImage {
    fn still(image: image::RgbaImage, is_vector: bool) -> Self {
        Self {
            frames: vec![DisplayFrame {
                image,
                delay_ms: DEFAULT_FRAME_DELAY_MS,
            }],
            loop_count: None,
            is_vector,
        }
    }

    pub fn is_animated(&self) -> bool {
        self.frames.len() > 1
    }
}

/// `path`의 렌더 바이트를 디코드한다. SVG는 긴 변이 `svg_max_side`를 넘지 않게
/// 래스터화한다.
pub fn decode_for_display(path: &Path, svg_max_side: u32) -> Result<DisplayImage, AppError> {
    let mime = crate::image::resolve_mime(path);
    match mime {
        Some("image/avif" | "image/heic" | "image/heif") => {
            let (image, _icc) = crate::heif::decode_primary_rgba(path)?;
            Ok(DisplayImage::still(image, false))
        }
        Some("image/svg+xml") => Ok(DisplayImage::still(
            crate::svg_raster::rasterize_rgba(path, svg_max_side)?,
            true,
        )),
        Some(mime)
            if mime == crate::psd_sidecar::PSD_MIME || crate::transcode::is_raster_mime(mime) =>
        {
            let decoder = crate::transcode::decoder_for(path)
                .ok_or_else(|| AppError::unsupported("Unsupported image format"))?;
            let rgb = crate::transcode::decode_rgb8(path, decoder)?;
            let img = image::RgbImage::from_raw(rgb.width, rgb.height, rgb.bytes)
                .ok_or_else(|| AppError::corrupt("Decoded image dimensions are invalid"))?;
            Ok(DisplayImage::still(
                image::DynamicImage::ImageRgb8(img).into_rgba8(),
                false,
            ))
        }
        Some("image/gif") => decode_animation(path, AnimationKind::Gif),
        Some("image/apng") => decode_animation(path, AnimationKind::Apng),
        Some("image/webp") => decode_animation(path, AnimationKind::WebP),
        mime => decode_still(path, mime),
    }
}

fn open_reader(path: &Path) -> Result<BufReader<File>, AppError> {
    File::open(path)
        .map(BufReader::new)
        .map_err(|e| AppError::io("Failed to open image", e, ErrorCode::Corrupt))
}

fn check_pixel_budget(width: u32, height: u32) -> Result<(), AppError> {
    if u64::from(width).saturating_mul(u64::from(height)) > MAX_DECODE_PIXELS {
        return Err(AppError::too_large("Image dimensions are too large"));
    }
    Ok(())
}

fn decode_still(path: &Path, mime: Option<&str>) -> Result<DisplayImage, AppError> {
    let mut reader = image::ImageReader::open(path)
        .map_err(|e| AppError::io("Failed to open image", e, ErrorCode::Corrupt))?
        .with_guessed_format()
        .map_err(|e| AppError::io("Failed to guess image format", e, ErrorCode::Corrupt))?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    reader.limits(limits);
    let decoder = reader
        .into_decoder()
        .map_err(|e| AppError::image_error("Failed to decode image", e))?;
    let (width, height) = decoder.dimensions();
    check_pixel_budget(width, height)?;
    let image = image::DynamicImage::from_decoder(decoder)
        .map_err(|e| AppError::image_error("Failed to decode image", e))?;
    // 회전은 표시 치수와 같은 기준(JPEG)으로만 반영한다.
    let image = if mime.is_some_and(crate::image::is_exif_orientation_format) {
        crate::orientation::apply_to_image(image, crate::orientation::read_orientation(path))
    } else {
        image
    };
    Ok(DisplayImage::still(image.into_rgba8(), false))
}

#[derive(Clone, Copy)]
enum AnimationKind {
    Gif,
    Apng,
    WebP,
}

/// 움직이는 포맷을 프레임 단위로 디코드한다. 정지 이미지이거나 프레임 예산을
/// 넘으면 정지 디코드로 내려간다.
fn decode_animation(path: &Path, kind: AnimationKind) -> Result<DisplayImage, AppError> {
    let (frames, loops) = match kind {
        AnimationKind::Gif => {
            let decoder = image::codecs::gif::GifDecoder::new(open_reader(path)?)
                .map_err(|e| AppError::image_error("Failed to decode image", e))?;
            let (width, height) = decoder.dimensions();
            check_pixel_budget(width, height)?;
            let loops = decoder.loop_count();
            (collect_frames(decoder.into_frames(), width, height), loops)
        }
        AnimationKind::Apng => {
            let decoder = image::codecs::png::PngDecoder::new(open_reader(path)?)
                .map_err(|e| AppError::image_error("Failed to decode image", e))?;
            let (width, height) = decoder.dimensions();
            check_pixel_budget(width, height)?;
            if !decoder
                .is_apng()
                .map_err(|e| AppError::image_error("Failed to decode image", e))?
            {
                return decode_still(path, Some("image/png"));
            }
            let apng = decoder
                .apng()
                .map_err(|e| AppError::image_error("Failed to decode image", e))?;
            let loops = apng.loop_count();
            (collect_frames(apng.into_frames(), width, height), loops)
        }
        AnimationKind::WebP => {
            let decoder = image::codecs::webp::WebPDecoder::new(open_reader(path)?)
                .map_err(|e| AppError::image_error("Failed to decode image", e))?;
            if !decoder.has_animation() {
                return decode_still(path, Some("image/webp"));
            }
            let (width, height) = decoder.dimensions();
            check_pixel_budget(width, height)?;
            let loops = decoder.loop_count();
            (collect_frames(decoder.into_frames(), width, height), loops)
        }
    };
    match frames {
        Some(frames) if !frames.is_empty() => Ok(DisplayImage {
            frames,
            // 유한 반복은 마지막 회차에서 멈춘다.
            loop_count: match loops {
                image::metadata::LoopCount::Finite(count) => Some(count.get()),
                image::metadata::LoopCount::Infinite => None,
            },
            is_vector: false,
        }),
        _ => decode_still(path, None),
    }
}

/// 프레임을 끝까지 모은다. 예산을 넘거나 중간 프레임이 깨졌으면 `None`이다.
fn collect_frames(frames: image::Frames<'_>, width: u32, height: u32) -> Option<Vec<DisplayFrame>> {
    let frame_bytes = u64::from(width) * u64::from(height) * 4;
    let mut total = 0u64;
    let mut out = Vec::new();
    for frame in frames {
        let frame = match frame {
            Ok(frame) => frame,
            // 뒤쪽 프레임만 깨진 파일은 읽은 데까지 재생한다.
            Err(_) if !out.is_empty() => break,
            Err(_) => return None,
        };
        total = total.saturating_add(frame_bytes);
        if total > MAX_ANIMATION_BYTES {
            return None;
        }
        let (numer, denom) = frame.delay().numer_denom_ms();
        let delay_ms = numer.checked_div(denom).unwrap_or(0);
        let delay_ms = if delay_ms < MIN_FRAME_DELAY_MS {
            DEFAULT_FRAME_DELAY_MS
        } else {
            delay_ms
        };
        out.push(DisplayFrame {
            image: frame.into_buffer(),
            delay_ms,
        });
    }
    Some(out)
}
