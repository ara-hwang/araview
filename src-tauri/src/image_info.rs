//! EXIF 패널용 부가 정보: RGB 히스토그램 + 파일 상세.
//!
//! - 히스토그램: `image` 크레이트로 디코드 가능한 포맷 + HEIC/HEIF/PSD(전용 디코더).
//!   최대 변 256px로 다운샘플 후 집계하므로 대용량 파일도 빠르게 처리한다.
//!   `image` 크레이트가 디코드 불가한 입력(SVG, AVIF 등)은 에러를 내고,
//!   프론트는 해당 섹션을 숨긴다.
//! - 파일 상세: 크기/치수/색상 모드/비트뎁스/생성·수정 시각/EXIF 해상도(DPI)/
//!   ICC 프로파일(JPEG APP2, PNG iCCP 스캔). EXIF가 없어도 명령은 성공한다.

use std::fs;
use std::io::BufReader;
use std::path::Path;
use std::time::UNIX_EPOCH;

use serde::Serialize;

use crate::app_error::{AppError, ErrorCode};
use crate::image::PaintStrategy;

#[derive(Serialize, Debug, PartialEq)]
pub struct Histogram {
    pub r: Vec<u32>,
    pub g: Vec<u32>,
    pub b: Vec<u32>,
    /// 집계에 실제 사용한 픽셀 수 (다운샘플 시 축소본 기준).
    pub sampled_pixels: u64,
}

/// 히스토그램 집계 전 다운샘플 상한 (정사각형 기준 한 변).
const HISTOGRAM_MAX_SIDE: u32 = 256;

fn is_transcoded_source(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some("heic" | "heif" | "psd")
    )
}

/// 히스토그램/색상 판정용 RGB 디코드. HEIC/HEIF/PSD는 전용 디코더를 쓴다.
fn decode_rgb8(path: &Path) -> Result<image::RgbImage, AppError> {
    if !path.is_file() {
        return Err(AppError::not_found("File not found"));
    }
    if is_transcoded_source(path) {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        if ext == "psd" {
            let rgb = crate::psd_sidecar::decode_psd_rgb8(path)?;
            return image::RgbImage::from_raw(rgb.width, rgb.height, rgb.bytes)
                .ok_or_else(|| AppError::corrupt("PSD decode produced invalid buffer"));
        }
        let rgb = crate::heif::decode_primary_rgb8(path)?;
        return image::RgbImage::from_raw(rgb.width, rgb.height, rgb.bytes)
            .ok_or_else(|| AppError::corrupt("HEIF decode produced invalid buffer"));
    }
    if crate::image::get_mime_type(path).is_none() {
        return Err(AppError::unsupported("Unsupported image format"));
    }
    let dyn_img = image::open(path).map_err(|e| match e {
        image::ImageError::Unsupported(_) => {
            AppError::unsupported(format!("Cannot compute histogram: {e}"))
        }
        _ => AppError::corrupt(format!("Cannot compute histogram: {e}")),
    })?;
    Ok(dyn_img.to_rgb8())
}

pub fn compute_histogram(rgb: &image::RgbImage) -> Histogram {
    let sampled = if rgb.width().max(rgb.height()) > HISTOGRAM_MAX_SIDE {
        image::DynamicImage::ImageRgba8(image::imageops::thumbnail(
            &image::DynamicImage::ImageRgb8(rgb.clone()),
            HISTOGRAM_MAX_SIDE,
            HISTOGRAM_MAX_SIDE,
        ))
        .to_rgb8()
    } else {
        rgb.clone()
    };
    let mut r = vec![0u32; 256];
    let mut g = vec![0u32; 256];
    let mut b = vec![0u32; 256];
    for pixel in sampled.pixels() {
        r[pixel[0] as usize] += 1;
        g[pixel[1] as usize] += 1;
        b[pixel[2] as usize] += 1;
    }
    Histogram {
        r,
        g,
        b,
        sampled_pixels: u64::from(sampled.width()) * u64::from(sampled.height()),
    }
}

pub fn histogram_for_path(path: &Path) -> Result<Histogram, AppError> {
    Ok(compute_histogram(&decode_rgb8(path)?))
}

