//! 단일 파일 이미지 로드와 픽셀 아트 판정 커맨드.

use std::path::Path;

use crate::app_error::AppError;
use crate::image::ImageInfo;

/// 디코드 같은 무거운 작업이라 호출자가 blocking 풀에서 실행한다.
pub fn load_image_blocking(
    file_path: &str,
    max_side: Option<u32>,
    image_scaling_mode: Option<String>,
    auto_detect_pixel_art: Option<bool>,
) -> Result<ImageInfo, AppError> {
    let mode = parse_image_scaling_mode(image_scaling_mode, auto_detect_pixel_art);
    let info = crate::image::load_viewable_with_limit_mode(Path::new(file_path), max_side, mode)?;
    Ok(info)
}

pub fn parse_image_scaling_mode(
    image_scaling_mode: Option<String>,
    auto_detect_pixel_art: Option<bool>,
) -> crate::scaled::ImageScalingMode {
    crate::scaled::ImageScalingMode::from_command(
        image_scaling_mode.as_deref(),
        auto_detect_pixel_art.unwrap_or(false),
    )
}
