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

/// SVG를 읽어 파스 트리를 만든다. 입력 크기 상한을 적용한다.
fn parse_tree(path: &Path) -> Result<Tree, AppError> {
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
    Tree::from_data(&data, &options)
        .map_err(|e| AppError::corrupt(format!("Failed to parse SVG: {e}")))
}

/// 긴 변이 `max_side`가 되도록 렌더링한다. 투명 영역은 흰 배경으로 합성한다.
pub fn rasterize(path: &Path, max_side: u32) -> Result<image::RgbImage, AppError> {
    let tree = parse_tree(path)?;
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

/// 내재 치수로 렌더링하고 알파를 유지한다. 긴 변이 `max_side`를 넘으면 그 값으로
/// 줄인다. 프론트가 createImageBitmap으로 내재 치수 래스터를 얻던 클립보드 복사
/// 경로와 맞춘 것으로, 투명 영역을 흰 배경으로 합성하지 않는다.
pub fn rasterize_rgba(path: &Path, max_side: u32) -> Result<image::RgbaImage, AppError> {
    let tree = parse_tree(path)?;
    let size = tree.size();
    let (sw, sh) = (size.width(), size.height());
    if !(sw > 0.0 && sh > 0.0) {
        return Err(AppError::corrupt("SVG has no drawable size"));
    }
    let scale = if sw.max(sh) > max_side as f32 {
        max_side as f32 / sw.max(sh)
    } else {
        1.0
    };
    let width = ((sw * scale).round() as u32).max(1);
    let height = ((sh * scale).round() as u32).max(1);
    let mut pixmap = Pixmap::new(width, height)
        .ok_or_else(|| AppError::corrupt("SVG render size is invalid"))?;
    resvg::render(
        &tree,
        Transform::from_scale(width as f32 / sw, height as f32 / sh),
        &mut pixmap.as_mut(),
    );
    // tiny-skia는 premultiplied RGBA. 곱셈을 풀어 알파를 그대로 보존한다.
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for px in pixmap.pixels() {
        let color = px.demultiply();
        rgba.push(color.red());
        rgba.push(color.green());
        rgba.push(color.blue());
        rgba.push(color.alpha());
    }
    image::RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| AppError::corrupt("SVG render produced invalid buffer"))
}
