use std::path::Path;

use crate::app_error::AppError;
use crate::display::DecodedFrames;
use crate::sidecar::{Rgb8, SidecarSpec};

/// 이 디코더의 JPEG sidecar 규칙. `transcode`가 디코더별로 골라 쓴다.
pub(crate) static SPEC: SidecarSpec = SidecarSpec {
    label: "HEIF sidecar lock",
    paint_prefix: "",
    thumb_prefix: "thumb-",
    decode: decode_primary_rgb8,
};

/// 디코드 허용 픽셀 상한 (약 150MP). 비정상적으로 큰 HEIF로 인한
/// 메모리 고갈과 연산 폭주를 막는다.
const MAX_DECODE_PIXELS: u64 = 150_000_000;

/// 헤더만 읽어 기본 이미지 치수를 구한다. 픽셀 디코더가 없는 AVIF도 치수는 얻는다.
pub(crate) fn primary_dimensions(source: &Path) -> Option<(u32, u32)> {
    let ctx = libheif_rs::HeifContext::read_from_file(source.to_str()?).ok()?;
    let handle = ctx.primary_image_handle().ok()?;
    Some((handle.width(), handle.height()))
}

/// 인터리브 평면과 원본 ICC 프로파일(nclx만 있으면 `None`).
struct Interleaved {
    width: u32,
    height: u32,
    bytes: Vec<u8>,
    icc: Option<Vec<u8>>,
}

/// 기본 이미지를 인터리브 평면으로 디코드한다.
fn decode_interleaved(
    source: &Path,
    chroma: libheif_rs::RgbChroma,
    bytes_per_pixel: usize,
) -> Result<Interleaved, AppError> {
    use libheif_rs::{ColorSpace, HeifContext, LibHeif};

    let path = source
        .to_str()
        .ok_or_else(|| AppError::invalid_input("HEIF path is not valid Unicode"))?;
    let lib_heif = LibHeif::new();
    let ctx = HeifContext::read_from_file(path)
        .map_err(|e| AppError::corrupt(format!("Failed to decode image: {e}")))?;
    let handle = ctx
        .primary_image_handle()
        .map_err(|e| AppError::corrupt(format!("Failed to decode image: {e}")))?;
    if u64::from(handle.width()).saturating_mul(u64::from(handle.height())) > MAX_DECODE_PIXELS {
        return Err(AppError::too_large("HEIF image dimensions are too large"));
    }
    let icc = handle.color_profile_raw().map(|profile| profile.data);
    let image = lib_heif
        .decode(&handle, ColorSpace::Rgb(chroma), None)
        .map_err(|e| AppError::corrupt(format!("Failed to decode image: {e}")))?;
    let (width, height, bytes) = interleaved_bytes(&image, bytes_per_pixel)?;
    Ok(Interleaved {
        width,
        height,
        bytes,
        icc,
    })
}

/// 디코드된 이미지의 인터리브 평면을 꺼낸다. 행 패딩(stride)이 있으면 행 단위로 모은다.
fn interleaved_bytes(
    image: &libheif_rs::Image,
    bytes_per_pixel: usize,
) -> Result<(u32, u32, Vec<u8>), AppError> {
    let width = image.width();
    let height = image.height();
    let planes = image.planes();
    let plane = planes
        .interleaved
        .ok_or_else(|| AppError::corrupt("HEIF decode produced no interleaved plane"))?;
    let stride = plane.stride;
    let row_bytes = width as usize * bytes_per_pixel;
    let capacity = row_bytes
        .checked_mul(height as usize)
        .ok_or_else(|| AppError::corrupt("HEIF image dimensions overflow"))?;
    // 행 패딩이 없으면 평면 전체를 한 번에 복사한다.
    if stride == row_bytes {
        let bytes = plane
            .data
            .get(..capacity)
            .ok_or_else(|| AppError::corrupt("HEIF plane data is shorter than expected"))?
            .to_vec();
        return Ok((width, height, bytes));
    }
    let mut bytes = Vec::with_capacity(capacity);
    for y in 0..height as usize {
        let start = y
            .checked_mul(stride)
            .ok_or_else(|| AppError::corrupt("HEIF plane stride overflow"))?;
        let end = start
            .checked_add(row_bytes)
            .ok_or_else(|| AppError::corrupt("HEIF plane stride overflow"))?;
        let row = plane
            .data
            .get(start..end)
            .ok_or_else(|| AppError::corrupt("HEIF plane data is shorter than expected"))?;
        bytes.extend_from_slice(row);
    }
    Ok((width, height, bytes))
}

pub(crate) fn decode_primary_rgb8(source: &Path) -> Result<Rgb8, AppError> {
    let Interleaved {
        width,
        height,
        bytes,
        ..
    } = decode_interleaved(source, libheif_rs::RgbChroma::Rgb, 3)?;
    Ok(Rgb8 {
        width,
        height,
        bytes,
    })
}

/// 클립보드 복사용: 알파 채널을 보존해 RGBA로 디코드하고 ICC 프로파일을 함께
/// 돌려준다. 컨테이너 변환(irot/imir)은 libheif가 반영하므로 별도의 EXIF 회전은
/// 적용하지 않는다.
pub(crate) fn decode_primary_rgba(
    source: &Path,
) -> Result<(image::RgbaImage, Option<Vec<u8>>), AppError> {
    let decoded = decode_interleaved(source, libheif_rs::RgbChroma::Rgba, 4)?;
    let image = image::RgbaImage::from_raw(decoded.width, decoded.height, decoded.bytes)
        .ok_or_else(|| AppError::corrupt("HEIF decode produced invalid buffer"))?;
    Ok((image, decoded.icc))
}

