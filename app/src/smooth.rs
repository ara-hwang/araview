//! 휠 스크롤을 부드럽게 만드는 스크롤 영역.
//!
//! GPUI의 기본 스크롤은 휠 한 칸마다 오프셋이 바로 점프한다. 여기서는 휠 입력을
//! 목표 오프셋으로만 쌓고, 프레임마다 현재 오프셋을 목표로 감속하며 따라간다.
//! 기본 스크롤 처리는 `overflow_y_hidden`으로 꺼 두고(추적과 클리핑만 남김) 이
//! 모듈이 휠을 직접 처리한다. 세로 영역은 `area`, 가로·세로가 설정에 따라 바뀌는 영역
//! (이미지 도크)은 `wheel_handler`와 `animate_to`로 같은 감속을 쓴다.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::*;

/// 목표까지 남은 거리가 이 값(px)보다 작아지면 멈춘다.
const SETTLE_DISTANCE: f32 = 0.5;
/// 감속 시간 상수(초). 작을수록 빠르게 따라붙는다.
const TIME_CONSTANT: f32 = 0.08;
/// 프레임 간격이 이 값(초)을 넘으면 건너뛴 것으로 보고 상한을 둔다.
const MAX_STEP: f32 = 0.05;

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
            let dt = now
                .duration_since(this.last_frame.replace(now))
                .as_secs_f32()
                .min(MAX_STEP);
            let offset = this.handle.offset();
            let horizontal = this.horizontal.get();
            let current = along(offset, horizontal);
            let remaining = this.target.get() - current;
            if remaining.abs() < SETTLE_DISTANCE {
                this.handle
                    .set_offset(offset_on(offset, horizontal, this.target.get()));
                this.running.set(false);
            } else {
                // 프레임 간격과 무관하게 같은 속도로 감속한다.
                let next = current + remaining * (1. - (-dt / TIME_CONSTANT).exp());
                this.handle.set_offset(offset_on(offset, horizontal, next));
                this.step(window);
            }
            window.refresh();
        });
    }
}
