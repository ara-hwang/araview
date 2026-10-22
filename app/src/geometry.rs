//! 줌/팬/맞춤 계산(SPEC §7). 좌표는 논리 픽셀이고 위치는 컨테이너 중심 기준 오프셋이다.

use crate::settings::FitMode;

pub const MAX_ZOOM: f32 = 10.0;
pub const MAX_ZOOM_VECTOR: f32 = 40.0;
pub const ZOOM_STEP: f32 = 1.25;
/// 키보드 팬 1회 이동량(px).
pub const PAN_STEP: f32 = 48.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size2 {
    pub w: f32,
    pub h: f32,
}

impl Size2 {
    pub fn new(w: f32, h: f32) -> Self {
        Self { w, h }
    }

    pub fn is_empty(self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Offset {
    pub x: f32,
    pub y: f32,
}

/// 줌 상태에서 이동 가능한 최대 오프셋(절반).
pub fn position_bounds(container: Size2, image: Size2, zoom: f32) -> Offset {
    let scaled_w = image.w * zoom;
    let scaled_h = image.h * zoom;
    Offset {
        x: if scaled_w >= container.w {
            (scaled_w - container.w) / 2.0
        } else {
            0.0
        },
        y: if scaled_h >= container.h {
            (scaled_h - container.h) / 2.0
        } else {
            0.0
        },
    }
}

pub fn clamp_position(position: Offset, max: Offset) -> Offset {
    Offset {
        x: if max.x > 0.0 {
            position.x.clamp(-max.x, max.x)
        } else {
            0.0
        },
        y: if max.y > 0.0 {
            position.y.clamp(-max.y, max.y)
        } else {
            0.0
        },
    }
}

fn valid(zoom: f32) -> f32 {
    if zoom.is_finite() && zoom > 0.0 {
        zoom
    } else {
        1.0
    }
}

/// 기억된 맞춤 모드에 따른 줌. `auto`는 큰 이미지만 맞추고 작은 이미지는 100%로 둔다.
pub fn zoom_for_fit(mode: FitMode, container: Size2, image: Size2) -> f32 {
    if container.is_empty() || image.is_empty() {
        return 1.0;
    }
    let by_width = container.w / image.w;
    let by_height = container.h / image.h;
    valid(match mode {
        FitMode::Width => by_width,
        FitMode::Height => by_height,
        FitMode::Screen => by_width.min(by_height),
        FitMode::Auto => by_width.min(by_height).min(1.0),
    })
}

/// 축소 하한. 작은 이미지는 100%까지, 큰 이미지는 화면 맞춤 배율까지 허용한다.
pub fn min_zoom(container: Size2, image: Size2) -> f32 {
    zoom_for_fit(FitMode::Auto, container, image)
}

/// `anchor`(컨테이너 중심 기준) 아래의 이미지 지점을 고정한 채 줌을 바꾼 뒤의 위치.
pub fn zoom_about(
    container: Size2,
    image: Size2,
    zoom: f32,
    next_zoom: f32,
    position: Offset,
    anchor: Offset,
) -> Offset {
    let ratio = if zoom > 0.0 { next_zoom / zoom } else { 1.0 };
    let moved = Offset {
        x: anchor.x - (anchor.x - position.x) * ratio,
        y: anchor.y - (anchor.y - position.y) * ratio,
    };
    clamp_position(moved, position_bounds(container, image, next_zoom))
}
