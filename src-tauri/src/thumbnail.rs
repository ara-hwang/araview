//! Thumbnail generation with on-disk cache (S2).
//!
//! `ImageNavBar` used to load full-size originals for its thumbnail strip.
//! This module decodes via the `image` crate, resizes to a bounded side
//! length and stores a JPEG under `process_temp/thumbs/`, reusing the
//! existing sidecar on repeat calls. Formats the `image` crate cannot decode
//! (e.g. SVG, HEIC, archive entry names) return an error so the frontend can
//! fall back to the original render path.

use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;

use crate::process_temp::process_temp_dir;

#[derive(Serialize, Debug)]
pub struct ThumbnailInfo {
    pub file_path: String,
    pub width: u32,
    pub height: u32,
}

const DEFAULT_MAX_SIDE: u32 = 256;
/// Upper bound for the thumbs directory; oldest files evicted past this.
const MAX_CACHE_BYTES: u64 = 500 * 1024 * 1024;

pub fn default_max_side() -> u32 {
    DEFAULT_MAX_SIDE
}

pub fn generate_thumbnail(source: &Path, max_side: u32) -> Result<ThumbnailInfo, String> {
    if !source.is_file() {
        return Err("File not found".to_string());
    }
    let max_side = max_side.clamp(32, 1024);
    let dest = thumb_path(source, max_side)?;
    if !dest.exists() {
        let img =
            image::open(source).map_err(|e| format!("Failed to decode image: {e}"))?;
        let thumb = img.thumbnail(max_side, max_side);
        write_jpeg_atomic(&dest, &thumb)?;
        // Best effort: eviction failures must not fail thumbnail delivery.
        enforce_cap(THUMBS_SUBDIR, MAX_CACHE_BYTES).ok();
    }
    let (width, height) =
        image::image_dimensions(&dest).map_err(|e| format!("Failed to read thumbnail: {e}"))?;
    Ok(ThumbnailInfo {
        file_path: dest.to_string_lossy().to_string(),
        width,
        height,
    })
}

const THUMBS_SUBDIR: &str = "thumbs";

fn thumbs_dir() -> Result<PathBuf, String> {
    let dir = process_temp_dir()?.join(THUMBS_SUBDIR);
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create thumbs dir: {e}"))?;
    Ok(dir)
}

fn thumb_path(source: &Path, max_side: u32) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(source).unwrap_or_else(|_| source.to_path_buf());
    let meta = fs::metadata(source).map_err(|e| format!("Failed to read metadata: {e}"))?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    canonical.hash(&mut hasher);
    mtime.hash(&mut hasher);
    meta.len().hash(&mut hasher);
    max_side.hash(&mut hasher);
    Ok(thumbs_dir()?.join(format!("{:016x}.jpg", hasher.finish())))
}

fn write_jpeg_atomic(dest: &Path, img: &image::DynamicImage) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create thumbs dir: {e}"))?;
    }
    let file_name = dest
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("thumb.jpg");
    // `.jpg` 접미사를 유지해야 `save`가 포맷을 추론할 수 있다.
    let tmp = dest.with_file_name(format!("{}.tmp-{}.jpg", file_name, std::process::id()));
    img.save(&tmp)
        .map_err(|e| format!("Failed to encode thumbnail: {e}"))?;
    match fs::rename(&tmp, dest) {
        Ok(()) => Ok(()),
        Err(_) if dest.exists() => {
            let _ = fs::remove_file(&tmp);
            Ok(())
        }
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(format!("Failed to publish thumbnail: {e}"))
        }
    }
}

/// Delete oldest files in `sub_dir` until total size is under `max_bytes`.
fn enforce_cap(sub_dir: &str, max_bytes: u64) -> Result<(), String> {
    let dir = process_temp_dir()?.join(sub_dir);
    enforce_cap_in(&dir, max_bytes)
}

/// Core of [`enforce_cap`] over an explicit directory (unit testable).
fn enforce_cap_in(dir: &Path, max_bytes: u64) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("Failed to list thumbs: {e}"))?;
    let mut files: Vec<(u128, u64, PathBuf)> = Vec::new();
    let mut total: u64 = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let (mtime, size) = entry
            .metadata()
            .map(|m| {
                (
                    m.modified()
                        .ok()
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_nanos())
                        .unwrap_or(0),
                    m.len(),
                )
            })
            .unwrap_or((0, 0));
        // Skip in-flight temp files, never evict partial writes.
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains(".tmp-"))
        {
            continue;
        }
        total = total.saturating_add(size);
        files.push((mtime, size, path));
    }
    if total <= max_bytes {
        return Ok(());
    }
    files.sort_by_key(|(mtime, _, _)| *mtime);
    for (_, size, path) in files {
        if total <= max_bytes {
            break;
        }
        if fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(size);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_png(path: &Path, width: u32, height: u32) {
        let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(
            width,
            height,
            |x, y| {
                image::Rgb([
                    (x % 256) as u8,
                    (y % 256) as u8,
                    ((x + y) % 256) as u8,
                ])
            },
        ));
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
        assert!(!err.is_empty());
    }

    #[test]
    fn missing_file_returns_not_found() {
        let err = generate_thumbnail(Path::new("no-such-thumb.png"), 64).unwrap_err();
        assert_eq!(err, "File not found");
    }

    #[test]
    fn enforce_cap_evicts_oldest_first() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["old.jpg", "mid.jpg", "new.jpg"] {
            fs::write(dir.path().join(name), vec![0u8; 100]).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        enforce_cap_in(dir.path(), 150).expect("evict");
        assert!(!dir.path().join("old.jpg").exists());
        assert!(!dir.path().join("mid.jpg").exists());
        assert!(dir.path().join("new.jpg").exists());
    }
}
