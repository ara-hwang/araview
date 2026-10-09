//! 휠 스크롤을 부드럽게 만드는 스크롤 영역.
//!
//! GPUI의 기본 스크롤은 휠 한 칸마다 오프셋이 바로 점프한다. 여기서는 휠 입력을
//! 목표 오프셋으로만 쌓고, 프레임마다 현재 오프셋을 목표로 감속하며 따라간다.
//! 기본 스크롤 처리는 `overflow_y_hidden`으로 꺼 두고(추적과 클리핑만 남김) 이
//! 모듈이 휠을 직접 처리한다.

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
    last_frame: Rc<Cell<Instant>>,
}

impl Default for SmoothScroll {
    fn default() -> Self {
        Self {
            handle: ScrollHandle::new(),
            target: Rc::new(Cell::new(0.)),
            running: Rc::new(Cell::new(false)),
            last_frame: Rc::new(Cell::new(Instant::now())),
        }
    }
}

impl SmoothScroll {
    /// 맨 위로 되돌린다(탭 전환 등).
    pub fn reset(&self) {
        self.target.set(0.);
        self.handle.set_offset(point(px(0.), px(0.)));
    }

    /// `content`를 세로 스크롤 영역으로 감싼다. 반환값은 크기를 정하는 바깥 요소이며,
    /// 스크롤바는 스크롤되는 내용 밖(바깥 요소의 자식)에 붙는다.
    pub fn area<E>(&self, id: impl Into<ElementId>, content: E) -> Stateful<Div>
    where
        E: Styled + IntoElement,
    {
        let id = id.into();
        let this = self.clone();
        let area = div()
            .id((id.clone(), "area"))
            .size_full()
            .flex()
            .flex_col()
            .overflow_y_hidden()
            .track_scroll(&self.handle)
            .on_scroll_wheel(move |event, window, cx| {
                this.on_wheel(event, window);
                cx.stop_propagation();
            })
            .child(content.flex_none().h_auto().min_h_full());
        div()
            .id(id)
            .size_full()
            .relative()
            .child(area)
            .vertical_scrollbar(&self.handle)
    }

    fn on_wheel(&self, event: &ScrollWheelEvent, window: &mut Window) {
        let delta = f32::from(event.delta.pixel_delta(window.line_height()).y);
        if delta == 0. {
            return;
        }
        let max = f32::from(self.handle.max_offset().y);
        if max <= 0. {
            return;
        }
        // 애니메이션이 끝난 상태에서는 스크롤바 드래그 등으로 바뀐 실제 위치를 기준으로 한다.
        if !self.running.get() {
            self.target.set(f32::from(self.handle.offset().y));
        }
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
            let current = f32::from(offset.y);
            let remaining = this.target.get() - current;
            if remaining.abs() < SETTLE_DISTANCE {
                this.handle
                    .set_offset(point(offset.x, px(this.target.get())));
                this.running.set(false);
            } else {
                // 프레임 간격과 무관하게 같은 속도로 감속한다.
                let next = current + remaining * (1. - (-dt / TIME_CONSTANT).exp());
                this.handle.set_offset(point(offset.x, px(next)));
                this.step(window);
            }
            window.refresh();
        });
    }
}
