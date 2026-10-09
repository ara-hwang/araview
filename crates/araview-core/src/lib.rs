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
pub mod file_assoc;
pub mod file_availability;
pub mod heif;
pub mod image;
pub mod image_info;
pub mod natural_sort;
pub mod ops;
pub mod orientation;
pub mod pixel_art;
pub mod process_temp;
pub mod psd_sidecar;
pub mod raster_sidecar;
pub mod scaled;
pub mod sidecar;
pub mod sniff;
pub mod stable_hash;
pub mod svg_raster;
pub mod svg_size;
pub mod thumb_shell;
pub mod thumbnail;
pub mod transcode;
