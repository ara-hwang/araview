//! 폴더 목록 조회와 드롭 경로 해석 커맨드.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use crate::app_error::{AppError, ErrorCode};
use crate::image::{is_supported_file, DirectoryImages};

pub fn get_directory_images_impl(
    file_path: &str,
    options: Option<DirListOptions>,
) -> Result<DirectoryImages, AppError> {
    let opts = options.unwrap_or_default();
    let path = Path::new(file_path);
    let parent = path
        .parent()
        .ok_or_else(|| AppError::not_found("Cannot get parent directory"))?;

    let (images, source) = crate::dir_cache::get_sorted_images_with_source(parent, &opts)?;
    // 지원 형식 판정은 스캔 시점에 끝났다. 비재귀 목록은 폴더 mtime으로
    // 검증되므로 삭제가 이미 반영되어 있고, 재귀 목록은 워처 이벤트가
    // 도착하기 전 틈이 있어 캐시에서 온 목록만 존재 여부를 다시 확인한다.
    // 방금 스캔한 목록은 디스크 상태 그대로라 항목별 stat을 다시 하지 않는다.
    let recheck_exists = opts.recursive && source == crate::dir_cache::ListingSource::Cached;
    let exists = if recheck_exists {
        files_exist(&images)
    } else {
        Vec::new()
    };
    let mut paths = Vec::with_capacity(images.len());
    let mut availability = Vec::with_capacity(images.len());
    for (index, entry) in images.iter().enumerate() {
        if recheck_exists && !exists[index] {
            continue;
        }
        paths.push(Arc::clone(&entry.path));
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

/// 이 개수부터 존재 확인을 병렬로 돌린다. stat은 항목끼리 독립이다.
const PARALLEL_EXISTS_MIN: usize = 256;

/// 목록 순서대로 각 파일이 아직 있는지 확인한다.
fn files_exist(images: &[crate::dir_cache::ImageEntry]) -> Vec<bool> {
    use rayon::prelude::*;

    let exists = |entry: &crate::dir_cache::ImageEntry| Path::new(&*entry.path).is_file();
    if images.len() >= PARALLEL_EXISTS_MIN {
        images.par_iter().map(exists).collect()
    } else {
        images.iter().map(exists).collect()
    }
}

/// 대소문자만 다른 철자로 들어와도 현재 파일을 찾는다. 목록은 호출자가 준
/// 부모 경로로 스캔되므로 보통 첫 비교에서 끝난다. 폴백에서 경로마다
/// canonicalize를 돌리면 수천 장 폴더에서 syscall 폭주가 나므로 하지 않는다.
fn index_of_current<S: AsRef<str>>(paths: &[S], current: &Path) -> Option<usize> {
    if let Some(pos) = paths.iter().position(|p| Path::new(p.as_ref()) == current) {
        return Some(pos);
    }
    let lowered = current.to_string_lossy().to_lowercase();
    paths
        .iter()
        .position(|p| p.as_ref().to_lowercase() == lowered)
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

/// 드롭된 경로를 해석한다. 파일이면 그대로, 디렉토리면 내부의 첫 이미지 경로를 반환.
pub fn resolve_dropped_path_impl(path: &str) -> Result<String, AppError> {
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
