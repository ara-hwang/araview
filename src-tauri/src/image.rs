use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::app_error::{AppError, ErrorCode};

#[derive(Serialize, Debug)]
pub struct ImageInfo {
    /// Bytes the WebView can decode. May be a JPEG sidecar, not the listing path.
    pub file_path: String,
    pub mime_type: String,
    pub file_name: String,
    pub file_size: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PaintStrategy {
    Native,
    TranscodeJpeg,
}

const TRANSCODE_MIMES: &[&str] = &["image/heic", "image/heif", crate::psd_sidecar::PSD_MIME];

pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico", "tiff", "tif", "avif", "heic",
    "heif", "psd", "cbz", "cb7", "cbr", "rar", "zip", "7z", "cbt",
];

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
        "psd" => Some(crate::psd_sidecar::PSD_MIME),
        "cbz" => Some("application/vnd.comicbook+zip"),
        "zip" => Some("application/zip"),
        "cb7" | "7z" => Some("application/x-7z-compressed"),
        "cbr" => Some("application/vnd.comicbook-rar"),
        "rar" => Some("application/x-rar-compressed"),
        "cbt" => Some("application/x-tar"),
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

/// 같은 폴더 내 새 파일명에 대한 공통 검증. trim된 이름을 반환
pub fn validate_new_file_name(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::invalid_input("File name is empty"));
    }
    if trimmed.contains(['/', '\\']) {
        return Err(AppError::invalid_input(
            "File name cannot contain path separators",
        ));
    }
    if trimmed.contains(['<', '>', ':', '"', '|', '?', '*']) {
        return Err(AppError::invalid_input(
            "File name contains invalid characters",
        ));
    }
    if trimmed.ends_with([' ', '.']) {
        return Err(AppError::invalid_input(
            "File name cannot end with a space or dot",
        ));
    }
    Ok(trimmed.to_string())
}

pub fn image_info(source: &Path, paint_path: PathBuf) -> Result<ImageInfo, AppError> {
    let mime_type = get_mime_type(source)
        .ok_or_else(|| AppError::unsupported("Unsupported image format"))?
        .to_string();
    let metadata = std::fs::metadata(source)
        .map_err(|e| AppError::io("Failed to read metadata", e, ErrorCode::Corrupt))?;
    let file_name = source
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    // Rendered bytes 기준 치수. SVG 등 image 크레이트 미지원분은 None으로 생략.
    let (width, height) = image::image_dimensions(&paint_path).ok().unzip();
    Ok(ImageInfo {
        file_path: paint_path.to_string_lossy().to_string(),
        mime_type,
        file_name,
        file_size: metadata.len(),
        width,
        height,
    })
}

