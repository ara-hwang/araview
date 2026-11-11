//! 디코드한 프레임을 GPU 아틀라스에 올릴 수 있는 타일과 축소 단계로 만든다.
//!
//! GPUI의 이미지 페인트에는 두 가지 제약이 있다. 텍스처 한 변이 16384px로
//! 제한되고, 샘플러가 밉맵 없는 선형 보간뿐이다. 그래서 큰 이미지는 타일로
//! 쪼개고(긴 웹툰 스트립), 축소 표시용으로 절반씩 줄인 단계를 CPU에서 만들어
//! 표시 배율에 가까운 단계를 고른다. 단계는 열 때 화면에 필요한 수준까지만
//! 만들고, 커밋 뒤 백그라운드로 나머지를 채운다.

use std::path::Path;
use std::sync::{Arc, Mutex};

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

#[derive(Clone)]
pub struct Tile {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub image: Arc<RenderImage>,
    /// HDR 출력에서 `image` 대신 그릴 확장 범위 픽셀의 등록. 타일이 버려질 때 함께 놓인다.
    _hdr: Option<Arc<HdrRegistration>>,
}

/// `RenderImage` 하나에 붙인 HDR 픽셀 등록. 버리면 렌더러에서도 놓는다. 지연 채움이
/// 단계 목록을 통째로 옮길 때 같은 등록을 함께 가리키므로 `Arc`로 공유하고, 마지막
/// 사본이 놓일 때 해제한다.
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

#[derive(Clone)]
pub struct Level {
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<Tile>,
}

/// 열릴 때 화면 맞춤의 근거. 읽기 영역 크기(논리 픽셀)와 기기 배율, 폭 맞춤 여부다.
/// 폭 맞춤(웹툰)은 페이지 폭을 읽기 영역 폭에 맞추고, 나머지는 읽기 영역 안에
/// 맞춘다. 정밀할 필요는 없다: 과하게 잡으면 작은 단계를 하나 더 만들 뿐이고,
/// 모자라면 `fill_levels`가 뒤따른다.
#[derive(Clone, Copy)]
pub struct ViewFit {
    pub container_w: f32,
    pub container_h: f32,
    pub scale_factor: f32,
    pub fit_width: bool,
}

pub struct Picture {
    /// 회전/반전을 반영한 원본 크기.
    pub width: u32,
    pub height: u32,
    /// 0번이 원본 크기, 뒤로 갈수록 절반. 애니메이션은 0번만 있고 타일은 하나다.
    /// 화면에 필요한 단계만 먼저 만들고 나머지는 `fill_levels`가 채운다. 그리는
    /// 동안 채움이 끼어들지 않게 스냅숏(`levels`)으로 본다.
    levels: Mutex<Arc<Vec<Level>>>,
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

    /// 지금 만들어진 단계의 스냅숏. 그리는 동안 `fill_levels`가 단계를 더해도
    /// 스냅숏은 바뀌지 않는다.
    pub fn levels(&self) -> Arc<Vec<Level>> {
        self.levels.lock().expect("picture levels").clone()
    }

    /// 지연해 둔 축소 단계를 만들어 넣는다. 디코드와 같은 리샘플 비용이 들므로
    /// 백그라운드에서 부른다. 애니메이션은 단계가 없어 바로 끝난다.
    pub fn fill_levels(&self) {
        if self.is_animated() {
            return;
        }
        let snapshot = self.levels();
        let defined = defined_levels(self.width, self.height);
        if snapshot.len() >= defined.len() {
            return;
        }
        // 가장 작은 만들어진 단계부터 이어서 절반씩 줄인다. 타일로 흩어진 픽셀을
        // 모아 쓴다. 박스 축소는 채널별로 독립적이라, 타일 바이트(BGRA)의 R과 B를
        // 되돌려 원본 RGBA로 만들면 처음부터 만든 단계와 같은 결과가 나온다.
        let Some(source) = untile(&snapshot[snapshot.len() - 1]) else {
            return;
        };
        let mut chain = (*snapshot).clone();
        let mut current = source;
        for (w, h) in &defined[chain.len()..] {
            let next = imageops::thumbnail(&current, *w, *h);
            // 새 단계는 타일로 흩어지면서도 다음 단계의 소스여야 한다. 지연 단계는
            // 원본보다 기하급수적으로 작아 복사 값이 싸다.
            chain.push(tile_level(next.clone(), None));
            current = next;
        }
        // 다른 채움 사이에 단계가 더 생겼을 수 있다. 더 완전한 목록만 놓는다.
        let mut levels = self.levels.lock().expect("picture levels");
        if levels.len() < chain.len() {
            *levels = Arc::new(chain);
        }
    }

