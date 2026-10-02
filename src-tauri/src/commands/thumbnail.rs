//! 파일/아카이브 썸네일 생성과 조회 커맨드.

use std::path::Path;

use crate::app_error::AppError;
use crate::archive;

use super::archive::archive_sub_dir;
use super::{allow_asset_path, run_blocking};

/// 썸네일 스트립용 축소 JPEG 경로 반환. 디코드 불가 시 에러 → 프론트는 원본으로 폴백.
#[tauri::command]
pub async fn generate_thumbnail(
    file_path: String,
    max_side: Option<u32>,
) -> Result<crate::thumbnail::ThumbnailInfo, AppError> {
    run_blocking("thumbnail", move || {
        crate::thumbnail::generate_thumbnail(
            Path::new(&file_path),
            max_side.unwrap_or_else(crate::thumbnail::default_max_side),
        )
    })
    .await
}

/// 캐시에 이미 있는 썸네일만 반환한다. 없으면 null이며 프론트는 바로 원본을 그린다.
#[tauri::command]
pub async fn get_cached_thumbnail(file_path: String) -> Option<crate::thumbnail::ThumbnailInfo> {
    // 캐시 조회만 하므로 조인 실패도 캐시 미스로 취급한다(폴백은 원본 렌더).
    tauri::async_runtime::spawn_blocking(move || {
        crate::thumbnail::cached_thumbnail(Path::new(&file_path))
    })
    .await
    .unwrap_or_default()
}

/// 썸네일 윈도우 배치 처리. 항목별 성공/실패를 함께 반환한다.
#[tauri::command]
pub async fn generate_thumbnails_batch(
    file_paths: Vec<String>,
    max_side: Option<u32>,
) -> Result<Vec<crate::thumbnail::BatchThumb>, AppError> {
    run_blocking("thumbnail batch", move || {
        Ok(crate::thumbnail::generate_thumbnails_batch(
            &file_paths,
            max_side.unwrap_or_else(crate::thumbnail::default_max_side),
        ))
    })
    .await
}

/// 아카이브 엔트리 썸네일 생성 코어. AppHandle 없이 테스트 가능하다.
pub(crate) fn generate_archive_thumbnail_impl(
    archive_path: &Path,
    entry_name: &str,
    max_side: u32,
) -> Result<crate::thumbnail::ThumbnailInfo, AppError> {
    if !archive_path.exists() {
        return Err(AppError::not_found("Archive not found"));
    }
    let sub_dir = archive_sub_dir(archive_path)?;
    let extracted = archive::extract_archive_image(archive_path, entry_name, &sub_dir)?;
    crate::thumbnail::generate_thumbnail(&extracted, max_side)
}

/// 아카이브 엔트리용 축소 JPEG 경로 반환. 추출물과 썸네일 캐시를 재사용하므로
/// 썸네일 그리드처럼 여러 엔트리를 동시에 볼 때 원본 풀사이즈 로드를 피한다.
#[tauri::command]
pub async fn generate_archive_thumbnail(
    app: tauri::AppHandle,
    archive_path: String,
    entry_name: String,
    max_side: Option<u32>,
) -> Result<crate::thumbnail::ThumbnailInfo, AppError> {
    run_blocking("archive thumbnail", move || {
        generate_archive_thumbnail_blocking(
            &app,
            Path::new(&archive_path),
            &entry_name,
            max_side.unwrap_or_else(crate::thumbnail::default_max_side),
        )
    })
    .await
}

fn generate_archive_thumbnail_blocking(
    app: &tauri::AppHandle,
    archive_path: &Path,
    entry_name: &str,
    max_side: u32,
) -> Result<crate::thumbnail::ThumbnailInfo, AppError> {
    let thumb = generate_archive_thumbnail_impl(archive_path, entry_name, max_side)?;
    allow_asset_path(app, Path::new(&thumb.file_path))?;
    Ok(thumb)
}

