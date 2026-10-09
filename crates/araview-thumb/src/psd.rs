//! PSD composite decode for the thumbnail handler.
//!
//! Mirrors the main app's `psd_sidecar::decode_psd_rgb8` over in-memory
//! bytes (the shell gives us an `IStream`, not a path). Transparent
//! pixels are flattened onto white, matching the app's JPEG sidecar rule.
//!
//! `psd 0.3.5`의 합성 디코드에는 패닉 경로가 남아 있어(미구현 ZIP 압축,
//! 16비트/꼬리 바이트 raw의 범위 초과 쓰기) `catch_unwind`로 감싼다.
//! Explorer의 썸네일 호스트 안에서 FFI 경계를 넘어 패닉이 새면 안 된다.

/// Decode budget (~150MP, same as the app). The PSD spec max
/// (30000x30000) decoded naively would exhaust the surrogate.
pub const MAX_DECODE_PIXELS: u64 = 150_000_000;

#[derive(Debug, PartialEq, Eq)]
pub struct Rgb8 {
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PsdError {
    /// PSB (`8BPS` version 2) or a bit depth the decoder does not support.
    UnsupportedFile,
    /// Truncated header, unknown compression, short buffer, oversize, ...
    CorruptFile,
}

impl std::fmt::Display for PsdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PsdError::UnsupportedFile => write!(f, "Unsupported PSD file"),
            PsdError::CorruptFile => write!(f, "Failed to decode image"),
        }
    }
}

/// Decode PSD composite pixels to RGB8, flattening alpha onto white.
pub fn decode_psd_rgb8(bytes: &[u8]) -> Result<Rgb8, PsdError> {
    // PSB는 PSD와 같은 `8BPS` 시그니처에 version 2를 쓴다. `psd` 크레이트는
    // version 1만 파싱해 PSB를 손상으로 오보고하므로 여기서 먼저 거른다.
    if bytes.len() >= 6 && &bytes[..4] == b"8BPS" && bytes[4..6] == [0, 2] {
        return Err(PsdError::UnsupportedFile);
    }
    // 내부 패닉을 손상 에러로 바꿔 FFI 경계 밖으로 내보내지 않는다.
    let decoded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let psd = psd::Psd::from_bytes(bytes).map_err(|_| PsdError::CorruptFile)?;
        // 16비트 raw는 red만 8비트로 줄고 green/blue/alpha는 2바이트/픽셀
        // 그대로 남아 `rgba()`에서 범위를 넘는다.
        if psd.depth() != psd::PsdDepth::Eight {
            return Err(PsdError::UnsupportedFile);
        }
        let (width, height) = (psd.width(), psd.height());
        if u64::from(width).saturating_mul(u64::from(height)) > MAX_DECODE_PIXELS {
            return Err(PsdError::CorruptFile);
        }
        Ok((width, height, psd.rgba()))
    }));
    let (width, height, rgba) = match decoded {
        Ok(result) => result?,
        Err(_) => return Err(PsdError::CorruptFile),
    };
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4));
    if expected.is_none_or(|n| rgba.len() < n) {
        return Err(PsdError::CorruptFile);
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

/// Fit `(width, height)` inside a `cx` square, preserving aspect ratio.
/// Never upscales: small sources are returned as-is.
pub fn fit_inside(width: u32, height: u32, cx: u32) -> (u32, u32) {
    let (width, height, cx) = (width.max(1), height.max(1), cx.max(1));
    let longest = width.max(height);
    if longest <= cx {
        return (width, height);
    }
    let scale = cx as f64 / longest as f64;
    let nw = ((width as f64 * scale).round() as u32).max(1);
    let nh = ((height as f64 * scale).round() as u32).max(1);
    (nw, nh)
}

/// Downscale RGB8 with a bilinear filter (thumbnail sizes look fine).
pub fn resize_rgb8(rgb: &Rgb8, nw: u32, nh: u32) -> Rgb8 {
    let src_w = rgb.width.max(1);
    let src_h = rgb.height.max(1);
    if rgb.bytes.len() < src_w as usize * src_h as usize * 3 || nw == 0 || nh == 0 {
        return Rgb8 {
            width: rgb.width,
            height: rgb.height,
            bytes: rgb.bytes.clone(),
        };
    }
    let Some(img) = image::RgbImage::from_raw(src_w, src_h, rgb.bytes.clone()) else {
        return Rgb8 {
            width: rgb.width,
            height: rgb.height,
            bytes: rgb.bytes.clone(),
        };
    };
    let resized = image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle);
    Rgb8 {
        width: nw,
        height: nh,
        bytes: resized.into_raw(),
    }
}
