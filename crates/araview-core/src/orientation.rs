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
