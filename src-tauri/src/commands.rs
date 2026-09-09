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
pub fn get_directory_images(
    file_path: String,
    options: Option<DirListOptions>,
) -> Result<DirectoryImages, String> {
    let opts = options.unwrap_or_default();
    let path = Path::new(&file_path);
    let parent = path.parent().ok_or("Cannot get parent directory")?;

    let mut images: Vec<ImageEntry> = Vec::new();
    collect_images(parent, opts.recursive, &mut images)?;

    sort_images(&mut images, &opts, parent);
    let paths: Vec<String> = images.into_iter().map(|e| e.path).collect();

    let current_index = paths.iter().position(|p| p == &file_path).unwrap_or(0);

    Ok(DirectoryImages {
        images: paths,
        current_index,
    })
}

/// 디렉토리 목록 정렬/수집 옵션 (프론트 settingsStore와 대응)
#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirListOptions {
    #[serde(default)]
    pub sort_key: DirSortKey,
    #[serde(default)]
    pub descending: bool,
    #[serde(default)]
    pub shuffle: bool,
    #[serde(default)]
    pub recursive: bool,
}

#[derive(Debug, Default, serde::Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum DirSortKey {
    #[default]
    Name,
    Date,
    Size,
}

struct ImageEntry {
    path: String,
    modified: Option<std::time::SystemTime>,
    size: u64,
}

fn collect_images(dir: &Path, recursive: bool, out: &mut Vec<ImageEntry>) -> Result<(), String> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries =
            fs::read_dir(&current).map_err(|e| format!("Failed to read directory: {}", e))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let entry_path = entry.path();
            if entry_path.is_dir() {
                if recursive {
                    stack.push(entry_path);
                }
                continue;
            }
            if !entry_path.is_file() || !is_supported_file(&entry_path) {
                continue;
            }
            let Some(path_str) = entry_path.to_str().map(str::to_string) else {
                continue;
            };
            let (modified, size) = entry
                .metadata()
                .map(|m| (m.modified().ok(), m.len()))
                .unwrap_or((None, 0));
            out.push(ImageEntry {
                path: path_str,
                modified,
                size,
            });
        }
    }
    Ok(())
}

fn sort_images(images: &mut [ImageEntry], opts: &DirListOptions, dir: &Path) {
    if opts.shuffle {
        // 새로고침해도 순서가 바뀌지 않게 폴더 경로 해시를 시드로 사용
        deterministic_shuffle(images, dir_seed(dir));
    } else {
        match opts.sort_key {
            DirSortKey::Name => {
                images.sort_by_key(|e| e.path.to_lowercase())
            }
            DirSortKey::Date => images.sort_by(|a, b| {
                a.modified
                    .cmp(&b.modified)
                    .then_with(|| a.path.to_lowercase().cmp(&b.path.to_lowercase()))
            }),
            DirSortKey::Size => images.sort_by(|a, b| {
                a.size
                    .cmp(&b.size)
                    .then_with(|| a.path.to_lowercase().cmp(&b.path.to_lowercase()))
            }),
        }
    }
    if opts.descending {
        images.reverse();
    }
}

fn dir_seed(dir: &Path) -> u64 {
    // djb2 해시 (추가 크레이트 없이 결정적 셔플용)
    let mut hash: u64 = 5381;
    for b in dir.to_string_lossy().bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(b as u64);
    }
    hash
}

fn deterministic_shuffle(images: &mut [ImageEntry], mut seed: u64) {
    if seed == 0 {
        seed = 0x9E3779B97F4A7C15;
    }
    // xorshift64* + Fisher-Yates
    let mut rand = move || {
        seed ^= seed >> 12;
        seed ^= seed << 25;
        seed ^= seed >> 27;
        seed.wrapping_mul(0x2545F4914F6CDD1D)
    };
    for i in (1..images.len()).rev() {
        let j = (rand() % (i as u64 + 1)) as usize;
        images.swap(i, j);
    }
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
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_file() && is_supported_file(&entry_path) {
                if let Some(s) = entry_path.to_str() {
                    images.push(s.to_string());
                }
            }
        }

        if images.is_empty() {
            return Err("No images found in directory".to_string());
        }

        images.sort_by_key(|s| s.to_lowercase());
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

