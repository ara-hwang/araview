//! 단일 파일 이미지 로드와 픽셀 아트 판정 커맨드.

use std::path::Path;

use crate::app_error::AppError;
use crate::image::ImageInfo;

use super::{allow_asset_path, run_blocking};

#[tauri::command]
pub async fn load_image(
    app: tauri::AppHandle,
    file_path: String,
    max_side: Option<u32>,
    image_scaling_mode: Option<String>,
    auto_detect_pixel_art: Option<bool>,
) -> Result<ImageInfo, AppError> {
    // 동기 커맨드는 메인 스레드에서 실행된다. 디코드 같은 무거운 작업은
    // blocking 풀로 넘겨 창 이벤트 루프와 다른 커맨드를 막지 않는다.
    run_blocking("image load", move || {
        load_image_blocking(
            &app,
            &file_path,
            max_side,
            image_scaling_mode,
            auto_detect_pixel_art,
        )
    })
    .await
}

fn load_image_blocking(
    app: &tauri::AppHandle,
    file_path: &str,
    max_side: Option<u32>,
    image_scaling_mode: Option<String>,
    auto_detect_pixel_art: Option<bool>,
) -> Result<ImageInfo, AppError> {
    let mode = parse_image_scaling_mode(image_scaling_mode, auto_detect_pixel_art);
    let info = crate::image::load_viewable_with_limit_mode(Path::new(file_path), max_side, mode)?;
    allow_asset_path(app, Path::new(&info.file_path))?;
    Ok(info)
}

pub(super) fn parse_image_scaling_mode(
    image_scaling_mode: Option<String>,
    auto_detect_pixel_art: Option<bool>,
) -> crate::scaled::ImageScalingMode {
    crate::scaled::ImageScalingMode::from_command(
        image_scaling_mode.as_deref(),
        auto_detect_pixel_art.unwrap_or(false),
    )
}

/// 이미지 표시에 사용할 픽셀 아트 힌트를 비동기로 분석한다.
/// 분석 실패는 이미지 열기 흐름에 영향을 주지 않도록 `uncertain`으로 안전하게 반환한다.
#[tauri::command]
pub async fn detect_pixel_art(
    file_path: String,
) -> Result<crate::pixel_art::PixelArtDetection, AppError> {
    run_blocking("pixel-art detection", move || {
        Ok(crate::pixel_art::detect_file(Path::new(&file_path)))
    })
    .await
}
