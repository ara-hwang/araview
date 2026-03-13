use base64::{engine::general_purpose, Engine as _};
use std::fs;
use std::io::Read;
use std::path::Path;

use crate::image::{get_mime_type, is_image_file, DirectoryImages, ImageInfo};

/// Separator used in virtual CBZ page paths: `<cbz_path>\0<entry_name>`.
/// The null byte cannot appear in valid file-system paths on any platform.
const CBZ_SEP: char = '\0';

/// Split a virtual CBZ page path into `(cbz_path, entry_name)`.
fn parse_cbz_virtual_path(file_path: &str) -> Option<(&str, &str)> {
    let sep = file_path.find(CBZ_SEP)?;
    Some((&file_path[..sep], &file_path[sep + 1..]))
}

/// Return `true` if the path refers to a `.cbz` file.
fn is_cbz_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("cbz"))
        .unwrap_or(false)
}

/// Load a single page (`entry_name`) from a CBZ archive at `cbz_path`.
fn load_cbz_page_impl(cbz_path: &str, entry_name: &str) -> Result<ImageInfo, String> {
    let file =
        fs::File::open(cbz_path).map_err(|e| format!("Failed to open CBZ file: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Failed to read CBZ archive: {}", e))?;

    let mut entry = archive
        .by_name(entry_name)
        .map_err(|_| format!("Page '{}' not found in CBZ", entry_name))?;

    let mut data = Vec::new();
    entry
        .read_to_end(&mut data)
        .map_err(|e| format!("Failed to read page from CBZ: {}", e))?;

    let mime_type = get_mime_type(Path::new(entry_name))
        .ok_or_else(|| "Unsupported image format in CBZ".to_string())?;

    let page_file_name = Path::new(entry_name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(entry_name)
        .to_string();

    let cbz_file_name = Path::new(cbz_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(cbz_path)
        .to_string();

    let file_size = data.len() as u64;
    let base64_str = general_purpose::STANDARD.encode(&data);

    Ok(ImageInfo {
        base64: base64_str,
        mime_type: mime_type.to_string(),
        file_name: format!("{} > {}", cbz_file_name, page_file_name),
        file_size,
    })
}

/// List all image pages inside a CBZ archive as virtual paths, returning a
/// `DirectoryImages` where each path has the form `<cbz_path>\0<entry_name>`.
fn get_cbz_pages(cbz_path: &str, current_entry: Option<&str>) -> Result<DirectoryImages, String> {
    let file =
        fs::File::open(cbz_path).map_err(|e| format!("Failed to open CBZ file: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Failed to read CBZ archive: {}", e))?;

    let mut pages: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read CBZ entry: {}", e))?;
        if !entry.is_dir() {
            let name = entry.name().to_string();
            // Use get_mime_type (not is_image_file) so that nested .cbz files
            // inside the archive are excluded from the page list.
            if get_mime_type(Path::new(&name)).is_some() {
                pages.push(format!("{}{}{}", cbz_path, CBZ_SEP, name));
            }
        }
    }

    pages.sort_by_cached_key(|p| {
        p.find(CBZ_SEP)
            .map(|pos| p[pos + 1..].to_lowercase())
            .unwrap_or_else(|| p.to_lowercase())
    });

    let current_index = if let Some(entry) = current_entry {
        let virtual_path = format!("{}{}{}", cbz_path, CBZ_SEP, entry);
        pages.iter().position(|p| p == &virtual_path).unwrap_or(0)
    } else {
        0
    };

    Ok(DirectoryImages {
        images: pages,
        current_index,
    })
}

#[tauri::command]
pub fn load_image(file_path: String) -> Result<ImageInfo, String> {
    // Handle virtual CBZ page paths: `<cbz_path>\0<entry_name>`
    if let Some((cbz_path, entry_name)) = parse_cbz_virtual_path(&file_path) {
        return load_cbz_page_impl(cbz_path, entry_name);
    }

    let path = Path::new(&file_path);

    if !path.exists() {
        return Err("File not found".to_string());
    }

    // Handle direct CBZ file: open and return its first page
    if is_cbz_path(path) {
        let dir_images = get_cbz_pages(&file_path, None)?;
        if let Some(first_page) = dir_images.images.first() {
            let (cbz_path, entry_name) = parse_cbz_virtual_path(first_page)
                .ok_or_else(|| "Invalid CBZ page path".to_string())?;
            return load_cbz_page_impl(cbz_path, entry_name);
        }
        return Err("CBZ archive is empty or contains no supported images".to_string());
    }

    let mime_type = get_mime_type(path).ok_or_else(|| "Unsupported image format".to_string())?;

    let data = fs::read(path).map_err(|e| format!("Failed to read file: {}", e))?;
    let metadata = fs::metadata(path).map_err(|e| format!("Failed to read metadata: {}", e))?;

    let (base64_str, served_mime) = (
        general_purpose::STANDARD.encode(&data),
        mime_type.to_string(),
    );

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(ImageInfo {
        base64: base64_str,
        mime_type: served_mime,
        file_name,
        file_size: metadata.len(),
    })
}

#[tauri::command]
pub fn get_directory_images(file_path: String) -> Result<DirectoryImages, String> {
    // Handle virtual CBZ page paths: resolve to the CBZ's page list
    let (actual_path, current_entry) = if let Some((cbz_path, entry_name)) =
        parse_cbz_virtual_path(&file_path)
    {
        (cbz_path.to_string(), Some(entry_name.to_string()))
    } else {
        (file_path.clone(), None)
    };

    let path = Path::new(&actual_path);

    // Handle CBZ files: return the list of pages inside the archive
    if is_cbz_path(path) {
        return get_cbz_pages(&actual_path, current_entry.as_deref());
    }

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
