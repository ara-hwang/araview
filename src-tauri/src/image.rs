use image::ImageEncoder;
use libheif_rs::{ColorSpace, HeifContext, LibHeif, RgbChroma};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
pub struct ImageInfo {
    pub base64: String,
    pub mime_type: String,
    pub file_name: String,
    pub file_size: u64,
}

#[derive(Serialize)]
pub struct DirectoryImages {
    pub images: Vec<String>,
    pub current_index: usize,
}

pub fn get_mime_type(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "bmp" => Some("image/bmp"),
        "webp" => Some("image/webp"),
        "svg" => Some("image/svg+xml"),
        "ico" => Some("image/x-icon"),
        "tiff" | "tif" => Some("image/tiff"),
        "avif" => Some("image/avif"),
        "heic" | "heif" => Some("image/heic"),
        _ => None,
    }
}

pub fn is_image_file(path: &Path) -> bool {
    get_mime_type(path).is_some()
}

/// Returns `true` for HEIC/HEIF files that need server-side conversion.
pub fn is_heif_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .as_deref(),
        Some("heic" | "heif")
    )
}

/// Decode a HEIF/HEIC file to JPEG bytes so that any WebView can display it.
pub fn heif_to_jpeg(data: &[u8]) -> Result<Vec<u8>, String> {
    let lib_heif = LibHeif::new();
    let ctx = HeifContext::read_from_bytes(data)
        .map_err(|e| format!("Failed to read HEIF data: {e}"))?;
    let handle = ctx
        .primary_image_handle()
        .map_err(|e| format!("Failed to get primary HEIF image: {e}"))?;

    let src_img = lib_heif
        .decode(&handle, ColorSpace::Rgb(RgbChroma::Rgb), None)
        .map_err(|e| format!("Failed to decode HEIF image: {e}"))?;

    let plane = src_img
        .planes()
        .interleaved
        .ok_or("Failed to decode HEIF image: no interleaved RGB plane available")?;

    let width = src_img.width();
    let height = src_img.height();

    // Re-pack rows without stride padding so the JPEG encoder gets a clean buffer.
    let row_bytes = (width as usize) * 3;
    let mut rgb: Vec<u8> = Vec::with_capacity(row_bytes * height as usize);
    for row in 0..height as usize {
        let start = row * plane.stride;
        rgb.extend_from_slice(&plane.data[start..start + row_bytes]);
    }

    let mut jpeg_buf: Vec<u8> = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_buf, 90);
    encoder
        .write_image(&rgb, width, height, image::ExtendedColorType::Rgb8)
        .map_err(|e| format!("Failed to encode HEIF as JPEG: {e}"))?;

    Ok(jpeg_buf)
}
