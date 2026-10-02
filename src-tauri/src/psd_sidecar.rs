//! PSD(Photoshop) 미리보기용 JPEG sidecar.
//!
//! WebView2는 PSD를 네이티브 렌더할 수 없어 HEIC/HEIF와 같은 전제로
//! 합성(composite) 픽셀을 JPEG sidecar로 변환해 렌더한다 (`heif.rs` 패턴).
//! - 디코더: `psd` 크레이트(순수 Rust, 네이티브 의존성 없음).
//! - PSB(`8BPS` + version 2)는 미지원으로 명시 차단한다 (`psd` 크레이트도
//!   PSB 미지원).
//! - 편집 저장은 지원하지 않는다(읽기 전용). `save.rs`에서 진입 차단.
//!
//! `psd 0.3.5`의 합성 디코드에는 패닉 경로가 남아 있어(미구현 ZIP 압축,
//! 16비트/꼬리 바이트 raw의 범위 초과 쓰기) `catch_unwind`로 감싸 손상
//! 에러로 변환한다. 16비트는 파서 진입 전에 미지원으로 거른다.
//!
//! 파일 식별 해시, per-file 락, JPEG 원자적 발행, 다운스케일은
//! 공용 `sidecar` 모듈을 사용한다.

use std::fs;
use std::path::Path;

use crate::app_error::{AppError, ErrorCode};
use crate::sidecar::{Rgb8, SidecarSpec};

/// 디코드 허용 픽셀 상한 (약 150MP, HEIC와 동일).
/// PSD 명세상 최대(30000x30000)를 그대로 디코드하면 메모리 고갈이 난다.
const MAX_DECODE_PIXELS: u64 = 150_000_000;

/// 파일 전체를 메모리에 올리기 전에 거르는 크기 상한.
/// 150MP를 8비트 RGBA raw로 저장한 값(600MB)에 헤더·레이어 여유를 더했다.
const MAX_PSD_FILE_BYTES: u64 = 768 * 1024 * 1024;

pub const PSD_MIME: &str = "image/vnd.adobe.photoshop";

/// 이 디코더의 JPEG sidecar 규칙. `transcode`가 디코더별로 골라 쓴다.
pub(crate) static SPEC: SidecarSpec = SidecarSpec {
    label: "PSD sidecar lock",
    paint_prefix: "psd-",
    thumb_prefix: "psd-thumb-",
    decode: decode_psd_rgb8,
};

/// PSD 합성 픽셀을 RGB8로 디코드. 투명은 흰 배경에 합성한다
/// (JPEG에 알파가 없어 `save.rs` flatten과 같은 규칙).
pub(crate) fn decode_psd_rgb8(source: &Path) -> Result<Rgb8, AppError> {
    let meta = fs::metadata(source)
        .map_err(|e| AppError::io("Failed to read metadata", e, ErrorCode::Corrupt))?;
    if meta.len() > MAX_PSD_FILE_BYTES {
        return Err(AppError::too_large("PSD file is too large"));
    }
    let bytes =
        fs::read(source).map_err(|e| AppError::io("Failed to read file", e, ErrorCode::Corrupt))?;
    reject_psb(&bytes)?;
    // `psd` 크레이트 내부 패닉(미구현 ZIP composite, raw 채널 범위 초과)을
    // 커맨드 스레드 밖으로 내보내지 않고 손상 에러로 바꾼다.
    let decoded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let psd = psd::Psd::from_bytes(&bytes)
            .map_err(|e| AppError::corrupt(format!("Failed to decode image: {e}")))?;
        // 16비트 raw는 red만 8비트로 줄이고 green/blue/alpha는 2바이트/픽셀
        // 그대로 남겨 `rgba()`에서 범위를 넘는다. 진입 전에 거른다.
        if psd.depth() != psd::PsdDepth::Eight {
            return Err(AppError::unsupported("Only 8-bit PSD files are supported"));
        }
        let (width, height) = (psd.width(), psd.height());
        if u64::from(width).saturating_mul(u64::from(height)) > MAX_DECODE_PIXELS {
            return Err(AppError::too_large("PSD image dimensions are too large"));
        }
        Ok((width, height, psd.rgba()))
    }));
    let (width, height, rgba) = match decoded {
        Ok(result) => result?,
        Err(_) => {
            return Err(AppError::corrupt(
                "Failed to decode image: decoder panicked",
            ))
        }
    };
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4));
    if expected.is_none_or(|n| rgba.len() < n) {
        return Err(AppError::corrupt("PSD decode produced invalid buffer"));
    }
    let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
    for px in rgba.as_chunks::<4>().0 {
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

/// PSB(Large Document)는 PSD와 같은 `8BPS` 시그니처에 version 2를 쓴다.
/// `psd` 크레이트는 version 1만 파싱해 PSB를 "손상"으로 오보고하므로,
/// 파서 진입 전에 미지원으로 차단한다.
fn reject_psb(bytes: &[u8]) -> Result<(), AppError> {
    if bytes.len() >= 6 && &bytes[..4] == b"8BPS" && bytes[4..6] == [0, 2] {
        return Err(AppError::unsupported("PSB is not supported"));
    }
    Ok(())
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
    fn psb_header_is_rejected_as_unsupported() {
        // 실제 PSB는 `8BPS` 시그니처 + version 2다.
        let mut bytes = b"8BPS".to_vec();
        bytes.extend_from_slice(&2u16.to_be_bytes());
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
    fn psb_file_reports_unsupported_not_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("large.psd");
        let mut bytes = b"8BPS".to_vec();
        bytes.extend_from_slice(&2u16.to_be_bytes()); // version 2 = PSB
        bytes.extend_from_slice(&[0u8; 64]);
        fs::write(&source, bytes).unwrap();
        let err = decode_psd_rgb8(&source).unwrap_err();
        assert_eq!(err.code, crate::app_error::ErrorCode::Unsupported);
    }

    #[test]
    fn sixteen_bit_psd_is_rejected_as_unsupported() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("deep.psd");
        let mut bytes = minimal_psd_bytes(2, 1, [255, 0, 0]);
        // 헤더의 depth 필드(오프셋 22..24)를 16비트로 바꾼다.
        bytes[22..24].copy_from_slice(&16u16.to_be_bytes());
        fs::write(&source, bytes).unwrap();
        let err = decode_psd_rgb8(&source).unwrap_err();
        assert_eq!(err.code, crate::app_error::ErrorCode::Unsupported);
    }

    #[test]
    fn trailing_bytes_do_not_panic_the_decoder() {
        // 8비트 raw에서 채널 길이가 w*h를 넘으면 `psd` 0.3.5가 범위 초과
        // 쓰기로 패닉한다. catch_unwind가 Corrupt로 변환해야 한다.
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("trailing.psd");
        let mut bytes = minimal_psd_bytes(2, 1, [255, 0, 0]);
        bytes.extend_from_slice(&[0u8; 6]);
        fs::write(&source, bytes).unwrap();
        let err = decode_psd_rgb8(&source).unwrap_err();
        assert_eq!(err.code, crate::app_error::ErrorCode::Corrupt);
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
        let dest = SPEC.paint_path(&source).unwrap();
        assert_eq!(dest.extension().and_then(|e| e.to_str()), Some("jpg"));
        let name = dest.file_name().and_then(|n| n.to_str()).unwrap();
        assert!(name.starts_with("psd-"), "got {name}");
        let thumb = SPEC.thumb_path(&source, 64).unwrap();
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
