use base64::{engine::general_purpose, Engine as _};
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Serialize)]
pub struct ImageInfo {
    base64: String,
    mime_type: String,
    file_name: String,
    file_size: u64,
}

#[derive(Serialize)]
pub struct DirectoryImages {
    images: Vec<String>,
    current_index: usize,
}

fn get_mime_type(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "bmp" => Some("image/bmp"),
        "webp" => Some("image/webp"),
        "svg" => Some("image/svg+xml"),
        "ico" => Some("image/x-icon"),
        "tiff" | "tif" => Some("image/tiff"),
        "avif" => Some("image/avif"),
        _ => None,
    }
}

fn is_image_file(path: &Path) -> bool {
    get_mime_type(path).is_some()
}

#[tauri::command]
fn load_image(file_path: String) -> Result<ImageInfo, String> {
    let path = Path::new(&file_path);

    if !path.exists() {
        return Err("File not found".to_string());
    }

    let mime_type = get_mime_type(path).ok_or_else(|| "Unsupported image format".to_string())?;

    let data = fs::read(path).map_err(|e| format!("Failed to read file: {}", e))?;
    let metadata = fs::metadata(path).map_err(|e| format!("Failed to read metadata: {}", e))?;

    let base64_str = general_purpose::STANDARD.encode(&data);

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(ImageInfo {
        base64: base64_str,
        mime_type: mime_type.to_string(),
        file_name,
        file_size: metadata.len(),
    })
}

#[tauri::command]
fn get_directory_images(file_path: String) -> Result<DirectoryImages, String> {
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![load_image, get_directory_images])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
