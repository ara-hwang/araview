//! Thumbnail generation with on-disk cache (S2).
//!
//! `ImageNavBar` used to load full-size originals for its thumbnail strip.
//! This module decodes via the `image` crate, resizes to a bounded side
//! length and stores a JPEG under `process_temp/thumbs/`, reusing the
//! existing sidecar on repeat calls. Formats the `image` crate cannot decode
//! (e.g. SVG, archive entry names) return an error so the frontend can
//! fall back to the original render path.
//!
//! Cache naming uses the stable hash in [`crate::sidecar`], and publishing
//! goes through a temp file + atomic rename so concurrent strip requests
//! never observe a partial JPEG.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::app_error::{AppError, ErrorCode};
use crate::process_temp::process_temp_dir;

#[derive(Serialize, Debug)]
pub struct ThumbnailInfo {
    pub file_path: String,
    pub width: u32,
    pub height: u32,
}

const DEFAULT_MAX_SIDE: u32 = 256;
/// Upper bound for the thumbs directory; oldest files evicted past this.
pub(crate) const MAX_CACHE_BYTES: u64 = 500 * 1024 * 1024;

pub fn default_max_side() -> u32 {
    DEFAULT_MAX_SIDE
}

/// 큰 이미지 첫 렌더용 프리뷰 후보. 도크 스트립(기본 128)과 그리드(256)가 실제로
/// 만드는 크기이며, 캐시에 있는 것 중 가장 큰 썸네일을 골라 쓴다.
const PREVIEW_THUMB_CANDIDATES: &[u32] = &[256, 128, 96, 72, 48, 32];

/// 프리뷰용: 이미 캐시에 있는 썸네일만 반환한다. 없으면 생성하지 않고 None.
/// 풀사이즈 디코드가 오래 걸리는 이미지에서 첫 페인트를 앞당기는 용도다.
pub fn cached_thumbnail(source: &Path) -> Option<ThumbnailInfo> {
    if !source.is_file() {
        return None;
    }
    let sidecar_source = is_sidecar_source(source);
    for &max_side in PREVIEW_THUMB_CANDIDATES {
        let path = if sidecar_source {
            if is_psd_source(source) {
                crate::psd_sidecar::cached_jpeg_sidecar_thumb(source, max_side)
            } else {
                crate::heif::cached_jpeg_sidecar_thumb(source, max_side)
            }
        } else {
            thumb_path(source, max_side).ok().filter(|p| p.exists())
        };
        let Some(path) = path else { continue };
        // 화면에 띄우는 동안 캐시 축출로 사라지지 않게 보호한다.
        crate::process_temp::mark_in_use(&path);
        crate::process_temp::touch_cache_file(&path);
        let Ok((width, height)) = image::image_dimensions(&path) else {
            continue;
        };
        return Some(ThumbnailInfo {
            file_path: path.to_string_lossy().to_string(),
            width,
            height,
        });
    }
    None
}
pub fn generate_thumbnail(source: &Path, max_side: u32) -> Result<ThumbnailInfo, AppError> {
    if !source.is_file() {
        return Err(AppError::not_found("File not found"));
    }
    let max_side = max_side.clamp(32, 1024);
    if is_sidecar_source(source) {
        let sidecar = if is_psd_source(source) {
            crate::psd_sidecar::ensure_jpeg_sidecar_thumb(source, max_side)?
        } else {
            crate::heif::ensure_jpeg_sidecar_thumb(source, max_side)?
        };
        let (width, height) = image::image_dimensions(&sidecar)
            .map_err(|e| AppError::corrupt(format!("Failed to read thumbnail: {e}")))?;
        return Ok(ThumbnailInfo {
            file_path: sidecar.to_string_lossy().to_string(),
            width,
            height,
        });
    }
    let dest = thumb_path(source, max_side)?;
    // 스트립이 이 경로로 asset URL을 유지하므로 캐시 축출에서 보호한다.
    crate::process_temp::mark_in_use(&dest);
    if dest.exists() {
        crate::process_temp::touch_cache_file(&dest);
    } else {
        let key = dest.to_string_lossy().into_owned();
        crate::sidecar::with_file_lock(&key, "thumbnail lock", || {
            if dest.exists() {
                crate::process_temp::touch_cache_file(&dest);
                return Ok(());
            }
            let img = image::open(source)
                .map_err(|e| AppError::image_error("Failed to decode image", e))?;
            let img = crate::orientation::apply_to_image(
                img,
                crate::orientation::read_orientation(source),
            );
            let thumb = img.thumbnail(max_side, max_side);
            write_jpeg_atomic(&dest, &thumb)?;
            // Best effort: eviction failures must not fail thumbnail delivery.
            crate::process_temp::enforce_cap(THUMBS_SUBDIR, MAX_CACHE_BYTES).ok();
            Ok(())
        })?;
    }
    let (width, height) = image::image_dimensions(&dest)
        .map_err(|e| AppError::corrupt(format!("Failed to read thumbnail: {e}")))?;
    Ok(ThumbnailInfo {
        file_path: dest.to_string_lossy().to_string(),
        width,
        height,
    })
}

fn is_sidecar_source(source: &Path) -> bool {
    is_heif_source(source) || is_psd_source(source)
}

fn is_heif_source(source: &Path) -> bool {
    matches!(
        source
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some("heic" | "heif")
    )
}

fn is_psd_source(source: &Path) -> bool {
    matches!(
        source
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some("psd")
    )
}

#[derive(Serialize, Debug)]
pub struct BatchThumb {
    pub source: String,
    pub thumb: Option<ThumbnailInfo>,
    pub error: Option<String>,
}

