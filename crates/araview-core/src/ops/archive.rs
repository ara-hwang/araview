//! 아카이브 목록, 엔트리 추출 로드, 선추출 커맨드.

use std::fs;
use std::path::Path;

use crate::app_error::{AppError, ErrorCode};
use crate::archive;
use crate::image::{is_archive_file, ArchiveImages, ImageInfo};
use crate::process_temp::process_temp_dir;

use super::load::parse_image_scaling_mode;

/// 아카이브(CBZ/ZIP) 파일 내부의 이미지 엔트리 목록을 반환
pub fn get_archive_images_impl(path: &Path) -> Result<ArchiveImages, AppError> {
    if !path.exists() {
        return Err(AppError::not_found("File not found"));
    }

    if !is_archive_file(path) {
        return Err(AppError::unsupported("Not an archive file"));
    }

    // 인덱스 캐시로 조회한다. 오픈 흐름에서 comic_info·표지 커맨드가
    // 같은 스캔을 재사용하므로 아카이브당 파싱은 1회로 수렴한다.
    let entries = crate::archive_index::get_archive_entries(path)?;

    if entries.images.is_empty() {
        return Err(AppError::not_found("No images found in archive"));
    }

    Ok(ArchiveImages {
        images: entries.images,
        current_index: 0,
        availability: Vec::new(),
    })
}

/// 아카이브에서 특정 엔트리를 추출하여 ImageInfo를 반환
/// entry_name은 get_archive_images에서 반환된 엔트리 이름
pub fn load_archive_image_blocking(
    archive_path: &str,
    entry_name: &str,
    max_side: Option<u32>,
    protect: Option<bool>,
    image_scaling_mode: Option<String>,
    auto_detect_pixel_art: Option<bool>,
) -> Result<ImageInfo, AppError> {
    let arch_path = Path::new(archive_path);

    if !arch_path.exists() {
        return Err(AppError::not_found("Archive not found"));
    }

    let sub_dir = archive_sub_dir(arch_path)?;

    let extracted_path = archive::extract_archive_image_with_protection(
        arch_path,
        entry_name,
        &sub_dir,
        protect.unwrap_or(true),
    )?;

    let mode = parse_image_scaling_mode(image_scaling_mode, auto_detect_pixel_art);
    let info = crate::image::load_viewable_with_limit_mode(&extracted_path, max_side, mode)?;
    Ok(info)
}

pub(crate) fn archive_sub_dir(archive_path: &Path) -> Result<std::path::PathBuf, AppError> {
    let temp_dir = process_temp_dir()?;
    // 같은 stem을 가진 다른 아카이브가 캐시를 공유하지 않도록 canonical 경로 +
    // mtime + 크기로 하위 디렉터리를 구분한다. 영속 파일명에 쓰이므로 안정
    // 해시와 캐시 포맷 revision을 사용한다.
    let hash = crate::sidecar::file_identity_hash(archive_path, &[])?;
    let sub_dir = temp_dir
        .join(crate::cache::ARCHIVES_SUBDIR)
        .join(format!("{hash:016x}"));
    // 새 아카이브를 처음 열 때만 전체 temp 상한을 강제한다. 재사용 시에는
    // 매번 전체를 훑지 않아 페이지 넘김/썸네일 비용을 늘리지 않는다.
    let is_new = !sub_dir.is_dir();
    fs::create_dir_all(&sub_dir)
        .map_err(|e| AppError::io("Failed to create sub dir", e, ErrorCode::Unknown))?;
    if is_new {
        // 전체 트리 순회라 파일 수가 많으면 수 초가 걸린다. 연속 요청은
        // 실행 중인 한 스윕으로 모아 중복 스캔과 detached 스레드를 막는다.
        crate::process_temp::request_total_cap_scan();
    }
    Ok(sub_dir)
}

/// 이웃 페이지 선추출 (zip은 오픈 1회).
pub fn archive_prefetch_blocking(
    archive_path: &str,
    entry_names: &[String],
) -> Result<usize, AppError> {
    let arch_path = Path::new(archive_path);
    if !arch_path.exists() {
        return Err(AppError::not_found("Archive not found"));
    }
    let sub_dir = archive_sub_dir(arch_path)?;
    Ok(archive::prefetch_archive_images(
        arch_path,
        entry_names,
        &sub_dir,
    ))
}
