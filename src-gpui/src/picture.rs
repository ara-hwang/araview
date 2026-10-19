//! 디코드한 프레임을 GPU 아틀라스에 올릴 수 있는 타일과 축소 단계로 만든다.
//!
//! GPUI의 이미지 페인트에는 두 가지 제약이 있다. 텍스처 한 변이 16384px로
//! 제한되고, 샘플러가 밉맵 없는 선형 보간뿐이다. 그래서 큰 이미지는 타일로
//! 쪼개고(긴 웹툰 스트립), 축소 표시용으로 절반씩 줄인 단계를 CPU에서 만들어
//! 표시 배율에 가까운 단계를 고른다.

use std::path::Path;
use std::sync::Arc;

use araview_core::app_error::AppError;
use araview_core::display::{DisplayImage, decode_for_display};
use araview_core::raster_sidecar::decode_linear;
use gpui_kit::RenderImage;
use image::{Frame, Rgba32FImage, RgbaImage, imageops};
use smallvec::SmallVec;

use crate::platform;

/// 타일 한 변 상한. 아틀라스 한계(16384)보다 작게 잡아 텍스처 할당을 나눈다.
pub const TILE_SIDE: u32 = 8192;
/// 긴 변이 이 값 이하로 내려가면 축소 단계를 더 만들지 않는다.
const MIN_LEVEL_SIDE: u32 = 1024;
/// SVG 래스터 긴 변 상한. 확대 화질과 메모리의 절충이다.
pub const SVG_MAX_SIDE: u32 = 8192;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Orientation {
    /// 시계 방향 0/90/180/270.
    pub rotation: u16,
    pub flip_h: bool,
    pub flip_v: bool,
}

pub struct Tile {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub image: Arc<RenderImage>,
    /// HDR 출력에서 `image` 대신 그릴 확장 범위 픽셀의 등록. 타일이 버려질 때 함께 놓인다.
    _hdr: Option<HdrRegistration>,
}

/// `RenderImage` 하나에 붙인 HDR 픽셀 등록. 버리면 렌더러에서도 놓는다.
pub struct HdrRegistration(usize);

impl HdrRegistration {
    fn new(image: &RenderImage, pixels: &Rgba32FImage) -> Self {
        let (width, height) = pixels.dimensions();
        let encoded: Vec<u16> = pixels
            .as_raw()
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|[r, g, b, a]| {
                [
                    encode_hdr(*r),
                    encode_hdr(*g),
                    encode_hdr(*b),
                    half::f16::from_f32(clean(*a).min(1.0)).to_bits(),
                ]
            })
            .collect();
        platform::register_hdr_image(image.id.0, width, height, &encoded);
        Self(image.id.0)
    }
}

impl Drop for HdrRegistration {
    fn drop(&mut self) {
        platform::unregister_hdr_image(self.0);
    }
}

/// 선형 값(1.0 = SDR 흰색)이 f16에서 무한대가 되지 않게 두는 상한.
const MAX_LINEAR: f32 = 10_000.0;

fn clean(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, MAX_LINEAR)
    }
}

/// GPUI 장면은 sRGB 인코딩 값으로 블렌딩하므로 같은 곡선으로 인코딩한다. 1.0을 넘는
/// 값도 곡선을 이어 써서 렌더러의 변환 패스가 하이라이트로 되돌린다.
fn encode_hdr(v: f32) -> u16 {
    let v = clean(v);
    let s = if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    half::f16::from_f32(s).to_bits()
}

pub struct Level {
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<Tile>,
}

pub struct Picture {
    /// 회전/반전을 반영한 원본 크기.
    pub width: u32,
    pub height: u32,
    /// 0번이 원본 크기, 뒤로 갈수록 절반. 애니메이션은 0번만 있고 타일은 하나다.
    pub levels: Vec<Level>,
    /// 프레임별 지연(ms). 정지 이미지는 길이 1.
    pub delays_ms: Vec<u32>,
    /// 유한 반복 횟수. `None`이면 무한 반복이다.
    pub loop_count: Option<u32>,
    pub is_vector: bool,
}

