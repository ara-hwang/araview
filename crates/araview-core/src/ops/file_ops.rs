//! 휴지통 이동과 이름 변경 같은 원본 파일 조작 커맨드.

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

    // 이름이 그대로면 이동을 생략한다.
    let moved = new_path != old;
    if moved {
        if !is_supported_file(&new_path) {
            return Err(AppError::unsupported("Unsupported image format"));
        }
        // 존재 확인과 이동을 따로 하지 않는다. 덮어쓰지 않는 이동이 대상이 있으면
        // 실패하므로, 그 사이에 같은 이름의 파일이 생겨도 지우지 않는다.
        rename_no_replace(old, &new_path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                AppError::already_exists("A file with that name already exists")
            } else {
                AppError::io("Failed to rename", e, ErrorCode::Unknown)
            }
        })?;
    }

    crate::image::load_viewable_with_limit_mode(&new_path, max_side, mode).inspect_err(|_| {
        // 새 이름으로 읽지 못하면 되돌려, 실패 보고와 디스크 상태가 어긋나지 않게 한다.
        // 그사이 옛 이름에 다른 파일이 생겼으면 덮어쓰지 않고 새 이름에 남긴다.
        if moved {
            if let Err(e) = rename_no_replace(&new_path, old) {
                log::warn!("[rename] failed to restore {}: {e}", old.display());
            }
        }
    })
}

/// 같은 폴더 안에서 `old`를 `new`로 옮기되 다른 파일은 덮어쓰지 않는다.
///
/// 대소문자만 다른 이름은 볼륨에 따라 같은 파일(대소문자 무시)이거나 별개의
/// 파일(Linux·macOS 대소문자 구분 볼륨, Windows 대소문자 구분 폴더)이다. 이름으로
/// 추측하지 않고 두 경로의 OS 파일 ID가 같을 때만 같은 파일로 보고 임시 이름을
/// 거쳐 바꾼다. 별개의 파일이면 일반 이름 변경처럼 `AlreadyExists`로 거부된다.
fn rename_no_replace(old: &Path, new: &Path) -> std::io::Result<()> {
    if differs_only_in_case(old, new) && crate::file_identity::same_file(old, new)? {
        return crate::fs_move::move_case_only(old, new);
    }
    crate::fs_move::move_no_replace(old, new)
}

fn differs_only_in_case(old: &Path, new: &Path) -> bool {
    match (old.file_name(), new.file_name()) {
        (Some(old), Some(new)) => {
            old.to_string_lossy().to_lowercase() == new.to_string_lossy().to_lowercase()
        }
        _ => false,
    }
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
    if trimmed.chars().any(|c| {
        c.is_control()
            || matches!(
                c,
                '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
            )
    }) {
        return Err(AppError::invalid_input(
            "File name cannot contain control or bidirectional characters",
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

#[cfg(test)]
mod tests {
    use super::*;

    fn write_png(path: &Path) {
        image::RgbImage::from_pixel(2, 2, image::Rgb([10, 20, 30]))
            .save(path)
            .unwrap();
    }

    /// 대소문자 구분 볼륨에서 `a.png`와 `A.png`는 별개의 파일이다. 대소문자만 다른
    /// 이름으로 바꿔도 다른 파일을 덮어쓰지 않고 중복으로 거부해야 한다.
    #[test]
    fn case_only_rename_does_not_overwrite_a_distinct_file() {
        let dir = tempfile::tempdir().unwrap();
        let lower = dir.path().join("a.png");
        let upper = dir.path().join("A.png");
        std::fs::write(&lower, b"lower").unwrap();
        std::fs::write(&upper, b"upper").unwrap();
        if std::fs::read(&lower).unwrap() != b"lower" {
            // 대소문자 무시 볼륨(macOS·Windows 기본)에서는 재현할 수 없다.
            return;
        }

        let error = rename_file_impl_with_mode(
            lower.to_str().unwrap(),
            "A.png",
            None,
            crate::scaled::ImageScalingMode::Smooth,
        )
        .unwrap_err();

        assert_eq!(error.code, ErrorCode::AlreadyExists);
        assert_eq!(std::fs::read(&lower).unwrap(), b"lower");
        assert_eq!(std::fs::read(&upper).unwrap(), b"upper");
    }

    /// 다른 파일이 없으면 대소문자만 바꾸는 이름 변경은 그대로 성공한다.
    #[test]
    fn case_only_rename_succeeds_when_the_name_is_free() {
        let dir = tempfile::tempdir().unwrap();
        let lower = dir.path().join("page.png");
        write_png(&lower);

        let info = rename_file_impl_with_mode(
            lower.to_str().unwrap(),
            "PAGE.png",
            None,
            crate::scaled::ImageScalingMode::Smooth,
        )
        .unwrap();

        assert!(info.source_path.ends_with("PAGE.png"));
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("PAGE.png")]);
    }

    #[test]
    fn rename_refuses_an_existing_name() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("one.png");
        let to = dir.path().join("two.png");
        write_png(&from);
        std::fs::write(&to, b"keep").unwrap();

        let error = rename_file_impl_with_mode(
            from.to_str().unwrap(),
            "two.png",
            None,
            crate::scaled::ImageScalingMode::Smooth,
        )
        .unwrap_err();

        assert_eq!(error.code, ErrorCode::AlreadyExists);
        assert!(from.is_file());
        assert_eq!(std::fs::read(&to).unwrap(), b"keep");
    }

    #[test]
    fn rename_rejects_control_and_bidi_names() {
        assert!(validate_new_file_name("bad\nname.jpg").is_err());
        assert!(validate_new_file_name("bad\u{202E}name.jpg").is_err());
        assert!(validate_new_file_name("bidi\u{2066}name.jpg").is_err());
        assert!(validate_new_file_name("fine-name.jpg").is_ok());
    }
}
