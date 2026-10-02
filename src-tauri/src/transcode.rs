//! Dispatch from a source file to its JPEG-sidecar decoder.
//!
//! Callers (loader, thumbnails, histogram, details) used to repeat the
//! HEIC-vs-PSD choice; adding a format meant touching each of them. They go
//! through here instead, and a new decoder only needs a [`Decoder`] variant.

use std::path::{Path, PathBuf};

use crate::app_error::AppError;
use crate::sidecar::{Rgb8, SidecarSpec};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Decoder {
    Heif,
    Psd,
    Raster,
}

/// Decoder for the file, by MIME. AVIF is decoded through libheif for
/// thumbnails and histograms even though WebView2 paints it natively.
pub fn decoder_for(path: &Path) -> Option<Decoder> {
    match crate::image::resolve_mime(path)? {
        "image/heic" | "image/heif" | "image/avif" => Some(Decoder::Heif),
        crate::psd_sidecar::PSD_MIME => Some(Decoder::Psd),
        mime if is_raster_mime(mime) => Some(Decoder::Raster),
        _ => None,
    }
}

/// TGA/DDS/EXR: formats the `image` crate decodes but WebView2 cannot paint.
pub const RASTER_MIMES: &[&str] = &["image/x-tga", "image/vnd.ms-dds", "image/x-exr"];

pub fn is_raster_mime(mime: &str) -> bool {
    RASTER_MIMES.contains(&mime)
}

fn spec_for(decoder: Decoder) -> &'static SidecarSpec {
    match decoder {
        Decoder::Heif => &crate::heif::SPEC,
        Decoder::Psd => &crate::psd_sidecar::SPEC,
        Decoder::Raster => &crate::raster_sidecar::SPEC,
    }
}

/// Full-size paint sidecar for a MIME whose strategy is `TranscodeJpeg`.
pub fn ensure_paint(source: &Path, mime: &str) -> Result<PathBuf, AppError> {
    let decoder = match mime {
        crate::psd_sidecar::PSD_MIME => Decoder::Psd,
        mime if is_raster_mime(mime) => Decoder::Raster,
        _ => Decoder::Heif,
    };
    spec_for(decoder).ensure(source)
}

/// 썸네일용 경량 sidecar. 풀해상도 디코드 후 max_side로 다운스케일해
/// `paint/`에 별도 캐시한다. 스트립 N회 호출의 디코드 비용을 줄인다.
pub fn ensure_thumb(source: &Path, decoder: Decoder, max_side: u32) -> Result<PathBuf, AppError> {
    spec_for(decoder).ensure_thumb(source, max_side)
}

/// 캐시에 이미 있는 썸네일 sidecar 경로만 돌려준다(생성하지 않음).
pub fn cached_thumb(source: &Path, decoder: Decoder, max_side: u32) -> Option<PathBuf> {
    spec_for(decoder).cached_thumb(source, max_side)
}

pub fn decode_rgb8(source: &Path, decoder: Decoder) -> Result<Rgb8, AppError> {
    (spec_for(decoder).decode)(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoder_follows_mime() {
        assert_eq!(decoder_for(Path::new("a.heic")), Some(Decoder::Heif));
        assert_eq!(decoder_for(Path::new("a.AVIF")), Some(Decoder::Heif));
        assert_eq!(decoder_for(Path::new("a.psd")), Some(Decoder::Psd));
        assert_eq!(decoder_for(Path::new("a.TGA")), Some(Decoder::Raster));
        assert_eq!(decoder_for(Path::new("a.dds")), Some(Decoder::Raster));
        assert_eq!(decoder_for(Path::new("a.exr")), Some(Decoder::Raster));
        assert_eq!(decoder_for(Path::new("a.png")), None);
    }
}
