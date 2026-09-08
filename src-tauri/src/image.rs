use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize, Debug)]
pub struct ImageInfo {
    /// Bytes the WebView can decode. May be a JPEG sidecar, not the listing path.
    pub file_path: String,
    pub mime_type: String,
    pub file_name: String,
    pub file_size: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PaintStrategy {
    Native,
    TranscodeJpeg,
}

const TRANSCODE_MIMES: &[&str] = &["image/heic", "image/heif"];

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
        "heic" => Some("image/heic"),
        "heif" => Some("image/heif"),
        "cbz" => Some("application/vnd.comicbook+zip"),
        _ => None,
    }
}

pub fn paint_strategy(mime: &str) -> PaintStrategy {
    if TRANSCODE_MIMES.contains(&mime) {
        PaintStrategy::TranscodeJpeg
    } else {
        PaintStrategy::Native
    }
}

pub fn image_info(source: &Path, paint_path: PathBuf) -> Result<ImageInfo, String> {
    let mime_type = get_mime_type(source)
        .ok_or_else(|| "Unsupported image format".to_string())?
        .to_string();
    let metadata =
        std::fs::metadata(source).map_err(|e| format!("Failed to read metadata: {}", e))?;
    let file_name = source
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    Ok(ImageInfo {
        file_path: paint_path.to_string_lossy().to_string(),
        mime_type,
        file_name,
        file_size: metadata.len(),
    })
}

pub fn load_viewable(source: &Path) -> Result<ImageInfo, String> {
    if !source.exists() {
        return Err("File not found".to_string());
    }
    let mime = get_mime_type(source).ok_or_else(|| "Unsupported image format".to_string())?;
    let paint_path = match paint_strategy(mime) {
        PaintStrategy::Native => source.to_path_buf(),
        PaintStrategy::TranscodeJpeg => crate::heif::ensure_jpeg_sidecar(source)?,
    };
    image_info(source, paint_path)
}

pub fn is_image_file(path: &Path) -> bool {
    match get_mime_type(path) {
        Some(mime) => !mime.starts_with("application/"),
        None => false,
    }
}

pub fn is_archive_file(path: &Path) -> bool {
    matches!(get_mime_type(path), Some("application/vnd.comicbook+zip"))
}

