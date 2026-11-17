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

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 0.0001;

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < EPSILON,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn position_bounds_are_zero_while_the_image_fits() {
        let bounds = position_bounds(Size2::new(200.0, 100.0), Size2::new(100.0, 50.0), 1.0);
        assert_eq!(bounds, Offset { x: 0.0, y: 0.0 });

        // 배율이 붙어야 넘치는 축만 이동할 수 있다.
        let bounds = position_bounds(Size2::new(200.0, 100.0), Size2::new(100.0, 50.0), 3.0);
        assert_close(bounds.x, 50.0);
        assert_close(bounds.y, 25.0);
    }

    #[test]
    fn position_bounds_halve_the_overflow() {
        let bounds = position_bounds(Size2::new(100.0, 100.0), Size2::new(400.0, 300.0), 1.0);
        assert_close(bounds.x, 150.0);
        assert_close(bounds.y, 100.0);
    }

    #[test]
    fn clamp_position_keeps_the_image_inside_its_bounds() {
        let max = Offset { x: 10.0, y: 20.0 };
        assert_eq!(
            clamp_position(Offset { x: 25.0, y: -40.0 }, max),
            Offset { x: 10.0, y: -20.0 }
        );
        // 이동 여지가 없는 축은 항상 중앙이다.
        assert_eq!(
            clamp_position(Offset { x: 5.0, y: 5.0 }, Offset { x: 0.0, y: 0.0 }),
            Offset { x: 0.0, y: 0.0 }
        );
    }

    #[test]
    fn zoom_for_fit_uses_the_remembered_mode() {
        let container = Size2::new(200.0, 100.0);
        let image = Size2::new(100.0, 100.0);
        assert_close(zoom_for_fit(FitMode::Width, container, image), 2.0);
        assert_close(zoom_for_fit(FitMode::Height, container, image), 1.0);
        assert_close(zoom_for_fit(FitMode::Screen, container, image), 1.0);
        // auto는 작은 이미지를 확대하지 않는다.
        assert_close(zoom_for_fit(FitMode::Auto, container, image), 1.0);

        let large = Size2::new(800.0, 400.0);
        assert_close(zoom_for_fit(FitMode::Width, container, large), 0.25);
        assert_close(zoom_for_fit(FitMode::Height, container, large), 0.25);
        assert_close(zoom_for_fit(FitMode::Screen, container, large), 0.25);
        assert_close(zoom_for_fit(FitMode::Auto, container, large), 0.25);
    }

    #[test]
    fn zoom_for_fit_falls_back_to_100_percent_for_empty_sizes() {
        let container = Size2::new(0.0, 100.0);
        let image = Size2::new(10.0, 10.0);
        assert_close(zoom_for_fit(FitMode::Screen, container, image), 1.0);
        assert_close(zoom_for_fit(FitMode::Auto, container, image), 1.0);
        assert_close(
            zoom_for_fit(
                FitMode::Auto,
                Size2::new(100.0, 100.0),
                Size2::new(0.0, 10.0),
            ),
            1.0,
        );
    }

    #[test]
    fn zoom_for_fit_distinguishes_the_limiting_axis_and_allows_explicit_upscaling() {
        let container = Size2::new(200.0, 100.0);
        for (image, width, height, screen, auto) in [
            (Size2::new(400.0, 100.0), 0.5, 1.0, 0.5, 0.5),
            (Size2::new(100.0, 400.0), 2.0, 0.25, 0.25, 0.25),
            (Size2::new(50.0, 25.0), 4.0, 4.0, 4.0, 1.0),
        ] {
            for (mode, expected) in [
                (FitMode::Width, width),
                (FitMode::Height, height),
                (FitMode::Screen, screen),
                (FitMode::Auto, auto),
            ] {
                assert_close(zoom_for_fit(mode, container, image), expected);
            }
            assert_close(min_zoom(container, image), auto);
        }
    }

    #[test]
    fn every_fit_mode_defaults_for_each_empty_dimension() {
        let valid = Size2::new(200.0, 100.0);
        for empty in [
            Size2::new(0.0, 100.0),
            Size2::new(100.0, 0.0),
            Size2::new(-1.0, 100.0),
            Size2::new(100.0, -1.0),
        ] {
            assert!(empty.is_empty());
            for mode in [
                FitMode::Width,
                FitMode::Height,
                FitMode::Screen,
                FitMode::Auto,
            ] {
                assert_close(zoom_for_fit(mode, empty, valid), 1.0);
                assert_close(zoom_for_fit(mode, valid, empty), 1.0);
            }
            assert_close(min_zoom(empty, valid), 1.0);
            assert_close(min_zoom(valid, empty), 1.0);
        }
        assert!(!valid.is_empty());
    }

    #[test]
    fn pan_bounds_and_clamping_are_independent_per_axis() {
        let container = Size2::new(200.0, 100.0);
        let bounds = position_bounds(container, Size2::new(200.0, 300.0), 1.0);
        assert_eq!(bounds, Offset { x: 0.0, y: 100.0 });
        assert_eq!(
            clamp_position(Offset { x: 10.0, y: -100.0 }, bounds),
            Offset { x: 0.0, y: -100.0 }
        );
        assert_eq!(
            clamp_position(Offset { x: -10.0, y: 30.0 }, bounds),
            Offset { x: 0.0, y: 30.0 }
        );
        let bounds = position_bounds(container, Size2::new(400.0, 50.0), 1.0);
        assert_eq!(bounds, Offset { x: 100.0, y: 0.0 });
        assert_eq!(
            clamp_position(Offset { x: 150.0, y: -30.0 }, bounds),
            Offset { x: 100.0, y: 0.0 }
        );
    }

    #[test]
    fn min_zoom_matches_the_auto_fit() {
        let big = Size2::new(400.0, 400.0);
        let container = Size2::new(200.0, 100.0);
        assert_close(min_zoom(container, big), 0.25);
        // 작은 이미지는 100%가 하한이다.
        assert_close(min_zoom(container, Size2::new(20.0, 20.0)), 1.0);
    }

    #[test]
    fn zoom_about_keeps_the_anchor_over_the_same_image_point() {
        let container = Size2::new(100.0, 100.0);
        let image = Size2::new(200.0, 200.0);
        let anchor = Offset { x: 10.0, y: 0.0 };
        let before = Offset { x: 0.0, y: 0.0 };

        let after = zoom_about(container, image, 1.0, 2.0, before, anchor);
        let point_before = (anchor.x - before.x) / 1.0;
        let point_after = (anchor.x - after.x) / 2.0;
        assert_close(point_before, point_after);
        assert_close(after.x, -10.0);
        assert_close(after.y, 0.0);
    }

    #[test]
    fn zoom_about_preserves_both_anchor_coordinates_with_an_existing_pan() {
        let container = Size2::new(100.0, 100.0);
        let image = Size2::new(800.0, 600.0);
        let anchor = Offset { x: 20.0, y: -30.0 };
        let position = Offset { x: -40.0, y: 50.0 };
        for next_zoom in [0.5, 2.0] {
            let after = zoom_about(container, image, 1.0, next_zoom, position, anchor);
            assert_close((anchor.x - after.x) / next_zoom, anchor.x - position.x);
            assert_close((anchor.y - after.y) / next_zoom, anchor.y - position.y);
        }
    }

    #[test]
    fn zoom_about_clamps_only_the_axis_that_reaches_its_boundary() {
        let after = zoom_about(
            Size2::new(100.0, 100.0),
            Size2::new(200.0, 400.0),
            1.0,
            0.5,
            Offset { x: 40.0, y: -40.0 },
            Offset { x: -20.0, y: 10.0 },
        );
        assert_eq!(after, Offset { x: 0.0, y: -15.0 });
    }

    #[test]
    fn zoom_about_clamps_position_when_zooming_out() {
        // 축소하면 이동 여지가 사라지므로 위치는 중앙으로 돌아간다.
        let container = Size2::new(100.0, 100.0);
        let image = Size2::new(200.0, 200.0);
        let position = Offset { x: 40.0, y: 40.0 };
        let after = zoom_about(
            container,
            image,
            2.0,
            0.5,
            position,
            Offset { x: 0.0, y: 0.0 },
        );
        assert_eq!(after, Offset { x: 0.0, y: 0.0 });
    }
}
