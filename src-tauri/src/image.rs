use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::app_error::{AppError, ErrorCode};
use crate::file_availability::FileAvailability;
use crate::scaled::ImageScalingMode;

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

const TRANSCODE_MIMES: &[&str] = &["image/heic", "image/heif", crate::psd_sidecar::PSD_MIME];

pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico", "avif", "heic", "heif", "psd", "cbz",
    "zip",
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
        "cbz" => Some("application/vnd.comicbook+zip"),
        "zip" => Some("application/zip"),
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

/// 렌더 기준 치수. image 크레이트가 디코드하면 그 값을 쓰고,
/// SVG처럼 미지원이면 헤더 파싱으로 복원한다.
/// WebView2가 `<img>`에 EXIF orientation을 적용하므로 JPEG/TIFF는 회전된
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

/// SVG `<svg>` 루트 태그의 width/height/viewBox에서 픽셀 치수를 구한다.
/// 의존성 없이 선두 64KB만 읽는다. `%`/폰트 단위 등 해석 불가분은 None.
pub fn svg_dimensions(path: &Path) -> Option<(u32, u32)> {
    use std::io::Read as _;

    const HEAD_LIMIT: u64 = 64 * 1024;
    let file = std::fs::File::open(path).ok()?;
    let mut buf = Vec::new();
    file.take(HEAD_LIMIT).read_to_end(&mut buf).ok()?;
    if buf.is_empty() {
        return None;
    }
    let head = String::from_utf8_lossy(&buf).to_ascii_lowercase();
    let start = head.find("<svg")?;
    let after = &head[start..];
    let end = after.find('>')?;
    let tag = &after[..end];
    let (width_raw, height_raw, viewbox_raw) = svg_root_attrs(tag);
    let width = width_raw.as_deref().and_then(svg_length_px);
    let height = height_raw.as_deref().and_then(svg_length_px);
    let viewbox = viewbox_raw.as_deref().and_then(parse_viewbox);
    match (width, height, viewbox) {
        (Some(w), Some(h), _) => finite_pixels(w, h),
        (Some(w), None, Some((vb_w, vb_h))) => finite_pixels(w, w * vb_h / vb_w),
        (None, Some(h), Some((vb_w, vb_h))) => finite_pixels(h * vb_w / vb_h, h),
        (None, None, Some((vb_w, vb_h))) => finite_pixels(vb_w, vb_h),
        _ => None,
    }
}

/// `<svg ...>` 태그에서 width/height/viewBox 원시값을 뽑는다.
/// `stroke-width` 같은 합성 이름과 겹치지 않게 토큰 단위로 비교한다.
fn svg_root_attrs(tag: &str) -> (Option<String>, Option<String>, Option<String>) {
    let bytes = tag.as_bytes();
    let mut i = 0;
    // "<svg" 건너뛰기
    if tag.starts_with("<svg") {
        i = 4;
    }
    let (mut width, mut height, mut viewbox) = (None, None, None);
    while i < bytes.len() {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'/') {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] == b'>' {
            break;
        }
        let name_start = i;
        while i < bytes.len()
            && (bytes[i].is_ascii_alphanumeric()
                || bytes[i] == b'-'
                || bytes[i] == b'_'
                || bytes[i] == b':'
                || bytes[i] == b'.')
        {
            i += 1;
        }
        if name_start == i {
            i += 1;
            continue;
        }
        let name = &tag[name_start..i];
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'=' {
            continue;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let value: String;
        if bytes[i] == b'"' || bytes[i] == b'\'' {
            let quote = bytes[i];
            i += 1;
            let value_start = i;
            while i < bytes.len() && bytes[i] != quote {
                i += 1;
            }
            value = tag[value_start..i].to_string();
            if i < bytes.len() {
                i += 1;
            }
        } else {
            let value_start = i;
            while i < bytes.len()
                && !bytes[i].is_ascii_whitespace()
                && bytes[i] != b'>'
                && bytes[i] != b'/'
            {
                i += 1;
            }
            value = tag[value_start..i].to_string();
        }
        match name {
            "width" => width = Some(value),
            "height" => height = Some(value),
            "viewbox" => viewbox = Some(value),
            _ => {}
        }
    }
    (width, height, viewbox)
}