#[derive(Serialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum IccStatus {
    /// JPEG APP2 또는 PNG iCCP에서 프로파일을 찾음.
    Present,
    /// 검사 대상 포맷(jpg/png)이나 프로파일 없음.
    Absent,
    /// 검사하지 않는 포맷.
    Unchecked,
}

#[derive(Serialize, Debug)]
pub struct ImageDetails {
    pub file_path: String,
    pub file_size: u64,
    /// 렌더 바이트 기준 치수. SVG 등 미지원분은 null.
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// "rgb" | "rgba" | "grayscale" | "grayscale-alpha" | "unknown".
    /// 디코드 표현 기준이다 (예: GIF는 RGBA로 보고될 수 있다).
    pub color_mode: String,
    pub bits_per_channel: Option<u8>,
    /// 유닉스 초. FS가 지원하지 않으면 null.
    pub created_unix: Option<i64>,
    pub modified_unix: Option<i64>,
    /// EXIF X/YResolution + ResolutionUnit 기준 DPI. EXIF가 없으면 null.
    pub dpi_x: Option<f32>,
    pub dpi_y: Option<f32>,
    pub icc_status: IccStatus,
    pub icc_name: Option<String>,
    pub icc_bytes: Option<u64>,
}

pub fn details_for_path(path: &Path) -> Result<ImageDetails, AppError> {
    if !path.is_file() {
        return Err(AppError::not_found("File not found"));
    }
    let mime = crate::image::get_mime_type(path)
        .ok_or_else(|| AppError::unsupported("Unsupported image format"))?;
    let meta = fs::metadata(path)
        .map_err(|e| AppError::io("Failed to read metadata", e, ErrorCode::Corrupt))?;
    // 치수는 렌더 경로 기준 (HEIC/PSD는 JPEG sidecar).
    let paint_path = match crate::image::paint_strategy(mime) {
        PaintStrategy::Native => path.to_path_buf(),
        PaintStrategy::TranscodeJpeg if mime == crate::psd_sidecar::PSD_MIME => {
            crate::psd_sidecar::ensure_jpeg_sidecar(path)?
        }
        PaintStrategy::TranscodeJpeg => crate::heif::ensure_jpeg_sidecar(path)?,
    };
    let (width, height) = image::image_dimensions(&paint_path).ok().unzip();
    let (color_mode, bits_per_channel) = color_info(path, mime);
    let (dpi_x, dpi_y) = resolution_dpi(path);
    let (icc_status, icc_name, icc_bytes) = icc_info(path);
    Ok(ImageDetails {
        file_path: path.to_string_lossy().to_string(),
        file_size: meta.len(),
        width,
        height,
        color_mode,
        bits_per_channel,
        created_unix: unix_time(meta.created().ok()),
        modified_unix: unix_time(meta.modified().ok()),
        dpi_x,
        dpi_y,
        icc_status,
        icc_name,
        icc_bytes,
    })
}

