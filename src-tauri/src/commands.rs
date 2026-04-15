use std::collections::HashMap;
use std::fs;
use std::io::BufReader;
use std::path::Path;

use crate::image::{get_mime_type, is_image_file, DirectoryImages, ImageInfo};

#[tauri::command]
pub fn load_image(file_path: String) -> Result<ImageInfo, String> {
    let path = Path::new(&file_path);

    if !path.exists() {
        return Err("File not found".to_string());
    }

    let mime_type = get_mime_type(path).ok_or_else(|| "Unsupported image format".to_string())?;

    let metadata = fs::metadata(path).map_err(|e| format!("Failed to read metadata: {}", e))?;

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(ImageInfo {
        file_path: file_path.clone(),
        mime_type: mime_type.to_string(),
        file_name,
        file_size: metadata.len(),
    })
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
            if entry_path.is_file() && is_image_file(&entry_path) {
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
        let entries =
            fs::read_dir(p).map_err(|e| format!("Failed to read directory: {}", e))?;

        let mut images: Vec<String> = Vec::new();
        for entry in entries {
            if let Ok(entry) = entry {
                let entry_path = entry.path();
                if entry_path.is_file() && is_image_file(&entry_path) {
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
