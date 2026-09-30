//! SVG 래스터화. `image` 크레이트는 SVG를 디코드하지 못하므로 썸네일과
//! 히스토그램처럼 픽셀이 필요한 경로에서만 resvg를 쓴다. 표시는 WebView가 맡는다.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{fontdb, Options, Tree};

use crate::app_error::AppError;

/// 비정상적으로 큰 SVG 입력으로 인한 메모리/연산 폭주를 막는다.
const MAX_SVG_BYTES: u64 = 32 * 1024 * 1024;

fn system_fonts() -> Arc<fontdb::Database> {
    static FONTS: OnceLock<Arc<fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = fontdb::Database::new();
            db.load_system_fonts();
            Arc::new(db)
        })
        .clone()
}

/// 긴 변이 `max_side`가 되도록 렌더링한다. 투명 영역은 흰 배경으로 합성한다.
pub fn rasterize(path: &Path, max_side: u32) -> Result<image::RgbImage, AppError> {
    let len = std::fs::metadata(path)
        .map_err(|e| {
            AppError::io(
                "Failed to read SVG",
                e,
                crate::app_error::ErrorCode::Unknown,
            )
        })?
        .len();
    if len > MAX_SVG_BYTES {
        return Err(AppError::too_large("SVG file is too large"));
    }
    let data = std::fs::read(path).map_err(|e| {
        AppError::io(
            "Failed to read SVG",
            e,
            crate::app_error::ErrorCode::Unknown,
        )
    })?;
    let options = Options {
        fontdb: system_fonts(),
        resources_dir: path.parent().map(Path::to_path_buf),
        ..Options::default()
    };
    let tree = Tree::from_data(&data, &options)
        .map_err(|e| AppError::corrupt(format!("Failed to parse SVG: {e}")))?;
    let size = tree.size();
    let (sw, sh) = (size.width(), size.height());
    if !(sw > 0.0 && sh > 0.0) {
        return Err(AppError::corrupt("SVG has no drawable size"));
    }
    let scale = max_side.max(1) as f32 / sw.max(sh);
    let width = ((sw * scale).round() as u32).max(1);
    let height = ((sh * scale).round() as u32).max(1);
    let mut pixmap = Pixmap::new(width, height)
        .ok_or_else(|| AppError::corrupt("SVG render size is invalid"))?;
    resvg::render(
        &tree,
        Transform::from_scale(width as f32 / sw, height as f32 / sh),
        &mut pixmap.as_mut(),
    );
    // tiny-skia는 premultiplied RGBA. 흰 배경 위로 합성한다.
    let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
    for px in pixmap.pixels() {
        let inv = 255 - u32::from(px.alpha());
        rgb.push((u32::from(px.red()) + inv).min(255) as u8);
        rgb.push((u32::from(px.green()) + inv).min(255) as u8);
        rgb.push((u32::from(px.blue()) + inv).min(255) as u8);
    }
    image::RgbImage::from_raw(width, height, rgb)
        .ok_or_else(|| AppError::corrupt("SVG render produced invalid buffer"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rasterizes_to_bounded_size_on_white() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.svg");
        std::fs::write(
            &path,
            r##"<svg width="400" height="200" xmlns="http://www.w3.org/2000/svg"><rect width="200" height="200" fill="#ff0000"/></svg>"##,
        )
        .unwrap();
        let img = rasterize(&path, 100).unwrap();
        assert_eq!((img.width(), img.height()), (100, 50));
        assert_eq!(img.get_pixel(10, 10).0, [255, 0, 0]);
        assert_eq!(img.get_pixel(90, 10).0, [255, 255, 255]);
    }

    #[test]
    fn invalid_svg_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("b.svg");
        std::fs::write(&path, b"not svg").unwrap();
        assert!(rasterize(&path, 64).is_err());
    }
}
