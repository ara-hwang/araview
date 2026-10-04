use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::app_error::{AppError, ErrorCode};
use crate::file_availability::FileAvailability;
use crate::scaled::ImageScalingMode;
use crate::svg_size::svg_dimensions;

#[derive(Serialize, Debug)]
pub struct ImageInfo {
    /// Bytes the WebView can decode. May be a JPEG/PNG sidecar, not the listing path.
    pub file_path: String,
    /// File the user opened. Never a sidecar; archive entries point at the
    /// extracted file. File operations (trash/rename/save/reveal) use this,
    /// not `file_path`.
    pub source_path: String,
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

const TRANSCODE_MIMES: &[&str] = &[
    "image/heic",
    "image/heif",
    crate::psd_sidecar::PSD_MIME,
    "image/x-tga",
    "image/vnd.ms-dds",
    "image/x-exr",
    "image/qoi",
];

pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico", "avif", "heic", "heif", "psd", "tga",
    "dds", "exr", "qoi", "cbz", "zip",
];

#[derive(Serialize)]
pub struct DirectoryImages {
    pub images: Vec<String>,
    pub current_index: usize,
    /// Same order as `images`. Empty for archive listings.
    pub availability: Vec<FileAvailability>,
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
        "avif" => Some("image/avif"),
        "heic" => Some("image/heic"),
        "heif" => Some("image/heif"),
        "psd" => Some(crate::psd_sidecar::PSD_MIME),
        "tga" => Some("image/x-tga"),
        "dds" => Some("image/vnd.ms-dds"),
        "exr" => Some("image/x-exr"),
        "qoi" => Some("image/qoi"),
        "cbz" => Some("application/vnd.comicbook+zip"),
        "zip" => Some("application/zip"),
        _ => None,
    }
}

/// 확장자 판별에 파일 내용(매직 바이트) 판별을 겹친 MIME.
/// 이름이 바뀐 파일(HEIC를 .jpg로 저장 등)도 올바른 렌더 경로를 타게 한다.
/// 아카이브와 SVG는 시그니처가 불확실해 확장자를 따른다.
pub fn resolve_mime(path: &Path) -> Option<&'static str> {
    let by_ext = get_mime_type(path);
    if by_ext.is_some_and(|m| m.starts_with("application/") || m == "image/svg+xml") {
        return by_ext;
    }
    crate::sniff::sniff_file(path).or(by_ext)
}

pub fn paint_strategy(mime: &str) -> PaintStrategy {
    if TRANSCODE_MIMES.contains(&mime) {
        PaintStrategy::TranscodeJpeg
    } else {
        PaintStrategy::Native
    }
}

/// 렌더 기준 치수. image 크레이트가 디코드하면 그 값을 쓰고,
/// SVG처럼 미지원이면 헤더 파싱으로 복원한다.
/// WebView2가 `<img>`에 EXIF orientation을 적용하므로 JPEG는 회전된
/// 기준(5~8은 가로/세로 교환)으로 보고해 초기 fit이 표시와 어긋나지 않게 한다.
pub(crate) fn render_dimensions(
    mime: &str,
    source: &Path,
    paint_path: &Path,
) -> (Option<u32>, Option<u32>) {
    let (width, height) = image::image_dimensions(paint_path).ok().unzip();
    if width.is_some() && height.is_some() {
        if is_exif_orientation_format(mime)
            && crate::orientation::swaps_axes(crate::orientation::read_orientation(source))
        {
            return (height, width);
        }
        return (width, height);
    }
    if mime == "image/svg+xml" {
        if let Some((w, h)) = svg_dimensions(paint_path).or_else(|| svg_dimensions(source)) {
            return (Some(w), Some(h));
        }
    }
    if mime == "image/avif" {
        if let Some((w, h)) = crate::heif::primary_dimensions(paint_path) {
            return (Some(w), Some(h));
        }
    }
    (width, height)
}

