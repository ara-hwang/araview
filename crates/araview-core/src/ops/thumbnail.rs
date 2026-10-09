//! 파일/아카이브 썸네일 생성과 조회 커맨드.

use std::path::Path;

use crate::app_error::AppError;
use crate::archive;

use super::archive::archive_sub_dir;

/// 아카이브 엔트리 썸네일 생성 코어. AppHandle 없이 테스트 가능하다.
pub fn generate_archive_thumbnail_impl(
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

/// 아카이브 엔트리 썸네일 배치 코어. 추출 디렉터리 해시는 한 번만 계산하고,
/// 워커마다 아카이브를 한 번만 열어 자기 몫의 엔트리를 추출·축소한다.
pub fn generate_archive_thumbnails_batch_impl(
    archive_path: &Path,
    entry_names: &[String],
    max_side: u32,
) -> Result<Vec<crate::thumbnail::BatchThumb>, AppError> {
    if !archive_path.exists() {
        return Err(AppError::not_found("Archive not found"));
    }
    let sub_dir = archive_sub_dir(archive_path)?;
    Ok(crate::thumbnail::map_with_worker_state(
        entry_names,
        || archive::ArchiveExtractor::new(archive_path, &sub_dir),
        |extractor, name| {
            let thumb = extractor
                .extract(name, true)
                .and_then(|extracted| crate::thumbnail::generate_thumbnail(&extracted, max_side));
            crate::thumbnail::BatchThumb::from_result(name, thumb)
        },
    ))
}

/// 폴더 목록에 있는 아카이브 파일(.cbz 등)의 표지 썸네일. 첫 이미지 엔트리를 사용한다.
pub fn generate_archive_file_thumbnail_blocking(
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
    generate_archive_thumbnail_impl(archive_path, first, max_side)
}

/// 폴더 목록의 아카이브 표지 썸네일 일괄 처리. 항목별 성공/실패를 함께 반환한다.
pub fn generate_archive_file_thumbnails_batch_impl(
    archive_paths: &[String],
    max_side: u32,
) -> Vec<crate::thumbnail::BatchThumb> {
    crate::thumbnail::map_with_workers(archive_paths, |path| {
        crate::thumbnail::BatchThumb::from_result(
            path,
            generate_archive_file_thumbnail_blocking(Path::new(path), max_side),
        )
    })
}