/// 폴더 목록에 있는 아카이브 파일(.cbz 등)의 표지 썸네일. 첫 이미지 엔트리를 사용한다.
#[tauri::command]
pub async fn generate_archive_file_thumbnail(
    app: tauri::AppHandle,
    archive_path: String,
    max_side: Option<u32>,
) -> Result<crate::thumbnail::ThumbnailInfo, AppError> {
    run_blocking("archive file thumbnail", move || {
        generate_archive_file_thumbnail_blocking(
            &app,
            Path::new(&archive_path),
            max_side.unwrap_or_else(crate::thumbnail::default_max_side),
        )
    })
    .await
}

fn generate_archive_file_thumbnail_blocking(
    app: &tauri::AppHandle,
    archive_path: &Path,
    max_side: u32,
) -> Result<crate::thumbnail::ThumbnailInfo, AppError> {
    // 인덱스 캐시로 조회한다. 폴더 표지 배치는 같은 아카이브를 여러 커맨드
    // (목록+표지)에서 건드리므로 파싱이 1회로 수렴한다.
    let entries = crate::archive_index::get_archive_entries(archive_path)?;
    let first = entries
        .images
        .first()
        .ok_or_else(|| AppError::not_found("No images in archive"))?;
    let thumb = generate_archive_thumbnail_impl(archive_path, first, max_side)?;
    allow_asset_path(app, Path::new(&thumb.file_path))?;
    Ok(thumb)
}

/// 폴더 목록의 아카이브 표지 썸네일 일괄 처리. 항목별 성공/실패를 함께 반환한다.
#[tauri::command]
pub async fn generate_archive_file_thumbnails_batch(
    app: tauri::AppHandle,
    archive_paths: Vec<String>,
    max_side: Option<u32>,
) -> Result<Vec<crate::thumbnail::BatchThumb>, AppError> {
    run_blocking("archive cover batch", move || {
        let max_side = max_side.unwrap_or_else(crate::thumbnail::default_max_side);
        Ok(crate::thumbnail::map_with_workers(&archive_paths, |path| {
            match generate_archive_file_thumbnail_blocking(&app, Path::new(path), max_side) {
                Ok(thumb) => crate::thumbnail::BatchThumb {
                    source: path.clone(),
                    thumb: Some(thumb),
                    error: None,
                },
                Err(e) => crate::thumbnail::BatchThumb {
                    source: path.clone(),
                    thumb: None,
                    error: Some(e.message),
                },
            }
        }))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_error::ErrorCode;
    use crate::commands::test_support::*;
    use std::fs;

    #[test]
    fn archive_thumbnail_downscales_and_reuses_cache() {
        let dir = unique_dir("archive-thumb");
        let archive = write_cbz_fixture(&dir, "comic.cbz");

        let first = generate_archive_thumbnail_impl(&archive, "page.png", 64).expect("first");
        assert!(first.file_path.ends_with(".jpg"));
        assert_eq!((first.width, first.height), (64, 32));

        let second = generate_archive_thumbnail_impl(&archive, "page.png", 64).expect("second");
        assert_eq!(first.file_path, second.file_path);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn archive_thumbnail_missing_archive_is_not_found() {
        let err =
            generate_archive_thumbnail_impl(Path::new("no-such.cbz"), "page.png", 64).unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
    }

    #[test]
    fn archive_file_cover_matches_first_listed_entry() {
        let dir = unique_dir("archive-file-thumb");
        let archive = write_cbz_fixture(&dir, "comic.cbz");
        let entries = archive::list_archive_images(&archive).expect("list");
        let first = entries.first().expect("entry");
        let cover = generate_archive_thumbnail_impl(&archive, first, 64).expect("cover");
        let direct = generate_archive_thumbnail_impl(&archive, "page.png", 64).expect("page");
        assert_eq!(cover.file_path, direct.file_path);
        fs::remove_dir_all(&dir).ok();
    }
}
