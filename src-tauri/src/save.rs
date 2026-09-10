use std::fs;
use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageFormat, Rgb, RgbImage};

use crate::app_error::{AppError, ErrorCode};
use crate::image::{validate_new_file_name, ImageInfo};

/// C8+C9: 회전/반전 적용 + 포맷 변환 저장 옵션 (프론트 SaveEditsDialog와 대응)
#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveImageOptions {
    /// 시계 방향 회전 각도 (0 | 90 | 180 | 270)
    #[serde(default)]
    pub rotation_cw: u16,
    #[serde(default)]
    pub flip_h: bool,
    #[serde(default)]
    pub flip_v: bool,
    /// None이면 원본 유지. "png" | "jpg" | "jpeg" | "webp"
    #[serde(default)]
    pub format: Option<String>,
    /// true면 같은 경로에 덮어쓰기 (포맷 변경 시에는 항상 새 파일)
    #[serde(default)]
    pub overwrite: bool,
    /// overwrite=false일 때 파일명 (없으면 "{이름}-edited.{확장자}")
    #[serde(default)]
    pub new_file_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum OutFormat {
    Png,
    Jpeg,
    WebP,
}

impl OutFormat {
    fn extension(self) -> &'static str {
        match self {
            OutFormat::Png => "png",
            OutFormat::Jpeg => "jpg",
            OutFormat::WebP => "webp",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_lowercase().as_str() {
            "png" => Some(OutFormat::Png),
            "jpg" | "jpeg" => Some(OutFormat::Jpeg),
            "webp" => Some(OutFormat::WebP),
            _ => None,
        }
    }

    /// 원본 확장자 → 유지 가능한 출력 포맷 (HEIC/GIF/BMP 등은 None)
    fn from_source_ext(ext: &str) -> Option<Self> {
        match ext {
            "png" => Some(OutFormat::Png),
            "jpg" | "jpeg" => Some(OutFormat::Jpeg),
            "webp" => Some(OutFormat::WebP),
            _ => None,
        }
    }
}

fn source_ext(source: &Path) -> String {
    source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
}

#[tauri::command]
pub fn save_image_edits(
    app: tauri::AppHandle,
    file_path: String,
    options: SaveImageOptions,
) -> Result<ImageInfo, AppError> {
    let info = save_image_edits_impl(&file_path, &options)?;
    crate::commands::allow_asset_path(&app, Path::new(&info.file_path))?;
    Ok(info)
}

fn save_image_edits_impl(
    file_path: &str,
    options: &SaveImageOptions,
) -> Result<ImageInfo, AppError> {
    let source = Path::new(file_path);
    if !source.is_file() {
        return Err(AppError::not_found("File not found"));
    }

    let (dest, out_format) = resolve_save_target(source, options)?;
    let decoded = decode_source(source)?;
    let rgb = flatten_to_rgb8(&decoded);
    let transformed = apply_transform(&rgb, options)?;
    encode_image(&transformed, out_format, &dest)?;

    crate::image::load_viewable(&dest)
}

fn resolve_save_target(
    source: &Path,
    options: &SaveImageOptions,
) -> Result<(PathBuf, OutFormat), AppError> {
    let ext = source_ext(source);

    let requested: Option<OutFormat> = match &options.format {
        None => None,
        Some(f) => Some(
            OutFormat::from_name(f)
                .ok_or_else(|| AppError::invalid_input("Unsupported output format"))?,
        ),
    };
    let source_format = OutFormat::from_source_ext(&ext);
    let out_format = requested.or(source_format).unwrap_or_else(|| {
        // HEIC/HEIF 사진은 JPG, 그 외(GIF/BMP/TIFF 등)는 PNG로
        if ext == "heic" || ext == "heif" {
            OutFormat::Jpeg
        } else {
            OutFormat::Png
        }
    });

    let changed = Some(out_format) != source_format;
    if options.overwrite && !changed {
        return Ok((source.to_path_buf(), out_format));
    }

    // 새 파일: 지정명 or "{stem}-edited.{ext}", 중복 시 -2, -3...
    let parent = source
        .parent()
        .ok_or_else(|| AppError::not_found("Cannot get parent directory"))?;
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    let base_name = match &options.new_file_name {
        Some(n) if !n.trim().is_empty() => validate_new_file_name(n)?,
        _ => format!("{}-edited.{}", stem, out_format.extension()),
    };
    // 확장자는 선택 포맷으로 통일
    let mut candidate = parent.join(base_name);
    candidate.set_extension(out_format.extension());

    if !candidate.exists() {
        return Ok((candidate, out_format));
    }
    let dedup_stem = candidate
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image")
        .to_string();
    for n in 2.. {
        let next = parent.join(format!("{dedup_stem}-{n}.{}", out_format.extension()));
        if !next.exists() {
            return Ok((next, out_format));
        }
    }
    unreachable!("dedup loop always terminates")
}