/// 썸네일 스트립 윈도우를 1회 invoke으로 처리. 실패 항목은 FE가 원본 폴백한다.
pub fn generate_thumbnails_batch(sources: &[String], max_side: u32) -> Vec<BatchThumb> {
    sources
        .iter()
        .map(|s| match generate_thumbnail(Path::new(s), max_side) {
            Ok(thumb) => BatchThumb {
                source: s.clone(),
                thumb: Some(thumb),
                error: None,
            },
            Err(e) => BatchThumb {
                source: s.clone(),
                thumb: None,
                error: Some(e.message),
            },
        })
        .collect()
}

pub(crate) const THUMBS_SUBDIR: &str = "thumbs";

fn thumbs_dir() -> Result<PathBuf, AppError> {
    let dir = process_temp_dir()?.join(THUMBS_SUBDIR);
    fs::create_dir_all(&dir)
        .map_err(|e| AppError::io("Failed to create thumbs dir", e, ErrorCode::Unknown))?;
    Ok(dir)
}

fn thumb_path(source: &Path, max_side: u32) -> Result<PathBuf, AppError> {
    let hash = crate::sidecar::file_identity_hash(source, &max_side.to_le_bytes())?;
    Ok(thumbs_dir()?.join(format!("{hash:016x}.jpg")))
}

fn write_jpeg_atomic(dest: &Path, img: &image::DynamicImage) -> Result<(), AppError> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AppError::io("Failed to create thumbs dir", e, ErrorCode::Unknown))?;
    }
    // `.jpg` 접미사를 유지해야 `save`가 포맷을 추론할 수 있다.
    let tmp = crate::sidecar::tmp_path_for(dest);
    img.save(&tmp)
        .map_err(|e| AppError::unknown(format!("Failed to encode thumbnail: {e}")))?;
    crate::sidecar::publish_atomic(&tmp, dest, "Failed to publish thumbnail")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_png(path: &Path, width: u32, height: u32) {
        let img =
            image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(width, height, |x, y| {
                image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
            }));
        img.save(path).expect("write png fixture");
    }

    #[test]
    fn generates_bounded_jpeg_thumbnail() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("wide.png");
        write_png(&source, 200, 100);

        let thumb = generate_thumbnail(&source, 64).expect("generate");
        assert!(thumb.file_path.ends_with(".jpg"));
        assert!(thumb.width <= 64 && thumb.height <= 64);
        assert_eq!((thumb.width, thumb.height), (64, 32));

        let bytes = fs::read(&thumb.file_path).unwrap();
        assert_eq!(&bytes[..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn same_input_reuses_thumbnail() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("photo.png");
        write_png(&source, 80, 80);

        let first = generate_thumbnail(&source, 64).expect("first");
        let second = generate_thumbnail(&source, 64).expect("second");
        assert_eq!(first.file_path, second.file_path);
    }

    #[test]
    fn unsupported_source_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("notes.txt");
        fs::write(&source, b"hello").unwrap();
        let err = generate_thumbnail(&source, 64).unwrap_err();
        assert!(!err.message.is_empty());
    }

    #[test]
    fn psd_uses_jpeg_sidecar_thumb() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("design.psd");
        fs::write(
            &source,
            crate::psd_sidecar::minimal_psd_bytes(200, 100, [9, 9, 9]),
        )
        .unwrap();

        let thumb = generate_thumbnail(&source, 64).expect("psd thumb");
        assert!(thumb.file_path.ends_with(".jpg"));
        assert!(thumb.width <= 64 && thumb.height <= 64);
        assert_eq!((thumb.width, thumb.height), (64, 32));

        let bytes = fs::read(&thumb.file_path).unwrap();
        assert_eq!(&bytes[..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn oriented_jpeg_thumbnail_uses_upright_dimensions() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("phone.jpg");
        // 40x20 + EXIF 6 → 썸네일도 세로(최소 32 클램프 기준 16x32)여야 한다.
        crate::orientation::write_test_jpeg_with_orientation(&source, 40, 20, 6);

        let thumb = generate_thumbnail(&source, 8).expect("thumb");
        assert_eq!((thumb.width, thumb.height), (16, 32));
    }

    #[test]
    fn cached_thumbnail_is_none_until_generated() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("preview.png");
        write_png(&source, 200, 100);

        assert!(cached_thumbnail(&source).is_none());
        let thumb = generate_thumbnail(&source, 96).expect("generate");
        let cached = cached_thumbnail(&source).expect("cached");
        assert_eq!(cached.file_path, thumb.file_path);
        assert_eq!((cached.width, cached.height), (thumb.width, thumb.height));
    }

    #[test]
    fn cached_thumbnail_prefers_largest_candidate() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("preview-wide.png");
        write_png(&source, 400, 100);

        let small = generate_thumbnail(&source, 48).expect("48");
        let large = generate_thumbnail(&source, 96).expect("96");
        let cached = cached_thumbnail(&source).expect("cached");
        assert_eq!(cached.file_path, large.file_path);
        assert_ne!(cached.file_path, small.file_path);
    }

    #[test]
    fn cached_thumbnail_reads_psd_sidecar_thumb() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("preview.psd");
        fs::write(
            &source,
            crate::psd_sidecar::minimal_psd_bytes(200, 100, [9, 9, 9]),
        )
        .unwrap();

        assert!(cached_thumbnail(&source).is_none());
        let thumb = generate_thumbnail(&source, 96).expect("psd thumb");
        let cached = cached_thumbnail(&source).expect("cached psd");
        assert_eq!(cached.file_path, thumb.file_path);
    }

    #[test]
    fn missing_file_returns_not_found() {
        let err = generate_thumbnail(Path::new("no-such-thumb.png"), 64).unwrap_err();
        assert_eq!(err.code, crate::app_error::ErrorCode::NotFound);
        assert_eq!(err.message, "File not found");
    }
}
