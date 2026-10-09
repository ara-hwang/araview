use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::app_error::{AppError, ErrorCode};
use crate::file_availability::FileAvailability;
use crate::scaled::ImageScalingMode;
use crate::svg_size::svg_dimensions;

#[derive(Serialize, Debug, Clone)]
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
    "png", "apng", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico", "avif", "heic", "heif",
    "psd", "tga", "dds", "exr", "qoi", "cbz", "zip",
];

#[derive(Serialize)]
pub struct DirectoryImages {
    /// 목록 캐시와 문자열을 공유한다. 수천 장 폴더에서 호출마다 복사하지 않는다.
    pub images: Vec<std::sync::Arc<str>>,
    pub current_index: usize,
    /// Same order as `images`.
    pub availability: Vec<FileAvailability>,
}

/// 아카이브 엔트리 목록. FE에는 `DirectoryImages`와 같은 모양으로 직렬화된다.
/// 엔트리 인덱스 캐시의 목록을 복사 없이 그대로 내보낸다.
#[derive(Serialize)]
pub struct ArchiveImages {
    pub images: std::sync::Arc<Vec<String>>,
    pub current_index: usize,
    /// Always empty: archive entries have no cloud availability.
    pub availability: Vec<FileAvailability>,
}

pub fn get_mime_type(path: &Path) -> Option<&'static str> {
    // 목록 스캔에서 파일마다 불리므로 String 할당 없이 스택에서 소문자화한다.
    let ext = path.extension()?.to_str()?.as_bytes();
    let mut lowered = [0u8; 8];
    let lowered = lowered.get_mut(..ext.len())?;
    lowered.copy_from_slice(ext);
    lowered.make_ascii_lowercase();
    match std::str::from_utf8(lowered).ok()? {
        "png" => Some("image/png"),
        "apng" => Some("image/apng"),
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
    // APNG는 PNG와 시그니처가 같다. 확장자가 밝혔거나 `acTL` 청크가 있으면 APNG다.
    let (sniffed, animated) = crate::sniff::sniff_file_with_animation(path);
    if sniffed == Some("image/png") && (by_ext == Some("image/apng") || animated) {
        return Some("image/apng");
    }
    sniffed.or(by_ext)
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
pub(crate) fn is_exif_orientation_format(mime: &str) -> bool {
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
