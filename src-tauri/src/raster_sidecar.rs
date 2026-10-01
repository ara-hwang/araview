//! TGA / DDS / OpenEXR 미리보기용 JPEG sidecar.
//!
//! WebView2는 이 포맷들을 네이티브 렌더하지 못해 HEIC/PSD와 같은 전제로
//! 디코드한 픽셀을 JPEG sidecar로 변환해 렌더한다.
//! - 디코더: `image` 크레이트(이미 의존 중, 추가 의존성 없음).
//! - DDS는 크레이트가 DXT1/3/5(BC1~BC3)만 지원한다. 그 외(BC7, 비압축)는
//!   `unsupported`로 보고한다.
//! - EXR은 선형 부동소수 값을 clamp 후 sRGB 감마로 8비트화한다(노출 보정 없음).
//! - 편집 저장은 지원하지 않는다(읽기 전용). `save.rs`에서 진입 차단.
//!
//! 파일 식별 해시, per-file 락, JPEG 원자적 발행, 다운스케일은
//! 공용 `sidecar` 모듈을 사용한다.

use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};

use crate::app_error::{AppError, ErrorCode};
use crate::sidecar::{Rgb8, SidecarSpec};

/// 디코드 허용 픽셀 상한. EXR은 픽셀당 16바이트(RGBA f32)라 HEIC/PSD보다 낮게 둔다.
const MAX_DECODE_PIXELS: u64 = 100_000_000;

/// 디코더가 할당할 수 있는 상한.
const MAX_DECODE_ALLOC: u64 = 2 * 1024 * 1024 * 1024;

/// 파일 전체를 읽기 전에 거르는 크기 상한.
const MAX_FILE_BYTES: u64 = 768 * 1024 * 1024;

/// 이 모듈이 담당하는 확장자와 `image` 포맷.
pub fn format_for_ext(ext: &str) -> Option<ImageFormat> {
    match ext {
        "tga" => Some(ImageFormat::Tga),
        "dds" => Some(ImageFormat::Dds),
        "exr" => Some(ImageFormat::OpenExr),
        _ => None,
    }
}

const SPEC: SidecarSpec = SidecarSpec {
    label: "Raster sidecar lock",
    paint_prefix: "raster-",
    thumb_prefix: "raster-thumb-",
    decode: decode_rgb8,
};

pub fn ensure_jpeg_sidecar(source: &Path) -> Result<PathBuf, AppError> {
    SPEC.ensure(source)
}

pub fn ensure_jpeg_sidecar_thumb(source: &Path, max_side: u32) -> Result<PathBuf, AppError> {
    SPEC.ensure_thumb(source, max_side)
}

pub fn cached_jpeg_sidecar_thumb(source: &Path, max_side: u32) -> Option<PathBuf> {
    SPEC.cached_thumb(source, max_side)
}

/// 합성 픽셀을 RGB8로 디코드. 투명은 흰 배경에 합성한다
/// (JPEG에 알파가 없어 `psd_sidecar`/`save.rs` flatten과 같은 규칙).
pub(crate) fn decode_rgb8(source: &Path) -> Result<Rgb8, AppError> {
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();
    let format =
        format_for_ext(&ext).ok_or_else(|| AppError::unsupported("Unsupported image format"))?;
    let meta = fs::metadata(source)
        .map_err(|e| AppError::io("Failed to read metadata", e, ErrorCode::Corrupt))?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(AppError::too_large("Image file is too large"));
    }
    let file = fs::File::open(source)
        .map_err(|e| AppError::io("Failed to read file", e, ErrorCode::Corrupt))?;
    let mut reader = ImageReader::with_format(BufReader::new(file), format);
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    reader.limits(limits);
    let decoder = reader
        .into_decoder()
        .map_err(|e| AppError::image_error("Failed to decode image", e))?;
    let (width, height) = decoder.dimensions();
    if u64::from(width).saturating_mul(u64::from(height)) > MAX_DECODE_PIXELS {
        return Err(AppError::too_large("Image dimensions are too large"));
    }
    let img = DynamicImage::from_decoder(decoder)
        .map_err(|e| AppError::image_error("Failed to decode image", e))?;
    Ok(flatten_to_rgb8(img))
}

fn flatten_to_rgb8(img: DynamicImage) -> Rgb8 {
    let (width, height) = (img.width(), img.height());
    // 선형 부동소수(EXR)는 sRGB 감마로 옮겨야 어둡게 보이지 않는다.
    let rgba = match img {
        DynamicImage::ImageRgb32F(_) | DynamicImage::ImageRgba32F(_) => {
            linear_to_srgb_rgba8(&img.into_rgba32f())
        }
        other => other.into_rgba8(),
    };
    let mut bytes = Vec::with_capacity(width as usize * height as usize * 3);
    for px in rgba.as_raw().as_chunks::<4>().0 {
        let a = f32::from(px[3]) / 255.0;
        let blend = |c: u8| (f32::from(c) * a + 255.0 * (1.0 - a)).round() as u8;
        bytes.extend_from_slice(&[blend(px[0]), blend(px[1]), blend(px[2])]);
    }
    Rgb8 {
        width,
        height,
        bytes,
    }
}

