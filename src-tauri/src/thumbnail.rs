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
use std::sync::Mutex;

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
    let decoder = crate::transcode::decoder_for(source);
    for &max_side in PREVIEW_THUMB_CANDIDATES {
        let path = if let Some(decoder) = decoder {
            crate::transcode::cached_thumb(source, decoder, max_side)
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
    if let Some(decoder) = crate::transcode::decoder_for(source) {
        let sidecar = crate::transcode::ensure_thumb(source, decoder, max_side)?;
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
            let thumb = if is_svg_source(source) {
                image::DynamicImage::ImageRgb8(crate::svg_raster::rasterize(source, max_side)?)
            } else {
                let img = match decode_jpeg_scaled(source, max_side) {
                    Some(img) => img,
                    None => image::open(source)
                        .map_err(|e| AppError::image_error("Failed to decode image", e))?,
                };
                let img = crate::orientation::apply_to_image(
                    img,
                    crate::orientation::read_orientation(source),
                );
                img.thumbnail(max_side, max_side)
            };
            write_jpeg_atomic(&dest, &thumb)?;
            // Best effort: eviction failures must not fail thumbnail delivery.
            // A full cache holds tens of thousands of thumbnails, so track the
            // running total instead of rescanning the directory per write.
            crate::process_temp::note_published(&dest, MAX_CACHE_BYTES);
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

/// JPEG는 DCT 단계에서 1/2, 1/4, 1/8로 줄여 디코드한다. 결과는 `max_side`보다
/// 작아지지 않는 가장 작은 배율이라 화질 손실 없이 풀해상도 디코드와 큰 버퍼를
/// 피한다. JPEG가 아니거나 CMYK처럼 여기서 다루지 않는 입력은 `None`을 돌려
/// 호출자가 `image::open`으로 처리한다.
pub(crate) fn decode_jpeg_scaled(source: &Path, max_side: u32) -> Option<image::DynamicImage> {
    use jpeg_decoder::PixelFormat;

    if crate::image::resolve_mime(source) != Some("image/jpeg") {
        return None;
    }
    let side = u16::try_from(max_side).ok()?;
    let mut decoder =
        jpeg_decoder::Decoder::new(std::io::BufReader::new(fs::File::open(source).ok()?));
    decoder.read_info().ok()?;
    let (width, height) = decoder.scale(side, side).ok()?;
    let format = decoder.info()?.pixel_format;
    let pixels = decoder.decode().ok()?;
    let (width, height) = (u32::from(width), u32::from(height));
    match format {
        PixelFormat::RGB24 => {
            image::RgbImage::from_raw(width, height, pixels).map(image::DynamicImage::ImageRgb8)
        }
        PixelFormat::L8 => {
            image::GrayImage::from_raw(width, height, pixels).map(image::DynamicImage::ImageLuma8)
        }
        _ => None,
    }
}

fn is_svg_source(source: &Path) -> bool {
    source
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
}

#[derive(Serialize, Debug)]
pub struct BatchThumb {
    pub source: String,
    pub thumb: Option<ThumbnailInfo>,
    pub error: Option<String>,
}

/// 썸네일 배치 내부 병렬 워커 수. 논리 코어의 절반을 쓰되 4~8로 묶어, 본문
/// 이미지 디코드와 UI가 쓸 코어를 남긴다.
fn batch_workers() -> usize {
    std::thread::available_parallelism()
        .map_or(4, |n| n.get() / 2)
        .clamp(4, 8)
}

impl BatchThumb {
    pub(crate) fn from_result(source: &str, result: Result<ThumbnailInfo, AppError>) -> Self {
        match result {
            Ok(thumb) => Self {
                source: source.to_string(),
                thumb: Some(thumb),
                error: None,
            },
            Err(e) => Self {
                source: source.to_string(),
                thumb: None,
                error: Some(e.message),
            },
        }
    }
}

/// 항목별 독립 작업을 bounded 워커에 나눠 입력 순서대로 모은다.
/// 생성 경로는 `with_file_lock`이 파일별로 직렬화하므로 디코드만 병렬화된다.
/// 워커가 패닉하면 scope가 그대로 전파해 호출자(join)가 에러로 받는다.
pub(crate) fn map_with_workers<I, T, F>(items: &[I], f: F) -> Vec<T>
where
    I: Sync,
    T: Send,
    F: Fn(&I) -> T + Sync,
{
    map_with_worker_state(items, || (), |(), item| f(item))
}

/// `map_with_workers`에 워커별 상태를 더한 형태. `init`은 워커 스레드마다 한 번
/// 불리므로, 열어 둔 아카이브 핸들처럼 스레드끼리 나눌 수 없는 자원을 둔다.
pub(crate) fn map_with_worker_state<I, S, T, N, F>(items: &[I], init: N, f: F) -> Vec<T>
where
    I: Sync,
    T: Send,
    N: Fn() -> S + Sync,
    F: Fn(&mut S, &I) -> T + Sync,
{
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results = Mutex::new(Vec::<(usize, T)>::with_capacity(items.len()));
    std::thread::scope(|scope| {
        for _ in 0..batch_workers().min(items.len()) {
            scope.spawn(|| {
                let mut state = init();
                loop {
                    let index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if index >= items.len() {
                        break;
                    }
                    let value = f(&mut state, &items[index]);
                    if let Ok(mut guard) = results.lock() {
                        guard.push((index, value));
                    }
                }
            });
        }
    });
    let mut results = results.into_inner().unwrap_or_default();
    results.sort_by_key(|(index, _)| *index);
    results.into_iter().map(|(_, value)| value).collect()
}

/// 썸네일 스트립 윈도우를 1회 invoke으로 처리. 실패 항목은 FE가 원본 폴백한다.
pub fn generate_thumbnails_batch(sources: &[String], max_side: u32) -> Vec<BatchThumb> {
    map_with_workers(sources, |s| {
        BatchThumb::from_result(s, generate_thumbnail(Path::new(s), max_side))
    })
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
    fn large_jpeg_thumbnail_keeps_aspect_through_scaled_decode() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("photo.jpg");
        write_png(&source.with_extension("png"), 1600, 800);
        image::open(source.with_extension("png"))
            .unwrap()
            .save(&source)
            .unwrap();

        let scaled = decode_jpeg_scaled(&source, 128).expect("scaled decode");
        assert_eq!((scaled.width(), scaled.height()), (200, 100));

        let thumb = generate_thumbnail(&source, 128).expect("generate");
        assert_eq!((thumb.width, thumb.height), (128, 64));
    }

    #[test]
    fn scaled_decode_skips_non_jpeg() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("plain.png");
        write_png(&source, 64, 64);
        assert!(decode_jpeg_scaled(&source, 32).is_none());
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
    fn svg_generates_bounded_jpeg_thumbnail() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("vector.svg");
        fs::write(
            &source,
            r##"<svg width="800" height="600" xmlns="http://www.w3.org/2000/svg"><circle cx="400" cy="300" r="200" fill="#0a0"/></svg>"##,
        )
        .unwrap();
        let info = generate_thumbnail(&source, 128).unwrap();
        assert_eq!((info.width, info.height), (128, 96));
    }

    #[test]
    fn avif_uses_jpeg_sidecar_thumb() {
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("../samples/sample.avif");
        let info = generate_thumbnail(&sample, 128).unwrap();
        assert_eq!((info.width, info.height), (128, 96));
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
