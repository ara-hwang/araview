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
        _ => None,
    }
}

pub fn is_image_file(path: &Path) -> bool {
    if path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("cbz"))
        .unwrap_or(false)
    {
        return true;
    }
    get_mime_type(path).is_some()
}