fn linear_to_srgb_rgba8(img: &image::Rgba32FImage) -> image::RgbaImage {
    let encode = |v: f32| -> u8 {
        // `clamp`는 NaN을 통과시키므로 먼저 거른다.
        let v = if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) };
        let s = if v <= 0.003_130_8 {
            v * 12.92
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        };
        (s * 255.0).round() as u8
    };
    image::RgbaImage::from_fn(img.width(), img.height(), |x, y| {
        let p = img.get_pixel(x, y).0;
        let alpha = if p[3].is_nan() {
            0.0
        } else {
            p[3].clamp(0.0, 1.0)
        };
        image::Rgba([
            encode(p[0]),
            encode(p[1]),
            encode(p[2]),
            (alpha * 255.0).round() as u8,
        ])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn save_as(dir: &Path, name: &str, img: &DynamicImage, format: ImageFormat) -> PathBuf {
        let path = dir.join(name);
        img.save_with_format(&path, format).unwrap();
        path
    }

    /// 4x4 DXT1 블록 하나로 만든 최소 DDS. 색 0번을 빨강(RGB565 0xF800)으로 둔다.
    fn minimal_dxt1_dds() -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"DDS ");
        let mut header = [0u32; 31];
        header[0] = 124; // dwSize
        header[1] = 0x1 | 0x2 | 0x4 | 0x1000 | 0x80000; // flags
        header[2] = 4; // height
        header[3] = 4; // width
        header[4] = 8; // linear size (one BC1 block)
        header[18] = 32; // ddspf.dwSize
        header[19] = 0x4; // DDPF_FOURCC
        header[20] = u32::from_le_bytes(*b"DXT1");
        header[26] = 0x1000; // caps: texture
        for v in header {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&0xF800u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&[0u8; 4]);
        out
    }

    #[test]
    fn tga_decodes_to_rgb8() {
        let dir = tempfile::tempdir().unwrap();
        let img =
            DynamicImage::ImageRgb8(image::RgbImage::from_pixel(3, 2, image::Rgb([10, 20, 30])));
        let path = save_as(dir.path(), "a.tga", &img, ImageFormat::Tga);
        let rgb = decode_rgb8(&path).unwrap();
        assert_eq!((rgb.width, rgb.height), (3, 2));
        assert_eq!(&rgb.bytes[..3], &[10, 20, 30]);
    }

    #[test]
    fn tga_alpha_is_flattened_on_white() {
        let dir = tempfile::tempdir().unwrap();
        let img = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            1,
            1,
            image::Rgba([0, 0, 0, 0]),
        ));
        let path = save_as(dir.path(), "a.tga", &img, ImageFormat::Tga);
        let rgb = decode_rgb8(&path).unwrap();
        assert_eq!(rgb.bytes, vec![255, 255, 255]);
    }

    #[test]
    fn exr_linear_values_are_gamma_encoded() {
        let dir = tempfile::tempdir().unwrap();
        let img = DynamicImage::ImageRgba32F(image::Rgba32FImage::from_pixel(
            2,
            1,
            image::Rgba([0.5, 1.0, 0.0, 1.0]),
        ));
        let path = save_as(dir.path(), "a.exr", &img, ImageFormat::OpenExr);
        let rgb = decode_rgb8(&path).unwrap();
        assert_eq!((rgb.width, rgb.height), (2, 1));
        // 선형 0.5는 sRGB로 약 188.
        assert!(
            (i32::from(rgb.bytes[0]) - 188).abs() <= 1,
            "{:?}",
            rgb.bytes
        );
        assert_eq!(rgb.bytes[1], 255);
        assert_eq!(rgb.bytes[2], 0);
    }

    #[test]
    fn dxt1_dds_decodes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.dds");
        fs::write(&path, minimal_dxt1_dds()).unwrap();
        let rgb = decode_rgb8(&path).unwrap();
        assert_eq!((rgb.width, rgb.height), (4, 4));
        assert!(
            rgb.bytes[0] > 200 && rgb.bytes[1] < 40,
            "{:?}",
            &rgb.bytes[..3]
        );
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["a.tga", "a.dds", "a.exr"] {
            let path = dir.path().join(name);
            fs::write(&path, b"not an image at all, just text").unwrap();
            let err = decode_rgb8(&path).unwrap_err();
            assert_ne!(err.code, ErrorCode::Unknown, "{name}");
        }
    }

    #[test]
    fn unknown_extension_is_unsupported() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.png");
        fs::write(&path, b"x").unwrap();
        let err = decode_rgb8(&path).unwrap_err();
        assert_eq!(err.code, ErrorCode::Unsupported);
    }

    #[test]
    fn sidecar_paths_are_namespaced_raster() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.tga");
        fs::write(&path, b"x").unwrap();
        let dest = SPEC.paint_path(&path).unwrap();
        let name = dest.file_name().and_then(|n| n.to_str()).unwrap();
        assert!(name.starts_with("raster-"), "got {name}");
        let thumb = SPEC.thumb_path(&path, 64).unwrap();
        let name = thumb.file_name().and_then(|n| n.to_str()).unwrap();
        assert!(name.starts_with("raster-thumb-"), "got {name}");
    }

    #[test]
    fn repo_samples_load_through_the_sidecar_pipeline() {
        let samples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../samples");
        for (name, mime, dims) in [
            ("sample.tga", "image/x-tga", (800, 600)),
            ("sample.dds", "image/vnd.ms-dds", (800, 600)),
            ("sample.exr", "image/x-exr", (400, 300)),
        ] {
            let source = samples.join(name);
            let info =
                crate::image::load_viewable(&source).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(info.mime_type, mime, "{name}");
            assert_eq!(
                (info.width, info.height),
                (Some(dims.0), Some(dims.1)),
                "{name}"
            );
            assert_ne!(
                info.file_path, info.source_path,
                "{name} must paint a sidecar"
            );
            assert!(info.file_path.ends_with(".jpg"), "{name}");
            let thumb = crate::thumbnail::generate_thumbnail(&source, 64)
                .unwrap_or_else(|e| panic!("{name} thumb: {e}"));
            assert!(thumb.width.max(thumb.height) <= 64, "{name}");
        }
    }
}
