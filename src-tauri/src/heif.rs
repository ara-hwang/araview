use std::path::Path;

use crate::app_error::AppError;
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

pub(crate) fn decode_primary_rgb8(source: &Path) -> Result<Rgb8, AppError> {
    use libheif_rs::{ColorSpace, HeifContext, LibHeif, RgbChroma};

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
    let image = lib_heif
        .decode(&handle, ColorSpace::Rgb(RgbChroma::Rgb), None)
        .map_err(|e| AppError::corrupt(format!("Failed to decode image: {e}")))?;
    let width = image.width();
    let height = image.height();
    let planes = image.planes();
    let plane = planes
        .interleaved
        .ok_or_else(|| AppError::corrupt("HEIF decode produced no interleaved RGB plane"))?;
    let stride = plane.stride;
    let row_bytes = width as usize * 3;
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
        return Ok(Rgb8 {
            width,
            height,
            bytes,
        });
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
    Ok(Rgb8 {
        width,
        height,
        bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn sidecar_path_is_under_paint_and_jpg() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("IMG_0001.heic");
        fs::write(&source, b"placeholder").unwrap();
        let dest = SPEC.paint_path(&source).unwrap();
        assert_eq!(dest.extension().and_then(|e| e.to_str()), Some("jpg"));
        assert_eq!(
            dest.parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str()),
            Some("paint")
        );
        assert_ne!(dest.parent(), Some(source.parent().unwrap()));
    }

    #[test]
    fn write_jpeg_atomic_publishes_dest_not_tmp() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("out.jpg");
        let rgb = Rgb8 {
            width: 1,
            height: 1,
            bytes: vec![255, 0, 0],
        };
        crate::sidecar::write_rgb8_jpeg_atomic(&dest, &rgb, 90).unwrap();
        assert!(dest.exists());
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name())
            .filter(|n| n.to_string_lossy().contains(".tmp-"))
            .collect();
        assert!(leftovers.is_empty());
        let bytes = fs::read(&dest).unwrap();
        assert_eq!(&bytes[..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn downscale_rgb8_bounds_long_side() {
        let rgb = Rgb8 {
            width: 800,
            height: 400,
            bytes: vec![0u8; 800 * 400 * 3],
        };
        let small = crate::sidecar::downscale_rgb8(rgb, 200);
        assert_eq!((small.width, small.height), (200, 100));
    }

    #[test]
    fn downscale_to_fit_u16_clamps_pano() {
        let w: u32 = 70000;
        let h: u32 = 2;
        let rgb = Rgb8 {
            width: w,
            height: h,
            bytes: vec![0u8; w as usize * h as usize * 3],
        };
        let fit = crate::sidecar::downscale_to_fit_u16(rgb);
        assert!(fit.width <= 65500 && fit.height <= 65500);
        assert_eq!(fit.width, 65500);
    }
}
