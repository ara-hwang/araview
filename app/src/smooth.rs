//! 휠 스크롤을 부드럽게 만드는 스크롤 영역.
//!
//! GPUI의 기본 스크롤은 휠 한 칸마다 오프셋이 바로 점프한다. 여기서는 휠 입력을
//! 목표 오프셋으로만 쌓고, 프레임마다 현재 오프셋을 목표로 감속하며 따라간다.
//! 기본 스크롤 처리는 `overflow_y_hidden`으로 꺼 두고(추적과 클리핑만 남김) 이
//! 모듈이 휠을 직접 처리한다. 세로 영역은 `area`, 가로·세로가 설정에 따라 바뀌는 영역
//! (이미지 도크)은 `wheel_handler`와 `animate_to`로 같은 감속을 쓴다. 스크롤 핸들이
//! 없는 웹툰 보기는 남은 거리만 쌓는 `Glide`로 같은 감속을 쓴다.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::*;

/// 목표까지 남은 거리가 이 값(px)보다 작아지면 멈춘다.
const SETTLE_DISTANCE: f32 = 0.5;
/// 감속 시간 상수(초). 작을수록 빠르게 따라붙는다.
const TIME_CONSTANT: f32 = 0.08;
/// 프레임 간격이 이 값(초)을 넘으면 건너뛴 것으로 보고 상한을 둔다.
const MAX_STEP: f32 = 0.05;
/// `Glide`가 남은 거리를 한 번에 반영하고 멈추는 거리(px).
const GLIDE_SETTLE: f32 = 1.0;

/// 남은 거리 `remaining` 가운데 지난 프레임 뒤 `elapsed` 동안 움직일 몫. 프레임 간격과
/// 무관하게 같은 속도로 감속하고, 건너뛴 프레임은 `MAX_STEP`으로 자른다.
fn eased(remaining: f32, elapsed: Duration) -> f32 {
    let dt = elapsed.as_secs_f32().min(MAX_STEP);
    remaining * (1. - (-dt / TIME_CONSTANT).exp())
}

/// 스크롤 핸들 없이 남은 거리만 쌓아 프레임마다 감속하며 나눠 주는 상태. 웹툰 보기의
/// 휠·키 스크롤이 쓴다. 프레임이 예약돼 있는 동안만 `last_frame`이 있다.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Glide {
    /// 아직 반영하지 못한 거리(px).
    remaining: f32,
    last_frame: Option<Instant>,
}

impl Glide {
    /// 거리를 쌓는다. 새로 프레임을 예약해야 하면 `true`.
    pub fn push(&mut self, delta: f32, now: Instant) -> bool {
        self.remaining += delta;
        if self.last_frame.is_some() {
            return false;
        }
        self.last_frame = Some(now);
        true
    }

    /// 이번 프레임에 반영할 거리. 거의 다 왔으면 남은 거리를 한 번에 돌려준다.
    pub fn step(&mut self, now: Instant) -> f32 {
        let last = self.last_frame.replace(now).unwrap_or(now);
        let step = if self.remaining.abs() < GLIDE_SETTLE {
            self.remaining
        } else {
            eased(self.remaining, now.duration_since(last))
        };
        self.remaining -= step;
        step
    }

    /// 프레임을 마친다. 끝에 닿아 움직이지 못했으면(`moved`가 `false`) 남은 거리를
    /// 버린다. 다음 프레임을 예약해야 하면 `true`, 끝났으면 `false`.
    pub fn end_frame(&mut self, moved: bool) -> bool {
        if !moved {
            self.remaining = 0.;
        }
        if self.remaining == 0. {
            self.last_frame = None;
            return false;
        }
        true
    }

    /// 남은 거리를 버린다(이동·목록 교체). 이미 예약된 프레임이 다음 `step`에서 끝낸다.
    pub fn discard(&mut self) {
        self.remaining = 0.;
    }

    #[cfg(test)]
    fn running(&self) -> bool {
        self.last_frame.is_some()
    }
}

#[derive(Clone)]
pub struct SmoothScroll {
    handle: ScrollHandle,
    /// 휠이 쌓은 목표 오프셋(0 이하).
    target: Rc<Cell<f32>>,
    running: Rc<Cell<bool>>,
    /// 지금 따라가는 축이 가로인지.
    horizontal: Rc<Cell<bool>>,
    last_frame: Rc<Cell<Instant>>,
}