fn decode_source(source: &Path) -> Result<DynamicImage, AppError> {
    let ext = source_ext(source);
    if ext == "heic" || ext == "heif" {
        let rgb = crate::heif::decode_primary_rgb8(source)?;
        return image::RgbImage::from_raw(rgb.width, rgb.height, rgb.bytes)
            .map(DynamicImage::ImageRgb8)
            .ok_or_else(|| AppError::corrupt("HEIF decode produced invalid buffer"));
    }
    image::open(source).map_err(|e| AppError::corrupt(format!("Cannot decode image: {e}")))
}

/// RGBA 등은 흰 배경에 합성해 RGB로 (투명 PNG가 검게 되는 것 방지)
fn flatten_to_rgb8(img: &DynamicImage) -> RgbImage {
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let mut out = RgbImage::new(w, h);
    for (x, y, p) in rgba.enumerate_pixels() {
        let a = f32::from(p[3]) / 255.0;
        let blend = |c: u8| (f32::from(c) * a + 255.0 * (1.0 - a)).round() as u8;
        out.put_pixel(x, y, Rgb([blend(p[0]), blend(p[1]), blend(p[2])]));
    }
    out
}

/// 화면 CSS 합성(translate * scale * rotate)과 일치하게 rotate 먼저, flip 나중
fn apply_transform(img: &RgbImage, options: &SaveImageOptions) -> Result<RgbImage, AppError> {
    let mut img = match options.rotation_cw % 360 {
        0 => img.clone(),
        90 => image::imageops::rotate90(img),
        180 => image::imageops::rotate180(img),
        270 => image::imageops::rotate270(img),
        _ => {
            return Err(AppError::invalid_input(
                "rotation must be 0, 90, 180 or 270",
            ))
        }
    };
    if options.flip_h {
        img = image::imageops::flip_horizontal(&img);
    }
    if options.flip_v {
        img = image::imageops::flip_vertical(&img);
    }
    Ok(img)
}

