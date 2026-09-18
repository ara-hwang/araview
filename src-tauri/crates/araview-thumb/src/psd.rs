//! PSD composite decode for the thumbnail handler.
//!
//! Mirrors the main app's `psd_sidecar::decode_psd_rgb8` over in-memory
//! bytes (the shell gives us an `IStream`, not a path). Transparent
//! pixels are flattened onto white, matching the app's JPEG sidecar rule.

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
    /// PSB (`8BPB`) large documents: no decoder, entry blocked.
    UnsupportedFile,
    /// Truncated header, unknown compression, short buffer, oversize, ...
    CorruptFile,
}

impl std::fmt::Display for PsdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PsdError::UnsupportedFile => write!(f, "PSB is not supported"),
            PsdError::CorruptFile => write!(f, "Failed to decode image"),
        }
    }
}

/// Decode PSD composite pixels to RGB8, flattening alpha onto white.
pub fn decode_psd_rgb8(bytes: &[u8]) -> Result<Rgb8, PsdError> {
    if bytes.len() >= 4 && &bytes[..4] == b"8BPB" {
        return Err(PsdError::UnsupportedFile);
    }
    let psd = psd::Psd::from_bytes(bytes).map_err(|_| PsdError::CorruptFile)?;
    let (width, height) = (psd.width(), psd.height());
    if u64::from(width).saturating_mul(u64::from(height)) > MAX_DECODE_PIXELS {
        return Err(PsdError::CorruptFile);
    }
    let rgba = psd.rgba();
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

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Minimal PSD bytes (w*h RGB, raw compression, no layers).
    /// Same layout as the main app's test fixture.
    pub(crate) fn minimal_psd_bytes(width: u32, height: u32, px: [u8; 3]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"8BPS");
        out.extend_from_slice(&1u16.to_be_bytes());
        out.extend_from_slice(&[0u8; 6]);
        out.extend_from_slice(&3u16.to_be_bytes());
        out.extend_from_slice(&height.to_be_bytes());
        out.extend_from_slice(&width.to_be_bytes());
        out.extend_from_slice(&8u16.to_be_bytes());
        out.extend_from_slice(&3u16.to_be_bytes());
        out.extend_from_slice(&0u32.to_be_bytes());
        out.extend_from_slice(&0u32.to_be_bytes());
        out.extend_from_slice(&0u32.to_be_bytes());
        out.extend_from_slice(&0u16.to_be_bytes());
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

    #[test]
    fn minimal_psd_decodes_to_rgb8() {
        let rgb = decode_psd_rgb8(&minimal_psd_bytes(2, 1, [255, 0, 0])).expect("decode");
        assert_eq!((rgb.width, rgb.height), (2, 1));
        assert_eq!(rgb.bytes, vec![255, 0, 0, 255, 0, 0]);
    }

    #[test]
    fn psb_signature_is_unsupported() {
        let mut bytes = b"8BPB".to_vec();
        bytes.extend_from_slice(&[0u8; 64]);
        assert_eq!(decode_psd_rgb8(&bytes), Err(PsdError::UnsupportedFile));
    }

    #[test]
    fn garbage_is_corrupt() {
        assert_eq!(
            decode_psd_rgb8(b"8BPS-not-really-a-psd"),
            Err(PsdError::CorruptFile)
        );
    }

    #[test]
    fn fit_inside_preserves_aspect_and_never_upscales() {
        assert_eq!(fit_inside(200, 100, 64), (64, 32));
        assert_eq!(fit_inside(100, 200, 64), (32, 64));
        assert_eq!(fit_inside(32, 32, 64), (32, 32));
        assert_eq!(fit_inside(64, 64, 64), (64, 64));
    }
}