impl Picture {
    pub fn frame_count(&self) -> usize {
        self.delays_ms.len()
    }

    pub fn is_animated(&self) -> bool {
        self.frame_count() > 1
    }

    /// 화면 픽셀당 이미지 픽셀 비율이 `device_scale`일 때 쓸 단계. 축소 비율이
    /// 2배를 넘지 않는 가장 작은 단계를 고른다.
    pub fn level_for(&self, device_scale: f32) -> &Level {
        let mut chosen = &self.levels[0];
        for level in &self.levels[1..] {
            let level_scale = level.width as f32 / self.width as f32;
            if level_scale >= device_scale {
                chosen = level;
            } else {
                break;
            }
        }
        chosen
    }

    pub fn images(&self) -> impl Iterator<Item = &Arc<RenderImage>> {
        self.levels
            .iter()
            .flat_map(|level| level.tiles.iter().map(|tile| &tile.image))
    }
}

/// 렌더 바이트(`path`, sidecar일 수 있다)를 디코드해 표시용 `Picture`를 만든다. HDR 픽셀은
/// 원본(`source`)에서 읽는다. 디코드와 리샘플을 하므로 백그라운드 executor에서 부른다.
pub fn load(path: &Path, source: &Path, orientation: Orientation) -> Result<Picture, AppError> {
    let decoded = decode_for_display(path, SVG_MAX_SIDE)?;
    // HDR 출력이 켜져 있으면 선형 포맷은 확장 범위 픽셀도 만든다. 실패해도 SDR로 보인다.
    let hdr = if platform::hdr_output_enabled() && !decoded.is_animated() {
        decode_linear(source).unwrap_or_else(|error| {
            log::warn!("HDR decode failed, showing SDR: {error}");
            None
        })
    } else {
        None
    };
    Ok(build(decoded, orientation, hdr))
}

pub fn build(
    decoded: DisplayImage,
    orientation: Orientation,
    hdr: Option<Rgba32FImage>,
) -> Picture {
    let is_vector = decoded.is_vector;
    let loop_count = decoded.loop_count;
    let delays_ms: Vec<u32> = decoded.frames.iter().map(|frame| frame.delay_ms).collect();
    let mut frames: Vec<RgbaImage> = decoded
        .frames
        .into_iter()
        .map(|frame| orient(frame.image, orientation))
        .collect();
    let (width, height) = frames[0].dimensions();

    if frames.len() > 1 {
        // 애니메이션은 프레임 인덱스가 한 `RenderImage` 안에 있어야 하므로 타일로
        // 나누지 않는다. 아틀라스 한계를 넘는 프레임만 줄인다.
        let limit = TILE_SIDE * 2;
        if width.max(height) > limit {
            let scale = limit as f32 / width.max(height) as f32;
            let (w, h) = scaled_size(width, height, scale);
            for frame in &mut frames {
                *frame = imageops::thumbnail(frame, w, h);
            }
        }
        let (level_w, level_h) = frames[0].dimensions();
        let image = render_image(
            frames
                .into_iter()
                .zip(&delays_ms)
                .map(|(frame, delay)| (frame, *delay)),
        );
        return Picture {
            width,
            height,
            levels: vec![Level {
                width: level_w,
                height: level_h,
                tiles: vec![Tile {
                    x: 0,
                    y: 0,
                    w: level_w,
                    h: level_h,
                    image,
                    _hdr: None,
                }],
            }],
            delays_ms,
            loop_count,
            is_vector,
        };
    }

    let mut current = frames.pop().expect("decoded image has a frame");
    // SDR 프레임과 크기가 다르면(축소 사본 등) 타일이 맞지 않으므로 쓰지 않는다.
    let mut current_hdr = hdr
        .map(|image| orient(image, orientation))
        .filter(|image| image.dimensions() == current.dimensions());
    let mut levels = Vec::new();
    loop {
        let (w, h) = current.dimensions();
        let (nw, nh) = scaled_size(w, h, 0.5);
        let more = w.max(h) > MIN_LEVEL_SIDE;
        // 박스 샘플링이라 빠르고, 절반 축소에서는 Triangle과 차이가 거의 없다.
        let next = more.then(|| imageops::thumbnail(&current, nw, nh));
        let next_hdr = current_hdr
            .as_ref()
            .filter(|_| more)
            .map(|image| imageops::thumbnail(image, nw, nh));
        levels.push(tile_level(current, current_hdr.as_ref()));
        match next {
            Some(image) => {
                current = image;
                current_hdr = next_hdr;
            }
            None => break,
        }
    }
    Picture {
        width,
        height,
        levels,
        delays_ms,
        loop_count,
        is_vector,
    }
}