/// 순수 이미지이거나 아카이브인 경우 모두 지원 파일로 간주
pub fn is_supported_file(path: &Path) -> bool {
    is_image_file(path) || is_archive_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_mime_type_png() {
        assert_eq!(get_mime_type(Path::new("photo.png")), Some("image/png"));
    }

    #[test]
    fn test_get_mime_type_jpg() {
        assert_eq!(get_mime_type(Path::new("photo.jpg")), Some("image/jpeg"));
    }

    #[test]
    fn test_get_mime_type_jpeg() {
        assert_eq!(get_mime_type(Path::new("photo.jpeg")), Some("image/jpeg"));
    }

    #[test]
    fn test_get_mime_type_gif() {
        assert_eq!(get_mime_type(Path::new("anim.gif")), Some("image/gif"));
    }

    #[test]
    fn test_get_mime_type_bmp() {
        assert_eq!(get_mime_type(Path::new("image.bmp")), Some("image/bmp"));
    }

    #[test]
    fn test_get_mime_type_webp() {
        assert_eq!(get_mime_type(Path::new("image.webp")), Some("image/webp"));
    }

    #[test]
    fn test_get_mime_type_svg() {
        assert_eq!(get_mime_type(Path::new("icon.svg")), Some("image/svg+xml"));
    }

    #[test]
    fn test_get_mime_type_ico() {
        assert_eq!(
            get_mime_type(Path::new("favicon.ico")),
            Some("image/x-icon")
        );
    }

    #[test]
    fn test_get_mime_type_tiff() {
        assert_eq!(get_mime_type(Path::new("scan.tiff")), Some("image/tiff"));
    }

    #[test]
    fn test_get_mime_type_tif() {
        assert_eq!(get_mime_type(Path::new("scan.tif")), Some("image/tiff"));
    }

    #[test]
    fn test_get_mime_type_avif() {
        assert_eq!(get_mime_type(Path::new("photo.avif")), Some("image/avif"));
    }

    #[test]
    fn test_get_mime_type_heic() {
        assert_eq!(
            get_mime_type(Path::new("IMG_0001.heic")),
            Some("image/heic")
        );
        assert_eq!(
            get_mime_type(Path::new("IMG_0001.HEIC")),
            Some("image/heic")
        );
    }

    #[test]
    fn test_get_mime_type_heif() {
        assert_eq!(get_mime_type(Path::new("photo.heif")), Some("image/heif"));
        assert_eq!(get_mime_type(Path::new("photo.HEIF")), Some("image/heif"));
    }

    #[test]
    fn test_get_mime_type_cbz() {
        assert_eq!(
            get_mime_type(Path::new("comic.cbz")),
            Some("application/vnd.comicbook+zip")
        );
    }

    #[test]
    fn test_get_mime_type_unknown_extension() {
        assert_eq!(get_mime_type(Path::new("doc.pdf")), None);
        assert_eq!(get_mime_type(Path::new("file.txt")), None);
        assert_eq!(get_mime_type(Path::new("archive.zip")), None);
    }

    #[test]
    fn test_get_mime_type_no_extension() {
        assert_eq!(get_mime_type(Path::new("README")), None);
    }

    #[test]
    fn test_get_mime_type_case_insensitive() {
        assert_eq!(get_mime_type(Path::new("PHOTO.PNG")), Some("image/png"));
        assert_eq!(get_mime_type(Path::new("image.JPG")), Some("image/jpeg"));
        assert_eq!(get_mime_type(Path::new("icon.Svg")), Some("image/svg+xml"));
    }

    #[test]
    fn test_get_mime_type_nested_path() {
        assert_eq!(
            get_mime_type(Path::new("/home/user/photos/vacation.png")),
            Some("image/png")
        );
    }

    #[test]
    fn test_is_image_file_valid() {
        assert!(is_image_file(Path::new("photo.png")));
        assert!(is_image_file(Path::new("photo.jpg")));
        assert!(is_image_file(Path::new("anim.gif")));
        assert!(is_image_file(Path::new("photo.avif")));
        assert!(is_image_file(Path::new("IMG_0001.heic")));
        assert!(is_image_file(Path::new("photo.heif")));
    }

    #[test]
    fn test_is_image_file_invalid() {
        assert!(!is_image_file(Path::new("doc.pdf")));
        assert!(!is_image_file(Path::new("README")));
        assert!(!is_image_file(Path::new("script.js")));
        assert!(!is_image_file(Path::new("style.css")));
        assert!(!is_image_file(Path::new("comic.cbz")));
    }

    #[test]
    fn test_is_archive_file() {
        assert!(is_archive_file(Path::new("comic.cbz")));
        assert!(is_archive_file(Path::new("comic.CBZ")));
        assert!(!is_archive_file(Path::new("photo.png")));
        assert!(!is_archive_file(Path::new("archive.zip")));
    }

    #[test]
    fn test_is_supported_file() {
        assert!(is_supported_file(Path::new("photo.png")));
        assert!(is_supported_file(Path::new("comic.cbz")));
        assert!(!is_supported_file(Path::new("doc.pdf")));
    }

    #[test]
    fn test_is_image_file_case_insensitive() {
        assert!(is_image_file(Path::new("PHOTO.PNG")));
        assert!(is_image_file(Path::new("image.JPEG")));
    }

    #[test]
    fn paint_strategy_transcodes_heic_heif_only() {
        assert_eq!(paint_strategy("image/heic"), PaintStrategy::TranscodeJpeg);
        assert_eq!(paint_strategy("image/heif"), PaintStrategy::TranscodeJpeg);
        assert_eq!(paint_strategy("image/png"), PaintStrategy::Native);
        assert_eq!(paint_strategy("image/jpeg"), PaintStrategy::Native);
        assert_eq!(paint_strategy("image/avif"), PaintStrategy::Native);
        assert_eq!(
            paint_strategy("application/vnd.comicbook+zip"),
            PaintStrategy::Native
        );
    }

    #[test]
    fn image_info_takes_identity_from_source_and_path_from_paint() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("IMG_0001.heic");
        let paint = dir.path().join("sidecar.jpg");
        std::fs::write(&source, b"not-a-real-heic").unwrap();
        let info = image_info(&source, paint.clone()).unwrap();
        assert_eq!(info.mime_type, "image/heic");
        assert_eq!(info.file_name, "IMG_0001.heic");
        assert_eq!(info.file_size, 15);
        assert_eq!(info.file_path, paint.to_string_lossy());
    }

    #[test]
    fn load_viewable_native_keeps_source_as_paint_path() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("photo.png");
        std::fs::write(&source, MIN_PNG).unwrap();
        let info = load_viewable(&source).unwrap();
        assert_eq!(info.mime_type, "image/png");
        assert_eq!(info.file_name, "photo.png");
        assert_eq!(info.file_size, MIN_PNG.len() as u64);
        assert_eq!(info.file_path, source.to_string_lossy());
    }

    #[test]
    fn load_viewable_missing_file() {
        let err = load_viewable(Path::new("missing-file.png")).unwrap_err();
        assert_eq!(err, "File not found");
    }

    #[test]
    fn load_viewable_unsupported_extension() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("notes.txt");
        std::fs::write(&source, b"hello").unwrap();
        let err = load_viewable(&source).unwrap_err();
        assert_eq!(err, "Unsupported image format");
    }

    #[test]
    fn load_viewable_heic_transcodes_when_fixture_exists() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures");
        let source = ["sample.heic", "sample.heif"]
            .into_iter()
            .map(|name| fixtures.join(name))
            .find(|p| p.exists());
        let Some(source) = source else {
            return;
        };
        let first = load_viewable(&source).expect("decode HEIC fixture");
        assert!(first.mime_type == "image/heic" || first.mime_type == "image/heif");
        assert_eq!(
            first.file_name,
            source.file_name().and_then(|n| n.to_str()).unwrap()
        );
        assert_ne!(first.file_path, source.to_string_lossy());
        assert!(Path::new(&first.file_path).exists());
        let jpeg = std::fs::read(&first.file_path).unwrap();
        assert_eq!(&jpeg[..2], &[0xFF, 0xD8]);
        let again = load_viewable(&source).expect("reuse sidecar");
        assert_eq!(again.file_path, first.file_path);
    }

    const MIN_PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
        0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8,
        0xCF, 0xC0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x00, 0x05, 0xFE, 0xD4, 0xEF, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
}
