use std::path::{Path, PathBuf};

use crate::app_error::AppError;
use crate::sidecar::{paint_dir, Rgb8, MAX_PAINT_BYTES, PAINT_SUBDIR};

pub fn ensure_jpeg_sidecar(source: &Path) -> Result<PathBuf, AppError> {
    let dest = sidecar_path(source)?;
    let key = dest.to_string_lossy().into_owned();
    crate::sidecar::with_file_lock(&key, "HEIF sidecar lock", || {
        crate::process_temp::mark_in_use(&dest);
        if dest.exists() {
            crate::process_temp::touch_cache_file(&dest);
            return Ok(dest.clone());
        }
        let rgb = decode_primary_rgb8(source)?;
        let rgb = crate::sidecar::downscale_to_fit_u16(rgb);
        crate::sidecar::write_rgb8_jpeg_atomic(&dest, &rgb, 90)?;
        crate::process_temp::enforce_cap(PAINT_SUBDIR, MAX_PAINT_BYTES).ok();
        Ok(dest.clone())
    })
}

/// 썸네일용 경량 sidecar. 풀해상도 디코드 후 max_side로 다운스케일해
/// `paint/`에 별도 캐시한다. 스트립 N회 호출의 디코드 비용을 줄인다.
pub fn ensure_jpeg_sidecar_thumb(source: &Path, max_side: u32) -> Result<PathBuf, AppError> {
    let max_side = max_side.clamp(32, 1024);
    let dest = thumb_sidecar_path(source, max_side)?;
    let key = dest.to_string_lossy().into_owned();
    crate::sidecar::with_file_lock(&key, "HEIF sidecar lock", || {
        crate::process_temp::mark_in_use(&dest);
        if dest.exists() {
            crate::process_temp::touch_cache_file(&dest);
            return Ok(dest.clone());
        }
        let rgb = decode_primary_rgb8(source)?;
        let rgb = crate::sidecar::downscale_rgb8(rgb, max_side);
        let rgb = crate::sidecar::downscale_to_fit_u16(rgb);
        crate::sidecar::write_rgb8_jpeg_atomic(&dest, &rgb, 80)?;
        crate::process_temp::enforce_cap(PAINT_SUBDIR, MAX_PAINT_BYTES).ok();
        Ok(dest.clone())
    })
}

fn thumb_sidecar_path(source: &Path, max_side: u32) -> Result<PathBuf, AppError> {
    let hash = crate::sidecar::file_identity_hash(source, &max_side.to_le_bytes())?;
    let name = format!("thumb-{hash:016x}.jpg");
    Ok(paint_dir()?.join(name))
}

/// 캐시에 이미 있는 썸네일 sidecar 경로만 돌려준다(생성하지 않음).
pub fn cached_jpeg_sidecar_thumb(source: &Path, max_side: u32) -> Option<PathBuf> {
    let dest = thumb_sidecar_path(source, max_side.clamp(32, 1024)).ok()?;
    if !dest.exists() {
        return None;
    }
    crate::process_temp::touch_cache_file(&dest);
    Some(dest)
}

fn sidecar_path(source: &Path) -> Result<PathBuf, AppError> {
    let hash = crate::sidecar::file_identity_hash(source, &[])?;
    let name = format!("{hash:016x}.jpg");
    Ok(paint_dir()?.join(name))
}

/// 디코드 허용 픽셀 상한 (약 150MP). 비정상적으로 큰 HEIF로 인한
/// 메모리 고갈과 연산 폭주를 막는다.
const MAX_DECODE_PIXELS: u64 = 150_000_000;

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
        let dest = sidecar_path(&source).unwrap();
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