pub fn load_viewable(source: &Path) -> Result<ImageInfo, AppError> {
    if !source.exists() {
        return Err(AppError::not_found("File not found"));
    }
    let mime =
        get_mime_type(source).ok_or_else(|| AppError::unsupported("Unsupported image format"))?;
    let paint_path = match paint_strategy(mime) {
        PaintStrategy::Native => source.to_path_buf(),
        PaintStrategy::TranscodeJpeg if mime == crate::psd_sidecar::PSD_MIME => {
            crate::psd_sidecar::ensure_jpeg_sidecar(source)?
        }
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
    matches!(
        get_mime_type(path),
        Some(
            "application/vnd.comicbook+zip"
                | "application/zip"
                | "application/x-7z-compressed"
                | "application/vnd.comicbook-rar"
                | "application/x-rar-compressed"
                | "application/x-tar"
        )
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
    fn test_get_mime_type_psd() {
        assert_eq!(
            get_mime_type(Path::new("design.psd")),
            Some("image/vnd.adobe.photoshop")
        );
        assert_eq!(
            get_mime_type(Path::new("design.PSD")),
            Some("image/vnd.adobe.photoshop")
        );
    }

    #[test]
    fn test_get_mime_type_cbz() {
        assert_eq!(
            get_mime_type(Path::new("comic.cbz")),
            Some("application/vnd.comicbook+zip")
        );
    }

    #[test]
    fn test_get_mime_type_cb7() {
        assert_eq!(
            get_mime_type(Path::new("comic.cb7")),
            Some("application/x-7z-compressed")
        );
        assert_eq!(
            get_mime_type(Path::new("comic.CB7")),
            Some("application/x-7z-compressed")
        );
    }

    #[test]
    fn test_get_mime_type_new_archives() {
        assert_eq!(
            get_mime_type(Path::new("comic.cbr")),
            Some("application/vnd.comicbook-rar")
        );
        assert_eq!(
            get_mime_type(Path::new("archive.rar")),
            Some("application/x-rar-compressed")
        );
        assert_eq!(
            get_mime_type(Path::new("archive.zip")),
            Some("application/zip")
        );
        assert_eq!(
            get_mime_type(Path::new("archive.7z")),
            Some("application/x-7z-compressed")
        );
        assert_eq!(
            get_mime_type(Path::new("comic.cbt")),
            Some("application/x-tar")
        );
    }

    #[test]
    fn test_get_mime_type_unknown_extension() {
        assert_eq!(get_mime_type(Path::new("doc.pdf")), None);
        assert_eq!(get_mime_type(Path::new("file.txt")), None);
        assert_eq!(get_mime_type(Path::new("archive.tar.gz")), None);
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
        assert!(is_image_file(Path::new("design.psd")));
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
        assert!(is_archive_file(Path::new("comic.cb7")));
        assert!(is_archive_file(Path::new("comic.CB7")));
        assert!(is_archive_file(Path::new("comic.cbr")));
        assert!(is_archive_file(Path::new("archive.rar")));
        assert!(is_archive_file(Path::new("archive.zip")));
        assert!(is_archive_file(Path::new("archive.7z")));
        assert!(is_archive_file(Path::new("comic.cbt")));
        assert!(!is_archive_file(Path::new("photo.png")));
    }

    #[test]
    fn test_is_supported_file() {
        assert!(is_supported_file(Path::new("photo.png")));
        assert!(is_supported_file(Path::new("comic.cbz")));
        assert!(is_supported_file(Path::new("comic.cb7")));
        assert!(is_supported_file(Path::new("comic.cbr")));
        assert!(is_supported_file(Path::new("comic.cbt")));
        assert!(!is_supported_file(Path::new("doc.pdf")));
    }

    #[test]
    fn supported_extensions_all_have_mime_types() {
        for ext in SUPPORTED_EXTENSIONS {
            assert!(
                get_mime_type(Path::new(&format!("file.{ext}"))).is_some(),
                "missing MIME mapping for .{ext}"
            );
        }
        assert_eq!(SUPPORTED_EXTENSIONS.len(), 21);
    }

    #[test]
    fn test_is_image_file_case_insensitive() {
        assert!(is_image_file(Path::new("PHOTO.PNG")));
        assert!(is_image_file(Path::new("image.JPEG")));
    }

    #[test]
    fn paint_strategy_transcodes_heic_heif_psd_only() {
        assert_eq!(paint_strategy("image/heic"), PaintStrategy::TranscodeJpeg);
        assert_eq!(paint_strategy("image/heif"), PaintStrategy::TranscodeJpeg);
        assert_eq!(
            paint_strategy("image/vnd.adobe.photoshop"),
            PaintStrategy::TranscodeJpeg
        );
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
        // 디코드 불가 paint 경로는 치수 생략
        assert_eq!(info.width, None);
        assert_eq!(info.height, None);
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
        assert_eq!(info.width, Some(1));
        assert_eq!(info.height, Some(1));
    }

    #[test]
    fn load_viewable_missing_file() {
        let err = load_viewable(Path::new("missing-file.png")).unwrap_err();
        assert_eq!(err.code, crate::app_error::ErrorCode::NotFound);
        assert_eq!(err.message, "File not found");
    }

    #[test]
    fn load_viewable_unsupported_extension() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("notes.txt");
        std::fs::write(&source, b"hello").unwrap();
        let err = load_viewable(&source).unwrap_err();
        assert_eq!(err.code, crate::app_error::ErrorCode::Unsupported);
        assert_eq!(err.message, "Unsupported image format");
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

    #[test]
    fn load_viewable_psd_transcodes_to_jpeg_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("design.psd");
        std::fs::write(
            &source,
            crate::psd_sidecar::minimal_psd_bytes(4, 2, [10, 20, 30]),
        )
        .unwrap();
        let first = load_viewable(&source).expect("decode PSD fixture");
        assert_eq!(first.mime_type, "image/vnd.adobe.photoshop");
        assert_eq!(first.file_name, "design.psd");
        assert_ne!(first.file_path, source.to_string_lossy());
        assert!(Path::new(&first.file_path).exists());
        let jpeg = std::fs::read(&first.file_path).unwrap();
        assert_eq!(&jpeg[..2], &[0xFF, 0xD8]);
        assert_eq!((first.width, first.height), (Some(4), Some(2)));
        let again = load_viewable(&source).expect("reuse PSD sidecar");
        assert_eq!(again.file_path, first.file_path);
    }

    #[test]
    fn load_viewable_psb_is_unsupported() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("large.psb");
        std::fs::write(&source, b"not-a-psb").unwrap();
        // .psb는 지원 확장자가 아니라 진입 차단된다.
        let err = load_viewable(&source).unwrap_err();
        assert_eq!(err.code, crate::app_error::ErrorCode::Unsupported);
    }

    #[test]
    fn load_viewable_psb_content_in_psd_is_unsupported() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("fake.psd");
        let mut bytes = b"8BPB".to_vec();
        bytes.extend_from_slice(&[0u8; 64]);
        std::fs::write(&source, bytes).unwrap();
        let err = load_viewable(&source).unwrap_err();
        assert_eq!(err.code, crate::app_error::ErrorCode::Unsupported);
        assert_eq!(err.message, "PSB is not supported");
    }

    #[test]
    fn load_viewable_psd_sample_fixture_when_present() {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("samples")
            .join("sample.psd");
        if !source.exists() {
            return;
        }
        let info = load_viewable(&source).expect("decode sample.psd");
        assert_eq!(info.mime_type, "image/vnd.adobe.photoshop");
        assert_eq!(info.file_name, "sample.psd");
        assert_ne!(info.file_path, source.to_string_lossy());
        assert!(Path::new(&info.file_path).exists());
        let jpeg = std::fs::read(&info.file_path).unwrap();
        assert_eq!(&jpeg[..2], &[0xFF, 0xD8]);
        assert_eq!((info.width, info.height), (Some(64), Some(64)));
    }

    const MIN_PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
        0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8,
        0xCF, 0xC0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x00, 0x05, 0xFE, 0xD4, 0xEF, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
}