/// EXIF orientation을 적용하는 표시 포맷. HEIC/HEIF/PSD는 sidecar 디코더가
/// 변환을 반영하므로 여기서 다루지 않는다.
fn is_exif_orientation_format(mime: &str) -> bool {
    matches!(mime, "image/jpeg")
}

pub fn image_info(source: &Path, paint_path: PathBuf) -> Result<ImageInfo, AppError> {
    let mime =
        resolve_mime(source).ok_or_else(|| AppError::unsupported("Unsupported image format"))?;
    image_info_with_dims(source, mime, paint_path, None)
}

/// `dims_override`: 표시 바이트가 원본과 다른 기준(축소 sidecar)일 때의 치수.
/// None이면 렌더 경로에서 계산한다(EXIF orientation 반영).
/// `mime`은 호출자가 이미 판별한 값이다. 여기서 다시 판별하면 파일 선두를
/// 한 번 더 읽게 된다.
fn image_info_with_dims(
    source: &Path,
    mime: &'static str,
    paint_path: PathBuf,
    dims_override: Option<(u32, u32)>,
) -> Result<ImageInfo, AppError> {
    let mime_type = mime.to_string();
    let metadata = std::fs::metadata(source)
        .map_err(|e| AppError::io("Failed to read metadata", e, ErrorCode::Corrupt))?;
    let file_name = source
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    let (width, height) = match dims_override {
        Some((width, height)) => (Some(width), Some(height)),
        // Rendered bytes 기준 치수. image 크레이트 미지원분(SVG)은 헤더 파싱으로 복원.
        None => render_dimensions(&mime_type, source, &paint_path),
    };
    Ok(ImageInfo {
        file_path: paint_path.to_string_lossy().to_string(),
        source_path: source.to_string_lossy().to_string(),
        mime_type,
        file_name,
        file_size: metadata.len(),
        width,
        height,
    })
}

pub fn load_viewable(source: &Path) -> Result<ImageInfo, AppError> {
    load_viewable_with_limit(source, None)
}

/// `max_side`: 표시용 긴 변 상한(px). `None`이면 원본 바이트 그대로 렌더한다.
/// 상한을 넘는 래스터는 `scaled/` sidecar로 한 번만 축소하고, 반환 치수는 그
/// 사본 기준이다. `source_path`는 항상 원본 파일을 가리켜 파일 작업이 캐시
/// 산출물을 건드리지 않게 한다.
pub fn load_viewable_with_limit(
    source: &Path,
    max_side: Option<u32>,
) -> Result<ImageInfo, AppError> {
    load_viewable_with_limit_mode(source, max_side, ImageScalingMode::Smooth)
}

/// 이미지 표시 정책까지 반영해 viewable ImageInfo를 만든다.
pub fn load_viewable_with_limit_mode(
    source: &Path,
    max_side: Option<u32>,
    mode: ImageScalingMode,
) -> Result<ImageInfo, AppError> {
    if !source.exists() {
        return Err(AppError::not_found("File not found"));
    }
    let mime =
        resolve_mime(source).ok_or_else(|| AppError::unsupported("Unsupported image format"))?;
    let base_path = match paint_strategy(mime) {
        PaintStrategy::Native => source.to_path_buf(),
        PaintStrategy::TranscodeJpeg => crate::transcode::ensure_paint(source, mime)?,
    };
    let mut dims_override = None;
    let paint_path = match max_side {
        Some(limit) => {
            match crate::scaled::ensure_scaled_sidecar_with_mode(&base_path, limit, mode)? {
                Some(scaled) => {
                    dims_override = Some((scaled.width, scaled.height));
                    scaled.path
                }
                None => base_path,
            }
        }
        None => base_path,
    };
    image_info_with_dims(source, mime, paint_path, dims_override)
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
        Some("application/vnd.comicbook+zip" | "application/zip")
    )
}

