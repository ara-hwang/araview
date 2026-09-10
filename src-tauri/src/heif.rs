use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

use jpeg_encoder::{ColorType, Encoder};
use libheif_rs::{ColorSpace, HeifContext, LibHeif, RgbChroma};
use once_cell::sync::Lazy;

use crate::app_error::{AppError, ErrorCode};
use crate::process_temp::process_temp_dir;

static SIDECAR_LOCKS: Lazy<Mutex<HashMap<String, Arc<Mutex<()>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub(crate) struct Rgb8 {
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

pub fn ensure_jpeg_sidecar(source: &Path) -> Result<PathBuf, AppError> {
    let dest = sidecar_path(source)?;
    let key = dest
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("sidecar")
        .to_string();
    let lock = {
        let mut map = SIDECAR_LOCKS
            .lock()
            .map_err(|_| AppError::lock_poisoned("HEIF sidecar locks"))?;
        map.entry(key)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    let _guard = lock
        .lock()
        .map_err(|_| AppError::lock_poisoned("HEIF sidecar lock"))?;
    if dest.exists() {
        return Ok(dest);
    }
    let rgb = decode_primary_rgb8(source)?;
    let rgb = downscale_to_fit_u16(rgb);
    write_jpeg_atomic(&dest, &rgb, 90)?;
    Ok(dest)
}

/// 썸네일용 경량 sidecar. 풀해상도 디코드 후 max_side로 다운스케일해
/// `paint/`에 별도 캐시한다. 스트립 N회 호출의 디코드 비용을 줄인다.
pub fn ensure_jpeg_sidecar_thumb(source: &Path, max_side: u32) -> Result<PathBuf, AppError> {
    let max_side = max_side.clamp(32, 1024);
    let dest = thumb_sidecar_path(source, max_side)?;
    if dest.exists() {
        return Ok(dest);
    }
    let rgb = decode_primary_rgb8(source)?;
    let rgb = downscale_rgb8(rgb, max_side);
    let rgb = downscale_to_fit_u16(rgb);
    write_jpeg_atomic(&dest, &rgb, 80)?;
    Ok(dest)
}

fn thumb_sidecar_path(source: &Path, max_side: u32) -> Result<PathBuf, AppError> {
    let canonical = fs::canonicalize(source).unwrap_or_else(|_| source.to_path_buf());
    let meta = fs::metadata(source)
        .map_err(|e| AppError::io("Failed to read metadata", e, ErrorCode::Corrupt))?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    canonical.hash(&mut hasher);
    mtime.hash(&mut hasher);
    meta.len().hash(&mut hasher);
    max_side.hash(&mut hasher);
    let name = format!("thumb-{:016x}.jpg", hasher.finish());
    let dir = process_temp_dir()?.join("paint");
    fs::create_dir_all(&dir)
        .map_err(|e| AppError::io("Failed to create paint dir", e, ErrorCode::Unknown))?;
    Ok(dir.join(name))
}

/// `max_side` 안에 들어가도록 비율 유지 다운스케일 (업스케일 없음).
fn downscale_rgb8(rgb: Rgb8, max_side: u32) -> Rgb8 {
    let max_side = max_side.max(1);
    if rgb.width <= max_side && rgb.height <= max_side {
        return rgb;
    }
    let scale = (max_side as f64 / rgb.width.max(rgb.height) as f64).min(1.0);
    let nw = ((rgb.width as f64 * scale).round() as u32).max(1);
    let nh = ((rgb.height as f64 * scale).round() as u32).max(1);
    resize_rgb8(&rgb, nw, nh)
}

/// JPEG 한계(65535)를 넘으면 비율 유지로 축소. 파노라마 HEIC 대응.
fn downscale_to_fit_u16(rgb: Rgb8) -> Rgb8 {
    const LIMIT: u32 = 65500;
    if rgb.width <= LIMIT && rgb.height <= LIMIT {
        return rgb;
    }
    let scale = (LIMIT as f64 / rgb.width.max(rgb.height) as f64).min(1.0);
    let nw = ((rgb.width as f64 * scale).floor() as u32).max(1);
    let nh = ((rgb.height as f64 * scale).floor() as u32).max(1);
    resize_rgb8(&rgb, nw, nh)
}

fn resize_rgb8(rgb: &Rgb8, nw: u32, nh: u32) -> Rgb8 {
    let src_w = rgb.width.max(1);
    let src_h = rgb.height.max(1);
    let expected = src_w as usize * src_h as usize * 3;
    if rgb.bytes.len() < expected || nw == 0 || nh == 0 {
        return Rgb8 {
            width: rgb.width,
            height: rgb.height,
            bytes: rgb.bytes.clone(),
        };
    }
    let img: image::RgbImage = match image::RgbImage::from_raw(src_w, src_h, rgb.bytes.clone()) {
        Some(v) => v,
        None => {
            return Rgb8 {
                width: rgb.width,
                height: rgb.height,
                bytes: rgb.bytes.clone(),
            }
        }
    };
    let resized = image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle);
    Rgb8 {
        width: nw,
        height: nh,
        bytes: resized.into_raw(),
    }
}

fn sidecar_path(source: &Path) -> Result<PathBuf, AppError> {
    let canonical = fs::canonicalize(source).unwrap_or_else(|_| source.to_path_buf());
    let meta = fs::metadata(source)
        .map_err(|e| AppError::io("Failed to read metadata", e, ErrorCode::Corrupt))?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    canonical.hash(&mut hasher);
    mtime.hash(&mut hasher);
    meta.len().hash(&mut hasher);
    let name = format!("{:016x}.jpg", hasher.finish());
    let dir = process_temp_dir()?.join("paint");
    fs::create_dir_all(&dir)
        .map_err(|e| AppError::io("Failed to create paint dir", e, ErrorCode::Unknown))?;
    Ok(dir.join(name))
}

/// 디코드 허용 픽셀 상한 (약 150MP). 비정상적으로 큰 HEIF로 인한
/// 메모리 고갈과 연산 폭주를 막는다.
const MAX_DECODE_PIXELS: u64 = 150_000_000;

pub(crate) fn decode_primary_rgb8(source: &Path) -> Result<Rgb8, AppError> {
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

fn write_jpeg_atomic(dest: &Path, rgb: &Rgb8, quality: u8) -> Result<(), AppError> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AppError::io("Failed to create paint dir", e, ErrorCode::Unknown))?;
    }
    let file_name = dest
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("sidecar.jpg");
    let tmp = dest.with_file_name(format!("{}.tmp-{}", file_name, std::process::id()));
    let width = u16::try_from(rgb.width)
        .map_err(|_| AppError::corrupt("image too wide for JPEG sidecar"))?;
    let height = u16::try_from(rgb.height)
        .map_err(|_| AppError::corrupt("image too tall for JPEG sidecar"))?;
    {
        let encoder = Encoder::new_file(&tmp, quality)
            .map_err(|e| AppError::unknown(format!("Failed to open JPEG sidecar: {e}")))?;
        encoder
            .encode(&rgb.bytes, width, height, ColorType::Rgb)
            .map_err(|e| AppError::unknown(format!("Failed to encode JPEG sidecar: {e}")))?;
    }
    match fs::rename(&tmp, dest) {
        Ok(()) => Ok(()),
        Err(_) if dest.exists() => {
            let _ = fs::remove_file(&tmp);
            Ok(())
        }
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(AppError::unknown(format!(
                "Failed to publish JPEG sidecar: {e}"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        write_jpeg_atomic(&dest, &rgb, 90).unwrap();
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
        let small = downscale_rgb8(rgb, 200);
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
        let fit = downscale_to_fit_u16(rgb);
        assert!(fit.width <= 65500 && fit.height <= 65500);
        assert_eq!(fit.width, 65500);
    }
}
