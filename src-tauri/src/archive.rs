use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::image::is_image_file;

/// 아카이브 내부의 이미지 엔트리 이름을 정렬된 순서로 반환
pub fn list_archive_images(archive_path: &Path) -> Result<Vec<String>, String> {
    let ext = archive_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "cbz" => list_zip_images(archive_path),
        _ => Err("Unsupported archive format".to_string()),
    }
}

/// 아카이브에서 특정 엔트리를 임시 파일로 추출하고 경로를 반환
pub fn extract_archive_image(
    archive_path: &Path,
    entry_name: &str,
    temp_dir: &Path,
) -> Result<PathBuf, String> {
    let ext = archive_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "cbz" => extract_zip_image(archive_path, entry_name, temp_dir),
        _ => Err("Unsupported archive format".to_string()),
    }
}

fn list_zip_images(archive_path: &Path) -> Result<Vec<String>, String> {
    let file =
        fs::File::open(archive_path).map_err(|e| format!("Failed to open archive: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Failed to read ZIP: {}", e))?;

    let mut images: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read entry: {}", e))?;
        let name = entry.name().to_string();

        // 디렉토리 스킵, 숨김 파일(__MACOSX 등) 스킵
        if entry.is_dir() || name.starts_with("__") || name.starts_with('.') {
            continue;
        }

        // 엔트리 이름을 Path로 변환하여 이미지 확장자 확인
        let entry_path = Path::new(&name);
        if is_image_file(entry_path) {
            images.push(name);
        }
    }

    images.sort_by_key(|n| n.to_lowercase());
    Ok(images)
}

fn extract_zip_image(
    archive_path: &Path,
    entry_name: &str,
    temp_dir: &Path,
) -> Result<PathBuf, String> {
    let file =
        fs::File::open(archive_path).map_err(|e| format!("Failed to open archive: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Failed to read ZIP: {}", e))?;

    let mut entry = archive
        .by_name(entry_name)
        .map_err(|e| format!("Entry not found: {}", e))?;

    // 엔트리 이름에서 파일명만 추출 (하위 디렉토리 구조 무시)
    let file_name = Path::new(entry_name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("image");

    let out_path = temp_dir.join(file_name);

    let mut buf = Vec::new();
    entry
        .read_to_end(&mut buf)
        .map_err(|e| format!("Failed to read entry data: {}", e))?;

    fs::write(&out_path, &buf).map_err(|e| format!("Failed to write temp file: {}", e))?;

    Ok(out_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unsupported_archive_ext() {
        let result = list_archive_images(Path::new("file.tar"));
        assert!(result.is_err());
    }
}