fn unix_time(time: Option<std::time::SystemTime>) -> Option<i64> {
    time?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

fn color_info(path: &Path, mime: &str) -> (String, Option<u8>) {
    if mime == "image/heic" || mime == "image/heif" {
        return ("rgb".to_string(), Some(8));
    }
    if mime == crate::psd_sidecar::PSD_MIME {
        return ("rgba".to_string(), Some(8));
    }
    let dyn_img = match image::open(path) {
        Ok(img) => img,
        Err(_) => return ("unknown".to_string(), None),
    };
    let (mode, bits) = match dyn_img.color() {
        image::ColorType::L8 => ("grayscale", 8),
        image::ColorType::La8 => ("grayscale-alpha", 8),
        image::ColorType::Rgb8 => ("rgb", 8),
        image::ColorType::Rgba8 => ("rgba", 8),
        image::ColorType::L16 => ("grayscale", 16),
        image::ColorType::La16 => ("grayscale-alpha", 16),
        image::ColorType::Rgb16 => ("rgb", 16),
        image::ColorType::Rgba16 => ("rgba", 16),
        image::ColorType::Rgb32F => ("rgb", 32),
        image::ColorType::Rgba32F => ("rgba", 32),
        _ => ("unknown", 0),
    };
    let bits = if mode == "unknown" { None } else { Some(bits) };
    (mode.to_string(), bits)
}

/// EXIF X/YResolution + ResolutionUnit을 DPI로. EXIF가 없으면 (None, None).
fn resolution_dpi(path: &Path) -> (Option<f32>, Option<f32>) {
    let read = || -> Option<(Option<f32>, Option<f32>)> {
        let file = fs::File::open(path).ok()?;
        let mut reader = BufReader::new(file);
        let exif = exif::Reader::new().read_from_container(&mut reader).ok()?;
        let number = |tag: exif::Tag| {
            let field = exif.get_field(tag, exif::In::PRIMARY)?;
            match &field.value {
                exif::Value::Rational(v) => v.first().map(|r| r.to_f32()),
                exif::Value::SRational(v) => v.first().map(|r| r.to_f32()),
                exif::Value::Short(v) => v.first().map(|n| f32::from(*n)),
                exif::Value::Long(v) => v.first().map(|n| *n as f32),
                exif::Value::Ascii(v) => v
                    .first()
                    .and_then(|bytes| std::str::from_utf8(bytes).ok())
                    .and_then(|s| s.trim_matches('\0').trim().parse::<f32>().ok()),
                _ => None,
            }
        };
        let x = number(exif::Tag::XResolution);
        let y = number(exif::Tag::YResolution);
        if x.is_none() && y.is_none() {
            return None;
        }
        Some(dpi_from_values(x, y, unit_factor(&exif)))
    };
    read().unwrap_or((None, None))
}

/// ResolutionUnit(2=inch 기본, 3=cm)을 DPI 환산 계수로.
fn unit_factor(exif: &exif::Exif) -> f32 {
    let unit = exif
        .get_field(exif::Tag::ResolutionUnit, exif::In::PRIMARY)
        .and_then(|f| f.value.get_uint(0));
    if unit == Some(3) {
        2.54
    } else {
        1.0
    }
}

fn dpi_from_values(x: Option<f32>, y: Option<f32>, unit_factor: f32) -> (Option<f32>, Option<f32>) {
    let factor = if unit_factor > 0.0 { unit_factor } else { 1.0 };
    let convert = |v: Option<f32>| {
        v.filter(|n| n.is_finite() && *n > 0.0)
            .map(|n| (n / factor * 10.0).round() / 10.0)
    };
    (convert(x), convert(y))
}

/// ICC 프로파일 검사. jpg/png만 스캔하고 나머지는 Unchecked.
fn icc_info(path: &Path) -> (IccStatus, Option<String>, Option<u64>) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    let found = match ext.as_str() {
        "jpg" | "jpeg" => icc_from_jpeg(path),
        "png" => icc_from_png(path),
        _ => return (IccStatus::Unchecked, None, None),
    };
    match found {
        Some((name, bytes)) => (IccStatus::Present, name, Some(bytes)),
        None => (IccStatus::Absent, None, None),
    }
}

/// JPEG APP2("ICC_PROFILE") 세그먼트 스캔. SOS 이후는 보지 않는다.
fn icc_from_jpeg(path: &Path) -> Option<(Option<String>, u64)> {
    let bytes = fs::read(path).ok()?;
    if bytes.len() < 4 || bytes[0..2] != [0xFF, 0xD8] {
        return None;
    }
    let mut pos = 2usize;
    let mut total: u64 = 0;
    let mut found = false;
    while pos + 4 <= bytes.len() && pos < 1_048_576 {
        if bytes[pos] != 0xFF {
            pos += 1;
            continue;
        }
        let marker = bytes[pos + 1];
        // 연속 FF 패딩, 단독 마커는 길이 없이 건너뛴다.
        if marker == 0xFF || marker == 0x00 {
            pos += 1;
            continue;
        }
        if marker == 0xDA {
            break; // SOS: 이후는 압축 스트림이라 메타데이터 없음.
        }
        if marker == 0xD8 || marker == 0xD9 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            pos += 2;
            continue;
        }
        let len = u16::from_be_bytes([bytes[pos + 2], bytes[pos + 3]]) as usize;
        let end = pos.checked_add(2)?.checked_add(len)?;
        if len < 2 || end > bytes.len() {
            break;
        }
        if marker == 0xE2 {
            let seg = &bytes[pos + 4..end];
            // "ICC_PROFILE\0"(12) + seq(1) + count(1) + 프로파일 데이터.
            if seg.starts_with(b"ICC_PROFILE\0") {
                found = true;
                total += (seg.len() as u64).saturating_sub(14);
            }
        }
        pos = end;
    }
    found.then_some((None, total))
}

