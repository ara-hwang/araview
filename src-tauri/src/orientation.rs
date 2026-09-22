//! EXIF Orientation 처리.
//!
//! WebView2는 `<img>` 렌더 시 EXIF orientation을 자동 적용하므로 화면 표시는
//! 이미 upright다. 반면 `image` 크레이트를 쓰는 경로(썸네일, 저장, 치수 계산,
//! 파일 정보)는 회전을 모르기 때문에, 여기서 명시적으로 적용해 표시와 맞춘다.
//!
//! 적용 범위는 JPEG/TIFF다. HEIC/HEIF는 libheif sidecar가 컨테이너 변환
//! (irot/imir)을 반영하고, sidecar JPEG에는 EXIF가 없어 이 파이프라인을 타지
//! 않는다. WebP/PNG의 orientation은 실사용 빈도가 낮아 다루지 않는다.

use std::io::BufReader;
use std::path::Path;

use image::metadata::Orientation;

/// EXIF Orientation 값을 읽는다. 태그가 없거나 비정상이면 1(정상)을 돌려준다.
pub fn read_orientation(path: &Path) -> u8 {
    let Ok(file) = std::fs::File::open(path) else {
        return 1;
    };
    let mut reader = BufReader::new(file);
    let Ok(exif) = exif::Reader::new().read_from_container(&mut reader) else {
        return 1;
    };
    match exif
        .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        .and_then(|field| field.value.get_uint(0))
    {
        Some(value) if (1..=8).contains(&value) => value as u8,
        _ => 1,
    }
}

/// 5..=8은 90도 회전 계열이라 가로/세로가 뒤바뀐다.
pub fn swaps_axes(orientation: u8) -> bool {
    (5..=8).contains(&orientation)
}

/// orientation을 반영해 픽셀을 돌린다. 1이면 그대로 돌려준다.
pub fn apply_to_image(mut img: image::DynamicImage, orientation: u8) -> image::DynamicImage {
    if orientation == 1 {
        return img;
    }
    if let Some(parsed) = Orientation::from_exif(orientation) {
        img.apply_orientation(parsed);
    }
    img
}

/// 테스트용: EXIF Orientation 태그를 가진 실제 JPEG 픽셀 파일을 만든다.
#[cfg(test)]
pub(crate) fn write_test_jpeg_with_orientation(
    path: &Path,
    width: u32,
    height: u32,
    orientation: u16,
) {
    use image::{Rgb, RgbImage};
    use std::io::Write as _;

    // 왼쪽 절반 빨강, 오른쪽 절반 파랑. 회전 방향 검증이 가능한 패턴.
    let img = RgbImage::from_fn(width, height, |x, _| {
        if x < width / 2 {
            Rgb([255, 0, 0])
        } else {
            Rgb([0, 0, 255])
        }
    });
    let mut encoded = Vec::new();
    image::codecs::jpeg::JpegEncoder::new(&mut encoded)
        .encode_image(&image::DynamicImage::ImageRgb8(img))
        .expect("encode jpeg fixture");

    // SOI(FFD8) 뒤에 EXIF APP1(Orientation만 가진 IFD0)을 끼워 넣는다.
    let mut payload = b"Exif\0\0".to_vec();
    payload.extend_from_slice(&[0x49, 0x49, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00]); // little endian TIFF
    payload.extend_from_slice(&[0x01, 0x00]); // IFD0 항목 1개
    payload.extend_from_slice(&0x0112u16.to_le_bytes()); // Orientation
    payload.extend_from_slice(&[0x03, 0x00]); // SHORT
    payload.extend_from_slice(&1u32.to_le_bytes()); // count
    payload.extend_from_slice(&u32::from(orientation).to_le_bytes()); // value (inline)
    payload.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // next IFD 없음

    let mut bytes = Vec::with_capacity(encoded.len() + payload.len() + 4);
    bytes.extend_from_slice(&encoded[..2]); // SOI
    bytes.extend_from_slice(&[0xFF, 0xE1]);
    let len = u16::try_from(payload.len() + 2).expect("fixture fits u16");
    bytes.extend_from_slice(&len.to_be_bytes());
    bytes.extend_from_slice(&payload);
    bytes.extend_from_slice(&encoded[2..]);

    let mut file = std::fs::File::create(path).expect("create jpeg fixture");
    file.write_all(&bytes).expect("write jpeg fixture");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_orientation_tag_from_jpeg() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rotated.jpg");
        write_test_jpeg_with_orientation(&path, 2, 1, 6);
        assert_eq!(read_orientation(&path), 6);
    }

    #[test]
    fn missing_or_broken_exif_defaults_to_one() {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("plain.png");
        image::RgbImage::new(2, 2).save(&png).unwrap();
        assert_eq!(read_orientation(&png), 1);
        assert_eq!(read_orientation(&dir.path().join("missing.jpg")), 1);

        let broken = dir.path().join("broken.jpg");
        std::fs::write(&broken, b"not an image").unwrap();
        assert_eq!(read_orientation(&broken), 1);
    }

    #[test]
    fn reads_orientation_from_archive_tar_bytes() {
        // 아카이브 엔트리 추출물은 확장자가 없을 수 있어 파일명이 아니라 내용으로 판별한다.
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("entry.jpg");
        write_test_jpeg_with_orientation(&source, 2, 1, 8);
        let bytes = std::fs::read(&source).unwrap();
        let extracted = dir.path().join("extracted-no-ext");
        std::fs::write(&extracted, bytes).unwrap();
        assert_eq!(read_orientation(&extracted), 8);
    }

    #[test]
    fn swaps_axes_only_for_rotated_orientations() {
        for orientation in 1..=8u8 {
            assert_eq!(swaps_axes(orientation), (5..=8).contains(&orientation));
        }
    }

    #[test]
    fn apply_rotates_pixels_clockwise() {
        // 2x1: R | B → orientation 6(시계 90°) → 1x2: R 위, B 아래
        let img = image::DynamicImage::ImageRgb8(
            image::RgbImage::from_raw(2, 1, vec![255, 0, 0, 0, 0, 255]).unwrap(),
        );
        let out = apply_to_image(img, 6).to_rgb8();
        assert_eq!((out.width(), out.height()), (1, 2));
        assert_eq!(out.get_pixel(0, 0), &image::Rgb([255, 0, 0]));
        assert_eq!(out.get_pixel(0, 1), &image::Rgb([0, 0, 255]));
    }

    #[test]
    fn apply_keeps_pixels_for_orientation_one() {
        let img = image::DynamicImage::ImageRgb8(
            image::RgbImage::from_raw(2, 1, vec![1, 2, 3, 4, 5, 6]).unwrap(),
        );
        let out = apply_to_image(img, 1).to_rgb8();
        assert_eq!((out.width(), out.height()), (2, 1));
        assert_eq!(out.get_pixel(0, 0), &image::Rgb([1, 2, 3]));
    }
}
