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

    fn write_png(path: &Path, width: u32, height: u32) {
        image::RgbImage::from_pixel(width, height, image::Rgb([20, 80, 160]))
            .save_with_format(path, image::ImageFormat::Png)
            .expect("write generated PNG");
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
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.png");
        write_png(&path, 800, 600);
        let info = load_image_blocking(path.to_str().unwrap(), None, None).expect("load");
        assert_eq!(info.mime_type, "image/png");
        assert_eq!((info.width, info.height), (Some(800), Some(600)));
        assert!(info.file_name.ends_with("sample.png"));
    }

    #[test]
    fn load_image_blocking_applies_the_display_cap() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.png");
        write_png(&path, 800, 600);
        let info =
            load_image_blocking(path.to_str().unwrap(), Some(512), None).expect("load capped");
        // 800x600은 상한을 넘으므로 축소 sidecar를 그린다.
        assert_ne!(info.file_path, info.source_path);
        assert_eq!(info.width, Some(512));
        assert_eq!(info.height, Some(384));
    }

    #[test]
    fn load_image_blocking_sniffs_content_and_preserves_source_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("renamed.heic");
        write_png(&path, 7, 3);
        let original = std::fs::read(&path).unwrap();

        let info = load_image_blocking(path.to_str().unwrap(), None, None).unwrap();

        assert_eq!(info.mime_type, "image/png");
        assert_eq!((info.width, info.height), (Some(7), Some(3)));
        assert_eq!(info.file_path, path.to_string_lossy());
        assert_eq!(info.source_path, path.to_string_lossy());
        assert_eq!(info.file_name, "renamed.heic");
        assert_eq!(info.file_size, original.len() as u64);
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }

    #[test]
    fn load_image_blocking_distinguishes_unknown_format_from_corrupt_pixels() {
        let dir = tempfile::tempdir().unwrap();
        for (name, bytes, code) in [
            ("notes.txt", &b"not an image"[..], ErrorCode::Unsupported),
            // PPM 헤더는 유효하지만 2픽셀에 필요한 RGB 데이터가 없다.
            (
                "truncated.ppm",
                &b"P6\n2 1\n255\n\xff"[..],
                ErrorCode::Corrupt,
            ),
        ] {
            let path = dir.path().join(name);
            std::fs::write(&path, bytes).unwrap();

            let err = load_image_blocking(path.to_str().unwrap(), None, None).unwrap_err();

            assert_eq!(err.code, code, "{name}: {}", err.message);
        }
    }

    #[test]
    fn load_image_blocking_does_not_upscale_at_the_minimum_cap() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("at-cap.png");
        write_png(&path, 512, 256);

        // 0도 최소 상한 512로 정규화되며, 긴 변이 상한과 같으면 원본을 쓴다.
        for max_side in [None, Some(0), Some(512)] {
            let info = load_image_blocking(path.to_str().unwrap(), max_side, None).unwrap();
            assert_eq!(info.file_path, path.to_string_lossy());
            assert_eq!((info.width, info.height), (Some(512), Some(256)));
        }
    }

    #[test]
    fn load_image_blocking_forwards_scaling_mode_and_preserves_alpha() {
        let dir = tempfile::tempdir().unwrap();
        for (name, alpha, mode, format) in [
            ("smooth.png", false, "smooth", image::ImageFormat::Jpeg),
            ("pixelated.png", false, "pixelated", image::ImageFormat::Png),
            ("alpha.png", true, "smooth", image::ImageFormat::Png),
        ] {
            let path = dir.path().join(name);
            if alpha {
                image::RgbaImage::from_pixel(800, 600, image::Rgba([20, 80, 160, 0]))
                    .save(&path)
                    .unwrap();
            } else {
                write_png(&path, 800, 600);
            }
            let original = std::fs::read(&path).unwrap();

            let info =
                load_image_blocking(path.to_str().unwrap(), Some(512), Some(mode.into())).unwrap();
            let output = std::fs::read(&info.file_path).unwrap();
            let decoded = image::load_from_memory(&output).unwrap();

            assert_ne!(info.file_path, info.source_path, "{name}");
            assert_eq!(info.source_path, path.to_string_lossy());
            assert_eq!(info.mime_type, "image/png");
            assert_eq!(info.file_size, original.len() as u64);
            assert_eq!((info.width, info.height), (Some(512), Some(384)));
            assert_eq!((decoded.width(), decoded.height()), (512, 384));
            assert_eq!(image::guess_format(&output).unwrap(), format, "{name}");
            if alpha {
                assert_eq!(decoded.to_rgba8().get_pixel(0, 0).0[3], 0);
            }
            assert_eq!(std::fs::read(&path).unwrap(), original);
        }
    }

    #[test]
    fn load_image_blocking_reports_a_missing_file() {
        let err = load_image_blocking("D:\\no-such-dir\\missing.png", None, None).unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
    }
}
