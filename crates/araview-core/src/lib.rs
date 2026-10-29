//! AraView의 UI 비의존 코어: 포맷 판별, 디코드/트랜스코드, 파생 이미지 캐시,
//! 아카이브, 썸네일, 메타데이터, 파일 작업. Tauri 앱과 GPUI 앱이 함께 쓴다.

pub mod app_error;
pub mod archive;
pub mod archive_index;
pub mod cache;
pub mod clipboard_png;
pub mod comic_info;
pub mod dir_cache;
pub mod display;
#[cfg(windows)]
pub mod file_assoc;
/// Windows 셸 연동이 없는 OS용 자리 표시. 설정 화면이 같은 타입을 쓰도록 모양만 맞추고
/// 모든 동작은 지원하지 않음을 알린다. 이 OS에서는 UI가 이 모듈을 호출하지 않는다.
#[cfg(not(windows))]
pub mod file_assoc {
    use crate::app_error::AppError;

    #[derive(serde::Serialize, Debug, Clone, PartialEq, Eq)]
    pub struct FileAssociation {
        pub extension: String,
        pub associated: bool,
        pub current_prog_id: Option<String>,
        pub needs_os_confirmation: bool,
    }

    pub fn list_associations() -> Result<Vec<FileAssociation>, AppError> {
        Err(AppError::unsupported(
            "File associations are only managed on Windows",
        ))
    }

    pub fn set_association(
        _extension: &str,
        _associate: bool,
        _parent_hwnd: isize,
    ) -> Result<FileAssociation, AppError> {
        list_associations().map(|mut list| list.remove(0))
    }

    pub fn open_default_apps_settings() -> Result<(), AppError> {
        list_associations().map(|_| ())
    }
}
pub mod file_availability;
pub mod file_identity;
pub mod heif;
pub mod image;
pub mod image_info;
pub mod jxl;
pub mod natural_sort;
pub mod ocr;
pub mod ops;
pub mod orientation;
pub mod process_temp;
mod preview;
pub mod psd_sidecar;
pub mod raster_sidecar;
pub mod raw;
pub mod scaled;
pub mod sidecar;
pub mod sniff;
pub mod stable_hash;
pub mod svg_raster;
pub mod svg_size;
#[cfg(windows)]
pub mod thumb_shell;
/// [`file_assoc`]와 같은 이유의 자리 표시.
#[cfg(not(windows))]
pub mod thumb_shell {
    use crate::app_error::AppError;

    #[derive(serde::Serialize, Debug, Clone, PartialEq, Eq)]
    pub struct PsdThumbStatus {
        pub registered: bool,
        pub clsid: String,
        pub dll_path: String,
        pub dll_exists: bool,
    }

    fn unsupported() -> AppError {
        AppError::unsupported("PSD thumbnails are only available in Windows Explorer")
    }

    pub fn status() -> Result<PsdThumbStatus, AppError> {
        Err(unsupported())
    }

    pub fn register() -> Result<PsdThumbStatus, AppError> {
        Err(unsupported())
    }

    pub fn unregister() -> Result<PsdThumbStatus, AppError> {
        Err(unsupported())
    }
}
pub mod thumbnail;
pub mod transcode;
pub mod video;
pub mod video_audio;
pub mod video_player;
