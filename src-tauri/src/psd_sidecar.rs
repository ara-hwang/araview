//! PSD(Photoshop) 미리보기용 JPEG sidecar.
//!
//! WebView2는 PSD를 네이티브 렌더할 수 없어 HEIC/HEIF와 같은 전제로
//! 합성(composite) 픽셀을 JPEG sidecar로 변환해 렌더한다 (`heif.rs` 패턴).
//! - 디코더: `psd` 크레이트(순수 Rust, 네이티브 의존성 없음).
//! - PSB(`8BPB`)는 미지원으로 명시 차단한다 (`psd` 크레이트도 PSB 미지원).
//! - 편집 저장은 지원하지 않는다(읽기 전용). `save.rs`에서 진입 차단.

use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::UNIX_EPOCH;

use jpeg_encoder::{ColorType, Encoder};

use crate::app_error::{AppError, ErrorCode};
use crate::process_temp::process_temp_dir;

static SIDECAR_LOCKS: LazyLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug)]
pub(crate) struct Rgb8 {
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

/// 디코드 허용 픽셀 상한 (약 150MP, HEIC와 동일).
/// PSD 명세상 최대(30000x30000)를 그대로 디코드하면 메모리 고갈이 난다.
const MAX_DECODE_PIXELS: u64 = 150_000_000;

pub const PSD_MIME: &str = "image/vnd.adobe.photoshop";

pub fn ensure_jpeg_sidecar(source: &Path) -> Result<PathBuf, AppError> {
    let dest = sidecar_path(source)?;
    let key = dest
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("psd-sidecar")
        .to_string();
    let lock = {
        let mut map = SIDECAR_LOCKS
            .lock()
            .map_err(|_| AppError::lock_poisoned("PSD sidecar locks"))?;
        map.entry(key)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    let _guard = lock
        .lock()
        .map_err(|_| AppError::lock_poisoned("PSD sidecar lock"))?;
    if dest.exists() {
        return Ok(dest);
    }
    let rgb = decode_psd_rgb8(source)?;
    let rgb = downscale_to_fit_u16(rgb);
    write_jpeg_atomic(&dest, &rgb, 90)?;
    Ok(dest)
}

/// 썸네일용 경량 sidecar. 풀해상도 디코드 후 max_side로 다운스케일해
/// `paint/`에 별도 캐시한다.
pub fn ensure_jpeg_sidecar_thumb(source: &Path, max_side: u32) -> Result<PathBuf, AppError> {
    let max_side = max_side.clamp(32, 1024);
    let dest = thumb_sidecar_path(source, max_side)?;
    if dest.exists() {
        return Ok(dest);
    }
    let rgb = decode_psd_rgb8(source)?;
    let rgb = downscale_rgb8(rgb, max_side);
    let rgb = downscale_to_fit_u16(rgb);
    write_jpeg_atomic(&dest, &rgb, 80)?;
    Ok(dest)
}

/// PSD 합성 픽셀을 RGB8로 디코드. 투명은 흰 배경에 합성한다
/// (JPEG에 알파가 없어 `save.rs` flatten과 같은 규칙).
pub(crate) fn decode_psd_rgb8(source: &Path) -> Result<Rgb8, AppError> {
    let bytes =
        fs::read(source).map_err(|e| AppError::io("Failed to read file", e, ErrorCode::Corrupt))?;
    reject_psb(&bytes)?;
    let psd = psd::Psd::from_bytes(&bytes)
        .map_err(|e| AppError::corrupt(format!("Failed to decode image: {e}")))?;
    let (width, height) = (psd.width(), psd.height());
    if u64::from(width).saturating_mul(u64::from(height)) > MAX_DECODE_PIXELS {
        return Err(AppError::too_large("PSD image dimensions are too large"));
    }
    let rgba = psd.rgba();
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4));
    if expected.is_none_or(|n| rgba.len() < n) {
        return Err(AppError::corrupt("PSD decode produced invalid buffer"));
    }
    let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
    for px in rgba.chunks_exact(4) {
        let a = f32::from(px[3]) / 255.0;
        let blend = |c: u8| (f32::from(c) * a + 255.0 * (1.0 - a)).round() as u8;
        rgb.extend_from_slice(&[blend(px[0]), blend(px[1]), blend(px[2])]);
    }
    Ok(Rgb8 {
        width,
        height,
        bytes: rgb,
    })
}

