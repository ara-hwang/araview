//! Frontend와 공유하는 구조화 백엔드 에러 (Step 6).
//!
//! 모든 호스트 커맨드는 `Result<T, AppError>`를 반환한다.
//! 프론트는 `code`로 분기하고(`src/utils/appError.ts`), `message`는
//! 사람용 상세로 토스트/에러 카드에 그대로 보여준다.
//! 새 코드를 추가하면 프론트 매핑표에도 행을 추가할 것.

use serde::Serialize;
use std::io;

/// `appError.ts`의 `BackendErrorCode`와 1:1 대응 (snake_case).
#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NotFound,
    Permission,
    Unsupported,
    TooLarge,
    Corrupt,
    InvalidInput,
    AlreadyExists,
    Unknown,
}

/// Tauri command의 Err 타입. `message` 문구는 기존과 동일하게 유지해
/// 토스트/로그의 가독성을 바꾸지 않는다.
#[derive(Serialize, Clone, PartialEq, Eq, Debug)]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    pub fn permission(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Permission, message)
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unsupported, message)
    }

    pub fn too_large(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::TooLarge, message)
    }

    pub fn corrupt(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Corrupt, message)
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidInput, message)
    }

    pub fn already_exists(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::AlreadyExists, message)
    }

    pub fn unknown(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unknown, message)
    }

    /// `std::io::Error` 매핑. NotFound/PermissionDenied는 정확히,
    /// 나머지는 호출자가 고른 fallback (기존 FE 분류와 일치하게 고를 것).
    pub fn io(context: &str, error: io::Error, fallback: ErrorCode) -> Self {
        let code = match error.kind() {
            io::ErrorKind::NotFound => ErrorCode::NotFound,
            io::ErrorKind::PermissionDenied => ErrorCode::Permission,
            _ => fallback,
        };
        Self::new(code, format!("{context}: {error}"))
    }

    /// `image::ImageError` 매핑. Unsupported는 그대로, IO 실패(권한/잠금)는
    /// `io()`로, 나머지 디코드 실패는 Corrupt로 분류한다.
    pub fn image_error(context: &str, error: image::ImageError) -> Self {
        match error {
            image::ImageError::Unsupported(_) => Self::unsupported(format!("{context}: {error}")),
            image::ImageError::IoError(io) => Self::io(context, io, ErrorCode::Corrupt),
            _ => Self::corrupt(format!("{context}: {error}")),
        }
    }

    /// Mutex poison 등 내부 동기화 실패용.
    pub fn lock_poisoned(what: &str) -> Self {
        Self::unknown(format!("Failed to lock {what}"))
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AppError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_serialize_snake_case() {
        let err = AppError::not_found("File not found");
        let json = serde_json::to_value(&err).expect("serialize");
        assert_eq!(json["code"], "not_found");
        assert_eq!(json["message"], "File not found");
    }

    #[test]
    fn io_maps_not_found_and_permission() {
        let nf = AppError::io(
            "Failed to open",
            io::Error::new(io::ErrorKind::NotFound, "nope"),
            ErrorCode::Corrupt,
        );
        assert_eq!(nf.code, ErrorCode::NotFound);
        let perm = AppError::io(
            "Failed to open",
            io::Error::new(io::ErrorKind::PermissionDenied, "deny"),
            ErrorCode::Corrupt,
        );
        assert_eq!(perm.code, ErrorCode::Permission);
        let other = AppError::io(
            "Failed to open",
            io::Error::other("boom"),
            ErrorCode::Corrupt,
        );
        assert_eq!(other.code, ErrorCode::Corrupt);
    }

    #[test]
    fn image_error_maps_io_kinds() {
        let denied = AppError::image_error(
            "Cannot decode",
            image::ImageError::IoError(io::Error::new(io::ErrorKind::PermissionDenied, "deny")),
        );
        assert_eq!(denied.code, ErrorCode::Permission);
        let missing = AppError::image_error(
            "Cannot decode",
            image::ImageError::IoError(io::Error::new(io::ErrorKind::NotFound, "gone")),
        );
        assert_eq!(missing.code, ErrorCode::NotFound);
    }
}
