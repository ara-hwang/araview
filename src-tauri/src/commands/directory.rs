//! 폴더 목록 조회와 드롭 경로 해석 커맨드.

use std::fs;
use std::path::Path;

use crate::app_error::{AppError, ErrorCode};
use crate::image::{is_supported_file, DirectoryImages};

use super::run_blocking;

#[tauri::command]
pub async fn get_directory_images(
    file_path: String,
    options: Option<DirListOptions>,
) -> Result<DirectoryImages, AppError> {
    run_blocking("directory listing", move || {
        get_directory_images_impl(&file_path, options)
    })
    .await
}

fn get_directory_images_impl(
    file_path: &str,
    options: Option<DirListOptions>,
) -> Result<DirectoryImages, AppError> {
    let opts = options.unwrap_or_default();
    let path = Path::new(file_path);
    let parent = path
        .parent()
        .ok_or_else(|| AppError::not_found("Cannot get parent directory"))?;

    let images = crate::dir_cache::get_sorted_images(parent, &opts)?;
    // 지원 형식 판정은 스캔 시점에 끝났다. 비재귀 목록은 폴더 mtime으로
    // 검증되므로 삭제가 이미 반영되어 있고, 재귀 목록은 워처 이벤트가
    // 도착하기 전 틈이 있어 그때만 존재 여부를 다시 확인한다.
    let recheck_exists = opts.recursive;
    let mut paths = Vec::with_capacity(images.len());
    let mut availability = Vec::with_capacity(images.len());
    for entry in images.iter() {
        if recheck_exists && !Path::new(&entry.path).is_file() {
            continue;
        }
        paths.push(entry.path.clone());
        availability.push(entry.availability);
    }

    // 보통은 문자열이 그대로 일치한다. 대소문자나 8.3 단축 경로처럼 철자가
    // 다르게 들어온 경우에만 canonical 비교로 폴백한다.
    let current_index = index_of_current(&paths, path).unwrap_or(0);

    Ok(DirectoryImages {
        images: paths,
        current_index,
        availability,
    })
}

/// 대소문자만 다른 철자로 들어와도 현재 파일을 찾는다. 목록은 호출자가 준
/// 부모 경로로 스캔되므로 보통 첫 비교에서 끝난다. 폴백에서 경로마다
/// canonicalize를 돌리면 수천 장 폴더에서 syscall 폭주가 나므로 하지 않는다.
fn index_of_current(paths: &[String], current: &Path) -> Option<usize> {
    if let Some(pos) = paths.iter().position(|p| Path::new(p) == current) {
        return Some(pos);
    }
    let lowered = current.to_string_lossy().to_lowercase();
    paths.iter().position(|p| p.to_lowercase() == lowered)
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

// 드롭된 경로를 해석한다. 파일이면 그대로, 디렉토리면 내부의 첫 이미지 경로를 반환.
#[tauri::command]
pub async fn resolve_dropped_path(path: String) -> Result<String, AppError> {
    run_blocking("dropped path", move || resolve_dropped_path_impl(&path)).await
}

fn resolve_dropped_path_impl(path: &str) -> Result<String, AppError> {
    let p = Path::new(path);

    if !p.exists() {
        return Err(AppError::not_found("Path not found"));
    }

    if p.is_file() {
        return Ok(path.to_string());
    }

    if p.is_dir() {
        let entries = fs::read_dir(p)
            .map_err(|e| AppError::io("Failed to read directory", e, ErrorCode::Corrupt))?;

        let mut images: Vec<String> = Vec::new();
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_file() && is_supported_file(&entry_path) {
                if let Some(s) = entry_path.to_str() {
                    images.push(s.to_string());
                }
            }
        }

        return images
            .into_iter()
            .min_by_key(|s| crate::natural_sort::natural_key(s))
            .ok_or_else(|| AppError::not_found("No images found in directory"));
    }

    Err(AppError::unsupported("Unsupported path type"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_error::ErrorCode;
    use crate::commands::test_support::*;

    fn list_paths(file_path: &str, options: Option<DirListOptions>) -> Vec<String> {
        get_directory_images_impl(file_path, options)
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

        let index = get_directory_images_impl(
            &new,
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
    fn dir_list_excludes_subdirectory_entries() {
        let dir = unique_dir("no-subdir-rows");
        let top = write_sized(&dir, "top.png", 10);
        let sub = dir.join("nested");
        fs::create_dir_all(&sub).expect("create sub");
        write_sized(&sub, "inner.png", 10);

        let names = file_names(&list_paths(&top, None));
        assert_eq!(names, vec!["top.png"]);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dir_list_drops_file_deleted_after_cached_listing() {
        let dir = unique_dir("deleted-after-cache");
        let a = write_sized(&dir, "a.png", 10);
        let b = write_sized(&dir, "b.png", 10);
        assert_eq!(file_names(&list_paths(&a, None)), vec!["a.png", "b.png"]);

        // 비재귀 목록은 존재 여부를 다시 확인하지 않으므로 삭제가 폴더
        // mtime 변경으로 캐시를 무효화해야 한다. Dev Drive(ReFS) 등 일부
        // 파일시스템은 삭제 시 mtime을 갱신하지 않으므로 상황을 직접 만든다.
        fs::remove_file(&b).expect("remove b");
        let before = fs::metadata(&dir)
            .and_then(|m| m.modified())
            .expect("dir mtime");
        crate::dir_cache::bump_dir_mtime(&dir, before + std::time::Duration::from_secs(2));
        assert_eq!(file_names(&list_paths(&a, None)), vec!["a.png"]);
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

    #[test]
    fn index_of_current_matches_exact_then_case_insensitive() {
        let paths = vec!["/pics/a.png".to_string(), "/pics/b.png".to_string()];
        assert_eq!(index_of_current(&paths, Path::new("/pics/b.png")), Some(1));
        assert_eq!(index_of_current(&paths, Path::new("/PICS/B.PNG")), Some(1));
        assert_eq!(index_of_current(&paths, Path::new("/pics/z.png")), None);
        assert_eq!(index_of_current(&[], Path::new("/pics/a.png")), None);
    }

    #[test]
    fn dir_list_options_default_to_name_ascending() {
        let opts = DirListOptions::default();
        assert_eq!(opts.sort_key, DirSortKey::Name);
        assert!(!opts.descending);
        assert!(!opts.recursive);
    }

    #[test]
    fn resolve_dropped_path_rejects_missing() {
        let err = resolve_dropped_path_impl("D:\\no-such-dir-commands\\nope.png").unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
    }

    #[test]
    fn resolve_dropped_path_returns_file_as_is() {
        let dir = unique_dir("drop-file");
        let file = write_sized(&dir, "photo.png", 8);
        let resolved = resolve_dropped_path_impl(&file).expect("file ok");
        assert_eq!(resolved, file);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn resolve_dropped_path_returns_first_image_in_dir() {
        let dir = unique_dir("drop-dir");
        write_sized(&dir, "b.png", 8);
        write_sized(&dir, "a.png", 8);
        write_sized(&dir, "notes.txt", 8);
        let resolved = resolve_dropped_path_impl(dir.to_str().unwrap()).expect("dir ok");
        assert!(resolved.ends_with("a.png"), "got {resolved}");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn resolve_dropped_path_rejects_dir_without_images() {
        let dir = unique_dir("drop-empty");
        write_sized(&dir, "notes.txt", 8);
        let err = resolve_dropped_path_impl(dir.to_str().unwrap()).unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
        fs::remove_dir_all(&dir).ok();
    }
}