    /// 지금 만들어진 단계에 담긴 그림들. 뒤늦게 채워진 단계는 이후 호출에 나타난다.
    pub fn images(&self) -> impl Iterator<Item = Arc<RenderImage>> {
        self.levels()
            .iter()
            .flat_map(|level| &level.tiles)
            .map(|tile| tile.image.clone())
            .collect::<Vec<_>>()
            .into_iter()
    }
}

/// 만들어진 단계 가운데 축소 비율이 2배를 넘지 않는 가장 작은 단계를 고른다.
/// `base_width`는 원본(`Picture`) 너비다. 뒤쪽 단계가 아직 없으면 그 시점에
/// 만들어진 가장 작은 단계를 쓴다. 채워지면 다음 그리기부터 맞는 단계를 쓴다.
pub fn choose_level(levels: &[Level], base_width: u32, device_scale: f32) -> &Level {
    let mut chosen = &levels[0];
    for level in &levels[1..] {
        let level_scale = level.width as f32 / base_width as f32;
        if level_scale >= device_scale {
            chosen = level;
        } else {
            break;
        }
    }
    chosen
}

/// 렌더 바이트(`path`, sidecar일 수 있다)를 디코드해 표시용 `Picture`를 만든다. HDR 픽셀은
/// 원본(`source`)에서 읽는다. 디코드와 리샘플을 하므로 백그라운드 executor에서 부른다.
/// `view_fit`은 열릴 때 화면 맞춤의 근거다(`ViewFit` 참고).
pub fn load(
    path: &Path,
    source: &Path,
    orientation: Orientation,
    view_fit: Option<ViewFit>,
) -> Result<Picture, AppError> {
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
    Ok(build(decoded, orientation, hdr, view_fit))
}

/// 디코드 결과가 GPU 아틀라스에 보존하는 바이트. 타일·밉 단계·애니메이션
/// 프레임 버퍼를 합계한다. 페이지 캐시의 바이트 예산이 이 값으로 잰다(SPEC §9.1).
/// HDR 확장 픽셀 등록은 셈에 넣지 않는다. 지연 생성한 밉 단계는 채워졌을 때와
/// 같은 크기가 되니, 만들어지기 전부터 예정 크기로 잰다.
pub fn retained_bytes(picture: &Picture) -> u64 {
    if picture.is_animated() {
        // 애니메이션은 밉 단계를 만들지 않는다. 프레임 버퍼 합계다.
        picture
            .levels()
            .iter()
            .flat_map(|level| &level.tiles)
            .map(|tile| tile_buffer_bytes(&tile.image))
            .sum()
    } else {
        // 타일 격자가 단계를 빠짐없이 나누므로 치수만으로 합계한다.
        defined_levels(picture.width, picture.height)
            .iter()
            .map(|(width, height)| 4 * *width as u64 * *height as u64)
            .sum()
    }
}

/// 원본 크기에서 만들어지는 단계 치수 전부(0번 원본 포함). 지연 단계의 예정
/// 크기와 채움 대상이 여기서 정해진다.
fn defined_levels(width: u32, height: u32) -> Vec<(u32, u32)> {
    let mut levels = vec![(width, height)];
    let (mut w, mut h) = (width, height);
    while w.max(h) > MIN_LEVEL_SIDE {
        (w, h) = scaled_size(w, h, 0.5);
        levels.push((w, h));
    }
    levels
}

/// 타일로 흩어진 단계 픽셀을 하나의 RGBA 버퍼로 모은다. 타일 바이트는 GPUI가
/// 쓰는 BGRA라서 R과 B를 되돌려 원본 RGBA로 돌려놓는다(`render_image`는 다시
/// 바꾼다). 타일 프레임을 읽지 못하면 `None`이다.
fn untile(level: &Level) -> Option<RgbaImage> {
    let mut raw = vec![0u8; level.width as usize * level.height as usize * 4];
    for tile in &level.tiles {
        let bytes = tile.image.as_bytes(0)?;
        let stride = tile.w as usize * 4;
        for (row, source) in bytes.chunks_exact(stride).enumerate() {
            let at = (tile.y as usize + row) * level.width as usize * 4 + tile.x as usize * 4;
            raw[at..at + stride].copy_from_slice(source);
        }
    }
    for pixel in raw.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    RgbaImage::from_raw(level.width, level.height, raw)
}

fn tile_buffer_bytes(image: &RenderImage) -> u64 {
    let mut total = 0;
    let mut frame = 0;
    while let Some(bytes) = image.as_bytes(frame) {
        total += bytes.len() as u64;
        frame += 1;
    }
    total
}