/// 순수 이미지이거나 아카이브인 경우 모두 지원 파일로 간주
pub fn is_supported_file(path: &Path) -> bool {
    is_image_file(path) || is_archive_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svg_size::write_svg;

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
    fn tiff_is_not_supported() {
        assert_eq!(get_mime_type(Path::new("scan.tiff")), None);
        assert_eq!(get_mime_type(Path::new("scan.tif")), None);
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
    fn test_get_mime_type_qoi() {
        assert_eq!(get_mime_type(Path::new("pixels.qoi")), Some("image/qoi"));
        assert_eq!(get_mime_type(Path::new("pixels.QOI")), Some("image/qoi"));
    }

    #[test]
    fn test_get_mime_type_cbz() {
        assert_eq!(
            get_mime_type(Path::new("comic.cbz")),
            Some("application/vnd.comicbook+zip")
        );
    }

    #[test]
    fn test_get_mime_type_zip() {
        assert_eq!(
            get_mime_type(Path::new("archive.zip")),
            Some("application/zip")
        );
    }

    #[test]
    fn test_removed_archive_extensions_have_no_mime() {
        for name in [
            "comic.cb7",
            "comic.CB7",
            "comic.cbr",
            "archive.rar",
            "archive.7z",
            "comic.cbt",
        ] {
            assert_eq!(get_mime_type(Path::new(name)), None, "{name}");
        }
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
        assert!(is_image_file(Path::new("pixels.qoi")));
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
        assert!(is_archive_file(Path::new("archive.zip")));
        assert!(!is_archive_file(Path::new("comic.cb7")));
        assert!(!is_archive_file(Path::new("comic.cbr")));
        assert!(!is_archive_file(Path::new("archive.rar")));
        assert!(!is_archive_file(Path::new("archive.7z")));
        assert!(!is_archive_file(Path::new("comic.cbt")));
        assert!(!is_archive_file(Path::new("photo.png")));
    }

    #[test]
    fn test_is_supported_file() {
        assert!(is_supported_file(Path::new("photo.png")));
        assert!(is_supported_file(Path::new("comic.cbz")));
        assert!(!is_supported_file(Path::new("comic.cb7")));
        assert!(!is_supported_file(Path::new("comic.cbr")));
        assert!(!is_supported_file(Path::new("comic.cbt")));
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
        assert_eq!(SUPPORTED_EXTENSIONS.len(), 18);
    }

    #[test]
    fn resolve_mime_prefers_content_over_extension() {
        let dir = tempfile::tempdir().unwrap();
        // 확장자가 틀린 PSD는 PSD 경로를 탄다.
        let renamed = dir.path().join("design.jpg");
        std::fs::write(
            &renamed,
            crate::psd_sidecar::minimal_psd_bytes(2, 2, [1, 2, 3]),
        )
        .unwrap();
        assert_eq!(resolve_mime(&renamed), Some(crate::psd_sidecar::PSD_MIME));
        assert_eq!(
            paint_strategy(resolve_mime(&renamed).unwrap()),
            PaintStrategy::TranscodeJpeg
        );
        // 확장자 없는 파일도 내용으로 판별한다.
        let bare = dir.path().join("download");
        std::fs::write(&bare, MIN_PNG).unwrap();
        assert_eq!(resolve_mime(&bare), Some("image/png"));
        // 시그니처가 불확실하면 확장자를 따른다.
        let svg = dir.path().join("a.svg");
        std::fs::write(&svg, b"<svg/>").unwrap();
        assert_eq!(resolve_mime(&svg), Some("image/svg+xml"));
        // QOI도 시그니처(qoif)로 판별한다.
        let qoi = dir.path().join("really-qoi.jpg");
        image::DynamicImage::ImageRgb8(image::RgbImage::new(2, 2))
            .save_with_format(&qoi, image::ImageFormat::Qoi)
            .unwrap();
        assert_eq!(resolve_mime(&qoi), Some("image/qoi"));
        let fake = dir.path().join("fake.heic");
        std::fs::write(&fake, b"not-a-real-heic").unwrap();
        assert_eq!(resolve_mime(&fake), Some("image/heic"));
        // 내용도 확장자도 모르면 None.
        let junk = dir.path().join("notes");
        std::fs::write(&junk, b"hello").unwrap();
        assert_eq!(resolve_mime(&junk), None);
    }

    #[test]
    fn mislabeled_psd_loads_through_the_psd_decoder() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("really-psd.png");
        std::fs::write(
            &source,
            crate::psd_sidecar::minimal_psd_bytes(4, 2, [9, 9, 9]),
        )
        .unwrap();
        let info = load_viewable(&source).expect("sniffed as PSD");
        assert_eq!(info.mime_type, crate::psd_sidecar::PSD_MIME);
        assert_ne!(info.file_path, info.source_path);
        assert_eq!((info.width, info.height), (Some(4), Some(2)));
    }

    #[test]
    fn mislabeled_qoi_loads_through_the_raster_decoder() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("really-qoi.png");
        let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            4,
            2,
            image::Rgb([9, 9, 9]),
        ));
        img.save_with_format(&source, image::ImageFormat::Qoi)
            .unwrap();
        let info = load_viewable(&source).expect("sniffed as QOI");
        assert_eq!(info.mime_type, "image/qoi");
        assert_ne!(info.file_path, info.source_path);
        assert_eq!((info.width, info.height), (Some(4), Some(2)));
    }

    #[test]
    fn test_is_image_file_case_insensitive() {
        assert!(is_image_file(Path::new("PHOTO.PNG")));
        assert!(is_image_file(Path::new("image.JPEG")));
    }

    #[test]
    fn paint_strategy_transcodes_only_formats_webview_cannot_paint() {
        assert_eq!(paint_strategy("image/heic"), PaintStrategy::TranscodeJpeg);
        assert_eq!(paint_strategy("image/heif"), PaintStrategy::TranscodeJpeg);
        assert_eq!(
            paint_strategy("image/vnd.adobe.photoshop"),
            PaintStrategy::TranscodeJpeg
        );
        for mime in [
            "image/x-tga",
            "image/vnd.ms-dds",
            "image/x-exr",
            "image/qoi",
        ] {
            assert_eq!(paint_strategy(mime), PaintStrategy::TranscodeJpeg, "{mime}");
        }
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
        assert_eq!(info.source_path, source.to_string_lossy());
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
        assert_eq!(info.source_path, source.to_string_lossy());
        assert_eq!(info.width, Some(1));
        assert_eq!(info.height, Some(1));
    }

    #[test]
    fn load_viewable_with_limit_scales_over_cap_and_keeps_source_path() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("huge.png");
        let img = image::RgbImage::new(2000, 1000);
        image::DynamicImage::ImageRgb8(img).save(&source).unwrap();

        let info = load_viewable_with_limit(&source, Some(512)).unwrap();
        assert_ne!(info.file_path, source.to_string_lossy());
        assert!(info.file_path.ends_with(".jpg"));
        assert_eq!(info.source_path, source.to_string_lossy());
        assert_eq!((info.width, info.height), (Some(512), Some(256)));
    }

    #[test]
    fn load_viewable_with_limit_keeps_original_when_within_cap() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("photo.png");
        std::fs::write(&source, MIN_PNG).unwrap();

        let info = load_viewable_with_limit(&source, Some(3840)).unwrap();
        assert_eq!(info.file_path, source.to_string_lossy());
        assert_eq!(info.source_path, source.to_string_lossy());
        assert_eq!((info.width, info.height), (Some(1), Some(1)));
    }

    #[test]
    fn load_viewable_with_limit_reports_oriented_scaled_dimensions() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("phone.jpg");
        // 저장 2048x1024 + EXIF 6(시계 90도) → 표시 1024x2048, 상한 512 → 256x512
        crate::orientation::write_test_jpeg_with_orientation(&source, 2048, 1024, 6);

        let info = load_viewable_with_limit(&source, Some(512)).unwrap();
        assert_ne!(info.file_path, source.to_string_lossy());
        assert_eq!(info.source_path, source.to_string_lossy());
        assert_eq!((info.width, info.height), (Some(256), Some(512)));
    }

    #[test]
    fn load_viewable_reports_oriented_dimensions_for_jpeg() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("phone.jpg");
        // 4x2 파일 + EXIF 6(시계 90°) → 표시 기준 2x4
        crate::orientation::write_test_jpeg_with_orientation(&source, 4, 2, 6);
        let info = load_viewable(&source).unwrap();
        assert_eq!((info.width, info.height), (Some(2), Some(4)));
        // 네이티브 렌더라 paint 경로는 원본 그대로다.
        assert_eq!(info.file_path, source.to_string_lossy());
    }

    #[test]
    fn load_viewable_keeps_dimensions_for_normal_orientation() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("plain.jpg");
        crate::orientation::write_test_jpeg_with_orientation(&source, 4, 2, 1);
        let info = load_viewable(&source).unwrap();
        assert_eq!((info.width, info.height), (Some(4), Some(2)));
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
    fn load_viewable_qoi_transcodes_to_jpeg_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("pixels.qoi");
        let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            4,
            2,
            image::Rgb([10, 20, 30]),
        ));
        img.save_with_format(&source, image::ImageFormat::Qoi)
            .unwrap();
        let first = load_viewable(&source).expect("decode QOI fixture");
        assert_eq!(first.mime_type, "image/qoi");
        assert_eq!(first.file_name, "pixels.qoi");
        assert_ne!(first.file_path, source.to_string_lossy());
        assert!(Path::new(&first.file_path).exists());
        let jpeg = std::fs::read(&first.file_path).unwrap();
        assert_eq!(&jpeg[..2], &[0xFF, 0xD8]);
        assert_eq!((first.width, first.height), (Some(4), Some(2)));
        let again = load_viewable(&source).expect("reuse QOI sidecar");
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
        // 실제 PSB 헤더: `8BPS` + version 2.
        let mut bytes = b"8BPS".to_vec();
        bytes.extend_from_slice(&2u16.to_be_bytes());
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

    #[test]
    fn avif_dimensions_come_from_heif_header() {
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("../samples/sample.avif");
        let (w, h) = render_dimensions("image/avif", &sample, &sample);
        assert_eq!((w, h), (Some(800), Some(600)));
    }

    #[test]
    fn load_viewable_svg_reports_header_dimensions() {
        let dir = tempfile::tempdir().unwrap();
        let source = write_svg(
            dir.path(),
            "vector.svg",
            r#"<svg width="800" height="600" xmlns="http://www.w3.org/2000/svg"><rect width="800" height="600"/></svg>"#,
        );
        let info = load_viewable(&source).expect("load svg");
        assert_eq!(info.mime_type, "image/svg+xml");
        assert_eq!((info.width, info.height), (Some(800), Some(600)));
        // 네이티브 렌더라 paint 경로는 원본 그대로
        assert_eq!(info.file_path, source.to_string_lossy());
    }

    #[test]
    fn load_viewable_svg_viewbox_only_reports_dimensions() {
        let dir = tempfile::tempdir().unwrap();
        let source = write_svg(
            dir.path(),
            "vector.svg",
            r#"<svg viewBox="0 0 400 300" xmlns="http://www.w3.org/2000/svg"></svg>"#,
        );
        let info = load_viewable(&source).expect("load svg");
        assert_eq!((info.width, info.height), (Some(400), Some(300)));
    }

    const MIN_PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
        0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8,
        0xCF, 0xC0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x00, 0x05, 0xFE, 0xD4, 0xEF, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
}
