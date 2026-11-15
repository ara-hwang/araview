//! 단일 파일 이미지 로드 커맨드.

use std::path::Path;

use crate::app_error::AppError;
use crate::image::ImageInfo;

/// 디코드 같은 무거운 작업이라 호출자가 blocking 풀에서 실행한다.
pub fn load_image_blocking(
    file_path: &str,
    max_side: Option<u32>,
    image_scaling_mode: Option<String>,
) -> Result<ImageInfo, AppError> {
    let mode = parse_image_scaling_mode(image_scaling_mode);
    let info = crate::image::load_viewable_with_limit_mode(Path::new(file_path), max_side, mode)?;
    Ok(info)
}

pub fn parse_image_scaling_mode(
    image_scaling_mode: Option<String>,
) -> crate::scaled::ImageScalingMode {
    crate::scaled::ImageScalingMode::from_command(image_scaling_mode.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_error::ErrorCode;

    fn sample(name: &str) -> String {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples")
            .join(name)
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn parse_image_scaling_mode_maps_the_command_values() {
        assert_eq!(
            parse_image_scaling_mode(Some("pixelated".into())),
            crate::scaled::ImageScalingMode::Pixelated
        );
        assert_eq!(
            parse_image_scaling_mode(Some("auto".into())),
            crate::scaled::ImageScalingMode::Smooth
        );
        assert_eq!(
            parse_image_scaling_mode(Some("unknown".into())),
            crate::scaled::ImageScalingMode::Smooth
        );
        assert_eq!(
            parse_image_scaling_mode(None),
            crate::scaled::ImageScalingMode::Smooth
        );
    }

    #[test]
    fn load_image_blocking_returns_viewable_info() {
        let path = sample("sample.png");
        let info = load_image_blocking(&path, None, None).expect("load");
        assert_eq!(info.mime_type, "image/png");
        assert_eq!((info.width, info.height), (Some(800), Some(600)));
        assert!(info.file_name.ends_with("sample.png"));
    }

    #[test]
    fn load_image_blocking_applies_the_display_cap() {
        let path = sample("sample.png");
        let info = load_image_blocking(&path, Some(512), None).expect("load capped");
        // 800x600은 상한을 넘으므로 축소 sidecar를 그린다.
        assert_ne!(info.file_path, info.source_path);
        assert_eq!(info.width, Some(512));
        assert_eq!(info.height, Some(384));
    }

    #[test]
    fn load_image_blocking_reports_a_missing_file() {
        let err = load_image_blocking("D:\\no-such-dir\\missing.png", None, None).unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
    }
}