/// PNG iCCP 청크 스캔. 프로파일 이름과 압축 데이터 길이를 반환.
fn icc_from_png(path: &Path) -> Option<(Option<String>, u64)> {
    const SIG: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    let bytes = fs::read(path).ok()?;
    if bytes.len() < 8 || bytes[0..8] != *SIG {
        return None;
    }
    let mut pos = 8usize;
    while pos + 8 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[pos..pos + 4].try_into().ok()?) as usize;
        let chunk_end = pos.checked_add(8)?.checked_add(len)?.checked_add(4)?;
        if chunk_end > bytes.len() {
            break;
        }
        let typ = &bytes[pos + 4..pos + 8];
        if typ == b"IEND" {
            break;
        }
        if typ == b"iCCP" {
            let data = &bytes[pos + 8..pos + 8 + len];
            let nul = data.iter().position(|&b| b == 0)?;
            let name = String::from_utf8_lossy(&data[..nul]).into_owned();
            // 이름 NUL(1) + 압축 방식(1) 뒤가 압축 프로파일.
            let profile_len = (len as u64).saturating_sub(nul as u64 + 2);
            return Some((Some(name), profile_len));
        }
        pos = chunk_end;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn write_rgb_png(path: &Path, width: u32, height: u32, px: [u8; 3]) {
        let img =
            image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(width, height, |_, _| {
                image::Rgb(px)
            }));
        img.save(path).expect("write png fixture");
    }

    #[test]
    fn histogram_counts_solid_color() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("red.png");
        write_rgb_png(&source, 4, 3, [255, 0, 0]);

        let hist = histogram_for_path(&source).expect("histogram");
        assert_eq!(hist.sampled_pixels, 12);
        assert_eq!(hist.r[255], 12);
        assert_eq!(hist.r.iter().sum::<u32>(), 12);
        assert_eq!(hist.g[0], 12);
        assert_eq!(hist.b[0], 12);
    }

    #[test]
    fn histogram_downsamples_large_images() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("big.png");
        write_rgb_png(&source, 512, 512, [10, 20, 30]);

        let hist = histogram_for_path(&source).expect("histogram");
        assert_eq!(hist.sampled_pixels, 256 * 256);
        assert_eq!(hist.r[10], 256 * 256);
        assert_eq!(hist.g[20], 256 * 256);
        assert_eq!(hist.b[30], 256 * 256);
    }

    #[test]
    fn histogram_grayscale_has_equal_channels() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("gray.png");
        let img = image::DynamicImage::ImageLuma8(image::GrayImage::from_fn(5, 5, |x, y| {
            image::Luma([((x + y * 5) % 256) as u8])
        }));
        img.save(&source).expect("write gray fixture");

        let hist = histogram_for_path(&source).expect("histogram");
        assert_eq!(hist.r, hist.g);
        assert_eq!(hist.g, hist.b);
        assert_eq!(hist.r.iter().sum::<u32>(), 25);
    }

    #[test]
    fn histogram_missing_file_is_not_found() {
        let err = histogram_for_path(Path::new("no-such-hist.png")).unwrap_err();
        assert_eq!(err.code, crate::app_error::ErrorCode::NotFound);
    }

    #[test]
    fn histogram_unsupported_extension() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("notes.txt");
        fs::write(&source, b"hello").unwrap();
        let err = histogram_for_path(&source).unwrap_err();
        assert_eq!(err.code, crate::app_error::ErrorCode::Unsupported);
    }

    #[test]
    fn histogram_and_details_support_psd() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("design.psd");
        fs::write(
            &source,
            crate::psd_sidecar::minimal_psd_bytes(4, 4, [255, 0, 0]),
        )
        .unwrap();

        let hist = histogram_for_path(&source).expect("psd histogram");
        assert_eq!(hist.sampled_pixels, 16);
        assert_eq!(hist.r[255], 16);
        assert_eq!(hist.g[0], 16);
        assert_eq!(hist.b[0], 16);

        let details = details_for_path(&source).expect("psd details");
        assert_eq!((details.width, details.height), (Some(4), Some(4)));
        assert_eq!(details.color_mode, "rgba");
        assert_eq!(details.bits_per_channel, Some(8));
        assert_eq!(details.icc_status, IccStatus::Unchecked);
    }

    #[test]
    fn details_png_reports_size_color_and_no_icc() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("photo.png");
        write_rgb_png(&source, 16, 9, [1, 2, 3]);

        let details = details_for_path(&source).expect("details");
        assert_eq!(details.file_size, fs::metadata(&source).unwrap().len());
        assert_eq!((details.width, details.height), (Some(16), Some(9)));
        assert_eq!(details.color_mode, "rgb");
        assert_eq!(details.bits_per_channel, Some(8));
        assert_eq!((details.dpi_x, details.dpi_y), (None, None));
        assert_eq!(details.icc_status, IccStatus::Absent);
        assert!(details.created_unix.is_some() || details.modified_unix.is_some());
    }

    #[test]
    fn details_unsupported_extension() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("notes.txt");
        fs::write(&source, b"hello").unwrap();
        let err = details_for_path(&source).unwrap_err();
        assert_eq!(err.code, crate::app_error::ErrorCode::Unsupported);
    }

    #[test]
    fn details_missing_file_is_not_found() {
        let err = details_for_path(Path::new("no-such-detail.png")).unwrap_err();
        assert_eq!(err.code, crate::app_error::ErrorCode::NotFound);
    }

    #[test]
    fn jpeg_app2_icc_is_detected() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("icc.jpg");
        // SOI + APP2(ICC_PROFILE, 데이터 4B) + EOI.
        let mut bytes = vec![0xFF, 0xD8];
        let mut seg = b"ICC_PROFILE\0".to_vec();
        seg.extend_from_slice(&[1, 1, 0xDE, 0xAD, 0xBE, 0xEF]);
        let len = (seg.len() + 2) as u16;
        bytes.extend_from_slice(&[0xFF, 0xE2]);
        bytes.extend_from_slice(&len.to_be_bytes());
        bytes.extend_from_slice(&seg);
        bytes.extend_from_slice(&[0xFF, 0xD9]);
        fs::write(&source, &bytes).unwrap();

        let (name, total) = icc_from_jpeg(&source).expect("icc found");
        assert_eq!(name, None);
        assert_eq!(total, 4);

        let details = details_for_path(&source).expect("details");
        assert_eq!(details.icc_status, IccStatus::Present);
        assert_eq!(details.icc_bytes, Some(4));
        // 디코드 불가 가짜 JPEG도 상세가 실패하지 않는다.
        assert_eq!(details.color_mode, "unknown");
        assert_eq!((details.width, details.height), (None, None));
    }

    #[test]
    fn jpeg_without_icc_is_absent() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("plain.jpg");
        let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(8, 8, |x, y| {
            image::Rgb([x as u8, y as u8, 0])
        }));
        img.save(&source).expect("write jpg fixture");

        assert!(icc_from_jpeg(&source).is_none());
        let details = details_for_path(&source).expect("details");
        assert_eq!(details.icc_status, IccStatus::Absent);
        assert_eq!(details.color_mode, "rgb");
    }

    #[test]
    fn png_iccp_reports_name_and_size() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("icc.png");
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut data = b"sRGB\0".to_vec();
        data.extend_from_slice(&[0, 0x78, 0x9C, 0x03, 0x00]); // 압축방식 + 가짜 zlib
        bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
        bytes.extend_from_slice(b"iCCP");
        bytes.extend_from_slice(&data);
        bytes.extend_from_slice(&[0, 0, 0, 0]); // CRC 더미
        bytes.extend_from_slice(&(0u32).to_be_bytes());
        bytes.extend_from_slice(b"IEND");
        bytes.extend_from_slice(&[0xAE, 0x42, 0x60, 0x82]);
        fs::write(&source, &bytes).unwrap();

        let (name, total) = icc_from_png(&source).expect("iccp found");
        assert_eq!(name.as_deref(), Some("sRGB"));
        assert_eq!(total, 4);

        let details = details_for_path(&source).expect("details");
        assert_eq!(details.icc_status, IccStatus::Present);
        assert_eq!(details.icc_name.as_deref(), Some("sRGB"));
    }

    #[test]
    fn png_without_iccp_is_absent() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("plain.png");
        write_rgb_png(&source, 4, 4, [9, 9, 9]);
        assert!(icc_from_png(&source).is_none());
        let details = details_for_path(&source).expect("details");
        assert_eq!(details.icc_status, IccStatus::Absent);
    }

    #[test]
    fn gif_is_icc_unchecked() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("anim.gif");
        fs::write(&source, b"GIF89a").unwrap();
        let details = details_for_path(&source).expect("details");
        assert_eq!(details.icc_status, IccStatus::Unchecked);
    }

    #[test]
    fn details_and_histogram_serialize_to_snake_case() {
        // 프론트 TS 타입(file_path, icc_status, sampled_pixels 등)과 1:1 대응.
        // rename_all 누락/오남용 시 여기서 실패한다.
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("photo.png");
        write_rgb_png(&source, 2, 2, [1, 2, 3]);

        let details = details_for_path(&source).expect("details");
        let v = serde_json::to_value(&details).unwrap();
        for key in [
            "file_path",
            "file_size",
            "color_mode",
            "bits_per_channel",
            "created_unix",
            "modified_unix",
            "dpi_x",
            "dpi_y",
            "icc_status",
            "icc_name",
            "icc_bytes",
        ] {
            assert!(v.get(key).is_some(), "missing snake_case key {key}");
        }

        let hist = histogram_for_path(&source).expect("histogram");
        let hv = serde_json::to_value(&hist).unwrap();
        for key in ["r", "g", "b", "sampled_pixels"] {
            assert!(hv.get(key).is_some(), "missing snake_case key {key}");
        }
    }

    #[test]
    fn dpi_conversion_handles_units() {
        assert_eq!(
            dpi_from_values(Some(300.0), Some(300.0), 1.0),
            (Some(300.0), Some(300.0))
        );
        // cm 단위는 inch로 환산한다 (118.11/2.54 = 46.5).
        assert_eq!(
            dpi_from_values(Some(118.11), None, 2.54),
            (Some(46.5), None)
        );
        // 0·음수·NaN은 버린다.
        assert_eq!(dpi_from_values(Some(0.0), Some(-5.0), 1.0), (None, None));
        assert_eq!(
            dpi_from_values(Some(f32::NAN), Some(72.0), 1.0),
            (None, Some(72.0))
        );
    }

    #[test]
    fn jpeg_exif_resolution_is_read_as_dpi() {
        // EXIF APP1(XResolution=300/1, YResolution=300/1, unit 생략→inch)를 손으로 조립.
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("dpi.jpg");
        let mut exif_payload = b"Exif\0\0".to_vec();
        // TIFF 헤더: little endian, IFD0 오프셋 8.
        exif_payload.extend_from_slice(&[0x49, 0x49, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00]);
        // IFD0 항목 2개: XResolution(0x011A, RATIONAL, off) + YResolution(0x011B, RATIONAL, off).
        exif_payload.extend_from_slice(&[0x02, 0x00]);
        // 값 영역은 IFD 뒤(8 + 2 + 24 = 34)에 배치.
        let x_off = 8u32 + 2 + 2 * 12 + 4;
        let y_off = x_off + 8;
        for (tag, off) in [(0x011Au16, x_off), (0x011Bu16, y_off)] {
            exif_payload.extend_from_slice(&tag.to_le_bytes());
            exif_payload.extend_from_slice(&[0x05, 0x00]); // RATIONAL
            exif_payload.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // count 1
            exif_payload.extend_from_slice(&off.to_le_bytes());
        }
        exif_payload.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // next IFD
        for _ in 0..2 {
            exif_payload.extend_from_slice(&300u32.to_le_bytes());
            exif_payload.extend_from_slice(&1u32.to_le_bytes());
        }
        let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE1];
        let len = (exif_payload.len() + 2) as u16;
        bytes.extend_from_slice(&len.to_be_bytes());
        bytes.extend_from_slice(&exif_payload);
        bytes.extend_from_slice(&[0xFF, 0xD9]);
        {
            let mut f = fs::File::create(&source).unwrap();
            f.write_all(&bytes).unwrap();
        }

        let (x, y) = resolution_dpi(&source);
        assert_eq!((x, y), (Some(300.0), Some(300.0)));
    }
}
