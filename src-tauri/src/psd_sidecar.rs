//! PSD(Photoshop) 미리보기용 JPEG sidecar.
//!
//! WebView2는 PSD를 네이티브 렌더할 수 없어 HEIC/HEIF와 같은 전제로
//! 합성(composite) 픽셀을 JPEG sidecar로 변환해 렌더한다 (`heif.rs` 패턴).
//! - 디코더: `psd` 크레이트(순수 Rust, 네이티브 의존성 없음).
//! - PSB(`8BPB`)는 미지원으로 명시 차단한다 (`psd` 크레이트도 PSB 미지원).
//! - 편집 저장은 지원하지 않는다(읽기 전용). `save.rs`에서 진입 차단.
//!
//! 파일 식별 해시, per-file 락, JPEG 원자적 발행, 다운스케일은
//! 공용 `sidecar` 모듈을 사용한다.

use std::fs;
use std::path::{Path, PathBuf};

use crate::app_error::{AppError, ErrorCode};
use crate::sidecar::{paint_dir, Rgb8, MAX_PAINT_BYTES, PAINT_SUBDIR};

/// 디코드 허용 픽셀 상한 (약 150MP, HEIC와 동일).
/// PSD 명세상 최대(30000x30000)를 그대로 디코드하면 메모리 고갈이 난다.
const MAX_DECODE_PIXELS: u64 = 150_000_000;

pub const PSD_MIME: &str = "image/vnd.adobe.photoshop";

pub fn ensure_jpeg_sidecar(source: &Path) -> Result<PathBuf, AppError> {
    let dest = sidecar_path(source)?;
    let key = dest.to_string_lossy().into_owned();
    crate::sidecar::with_file_lock(&key, "PSD sidecar lock", || {
        crate::process_temp::mark_in_use(&dest);
        if dest.exists() {
            return Ok(dest.clone());
        }
        let rgb = decode_psd_rgb8(source)?;
        let rgb = crate::sidecar::downscale_to_fit_u16(rgb);
        crate::sidecar::write_rgb8_jpeg_atomic(&dest, &rgb, 90)?;
        crate::process_temp::enforce_cap(PAINT_SUBDIR, MAX_PAINT_BYTES).ok();
        Ok(dest.clone())
    })
}

/// 썸네일용 경량 sidecar. 풀해상도 디코드 후 max_side로 다운스케일해
/// `paint/`에 별도 캐시한다.
pub fn ensure_jpeg_sidecar_thumb(source: &Path, max_side: u32) -> Result<PathBuf, AppError> {
    let max_side = max_side.clamp(32, 1024);
    let dest = thumb_sidecar_path(source, max_side)?;
    let key = dest.to_string_lossy().into_owned();
    crate::sidecar::with_file_lock(&key, "PSD sidecar lock", || {
        crate::process_temp::mark_in_use(&dest);
        if dest.exists() {
            return Ok(dest.clone());
        }
        let rgb = decode_psd_rgb8(source)?;
        let rgb = crate::sidecar::downscale_rgb8(rgb, max_side);
        let rgb = crate::sidecar::downscale_to_fit_u16(rgb);
        crate::sidecar::write_rgb8_jpeg_atomic(&dest, &rgb, 80)?;
        crate::process_temp::enforce_cap(PAINT_SUBDIR, MAX_PAINT_BYTES).ok();
        Ok(dest.clone())
    })
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
    let hash = crate::sidecar::file_identity_hash(source, &max_side.to_le_bytes())?;
    let name = format!("psd-thumb-{hash:016x}.jpg");
    Ok(paint_dir()?.join(name))
}

fn sidecar_path(source: &Path) -> Result<PathBuf, AppError> {
    let hash = crate::sidecar::file_identity_hash(source, &[])?;
    let name = format!("psd-{hash:016x}.jpg");
    Ok(paint_dir()?.join(name))
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
        crate::sidecar::write_rgb8_jpeg_atomic(&dest, &rgb, 90).unwrap();
        assert!(dest.exists());
        let bytes = fs::read(&dest).unwrap();
        assert_eq!(&bytes[..2], &[0xFF, 0xD8]);
    }
}
