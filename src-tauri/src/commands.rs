use std::collections::HashMap;
use std::fs;
use std::io::BufReader;
use std::path::Path;

use crate::archive;
use crate::image::{is_archive_file, is_supported_file, DirectoryImages, ImageInfo};
use crate::process_temp::process_temp_dir;

#[tauri::command]
pub fn load_image(file_path: String) -> Result<ImageInfo, String> {
    crate::image::load_viewable(Path::new(&file_path))
}

#[tauri::command]
pub fn get_directory_images(file_path: String) -> Result<DirectoryImages, String> {
    let path = Path::new(&file_path);
    let parent = path.parent().ok_or("Cannot get parent directory")?;

    let mut images: Vec<String> = Vec::new();

    let entries = fs::read_dir(parent).map_err(|e| format!("Failed to read directory: {}", e))?;

    for entry in entries {
        if let Ok(entry) = entry {
            let entry_path = entry.path();
            if entry_path.is_file() && is_supported_file(&entry_path) {
                if let Some(path_str) = entry_path.to_str() {
                    images.push(path_str.to_string());
                }
            }
        }
    }

    images.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

    let current_index = images.iter().position(|p| p == &file_path).unwrap_or(0);

    Ok(DirectoryImages {
        images,
        current_index,
    })
}

// 드롭된 경로를 해석한다. 파일이면 그대로, 디렉토리면 내부의 첫 이미지 경로를 반환.
#[tauri::command]
pub fn resolve_dropped_path(path: String) -> Result<String, String> {
    let p = Path::new(&path);

    if !p.exists() {
        return Err("Path not found".to_string());
    }

    if p.is_file() {
        return Ok(path);
    }

    if p.is_dir() {
        let entries = fs::read_dir(p).map_err(|e| format!("Failed to read directory: {}", e))?;

        let mut images: Vec<String> = Vec::new();
        for entry in entries {
            if let Ok(entry) = entry {
                let entry_path = entry.path();
                if entry_path.is_file() && is_supported_file(&entry_path) {
                    if let Some(s) = entry_path.to_str() {
                        images.push(s.to_string());
                    }
                }
            }
        }

        if images.is_empty() {
            return Err("No images found in directory".to_string());
        }

        images.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
        return Ok(images.remove(0));
    }

    Err("Unsupported path type".to_string())
}

#[tauri::command]
pub fn get_exif_data(file_path: String) -> Result<HashMap<String, String>, String> {
    let path = Path::new(&file_path);

    if !path.exists() {
        return Err("File not found".to_string());
    }

    let file = fs::File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mut reader = BufReader::new(file);

    let exif_reader = exif::Reader::new();
    let exif = exif_reader
        .read_from_container(&mut reader)
        .map_err(|e| format!("No EXIF data found: {}", e))?;

    let mut data = HashMap::new();

    for field in exif.fields() {
        let tag_name = field.tag.to_string();
        let value = field.display_value().with_unit(&exif).to_string();
        data.insert(tag_name, value);
    }

    Ok(data)
}

/// 아카이브(CBZ/CBR) 파일 내부의 이미지 엔트리 목록을 반환
#[tauri::command]
pub fn get_archive_images(file_path: String) -> Result<DirectoryImages, String> {
    let path = Path::new(&file_path);

    if !path.exists() {
        return Err("File not found".to_string());
    }

    if !is_archive_file(path) {
        return Err("Not an archive file".to_string());
    }

    let images = archive::list_archive_images(path)?;

    if images.is_empty() {
        return Err("No images found in archive".to_string());
    }

    Ok(DirectoryImages {
        images,
        current_index: 0,
    })
}

/// 아카이브에서 특정 엔트리를 추출하여 ImageInfo를 반환
/// entry_name은 get_archive_images에서 반환된 엔트리 이름
#[tauri::command]
pub fn load_archive_image(archive_path: String, entry_name: String) -> Result<ImageInfo, String> {
    let arch_path = Path::new(&archive_path);

    if !arch_path.exists() {
        return Err("Archive not found".to_string());
    }

    let temp_dir = process_temp_dir()?;

    // 아카이브 이름 기반 서브 디렉토리를 만들어 충돌 방지
    let archive_stem = arch_path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("archive");
    let sub_dir = temp_dir.join(archive_stem);
    fs::create_dir_all(&sub_dir).map_err(|e| format!("Failed to create sub dir: {}", e))?;

    let extracted_path = archive::extract_archive_image(arch_path, &entry_name, &sub_dir)?;

    crate::image::load_viewable(&extracted_path)
}