fn scaled_size(width: u32, height: u32, scale: f32) -> (u32, u32) {
    (
        ((width as f32 * scale).round() as u32).max(1),
        ((height as f32 * scale).round() as u32).max(1),
    )
}

fn orient<P>(
    image: image::ImageBuffer<P, Vec<P::Subpixel>>,
    orientation: Orientation,
) -> image::ImageBuffer<P, Vec<P::Subpixel>>
where
    P: image::Pixel + 'static,
{
    let mut image = match orientation.rotation {
        90 => imageops::rotate90(&image),
        180 => imageops::rotate180(&image),
        270 => imageops::rotate270(&image),
        _ => image,
    };
    if orientation.flip_h {
        imageops::flip_horizontal_in_place(&mut image);
    }
    if orientation.flip_v {
        imageops::flip_vertical_in_place(&mut image);
    }
    image
}

/// GPUI는 BGRA 프레임을 받는다.
fn render_image(frames: impl Iterator<Item = (RgbaImage, u32)>) -> Arc<RenderImage> {
    let frames: SmallVec<[Frame; 1]> = frames
        .map(|(mut buffer, delay_ms)| {
            for pixel in buffer.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
            Frame::from_parts(
                buffer,
                0,
                0,
                image::Delay::from_numer_denom_ms(delay_ms.max(1), 1),
            )
        })
        .collect();
    Arc::new(RenderImage::new(frames))
}

/// 타일 하나를 만든다. HDR 픽셀이 있으면 같은 영역을 잘라 함께 등록한다.
fn tile(image: RgbaImage, hdr: Option<Rgba32FImage>, x: u32, y: u32) -> Tile {
    let (w, h) = image.dimensions();
    let image = render_image(std::iter::once((image, 0)));
    let hdr = hdr.map(|pixels| HdrRegistration::new(&image, &pixels));
    Tile {
        x,
        y,
        w,
        h,
        image,
        _hdr: hdr,
    }
}

fn tile_level(image: RgbaImage, hdr: Option<&Rgba32FImage>) -> Level {
    let (width, height) = image.dimensions();
    if width <= TILE_SIDE && height <= TILE_SIDE {
        return Level {
            width,
            height,
            tiles: vec![tile(image, hdr.cloned(), 0, 0)],
        };
    }
    let mut tiles = Vec::new();
    let mut y = 0;
    while y < height {
        let h = TILE_SIDE.min(height - y);
        let mut x = 0;
        while x < width {
            let w = TILE_SIDE.min(width - x);
            let part = imageops::crop_imm(&image, x, y, w, h).to_image();
            let hdr_part = hdr.map(|pixels| imageops::crop_imm(pixels, x, y, w, h).to_image());
            tiles.push(tile(part, hdr_part, x, y));
            x += w;
        }
        y += h;
    }
    Level {
        width,
        height,
        tiles,
    }
}
