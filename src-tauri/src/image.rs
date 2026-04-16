use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
pub struct ImageInfo {
    pub file_path: String,
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
        "cbz" => Some("application/vnd.comicbook+zip"),
        _ => None,
    }
}

pub fn is_image_file(path: &Path) -> bool {
    match get_mime_type(path) {
        Some(mime) => !mime.starts_with("application/"),
        None => false,
    }
}

pub fn is_archive_file(path: &Path) -> bool {
    matches!(
        get_mime_type(path),
        Some("application/vnd.comicbook+zip")
    )
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
        assert_eq!(
            get_mime_type(Path::new("icon.svg")),
            Some("image/svg+xml")
        );
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
}