/// 표시용 디코드. 이미지 시퀀스(움직이는 AVIF)면 첫 영상 트랙의 프레임을 모두
/// 디코드하되, 전체 RGBA 크기가 `max_animation_bytes`를 넘으면 첫 프레임만
/// 돌려준다. 시퀀스가 없으면 기본 이미지 한 장이다.
pub(crate) fn decode_frames(
    source: &Path,
    max_animation_bytes: u64,
) -> Result<DecodedFrames, AppError> {
    let frames = decode_sequence(source, max_animation_bytes)?;
    if !frames.is_empty() {
        return Ok(DecodedFrames {
            frames,
            // 반복 횟수(edit list)는 바인딩이 읽지 못해 무한 반복으로 둔다.
            loop_count: None,
        });
    }
    let (image, _icc) = decode_primary_rgba(source)?;
    Ok(DecodedFrames {
        frames: vec![(image, 0)],
        loop_count: None,
    })
}

/// 시퀀스 트랙의 프레임과 표시 시간(ms). 시퀀스가 없으면 비어 있다.
fn decode_sequence(
    source: &Path,
    max_animation_bytes: u64,
) -> Result<Vec<(image::RgbaImage, u32)>, AppError> {
    use libheif_rs::{ColorSpace, DecodingOptions, HeifContext, HeifErrorCode, RgbChroma};

    let decode_error = |e| AppError::corrupt(format!("Failed to decode image: {e}"));
    let path = source
        .to_str()
        .ok_or_else(|| AppError::invalid_input("HEIF path is not valid Unicode"))?;
    let ctx = HeifContext::read_from_file(path).map_err(decode_error)?;
    // id 0은 첫 영상 트랙이다.
    let Some(track) = Some(&ctx)
        .filter(|ctx| ctx.has_sequence())
        .and_then(|ctx| ctx.track(0))
    else {
        return Ok(Vec::new());
    };
    let resolution = track.image_resolution().map_err(decode_error)?;
    let frame_pixels = u64::from(resolution.width) * u64::from(resolution.height);
    if frame_pixels > MAX_DECODE_PIXELS {
        return Err(AppError::too_large("HEIF image dimensions are too large"));
    }
    let frame_bytes = frame_pixels * 4;
    let timescale = u64::from(track.timescale().max(1));
    let mut frames: Vec<(image::RgbaImage, u32)> = Vec::new();
    let mut total_bytes = 0u64;
    loop {
        // edit list를 따르면 무한 반복 파일에서 시퀀스가 끝나지 않는다. 미디어
        // 타임라인을 한 번만 읽고 반복은 재생 쪽에 맡긴다.
        let mut options = DecodingOptions::new()
            .ok_or_else(|| AppError::corrupt("Failed to allocate HEIF decoding options"))?;
        options.set_ignore_sequence_edit_list(true);
        let image = match track.decode_next_image(ColorSpace::Rgb(RgbChroma::Rgba), Some(options)) {
            Ok(image) => image,
            Err(e) if matches!(e.code, HeifErrorCode::EndOfSequence) => break,
            // 뒤쪽 프레임만 깨진 파일은 읽은 데까지 재생한다.
            Err(_) if !frames.is_empty() => break,
            Err(e) => return Err(decode_error(e)),
        };
        total_bytes = total_bytes.saturating_add(frame_bytes);
        if total_bytes > max_animation_bytes && !frames.is_empty() {
            frames.truncate(1);
            break;
        }
        let delay_ms = u64::from(image.duration()) * 1000 / timescale;
        let (width, height, bytes) = interleaved_bytes(&image, 4)?;
        let frame = image::RgbaImage::from_raw(width, height, bytes)
            .ok_or_else(|| AppError::corrupt("HEIF decode produced invalid buffer"))?;
        // 재생 경로는 프레임 크기가 모두 같다고 본다.
        if frames
            .first()
            .is_some_and(|(first, _)| first.dimensions() != frame.dimensions())
        {
            break;
        }
        frames.push((frame, u32::try_from(delay_ms).unwrap_or(u32::MAX)));
    }
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples")
            .join(name)
    }

    #[test]
    fn animated_avif_decodes_every_frame() {
        let decoded = decode_frames(&sample("sample-animated.avif"), u64::MAX).unwrap();
        assert_eq!(decoded.frames.len(), 12);
        assert!(decoded
            .frames
            .iter()
            .all(|(frame, delay_ms)| frame.dimensions() == (400, 300) && *delay_ms == 100));
        assert_ne!(decoded.frames[0].0, decoded.frames[11].0);
    }

    #[test]
    fn animated_avif_over_budget_keeps_first_frame() {
        let decoded = decode_frames(&sample("sample-animated.avif"), 400 * 300 * 4 * 3).unwrap();
        assert_eq!(decoded.frames.len(), 1);
    }

    #[test]
    fn still_avif_is_single_frame() {
        let decoded = decode_frames(&sample("sample.avif"), u64::MAX).unwrap();
        assert_eq!(decoded.frames.len(), 1);
    }

    #[test]
    fn animated_avif_primary_image_decodes() {
        let (image, _icc) = decode_primary_rgba(&sample("sample-animated.avif")).unwrap();
        assert_eq!(image.dimensions(), (400, 300));
    }
}
