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