#[tauri::command]
pub fn get_file_associations() -> Result<Vec<crate::file_assoc::FileAssociation>, String> {
    crate::file_assoc::list_associations()
}

#[tauri::command]
pub fn set_file_association(
    window: tauri::WebviewWindow,
    extension: String,
    associate: bool,
) -> Result<crate::file_assoc::FileAssociation, String> {
    crate::file_assoc::set_association(&extension, associate, window_hwnd(&window))
}

fn window_hwnd(window: &tauri::WebviewWindow) -> *mut std::ffi::c_void {
    window
        .hwnd()
        .map(|hwnd| hwnd.0)
        .unwrap_or(std::ptr::null_mut())
}

#[tauri::command]
pub fn set_all_file_associations(
    associate: bool,
) -> Result<Vec<crate::file_assoc::FileAssociation>, String> {
    crate::file_assoc::set_all_associations(associate)
}

#[tauri::command]
pub fn open_default_apps_settings() -> Result<(), String> {
    crate::file_assoc::open_default_apps_settings()
}

/// 현재 이미지를 OS 휴지통으로 이동 (영구 삭제 아님)
#[tauri::command]
pub fn trash_file(file_path: String) -> Result<(), String> {
    let path = Path::new(&file_path);

    if !path.exists() {
        return Err("File not found".to_string());
    }

    if !path.is_file() {
        return Err("Only files can be moved to trash".to_string());
    }

    trash::delete(path).map_err(|e| format!("Failed to move to trash: {}", e))
}