/// CSS 길이 → px. `%`/폰트·뷰포트 단위는 해석 불가라 None.
fn svg_length_px(raw: &str) -> Option<f32> {
    let s = raw.trim().to_ascii_lowercase();
    if s.is_empty() || s == "auto" {
        return None;
    }
    if s.ends_with('%') {
        return None;
    }
    const ABSOLUTE: &[(&str, f32)] = &[
        ("px", 1.0),
        ("pt", 96.0 / 72.0),
        ("pc", 16.0),
        ("mm", 96.0 / 25.4),
        ("cm", 96.0 / 2.54),
        ("in", 96.0),
        ("q", 96.0 / 25.4 / 4.0),
    ];
    let mut number = s.as_str();
    let mut factor = 1.0;
    for (suffix, scale) in ABSOLUTE {
        if let Some(prefix) = s.strip_suffix(suffix) {
            number = prefix;
            factor = *scale;
            break;
        }
    }
    // 숫자로 끝나지 않으면 폰트·뷰포트 등 미지 단위로 버린다.
    let value: f32 = number.trim().parse().ok()?;
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    let px = value * factor;
    if !px.is_finite() || px <= 0.0 {
        return None;
    }
    Some(px)
}

fn parse_viewbox(raw: &str) -> Option<(f32, f32)> {
    let nums: Vec<f32> = raw
        .replace(',', " ")
        .split_whitespace()
        .filter_map(|p| p.parse::<f32>().ok())
        .collect();
    if nums.len() != 4 {
        return None;
    }
    let (w, h) = (nums[2], nums[3]);
    if !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 {
        return None;
    }
    Some((w, h))
}

fn finite_pixels(w: f32, h: f32) -> Option<(u32, u32)> {
    const MAX_SIDE: f32 = 1_000_000.0;
    if !w.is_finite() || !h.is_finite() {
        return None;
    }
    if w <= 0.0 || h <= 0.0 || w > MAX_SIDE || h > MAX_SIDE {
        return None;
    }
    let (w, h) = (w.round() as u32, h.round() as u32);
    if w == 0 || h == 0 {
        return None;
    }
    Some((w, h))
}

/// Windows가 장치 이름으로 예약해 `File::create`가 원인 불명 에러로
/// 실패하는 이름들(`CON`, `NUL`, `COM1`...). 확장자가 붙어도(`CON.txt`)
/// 예약은 유지되므로 첫 점 앞부분으로 판정한다.
const WINDOWS_RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

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
    let stem = trimmed.split('.').next().unwrap_or(trimmed);
    if WINDOWS_RESERVED_NAMES
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
    {
        return Err(AppError::invalid_input("File name is reserved by Windows"));
    }
    Ok(trimmed.to_string())
}

pub fn image_info(source: &Path, paint_path: PathBuf) -> Result<ImageInfo, AppError> {
    image_info_with_dims(source, paint_path, None)
}

