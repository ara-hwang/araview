use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

use jpeg_encoder::{ColorType, Encoder};
use libheif_rs::{ColorSpace, HeifContext, LibHeif, RgbChroma};
use once_cell::sync::Lazy;

use crate::process_temp::process_temp_dir;

static SIDECAR_LOCKS: Lazy<Mutex<HashMap<String, Arc<Mutex<()>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

struct Rgb8 {
    width: u32,
    height: u32,
    bytes: Vec<u8>,
}

pub fn ensure_jpeg_sidecar(source: &Path) -> Result<PathBuf, String> {
    let dest = sidecar_path(source)?;
    let key = dest
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("sidecar")
        .to_string();
    let lock = {
        let mut map = SIDECAR_LOCKS.lock().map_err(|e| e.to_string())?;
        map.entry(key)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    let _guard = lock.lock().map_err(|e| e.to_string())?;
    if dest.exists() {
        return Ok(dest);
    }
    let rgb = decode_primary_rgb8(source)?;
    write_jpeg_atomic(&dest, &rgb)?;
    Ok(dest)
}

fn sidecar_path(source: &Path) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(source).unwrap_or_else(|_| source.to_path_buf());
    let meta = fs::metadata(source).map_err(|e| format!("Failed to read metadata: {}", e))?;
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
    let name = format!("{:016x}.jpg", hasher.finish());
    let dir = process_temp_dir()?.join("paint");
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create paint dir: {}", e))?;
    Ok(dir.join(name))
}

fn decode_primary_rgb8(source: &Path) -> Result<Rgb8, String> {
    let path = source
        .to_str()
        .ok_or_else(|| "HEIF path is not valid Unicode".to_string())?;
    let lib_heif = LibHeif::new();
    let ctx = HeifContext::read_from_file(path).map_err(|e| e.to_string())?;
    let handle = ctx.primary_image_handle().map_err(|e| e.to_string())?;
    let image = lib_heif
        .decode(&handle, ColorSpace::Rgb(RgbChroma::Rgb), None)
        .map_err(|e| e.to_string())?;
    let width = image.width();
    let height = image.height();
    let planes = image.planes();
    let plane = planes
        .interleaved
        .ok_or_else(|| "HEIF decode produced no interleaved RGB plane".to_string())?;
    let stride = plane.stride;
    let row_bytes = width as usize * 3;
    let mut bytes = Vec::with_capacity(row_bytes * height as usize);
    for y in 0..height as usize {
        let start = y * stride;
        let end = start + row_bytes;
        bytes.extend_from_slice(&plane.data[start..end]);
    }
    Ok(Rgb8 {
        width,
        height,
        bytes,
    })
}

fn write_jpeg_atomic(dest: &Path, rgb: &Rgb8) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create paint dir: {}", e))?;
    }
    let file_name = dest
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("sidecar.jpg");
    let tmp = dest.with_file_name(format!("{}.tmp-{}", file_name, std::process::id()));
    let width =
        u16::try_from(rgb.width).map_err(|_| "image too wide for JPEG sidecar".to_string())?;
    let height =
        u16::try_from(rgb.height).map_err(|_| "image too tall for JPEG sidecar".to_string())?;
    {
        let encoder = Encoder::new_file(&tmp, 90)
            .map_err(|e| format!("Failed to open JPEG sidecar: {}", e))?;
        encoder
            .encode(&rgb.bytes, width, height, ColorType::Rgb)
            .map_err(|e| format!("Failed to encode JPEG sidecar: {}", e))?;
    }
    match fs::rename(&tmp, dest) {
        Ok(()) => Ok(()),
        Err(_) if dest.exists() => {
            let _ = fs::remove_file(&tmp);
            Ok(())
        }
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(format!("Failed to publish JPEG sidecar: {}", e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidecar_path_is_under_paint_and_jpg() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("IMG_0001.heic");
        fs::write(&source, b"placeholder").unwrap();
        let dest = sidecar_path(&source).unwrap();
        assert_eq!(dest.extension().and_then(|e| e.to_str()), Some("jpg"));
        assert_eq!(
            dest.parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str()),
            Some("paint")
        );
        assert_ne!(dest.parent(), Some(source.parent().unwrap()));
    }

    #[test]
    fn write_jpeg_atomic_publishes_dest_not_tmp() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("out.jpg");
        let rgb = Rgb8 {
            width: 1,
            height: 1,
            bytes: vec![255, 0, 0],
        };
        write_jpeg_atomic(&dest, &rgb).unwrap();
        assert!(dest.exists());
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name())
            .filter(|n| n.to_string_lossy().contains(".tmp-"))
            .collect();
        assert!(leftovers.is_empty());
        let bytes = fs::read(&dest).unwrap();
        assert_eq!(&bytes[..2], &[0xFF, 0xD8]);
    }
}