/// 디코드 결과로 표시용 `Picture`를 만든다. `view_fit`은 열릴 때 화면 맞춤의 근거로,
/// 이보다 작아지는 단계는 만들지 않고 `fill_levels`가 뒤늦게 채운다(`ViewFit`
/// 참고). `None`이면 단계를 전부 만든다. HDR은 단계마다 등록이 따라가야 해서 HDR
/// 픽셀이 있으면 전부 만든다.
pub fn build(
    decoded: DisplayImage,
    orientation: Orientation,
    hdr: Option<Rgba32FImage>,
    view_fit: Option<ViewFit>,
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
            levels: Mutex::new(Arc::new(vec![Level {
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
            }])),
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
    // 화면에 필요한 수준까지만 먼저 만든다. 한도는 열릴 때의 표시 긴 변(기기
    // 픽셀)이다: 폭 맞춤은 페이지 폭을 읽기 영역 폭에 맞추고, 나머지는 읽기
    // 영역 안에 맞춘다. 이보다 작은 단계는 `fill_levels`가 뒤늦게 만든다. HDR이
    // 있으면 지연하지 않는다. 지연하지 않으면 한도가 0이라 오늘처럼 전부 만든다.
    let needed_long = view_fit
        .filter(|_| current_hdr.is_none())
        .map(|fit| {
            let (pw, ph) = (width as f32, height as f32);
            let long0 = pw.max(ph);
            let zoom = if fit.fit_width {
                (fit.container_w / pw).max(fit.container_h / ph)
            } else {
                (fit.container_w / pw).min(fit.container_h / ph)
            };
            long0 * zoom * fit.scale_factor
        })
        .unwrap_or(0.0);
    let mut levels = Vec::new();
    loop {
        let (w, h) = current.dimensions();
        let (nw, nh) = scaled_size(w, h, 0.5);
        let more = w.max(h) > MIN_LEVEL_SIDE;
        // 박스 샘플링이라 빠르고, 절반 축소에서는 Triangle과 차이가 거의 없다.
        let needed = more && nw.max(nh) as f32 >= needed_long;
        let next = needed.then(|| imageops::thumbnail(&current, nw, nh));
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
        levels: Mutex::new(Arc::new(levels)),
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
    let hdr = hdr.map(|pixels| Arc::new(HdrRegistration::new(&image, &pixels)));
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

#[cfg(test)]
mod tests {
    use super::*;
    use araview_core::display::DisplayFrame;

    /// 사분면 색에 그라데이션을 섞은 그림. 채널이 바뀌면 드러난다.
    fn sample(width: u32, height: u32) -> RgbaImage {
        let mut image = RgbaImage::new(width, height);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            let (r, g, b): (u8, u8, u8) = match (x * 2 < width, y * 2 < height) {
                (true, true) => (255u8, 0, 0),
                (false, true) => (0, 255, 0),
                (true, false) => (0, 0, 255),
                (false, false) => (255, 255, 0),
            };
            let fade = (x % 17 + y % 17) as u8;
            *pixel = [
                r.saturating_sub(fade),
                g.saturating_sub(fade),
                b.saturating_sub(fade),
                255,
            ]
            .into();
        }
        image
    }

    fn still(width: u32, height: u32) -> DisplayImage {
        DisplayImage {
            frames: vec![DisplayFrame {
                image: sample(width, height),
                delay_ms: 100,
            }],
            loop_count: None,
            is_vector: false,
        }
    }

    /// 단계의 타일 위치와 바이트를 비교 가능한 순서쌍으로 모은다.
    type TileBytes = ((u32, u32, u32, u32), Vec<u8>);

    fn level_bytes(level: &Level) -> Vec<TileBytes> {
        level
            .tiles
            .iter()
            .map(|tile| {
                (
                    (tile.x, tile.y, tile.w, tile.h),
                    tile.image.as_bytes(0).expect("tile frame").to_vec(),
                )
            })
            .collect()
    }

    fn view_fit(container_w: f32, container_h: f32, fit_width: bool) -> ViewFit {
        ViewFit {
            container_w,
            container_h,
            scale_factor: 1.25,
            fit_width,
        }
    }

    #[test]
    fn deferred_chain_fills_to_the_same_levels() {
        // 2048px 한 변의 전체 단계는 2048과 1024 둘이다. 3000×3000 읽기 영역에
        // 맞춰 열면 표시 긴 변은 3000px(기기)이라 1024px 단계는 지연된다.
        let eager = build(still(2048, 2048), Orientation::default(), None, None);
        let deferred = build(
            still(2048, 2048),
            Orientation::default(),
            None,
            Some(view_fit(3000.0, 3000.0, false)),
        );
        assert_eq!(deferred.levels().len(), 1);
        // 지연 단계도 예정 크기로 잰다.
        assert_eq!(retained_bytes(&deferred), retained_bytes(&eager));
        deferred.fill_levels();
        let filled = deferred.levels();
        let eager_levels = eager.levels();
        assert_eq!(filled.len(), eager_levels.len());
        for (filled, eager) in filled.iter().zip(eager_levels.iter()) {
            assert_eq!((filled.width, filled.height), (eager.width, eager.height));
            assert_eq!(level_bytes(filled), level_bytes(eager));
        }
    }

    #[test]
    fn deferred_display_hints_cover_the_fit_and_defer_the_tail() {
        // 1280×800 읽기 영역(배율 1.25)에 맞춰 열면 세로 사진의 표시 긴 변은
        // 1000px(기기)이다. 이보다 작은 781px 단계는 지연하고, fit 배율에서
        // 고르는 1562px 단계는 만들어 둔다.
        let deferred = build(
            still(1280, 12500),
            Orientation::default(),
            None,
            Some(view_fit(1280.0, 800.0, false)),
        );
        let eager = build(still(1280, 12500), Orientation::default(), None, None);
        let defined = defined_levels(1280, 12500);
        assert_eq!(eager.levels().len(), defined.len());
        let deferred_levels = deferred.levels();
        assert_eq!(deferred_levels.len(), defined.len() - 1);
        let level = choose_level(&deferred_levels, 1280, 800.0 / 12500.0 * 1.25);
        let expected = defined[deferred_levels.len() - 1];
        assert_eq!((level.width, level.height), expected);
        deferred.fill_levels();
        assert_eq!(deferred.levels().len(), defined.len());
    }

    #[test]
    fn webtoon_fit_width_defers_everything_but_the_original() {
        // 폭 맞춤은 스트립을 원본 단계로 그린다(폭 맞춤 배율이 원본보다 크다).
        // 이보다 작은 단계는 전부 지연한다.
        let deferred = build(
            still(300, 30000),
            Orientation::default(),
            None,
            Some(view_fit(1280.0, 800.0, true)),
        );
        assert_eq!(deferred.levels().len(), 1);
        deferred.fill_levels();
        let eager = build(still(300, 30000), Orientation::default(), None, None);
        assert_eq!(deferred.levels().len(), eager.levels().len());
    }

    #[test]
    fn choose_level_falls_back_to_the_smallest_made_level() {
        let picture = build(
            still(2048, 2048),
            Orientation::default(),
            None,
            Some(view_fit(3000.0, 3000.0, false)),
        );
        let levels = picture.levels();
        // 1/4 화면에는 1024px 단계가 맞지만 아직 없다. 만들어진 가장 작은 단계를
        // 쓰고, 채워지면 다음 그리기부터 맞는 단계를 쓴다.
        assert_eq!(choose_level(&levels, picture.width, 0.25).width, 2048);
        picture.fill_levels();
        let levels = picture.levels();
        assert_eq!(choose_level(&levels, picture.width, 0.25).width, 1024);
    }

    #[test]
    fn animated_build_keeps_one_level_and_counts_frame_bytes() {
        let decoded = DisplayImage {
            frames: vec![
                DisplayFrame {
                    image: sample(300, 200),
                    delay_ms: 100,
                },
                DisplayFrame {
                    image: sample(300, 200),
                    delay_ms: 100,
                },
            ],
            loop_count: None,
            is_vector: false,
        };
        // 화면 힌트가 있어도 애니메이션은 단계를 만들지 않는다.
        let picture = build(
            decoded,
            Orientation::default(),
            None,
            Some(view_fit(3000.0, 3000.0, false)),
        );
        assert_eq!(picture.levels().len(), 1);
        assert_eq!(retained_bytes(&picture), 2 * 4 * 300 * 200);
        picture.fill_levels();
        assert_eq!(picture.levels().len(), 1);
    }

    #[test]
    fn untile_restores_the_level_pixels() {
        // 타일로 흩어진 단계를 모으면 원본과 같은 RGBA 값이 나와야 한다. 타일
        // 경계를 가로지르는 두 방향을 모두 본다.
        for (width, height) in [(9000u32, 300u32), (300, 9000)] {
            let level = tile_level(sample(width, height), None);
            assert!(level.tiles.len() > 1, "타일 격자를 지나는 크기가 필요하다");
            let restored = untile(&level).expect("level untiles");
            assert_eq!(restored.dimensions(), (width, height));
            let expected = sample(width, height);
            for (x, y, pixel) in restored.enumerate_pixels() {
                assert_eq!(pixel, expected.get_pixel(x, y), "픽셀 ({x}, {y})이 다르다");
            }
        }
    }
}