/// `dims_override`: 표시 바이트가 원본과 다른 기준(축소 sidecar)일 때의 치수.
/// None이면 렌더 경로에서 계산한다(EXIF orientation 반영).
fn image_info_with_dims(
    source: &Path,
    paint_path: PathBuf,
    dims_override: Option<(u32, u32)>,
) -> Result<ImageInfo, AppError> {
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
        get_mime_type(source).ok_or_else(|| AppError::unsupported("Unsupported image format"))?;
    let base_path = match paint_strategy(mime) {
        PaintStrategy::Native => source.to_path_buf(),
        PaintStrategy::TranscodeJpeg if mime == crate::psd_sidecar::PSD_MIME => {
            crate::psd_sidecar::ensure_jpeg_sidecar(source)?
        }
        PaintStrategy::TranscodeJpeg => crate::heif::ensure_jpeg_sidecar(source)?,
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
    image_info_with_dims(source, paint_path, dims_override)
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
        assert_eq!(SUPPORTED_EXTENSIONS.len(), 14);
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

    fn write_svg(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn validate_new_file_name_rejects_reserved_device_names() {
        for name in ["CON", "nul", "Com1", "LPT9.txt", "aux.png"] {
            assert!(
                validate_new_file_name(name).is_err(),
                "{name} should be rejected"
            );
        }
        // 예약명이 부분 문자열로만 들어간 정상 이름은 허용한다.
        assert!(validate_new_file_name("CONX.png").is_ok());
        assert!(validate_new_file_name("my-nul.png").is_ok());
    }

    #[test]
    fn validate_new_file_name_rejects_invalid_shapes() {
        assert!(validate_new_file_name("").is_err());
        assert!(validate_new_file_name("a/b.png").is_err());
        assert!(validate_new_file_name("a?.png").is_err());
        assert!(validate_new_file_name("name.").is_err());
        assert_eq!(
            validate_new_file_name("  photo.png  ").unwrap(),
            "photo.png"
        );
    }

    #[test]
    fn avif_dimensions_come_from_heif_header() {
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("../samples/sample.avif");
        let (w, h) = render_dimensions("image/avif", &sample, &sample);
        assert_eq!((w, h), (Some(800), Some(600)));
    }

    #[test]
    fn svg_dimensions_explicit_width_height() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_svg(
            dir.path(),
            "icon.svg",
            r#"<svg width="800" height="600" xmlns="http://www.w3.org/2000/svg"></svg>"#,
        );
        assert_eq!(svg_dimensions(&path), Some((800, 600)));
    }

    #[test]
    fn svg_dimensions_viewbox_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_svg(
            dir.path(),
            "icon.svg",
            r#"<svg viewBox="0 0 400 300" xmlns="http://www.w3.org/2000/svg"></svg>"#,
        );
        assert_eq!(svg_dimensions(&path), Some((400, 300)));
    }

    #[test]
    fn svg_dimensions_width_plus_viewbox_ratio() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_svg(
            dir.path(),
            "icon.svg",
            r#"<svg width="200" viewBox="0 0 400 300" xmlns="http://www.w3.org/2000/svg"></svg>"#,
        );
        assert_eq!(svg_dimensions(&path), Some((200, 150)));
    }

    #[test]
    fn svg_dimensions_percent_falls_back_to_viewbox() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_svg(
            dir.path(),
            "icon.svg",
            r#"<svg width="100%" height="100%" viewBox="0 0 64 64" xmlns="http://www.w3.org/2000/svg"></svg>"#,
        );
        assert_eq!(svg_dimensions(&path), Some((64, 64)));
    }

    #[test]
    fn svg_dimensions_absolute_units_convert_to_px() {
        let dir = tempfile::tempdir().unwrap();
        // 1in x 72pt = 96px x 96px
        let path = write_svg(
            dir.path(),
            "icon.svg",
            r#"<svg width="1in" height="72pt" xmlns="http://www.w3.org/2000/svg"></svg>"#,
        );
        assert_eq!(svg_dimensions(&path), Some((96, 96)));
    }

    #[test]
    fn svg_dimensions_unresolvable_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        // 크기 정보 없음
        let bare = write_svg(
            dir.path(),
            "bare.svg",
            r#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#,
        );
        assert_eq!(svg_dimensions(&bare), None);
        // 폰트 단위는 해석 불가
        let em = write_svg(
            dir.path(),
            "em.svg",
            r#"<svg width="10em" height="10em" xmlns="http://www.w3.org/2000/svg"></svg>"#,
        );
        assert_eq!(svg_dimensions(&em), None);
        // SVG가 아님
        let not_svg = write_svg(dir.path(), "icon.svg", r#"<html></html>"#);
        assert_eq!(svg_dimensions(&not_svg), None);
        // stroke-width의 width와 겹치지 않아야 함
        let stroke = write_svg(
            dir.path(),
            "stroke.svg",
            r#"<svg viewBox="0 0 20 10" xmlns="http://www.w3.org/2000/svg"><path stroke-width="5" d="M0 0h20"/></svg>"#,
        );
        assert_eq!(svg_dimensions(&stroke), Some((20, 10)));
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