impl Default for SmoothScroll {
    fn default() -> Self {
        Self::from_handle(ScrollHandle::new())
    }
}

impl SmoothScroll {
    /// 이미 다른 요소가 추적하는 핸들을 움직이는 스크롤. `VirtualList`처럼
    /// 목록 요소가 핸들을 소유하는 영역에 쓴다.
    pub fn from_handle(handle: ScrollHandle) -> Self {
        Self {
            handle,
            target: Rc::new(Cell::new(0.)),
            running: Rc::new(Cell::new(false)),
            horizontal: Rc::new(Cell::new(false)),
            last_frame: Rc::new(Cell::new(Instant::now())),
        }
    }
}

/// 오프셋에서 스크롤 축 성분만 꺼낸다.
fn along(point: Point<Pixels>, horizontal: bool) -> f32 {
    f32::from(if horizontal { point.x } else { point.y })
}

/// 스크롤 축 성분만 `value`로 바꾼 오프셋.
fn offset_on(offset: Point<Pixels>, horizontal: bool, value: f32) -> Point<Pixels> {
    if horizontal {
        point(px(value), offset.y)
    } else {
        point(offset.x, px(value))
    }
}

impl SmoothScroll {
    /// 위치 읽기와 스크롤바·`track_scroll` 연결에 쓰는 핸들.
    pub fn handle(&self) -> &ScrollHandle {
        &self.handle
    }

