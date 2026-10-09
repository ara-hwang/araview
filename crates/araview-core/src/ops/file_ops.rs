//! 휴지통 이동과 이름 변경 같은 원본 파일 조작 커맨드.

use std::fs;
use std::path::Path;

use crate::app_error::{AppError, ErrorCode};
use crate::image::{is_supported_file, ImageInfo};

/// 현재 이미지를 OS 휴지통으로 이동 (영구 삭제 아님)
pub fn trash_file_impl(file_path: &str) -> Result<(), AppError> {
    let path = Path::new(file_path);

    if !path.exists() {
        return Err(AppError::not_found("File not found"));
    }

    if !path.is_file() {
        return Err(AppError::invalid_input("Only files can be moved to trash"));
    }

    trash::delete(path).map_err(|e| AppError::unknown(format!("Failed to move to trash: {e}")))
}

/// 같은 폴더 안에서 파일 이름 변경. 새 ImageInfo를 반환해 이름/MIME/크기를 일괄 갱신
pub fn rename_file_impl_with_mode(
    old_path: &str,
    new_name: &str,
    max_side: Option<u32>,
    mode: crate::scaled::ImageScalingMode,
) -> Result<ImageInfo, AppError> {
    let old = Path::new(old_path);

    if !old.is_file() {
        return Err(AppError::not_found("File not found"));
    }

    let name = validate_new_file_name(new_name)?;

    let parent = old
        .parent()
        .ok_or_else(|| AppError::not_found("Cannot get parent directory"))?;
    let new_path = parent.join(name);

    // 대소문자만 바꾸는 경우 등 동일 파일이면 이동 생략
    if new_path != old {
        if !is_supported_file(&new_path) {
            return Err(AppError::unsupported("Unsupported image format"));
        }
        if new_path.exists() {
            return Err(AppError::already_exists(
                "A file with that name already exists",
            ));
        }
        fs::rename(old, &new_path)
            .map_err(|e| AppError::io("Failed to rename", e, ErrorCode::Unknown))?;
    }

    crate::image::load_viewable_with_limit_mode(&new_path, max_side, mode)
}

/// Windows가 장치 이름으로 예약해 `File::create`가 원인 불명 에러로
/// 실패하는 이름들(`CON`, `NUL`, `COM1`...). 확장자가 붙어도(`CON.txt`)
/// 예약은 유지되므로 첫 점 앞부분으로 판정한다.
const WINDOWS_RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// 같은 폴더 내 새 파일명에 대한 공통 검증. trim된 이름을 반환
fn validate_new_file_name(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::invalid_input("File name is empty"));
    }
    if trimmed.contains(['/', '\\']) {
        return Err(AppError::invalid_input(
            "File name cannot contain path separators",
        ));
    }
    if trimmed.contains(['<', '>', ':', '"', '|', '?', '*']) {
        return Err(AppError::invalid_input(
            "File name contains invalid characters",
        ));
    }
    if trimmed.ends_with([' ', '.']) {
        return Err(AppError::invalid_input(
            "File name cannot end with a space or dot",
        ));
    }
    let stem = trimmed.split('.').next().unwrap_or(trimmed);
    if WINDOWS_RESERVED_NAMES
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
    {
        return Err(AppError::invalid_input("File name is reserved by Windows"));
    }
    Ok(trimmed.to_string())
}