/// 같은 폴더 안에서 파일 이름 변경. 새 ImageInfo를 반환해 이름/MIME/크기를 일괄 갱신
#[tauri::command]
pub fn rename_file(old_path: String, new_name: String) -> Result<ImageInfo, String> {
    use crate::image::is_supported_file;

    let old = Path::new(&old_path);

    if !old.is_file() {
        return Err("File not found".to_string());
    }

    let name = crate::image::validate_new_file_name(&new_name)?;

    let parent = old.parent().ok_or("Cannot get parent directory")?;
    let new_path = parent.join(name);

    // 대소문자만 바꾸는 경우 등 동일 파일이면 이동 생략
    if new_path != old {
        if !is_supported_file(&new_path) {
            return Err("Unsupported image format".to_string());
        }
        if new_path.exists() {
            return Err("A file with that name already exists".to_string());
        }
        fs::rename(old, &new_path).map_err(|e| format!("Failed to rename: {}", e))?;
    }

    crate::image::load_viewable(&new_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_dir(suffix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("tiv-rename-{suffix}-{nanos}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn rename_rejects_missing_file() {
        let err = rename_file(
            "D:\\no-such-dir\\nope.png".to_string(),
            "new.png".to_string(),
        )
        .unwrap_err();
        assert_eq!(err, "File not found");
    }

    #[test]
    fn rename_rejects_bad_names() {
        let dir = unique_dir("bad");
        let old = dir.join("a.png");
        fs::write(&old, []).expect("write dummy");
        let old_str = old.to_str().unwrap().to_string();

        for bad in ["", "   ", "sub/dir.png", "a<b.png", "trail."] {
            let err = rename_file(old_str.clone(), bad.to_string()).unwrap_err();
            assert!(!err.is_empty(), "name {bad:?} should fail");
        }
        // 실패한 검증 뒤 원본은 그대로
        assert!(old.exists());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rename_rejects_unsupported_extension_before_moving() {
        let dir = unique_dir("ext");
        let old = dir.join("a.png");
        fs::write(&old, []).expect("write dummy");

        let err = rename_file(old.to_str().unwrap().to_string(), "b.txt".to_string()).unwrap_err();
        assert_eq!(err, "Unsupported image format");
        // rename 전에 차단되므로 원본 유지
        assert!(old.exists());
        assert!(!dir.join("b.txt").exists());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rename_rejects_duplicate() {
        let dir = unique_dir("dup");
        let old = dir.join("a.png");
        fs::write(&old, []).expect("write dummy");
        fs::write(dir.join("b.png"), []).expect("write dummy");

        let err = rename_file(old.to_str().unwrap().to_string(), "b.png".to_string()).unwrap_err();
        assert_eq!(err, "A file with that name already exists");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rename_success_returns_updated_info() {
        let dir = unique_dir("ok");
        let old = dir.join("a.png");
        fs::write(&old, [0u8; 16]).expect("write dummy");

        let info =
            rename_file(old.to_str().unwrap().to_string(), "b.png".to_string()).expect("rename ok");
        assert_eq!(info.file_name, "b.png");
        assert_eq!(info.file_size, 16);
        assert!(!old.exists());
        assert!(dir.join("b.png").exists());
        fs::remove_dir_all(&dir).ok();
    }

    fn write_sized(dir: &std::path::Path, name: &str, size: usize) -> String {
        let path = dir.join(name);
        fs::write(&path, vec![0u8; size]).expect("write dummy");
        path.to_str().unwrap().to_string()
    }

    fn list_paths(file_path: &str, options: Option<DirListOptions>) -> Vec<String> {
        get_directory_images(file_path.to_string(), options)
            .expect("list ok")
            .images
    }

    fn file_names(paths: &[String]) -> Vec<String> {
        paths
            .iter()
            .map(|p| {
                Path::new(p)
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn dir_list_default_is_name_ascending() {
        let dir = unique_dir("sort-default");
        let b = write_sized(&dir, "b.png", 10);
        write_sized(&dir, "a.png", 30);
        write_sized(&dir, "c.png", 20);

        let names = file_names(&list_paths(&b, None));
        assert_eq!(names, vec!["a.png", "b.png", "c.png"]);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dir_list_size_and_descending() {
        let dir = unique_dir("sort-size");
        let a = write_sized(&dir, "a.png", 30);
        write_sized(&dir, "b.png", 10);
        write_sized(&dir, "c.png", 20);

        let by_size = list_paths(
            &a,
            Some(DirListOptions {
                sort_key: DirSortKey::Size,
                ..Default::default()
            }),
        );
        assert_eq!(file_names(&by_size), vec!["b.png", "c.png", "a.png"]);

        let desc = list_paths(
            &a,
            Some(DirListOptions {
                descending: true,
                ..Default::default()
            }),
        );
        assert_eq!(file_names(&desc), vec!["c.png", "b.png", "a.png"]);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dir_list_date_orders_by_mtime() {
        let dir = unique_dir("sort-date");
        let old = write_sized(&dir, "old.png", 10);
        std::thread::sleep(std::time::Duration::from_millis(20));
        let new = write_sized(&dir, "new.png", 10);

        let names = file_names(&list_paths(
            &old,
            Some(DirListOptions {
                sort_key: DirSortKey::Date,
                ..Default::default()
            }),
        ));
        assert_eq!(names, vec!["old.png", "new.png"]);

        let index = get_directory_images(
            new.clone(),
            Some(DirListOptions {
                sort_key: DirSortKey::Date,
                ..Default::default()
            }),
        )
        .expect("list ok")
        .current_index;
        assert_eq!(index, 1);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dir_list_shuffle_is_stable_per_directory() {
        let dir = unique_dir("shuffle");
        let a = write_sized(&dir, "a.png", 10);
        for name in [
            "b.png", "c.png", "d.png", "e.png", "f.png", "g.png", "h.png",
        ] {
            write_sized(&dir, name, 10);
        }
        let opts = || {
            Some(DirListOptions {
                shuffle: true,
                ..Default::default()
            })
        };

        // 같은 폴더는 반복 조회해도 같은 순서
        let first = list_paths(&a, opts());
        let second = list_paths(&a, opts());
        assert_eq!(first, second);
        // 이름순과는 다를 가능성이 극히 높음 (8! = 40320)
        let ordered = list_paths(&a, None);
        assert_ne!(first, ordered);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dir_list_recursive_includes_subfolders() {
        let dir = unique_dir("recursive");
        let top = write_sized(&dir, "top.png", 10);
        let sub = dir.join("sub");
        fs::create_dir_all(&sub).expect("create sub");
        write_sized(&sub, "nested.png", 10);

        let flat = list_paths(&top, None);
        assert_eq!(flat.len(), 1);

        let all = list_paths(
            &top,
            Some(DirListOptions {
                recursive: true,
                ..Default::default()
            }),
        );
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|p| p.ends_with("nested.png")));
        fs::remove_dir_all(&dir).ok();
    }
}