    /// 휠을 `horizontal` 축으로 부드럽게 처리하는 핸들러. 기본 스크롤을 끈
    /// (`overflow_*_hidden`) 요소의 `on_scroll`에 붙인다.
    pub fn wheel_handler(
        &self,
        horizontal: bool,
    ) -> impl Fn(&ScrollEvent, &mut Window, &mut App) + 'static {
        let this = self.clone();
        move |event, window, cx| {
            this.on_wheel(event, horizontal, window);
            cx.stop_propagation();
        }
    }

    /// 목표 오프셋(0 이하)으로 부드럽게 이동한다.
    pub fn animate_to(&self, target: f32, horizontal: bool, window: &mut Window) {
        self.horizontal.set(horizontal);
        self.target.set(target);
        if !self.running.replace(true) {
            self.last_frame.set(Instant::now());
            self.step(window);
        }
    }

    /// 맨 위로 되돌린다(탭 전환 등).
    pub fn reset(&self) {
        self.target.set(0.);
        self.handle.set_offset(point(px(0.), px(0.)));
    }

    /// 진행 중인 감속을 멈추고 목표를 현재 위치에 맞춘다. `scroll_to_item` 같은
    /// 즉시 이동과 휠 감속이 겹치지 않게 그 전에 부른다.
    pub fn cancel(&self) {
        let horizontal = self.horizontal.get();
        self.target.set(along(self.handle.offset(), horizontal));
    }

    /// `content`를 세로 스크롤 영역으로 감싼다. 반환값은 크기를 정하는 바깥 요소이며,
    /// 스크롤바는 스크롤되는 내용 밖(바깥 요소의 자식)에 붙는다.
    pub fn area<E>(&self, id: impl Into<ElementId>, content: E) -> Stateful<Div>
    where
        E: Styled + IntoElement,
    {
        let id = id.into();
        let wheel = self.wheel_handler(false);
        let area = div()
            .id((id.clone(), "area"))
            .size_full()
            .flex()
            .flex_col()
            .overflow_y_hidden()
            .track_scroll(&self.handle)
            .on_scroll(wheel)
            .child(content.flex_none().h_auto().min_h_full());
        div()
            .id(id)
            .size_full()
            .relative()
            .child(area)
            .vertical_scrollbar(&self.handle)
    }

    /// `content`를 가로 스크롤 영역으로 감싼다. `area`의 가로판이며, 세로 휠도
    /// 가로 이동으로 받는다. 세로 크기는 내용을 따라가고 가로는 바깥 요소를 채운다.
    pub fn h_area<E>(&self, id: impl Into<ElementId>, content: E) -> Stateful<Div>
    where
        E: Styled + IntoElement,
    {
        let id = id.into();
        let wheel = self.wheel_handler(true);
        let area = div()
            .id((id.clone(), "area"))
            .w_full()
            .flex()
            .flex_row()
            .overflow_x_hidden()
            .track_scroll(&self.handle)
            .on_scroll(wheel)
            .child(content.flex_none().w_auto().min_w_full());
        div()
            .id(id)
            .w_full()
            .relative()
            .child(area)
            .horizontal_scrollbar(&self.handle)
    }

    fn on_wheel(&self, event: &ScrollEvent, horizontal: bool, window: &mut Window) {
        let pixels = event.delta.pixel_delta(window.line_height());
        // 가로 영역은 세로 휠(일반 마우스)도 가로 이동으로 받는다.
        let delta = if horizontal && pixels.x == px(0.) {
            f32::from(pixels.y)
        } else {
            along(pixels, horizontal)
        };
        if delta == 0. {
            return;
        }
        let max = along(self.handle.max_offset(), horizontal);
        if max <= 0. {
            return;
        }
        // 애니메이션이 끝난 상태에서는 스크롤바 드래그 등으로 바뀐 실제 위치를 기준으로 한다.
        if !self.running.get() || self.horizontal.get() != horizontal {
            self.target.set(along(self.handle.offset(), horizontal));
        }
        self.horizontal.set(horizontal);
        self.target.set((self.target.get() + delta).clamp(-max, 0.));
        if !self.running.replace(true) {
            self.last_frame.set(Instant::now());
            self.step(window);
        }
    }

    fn step(&self, window: &mut Window) {
        let this = self.clone();
        window.on_next_frame(move |window, _| {
            let now = Instant::now();
            let elapsed = now.duration_since(this.last_frame.replace(now));
            let offset = this.handle.offset();
            let horizontal = this.horizontal.get();
            let current = along(offset, horizontal);
            let remaining = this.target.get() - current;
            if remaining.abs() < SETTLE_DISTANCE {
                this.handle
                    .set_offset(offset_on(offset, horizontal, this.target.get()));
                this.running.set(false);
            } else {
                let next = current + eased(remaining, elapsed);
                this.handle.set_offset(offset_on(offset, horizontal, next));
                this.step(window);
            }
            window.refresh();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    /// `use gpui_kit::*`가 gpui의 `test` 매크로를 끌어오므로 빌트인 `#[test]`를
    /// 전체 경로로 고른다.
    #[::core::prelude::v1::test]
    fn easing_caps_skipped_frames() {
        assert_eq!(eased(100., Duration::ZERO), 0.);
        let one = eased(100., ms(16));
        assert!(one > 0. && one < 100.);
        // 50ms보다 긴 간격은 50ms로 본다.
        assert_eq!(eased(100., ms(500)), eased(100., ms(50)));
    }

    #[::core::prelude::v1::test]
    fn glide_eases_and_settles() {
        let start = Instant::now();
        let mut glide = Glide::default();
        assert!(glide.push(300., start));
        // 프레임이 예약된 동안 더 쌓아도 새 프레임은 필요 없다.
        assert!(!glide.push(100., start));
        let mut moved = 0.;
        let mut now = start;
        for _ in 0..200 {
            now += ms(16);
            moved += glide.step(now);
            if !glide.end_frame(true) {
                break;
            }
        }
        assert!(!glide.running());
        assert!((moved - 400.).abs() < 1e-3);
        // 끝난 뒤에 쌓으면 다시 프레임을 예약한다.
        assert!(glide.push(5., now));
    }

    #[::core::prelude::v1::test]
    fn glide_stops_at_the_end_or_when_discarded() {
        let start = Instant::now();
        let mut glide = Glide::default();
        glide.push(300., start);
        assert!(glide.step(start + ms(16)) > 0.);
        // 끝에 닿아 움직이지 못하면 남은 거리를 버리고 멈춘다.
        assert!(!glide.end_frame(false));
        assert!(!glide.running());

        glide.push(300., start);
        glide.discard();
        // 예약된 프레임은 움직이지 않고 끝난다.
        assert_eq!(glide.step(start + ms(16)), 0.);
        assert!(!glide.end_frame(true));
    }
}