/// PSB(Large Document, `8BPB`)는 파서 진입 전에 차단한다.
/// `psd` 크레이트도 PSB를 지원하지 않아 하위 호환 파싱 시도가 없다.
fn reject_psb(bytes: &[u8]) -> Result<(), AppError> {
    if bytes.len() >= 4 && &bytes[..4] == b"8BPB" {
        return Err(AppError::unsupported("PSB is not supported"));
    }
    Ok(())
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
    let name = format!("psd-thumb-{:016x}.jpg", hasher.finish());
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

/// JPEG 한계(65535)를 넘으면 비율 유지로 축소.
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
    let name = format!("psd-{:016x}.jpg", hasher.finish());
    let dir = process_temp_dir()?.join("paint");
    fs::create_dir_all(&dir)
        .map_err(|e| AppError::io("Failed to create paint dir", e, ErrorCode::Unknown))?;
    Ok(dir.join(name))
}

fn write_jpeg_atomic(dest: &Path, rgb: &Rgb8, quality: u8) -> Result<(), AppError> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AppError::io("Failed to create paint dir", e, ErrorCode::Unknown))?;
    }
    let file_name = dest
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("psd-sidecar.jpg");
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

/// 테스트용 최소 PSD 바이트 (w*h RGB, raw 압축, 레이어 없음).
/// 헤더 + 빈 섹션 + Image Data로만 구성한다.
#[cfg(test)]
pub(crate) fn minimal_psd_bytes(width: u32, height: u32, px: [u8; 3]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"8BPS"); // signature
    out.extend_from_slice(&1u16.to_be_bytes()); // version
    out.extend_from_slice(&[0u8; 6]); // reserved
    out.extend_from_slice(&3u16.to_be_bytes()); // channels
    out.extend_from_slice(&height.to_be_bytes());
    out.extend_from_slice(&width.to_be_bytes());
    out.extend_from_slice(&8u16.to_be_bytes()); // depth
    out.extend_from_slice(&3u16.to_be_bytes()); // color mode: RGB
    out.extend_from_slice(&0u32.to_be_bytes()); // color mode data len
    out.extend_from_slice(&0u32.to_be_bytes()); // image resources len
    out.extend_from_slice(&0u32.to_be_bytes()); // layer & mask len
    out.extend_from_slice(&0u16.to_be_bytes()); // compression: raw
    let n = width as usize * height as usize;
    for _ in 0..n {
        out.push(px[0]);
    }
    for _ in 0..n {
        out.push(px[1]);
    }
    for _ in 0..n {
        out.push(px[2]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn psb_signature_is_rejected_as_unsupported() {
        let mut bytes = b"8BPB".to_vec();
        bytes.extend_from_slice(&[0u8; 64]);
        let err = reject_psb(&bytes).unwrap_err();
        assert_eq!(err.code, crate::app_error::ErrorCode::Unsupported);
        assert_eq!(err.message, "PSB is not supported");
    }

    #[test]
    fn psd_signature_passes_psb_guard() {
        let bytes = minimal_psd_bytes(2, 2, [10, 20, 30]);
        assert!(reject_psb(&bytes).is_ok());
    }

    #[test]
    fn minimal_psd_decodes_to_rgb8() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("mini.psd");
        fs::write(&source, minimal_psd_bytes(2, 1, [255, 0, 0])).unwrap();
        let rgb = decode_psd_rgb8(&source).expect("decode minimal PSD");
        assert_eq!((rgb.width, rgb.height), (2, 1));
        assert_eq!(rgb.bytes, vec![255, 0, 0, 255, 0, 0]);
    }

    #[test]
    fn garbage_psd_is_corrupt_not_unsupported() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("broken.psd");
        fs::write(&source, b"8BPS-not-really-a-psd").unwrap();
        let err = decode_psd_rgb8(&source).unwrap_err();
        assert_eq!(err.code, crate::app_error::ErrorCode::Corrupt);
    }

    #[test]
    fn sidecar_paths_are_namespaced_psd() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("a.psd");
        fs::write(&source, minimal_psd_bytes(1, 1, [1, 2, 3])).unwrap();
        let dest = sidecar_path(&source).unwrap();
        assert_eq!(dest.extension().and_then(|e| e.to_str()), Some("jpg"));
        let name = dest.file_name().and_then(|n| n.to_str()).unwrap();
        assert!(name.starts_with("psd-"), "got {name}");
        let thumb = thumb_sidecar_path(&source, 64).unwrap();
        let thumb_name = thumb.file_name().and_then(|n| n.to_str()).unwrap();
        assert!(thumb_name.starts_with("psd-thumb-"), "got {thumb_name}");
    }

    #[test]
    fn write_jpeg_atomic_publishes_dest_not_tmp() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("out.jpg");
        let rgb = Rgb8 {
            width: 1,
            height: 1,
            bytes: vec![0, 128, 255],
        };
        write_jpeg_atomic(&dest, &rgb, 90).unwrap();
        assert!(dest.exists());
        let bytes = fs::read(&dest).unwrap();
        assert_eq!(&bytes[..2], &[0xFF, 0xD8]);
    }
}