fn encode_image(img: &RgbImage, format: OutFormat, dest: &Path) -> Result<(), AppError> {
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| AppError::io("Failed to create dir", e, ErrorCode::Unknown))?;
        }
    }
    match format {
        OutFormat::Png | OutFormat::WebP => {
            let tauri_format = match format {
                OutFormat::Png => ImageFormat::Png,
                _ => ImageFormat::WebP,
            };
            img.save_with_format(dest, tauri_format)
                .map_err(|e| AppError::unknown(format!("Failed to save image: {e}")))
        }
        OutFormat::Jpeg => {
            let file = fs::File::create(dest)
                .map_err(|e| AppError::io("Failed to save image", e, ErrorCode::Unknown))?;
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(file, 90);
            encoder
                .encode_image(&DynamicImage::ImageRgb8(img.clone()))
                .map_err(|e| AppError::unknown(format!("Failed to save image: {e}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(suffix: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("tiv-save-{suffix}-{nanos}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write_rgb(path: &Path, w: u32, h: u32, pixels: &[(u8, u8, u8)]) {
        let mut img = RgbImage::new(w, h);
        for (i, (r, g, b)) in pixels.iter().enumerate() {
            let x = (i as u32) % w;
            let y = (i as u32) / w;
            img.put_pixel(x, y, Rgb([*r, *g, *b]));
        }
        img.save_with_format(path, ImageFormat::Png).unwrap();
    }

    fn opts(rotation_cw: u16, flip_h: bool, flip_v: bool) -> SaveImageOptions {
        SaveImageOptions {
            rotation_cw,
            flip_h,
            flip_v,
            ..Default::default()
        }
    }

    #[test]
    fn rotate90_mapping_is_clockwise() {
        // 2x1: R | B → 시계 90° → 1x2: R 위, B 아래
        let img = RgbImage::from_raw(2, 1, vec![255, 0, 0, 0, 0, 255]).unwrap();
        let out = apply_transform(&img, &opts(90, false, false)).unwrap();
        assert_eq!((out.width(), out.height()), (1, 2));
        assert_eq!(out.get_pixel(0, 0), &Rgb([255, 0, 0]));
        assert_eq!(out.get_pixel(0, 1), &Rgb([0, 0, 255]));
    }

    #[test]
    fn flip_then_order_matches_screen() {
        // 화면 CSS: rotate 적용 후 flip. 2x1 R|B, 회전 90 + 좌우반전
        // 회전 후 1x2 [R, B] → 좌우반전은 너비 1이라 불변
        let img = RgbImage::from_raw(2, 1, vec![255, 0, 0, 0, 0, 255]).unwrap();
        let out = apply_transform(&img, &opts(90, true, false)).unwrap();
        assert_eq!((out.width(), out.height()), (1, 2));

        // 2x2 체커로 flip_h 단독 검증
        let square = RgbImage::from_raw(
            2,
            2,
            vec![
                255, 0, 0, 0, 255, 0, // R G
                0, 0, 255, 255, 255, 0, // B Y
            ],
        )
        .unwrap();
        let flipped = apply_transform(&square, &opts(0, true, false)).unwrap();
        assert_eq!(flipped.get_pixel(0, 0), &Rgb([0, 255, 0]));
        assert_eq!(flipped.get_pixel(1, 0), &Rgb([255, 0, 0]));
    }

    #[test]
    fn rejects_bad_rotation() {
        let img = RgbImage::new(2, 2);
        assert!(apply_transform(&img, &opts(45, false, false)).is_err());
    }

    #[test]
    fn overwrite_png_applies_transform() {
        let dir = test_dir("overwrite");
        let path = dir.join("a.png");
        write_rgb(&path, 2, 1, &[(255, 0, 0), (0, 0, 255)]);

        let info = save_image_edits_impl(
            path.to_str().unwrap(),
            &SaveImageOptions {
                flip_h: true,
                overwrite: true,
                ..Default::default()
            },
        )
        .expect("save ok");
        assert_eq!(info.file_path, path.to_str().unwrap());
        let back = image::open(&path).unwrap().to_rgb8();
        assert_eq!(back.get_pixel(0, 0), &Rgb([0, 0, 255]));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn new_file_dedups_and_converts_format() {
        let dir = test_dir("newfile");
        let path = dir.join("a.png");
        write_rgb(&path, 1, 1, &[(1, 2, 3)]);

        let first = save_image_edits_impl(
            path.to_str().unwrap(),
            &SaveImageOptions {
                format: Some("jpg".to_string()),
                ..Default::default()
            },
        )
        .expect("save ok");
        assert!(first.file_path.ends_with("a-edited.jpg"));

        let second = save_image_edits_impl(
            path.to_str().unwrap(),
            &SaveImageOptions {
                format: Some("jpg".to_string()),
                ..Default::default()
            },
        )
        .expect("save ok");
        assert!(second.file_path.ends_with("a-edited-2.jpg"));
        // 원본 유지
        assert!(path.exists());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unsupported_source_fails_without_touching() {
        let dir = test_dir("unsupported");
        let path = dir.join("a.svg");
        fs::write(&path, "<svg></svg>").unwrap();
        let before = fs::read(&path).unwrap();

        let err = save_image_edits_impl(
            path.to_str().unwrap(),
            &SaveImageOptions {
                overwrite: true,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(!err.message.is_empty());
        assert_eq!(fs::read(&path).unwrap(), before);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rejects_bad_output_format() {
        let dir = test_dir("badfmt");
        let path = dir.join("a.png");
        write_rgb(&path, 1, 1, &[(1, 2, 3)]);

        let err = save_image_edits_impl(
            path.to_str().unwrap(),
            &SaveImageOptions {
                format: Some("tiff".to_string()),
                overwrite: true,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(err.code, crate::app_error::ErrorCode::InvalidInput);
        assert_eq!(err.message, "Unsupported output format");
        fs::remove_dir_all(&dir).ok();
    }
}
