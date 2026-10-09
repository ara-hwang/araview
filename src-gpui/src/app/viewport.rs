//! 읽기 영역: 단일·양쪽·웹툰 배치, 줌/팬, 휠·마우스 입력, 픽셀 보존 표시
//! (SPEC §6, §7, §18).

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use araview_core::pixel_art::PixelArtClassification;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use image::{Frame, RgbaImage};
use smallvec::SmallVec;

use super::{AraView, DragState, paint_checker};
use crate::geometry::{
    self, MAX_ZOOM, MAX_ZOOM_VECTOR, Offset, Size2, ZOOM_STEP, clamp_position, position_bounds,
    zoom_about, zoom_for_fit,
};
use crate::i18n::{t, t_with};
use crate::picture::Picture;
use crate::platform;
use crate::settings::{
    ImageScalingMode, MouseAction, SettingsStore, ViewMode, ViewerBackground, WheelAction,
};

/// 크기를 모르는 웹툰 페이지의 자리 표시 비율(세로/가로)과 기본 너비.
const WT_UNKNOWN_ASPECT: f32 = 1.414;
const WT_UNKNOWN_WIDTH: f32 = 720.0;
const WT_MIN_WIDTH: f32 = 64.0;
/// 웹툰 `↑/↓` 1회 스크롤량(px).
pub(super) const WT_KEY_SCROLL: f32 = 120.0;
/// 웹툰 스크롤이 목표 위치로 수렴하는 감속 시간 상수(초).
const WT_SMOOTH_TIME: f32 = 0.08;
/// 양쪽·웹툰 보기의 상대 배율 범위.
const RELATIVE_ZOOM_MIN: f32 = 0.25;
const RELATIVE_ZOOM_MAX: f32 = 4.0;

/// 캔버스가 그릴 그림 한 장. 좌표는 읽기 영역 왼쪽 위 기준이다.
struct PaintItem {
    picture: Arc<Picture>,
    rect: Bounds<Pixels>,
    frame: usize,
}

/// 픽셀 보존 표시용으로 보이는 영역만 최근접 확대해 둔 래스터.
pub(super) struct PixelView {
    key: PixelKey,
    image: Arc<RenderImage>,
    rect: Bounds<Pixels>,
}

#[derive(PartialEq)]
struct PixelKey {
    picture: usize,
    frame: usize,
    zoom: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    scale: u32,
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
}

impl AraView {
    pub(super) fn container(&self) -> Size2 {
        let size = self.viewport.get().size;
        Size2::new(size.width.into(), size.height.into())
    }

    pub(super) fn image_size(&self) -> Option<Size2> {
        self.picture
            .as_ref()
            .map(|picture| Size2::new(picture.width as f32, picture.height as f32))
    }

    /// 헤더와 상태바에 보여줄 배율(%).
    pub(super) fn zoom_percent(&self, cx: &App) -> i32 {
        let zoom = if self.view_mode(cx) == ViewMode::Webtoon {
            self.wt_zoom
        } else {
            self.zoom
        };
        (zoom * 100.0).round() as i32
    }

    pub(super) fn apply_fit(&mut self, cx: &mut Context<Self>) {
        self.position = Offset::default();
        if self.view_mode(cx) != ViewMode::Single {
            // 양쪽 보기의 배율은 화면 맞춤 대비 상대값이다.
            self.zoom = 1.0;
            return;
        }
        let Some(image) = self.image_size() else {
            return;
        };
        let mode = SettingsStore::global(cx).settings.fit_mode;
        self.zoom = zoom_for_fit(mode, self.container(), image);
    }

    pub(super) fn zoom_by(&mut self, factor: f32, anchor: Option<Offset>, cx: &mut Context<Self>) {
        match self.view_mode(cx) {
            ViewMode::Webtoon => {
                let next = (self.wt_zoom * factor).clamp(RELATIVE_ZOOM_MIN, RELATIVE_ZOOM_MAX);
                if next != self.wt_zoom {
                    // 페이지 높이가 같은 비율로 바뀌므로 읽던 위치도 함께 옮긴다.
                    self.wt_offset *= next / self.wt_zoom;
                    self.wt_zoom = next;
                    cx.notify();
                }
            }
            ViewMode::LeftToRight | ViewMode::RightToLeft => {
                let next = (self.zoom * factor).clamp(1.0, RELATIVE_ZOOM_MAX);
                if next != self.zoom {
                    let spread = self.spread_size(cx);
                    self.position = zoom_about(
                        self.container(),
                        spread,
                        self.zoom,
                        next,
                        self.position,
                        anchor.unwrap_or_default(),
                    );
                    self.zoom = next;
                    cx.notify();
                }
            }
            ViewMode::Single => {
                let Some(image) = self.image_size() else {
                    return;
                };
                let container = self.container();
                let max = if self
                    .picture
                    .as_ref()
                    .is_some_and(|picture| picture.is_vector)
                {
                    MAX_ZOOM_VECTOR
                } else {
                    MAX_ZOOM
                };
                let min = geometry::min_zoom(container, image).min(self.zoom);
                let next = (self.zoom * factor).clamp(min, max);
                if next == self.zoom {
                    return;
                }
                self.position = zoom_about(
                    container,
                    image,
                    self.zoom,
                    next,
                    self.position,
                    anchor.unwrap_or_default(),
                );
                self.zoom = next;
                self.fit_locked = false;
                cx.notify();
            }
        }
    }

    /// 팬 한계 계산에 쓰는 그림 크기(배율 1 기준).
    fn pan_extent(&self, cx: &App) -> Option<Size2> {
        if self.view_mode(cx).is_dual() {
            Some(self.spread_size(cx))
        } else {
            self.image_size()
        }
    }

    /// 단일·양쪽 보기에서 팬 여유가 있는지.
    fn can_pan(&self, cx: &App) -> bool {
        let Some(extent) = self.pan_extent(cx) else {
            return false;
        };
        let max = position_bounds(self.container(), extent, self.zoom);
        max.x > 0.0 || max.y > 0.0
    }

    /// 웹툰 전체 높이가 읽기 영역을 넘는지.
    fn webtoon_scrollable(&self) -> bool {
        let height = self.container().h;
        if height <= 0.0 || self.list.is_empty() {
            return false;
        }
        let total: f32 = (0..self.list.len())
            .map(|index| self.webtoon_size(index).1)
            .sum();
        total > height + 0.5
    }

    pub(super) fn pan_by(&mut self, dx: f32, dy: f32, window: &mut Window, cx: &mut Context<Self>) {
        if self.view_mode(cx) == ViewMode::Webtoon {
            self.webtoon_scroll(-dy, window, cx);
            return;
        }
        let Some(extent) = self.pan_extent(cx) else {
            return;
        };
        let max = position_bounds(self.container(), extent, self.zoom);
        let next = clamp_position(
            Offset {
                x: self.position.x + dx,
                y: self.position.y + dy,
            },
            max,
        );
        if next != self.position {
            self.position = next;
            cx.notify();
        }
    }

    /// 캔버스가 읽기 영역 크기 변화를 알릴 때 부른다.
    pub(super) fn viewport_resized(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.view_mode(cx) {
            ViewMode::Webtoon => self.webtoon_sync(window, cx),
            ViewMode::Single if self.fit_locked => self.apply_fit(cx),
            _ => {
                if let Some(extent) = self.pan_extent(cx) {
                    self.position = clamp_position(
                        self.position,
                        position_bounds(self.container(), extent, self.zoom),
                    );
                }
            }
        }
        self.request_thumbs(window, cx);
        cx.notify();
    }

    pub(super) fn anchor_at(&self, position: Point<Pixels>) -> Offset {
        let center = self.viewport.get().center();
        Offset {
            x: f32::from(position.x - center.x),
            y: f32::from(position.y - center.y),
        }
    }

    // ----- 입력 -----

    pub(super) fn run_wheel_action(
        &mut self,
        action: WheelAction,
        anchor: Offset,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            WheelAction::Prev => self.navigate(false, window, cx),
            WheelAction::Next => self.navigate(true, window, cx),
            WheelAction::ZoomIn => self.zoom_by(ZOOM_STEP, Some(anchor), cx),
            WheelAction::ZoomOut => self.zoom_by(1.0 / ZOOM_STEP, Some(anchor), cx),
            WheelAction::None => {}
        }
    }

    fn run_mouse_action(
        &mut self,
        action: MouseAction,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            MouseAction::Prev => self.navigate(false, window, cx),
            MouseAction::Next => self.navigate(true, window, cx),
            MouseAction::ZoomIn => self.zoom_by(ZOOM_STEP, None, cx),
            MouseAction::ZoomOut => self.zoom_by(1.0 / ZOOM_STEP, None, cx),
            MouseAction::ToggleFullscreen => window.toggle_fullscreen(),
            // 메뉴는 읽기 영역 요소가 띄운다. 여기서는 표지 지정 대상만 기억한다.
            MouseAction::ContextMenu => self.context_page = self.page_at(position, cx),
            MouseAction::Pan | MouseAction::None => {}
        }
    }

    /// 휠 이벤트를 설정된 동작으로 바꾼다. 수식키 없는 휠만 `plain`이다.
    pub(super) fn wheel_action(
        event: &ScrollWheelEvent,
        up: bool,
        cx: &App,
    ) -> (WheelAction, bool) {
        let wheel = &SettingsStore::global(cx).settings.wheel;
        let modifiers = event.modifiers;
        if modifiers.control {
            (
                if up {
                    wheel.ctrl_wheel_up
                } else {
                    wheel.ctrl_wheel_down
                },
                false,
            )
        } else if modifiers.shift {
            (
                if up {
                    wheel.shift_wheel_up
                } else {
                    wheel.shift_wheel_down
                },
                false,
            )
        } else if modifiers.alt {
            (
                if up {
                    wheel.alt_wheel_up
                } else {
                    wheel.alt_wheel_down
                },
                false,
            )
        } else {
            (if up { wheel.wheel_up } else { wheel.wheel_down }, true)
        }
    }

    fn on_wheel(&mut self, event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        let delta = event.delta.pixel_delta(window.line_height()).y;
        if delta == px(0.) {
            return;
        }
        let (action, plain) = Self::wheel_action(event, delta > px(0.), cx);
        if plain && self.view_mode(cx) == ViewMode::Webtoon {
            // 웹툰의 일반 휠은 연속 스크롤이다.
            self.webtoon_scroll_smooth(-f32::from(delta), window, cx);
            return;
        }
        let anchor = self.anchor_at(event.position);
        self.run_wheel_action(action, anchor, window, cx);
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus.focus(window, cx);
        let mouse = SettingsStore::global(cx).settings.mouse.clone();
        match event.button {
            MouseButton::Left if event.click_count >= 2 => {
                self.drag = None;
                self.run_mouse_action(mouse.double_click, event.position, window, cx);
            }
            MouseButton::Left if mouse.left_drag == MouseAction::Pan => {
                // 팬·스크롤 여유가 없으면 이미지 드래그가 창 이동이 된다.
                // 더블클릭은 위 분기에서 먼저 처리되므로 여기로 오지 않는다.
                let webtoon = self.view_mode(cx) == ViewMode::Webtoon;
                let content_movable = if webtoon {
                    self.webtoon_scrollable()
                } else {
                    self.can_pan(cx)
                };
                let has_content = if webtoon {
                    !self.list.is_empty()
                } else {
                    self.picture.is_some()
                };
                if !content_movable
                    && has_content
                    && !window.is_fullscreen()
                    && !window.is_maximized()
                {
                    self.drag = None;
                    platform::start_window_drag(window);
                    return;
                }
                self.drag = Some(DragState {
                    start: event.position,
                    origin: self.position,
                    last_y: event.position.y,
                });
            }
            MouseButton::Middle => {
                self.run_mouse_action(mouse.middle_click, event.position, window, cx)
            }
            MouseButton::Right => {
                self.run_mouse_action(mouse.right_click, event.position, window, cx)
            }
            _ => {}
        }
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let webtoon = self.view_mode(cx) == ViewMode::Webtoon;
        if event.pressed_button != Some(MouseButton::Left) {
            self.drag = None;
            return;
        }
        let Some(drag) = &mut self.drag else {
            return;
        };
        if webtoon {
            let dy = f32::from(event.position.y - drag.last_y);
            drag.last_y = event.position.y;
            self.webtoon_scroll(-dy, window, cx);
            return;
        }
        let wanted = Offset {
            x: drag.origin.x + f32::from(event.position.x - drag.start.x),
            y: drag.origin.y + f32::from(event.position.y - drag.start.y),
        };
        let Some(extent) = self.pan_extent(cx) else {
            return;
        };
        let next = clamp_position(wanted, position_bounds(self.container(), extent, self.zoom));
        if next != self.position {
            self.position = next;
            cx.notify();
        }
    }

    // ----- 애니메이션 -----

    /// 단일 보기에서 프레임을 제어할 수 있는 애니메이션인지.
    pub(super) fn has_animation(&self, cx: &App) -> bool {
        self.view_mode(cx) == ViewMode::Single
            && self
                .picture
                .as_ref()
                .is_some_and(|picture| picture.is_animated())
    }

    /// 재생 중이면 경과 시간에 맞는 프레임으로 넘기고 다음 프레임을 예약한다.
    pub(super) fn advance_animation(&mut self, window: &mut Window) {
        let Some(picture) = &self.picture else {
            return;
        };
        if !self.playing || !picture.is_animated() {
            return;
        }
        let mut elapsed = self.frame_started.elapsed().as_millis() as u64;
        let count = picture.frame_count();
        loop {
            let delay = u64::from(picture.delays_ms[self.frame % count]);
            if elapsed < delay {
                break;
            }
            if self.frame + 1 == count {
                self.loops_done += 1;
                // 유한 반복은 마지막 회차의 마지막 프레임에서 멈춘다.
                if picture
                    .loop_count
                    .is_some_and(|loops| self.loops_done >= loops)
                {
                    self.playing = false;
                    return;
                }
            }
            elapsed -= delay;
            self.frame = (self.frame + 1) % count;
            self.frame_started += Duration::from_millis(delay);
        }
        window.request_animation_frame();
    }

    pub(super) fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        if self.has_animation(cx) {
            self.playing = !self.playing;
            self.loops_done = 0;
            self.frame_started = Instant::now();
            cx.notify();
        }
    }

    pub(super) fn step_frame(&mut self, delta: isize, cx: &mut Context<Self>) {
        if !self.has_animation(cx) {
            return;
        }
        let Some(picture) = &self.picture else {
            return;
        };
        let count = picture.frame_count() as isize;
        self.playing = false;
        self.frame = (self.frame as isize + delta).rem_euclid(count) as usize;
        cx.notify();
    }

    // ----- 양쪽 보기 배치 -----

    /// 화면에 올린 페이지를 보이는 순서(왼쪽→오른쪽)로 돌려준다.
    fn dual_pages(&self, cx: &App) -> Vec<(usize, Arc<Picture>)> {
        let mut pages: Vec<(usize, Arc<Picture>)> = self
            .shown
            .iter()
            .filter_map(|index| {
                self.pages
                    .get(index)
                    .map(|page| (*index, page.picture.clone()))
            })
            .collect();
        if self.view_mode(cx) == ViewMode::RightToLeft {
            pages.reverse();
        }
        pages
    }

    /// 양쪽 보기에서 각 페이지의 사각형(읽기 영역 기준). 두 장이면 가운데 이음매에
    /// 붙이고, 한 장이면 가운데 둔다.
    pub(super) fn dual_rects(&self, cx: &App) -> Vec<(usize, Arc<Picture>, Bounds<Pixels>)> {
        let container = self.container();
        if container.is_empty() {
            return Vec::new();
        }
        let pages = self.dual_pages(cx);
        let slot_w = if pages.len() > 1 {
            container.w / 2.0
        } else {
            container.w
        };
        let center_x = container.w / 2.0 + self.position.x;
        let center_y = container.h / 2.0 + self.position.y;
        let count = pages.len();
        pages
            .into_iter()
            .enumerate()
            .map(|(slot, (index, picture))| {
                let (w, h) = (picture.width as f32, picture.height as f32);
                let scale = (slot_w / w).min(container.h / h) * self.zoom;
                let (dw, dh) = (w * scale, h * scale);
                let x = match (count, slot) {
                    (1, _) => center_x - dw / 2.0,
                    (_, 0) => center_x - dw,
                    _ => center_x,
                };
                (index, picture, rect(x, center_y - dh / 2.0, dw, dh))
            })
            .collect()
    }

    /// 배율 1에서 펼침면이 차지하는 크기. 팬 한계 계산에 쓴다.
    fn spread_size(&self, cx: &App) -> Size2 {
        let container = self.container();
        let pages = self.dual_pages(cx);
        if pages.is_empty() || container.is_empty() {
            return Size2::default();
        }
        let slot_w = if pages.len() > 1 {
            container.w / 2.0
        } else {
            container.w
        };
        let mut widest = 0.0f32;
        let mut tallest = 0.0f32;
        for (_, picture) in &pages {
            let (w, h) = (picture.width as f32, picture.height as f32);
            let scale = (slot_w / w).min(container.h / h);
            widest = widest.max(w * scale);
            tallest = tallest.max(h * scale);
        }
        // 두 장은 이음매 기준으로 좌우에 놓이므로 넓은 쪽의 두 배를 폭으로 본다.
        Size2::new(widest * pages.len() as f32, tallest)
    }

    /// 창 좌표의 지점 아래에 있는 페이지 인덱스(표지 지정 대상).
    pub(super) fn page_at(&self, position: Point<Pixels>, cx: &App) -> Option<usize> {
        let origin = self.viewport.get().origin;
        let local = point(position.x - origin.x, position.y - origin.y);
        match self.view_mode(cx) {
            ViewMode::Single => Some(self.index),
            ViewMode::Webtoon => self
                .webtoon_rects()
                .into_iter()
                .find(|(_, bounds)| bounds.contains(&local))
                .map(|(index, _)| index),
            _ => self
                .dual_rects(cx)
                .into_iter()
                .find(|(_, _, bounds)| bounds.contains(&local))
                .map(|(index, _, _)| index)
                .or(Some(self.index)),
        }
    }

    // ----- 웹툰 -----

    /// 웹툰 페이지의 표시 크기. 로드한 적 없는 페이지는 자리 표시 비율을 쓴다.
    fn webtoon_size(&self, index: usize) -> (f32, f32) {
        let avail = self.container().w.max(WT_MIN_WIDTH);
        let known = self
            .pages
            .get(&index)
            .map(|page| (page.picture.width, page.picture.height))
            .or_else(|| self.wt_dims.get(&index).copied());
        let (natural_w, aspect) = match known {
            Some((w, h)) if w > 0 => (w as f32, h as f32 / w as f32),
            _ => (WT_UNKNOWN_WIDTH, WT_UNKNOWN_ASPECT),
        };
        let base = if self.wt_fit_width {
            avail
        } else {
            natural_w.min(avail)
        };
        let width = (base * self.wt_zoom).clamp(WT_MIN_WIDTH.min(avail), avail);
        (width, width * aspect)
    }

    /// 화면 위쪽 `above`부터 아래쪽 `below`까지 걸치는 페이지의 사각형(읽기 영역 기준).
    fn webtoon_span(&self, above: f32, below: f32) -> Vec<(usize, Bounds<Pixels>)> {
        let container = self.container();
        let total = self.list.len();
        let mut out = Vec::new();
        if total == 0 || container.is_empty() {
            return out;
        }
        let center_x = container.w / 2.0;
        // 기준 페이지 위쪽으로 먼저 채운다.
        let mut y = -self.wt_offset;
        let mut index = self.wt_anchor.min(total - 1);
        while index > 0 && y > -above {
            index -= 1;
            y -= self.webtoon_size(index).1;
        }
        while index < total && y < container.h + below {
            let (w, h) = self.webtoon_size(index);
            if y + h > -above {
                out.push((index, rect(center_x - w / 2.0, y, w, h)));
            }
            y += h;
            index += 1;
        }
        out
    }

    fn webtoon_rects(&self) -> Vec<(usize, Bounds<Pixels>)> {
        self.webtoon_span(0.0, 0.0)
    }

    /// 기준 페이지와 오프셋을 정규화하고 목록 양 끝에서 멈춘다.
    fn webtoon_normalize(&mut self) {
        let total = self.list.len();
        if total == 0 {
            return;
        }
        self.wt_anchor = self.wt_anchor.min(total - 1);
        loop {
            let height = self.webtoon_size(self.wt_anchor).1;
            if self.wt_offset >= height && self.wt_anchor + 1 < total {
                self.wt_offset -= height;
                self.wt_anchor += 1;
            } else {
                break;
            }
        }
        while self.wt_offset < 0.0 && self.wt_anchor > 0 {
            self.wt_anchor -= 1;
            self.wt_offset += self.webtoon_size(self.wt_anchor).1;
        }
        if self.wt_offset < 0.0 {
            self.wt_offset = 0.0;
        }
    }

    /// 거리를 쌓아 두고 프레임마다 감속하며 나눠 스크롤한다.
    pub(super) fn webtoon_scroll_smooth(
        &mut self,
        dy: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if dy == 0.0 || self.list.is_empty() {
            return;
        }
        self.wt_pending += dy;
        if !self.wt_animating {
            self.wt_animating = true;
            self.wt_frame = Instant::now();
            self.webtoon_next_frame(window, cx);
        }
    }

    fn webtoon_next_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        window.on_next_frame(move |window, cx| {
            view.update(cx, |this, cx| this.webtoon_smooth_frame(window, cx));
        });
    }

    fn webtoon_smooth_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = Instant::now();
        let dt = now
            .duration_since(std::mem::replace(&mut self.wt_frame, now))
            .as_secs_f32()
            .min(0.05);
        let mut step = self.wt_pending * (1.0 - (-dt / WT_SMOOTH_TIME).exp());
        // 거의 다 왔으면 남은 거리를 한 번에 반영하고 끝낸다.
        let settled = self.wt_pending.abs() < 1.0;
        if settled {
            step = self.wt_pending;
        }
        self.wt_pending -= step;
        let before = (self.wt_anchor, self.wt_offset);
        self.webtoon_scroll(step, window, cx);
        // 끝에 닿아 더 움직이지 못하면 남은 거리를 버린다.
        if (self.wt_anchor, self.wt_offset) == before {
            self.wt_pending = 0.0;
        }
        if settled || self.wt_pending == 0.0 {
            self.wt_pending = 0.0;
            self.wt_animating = false;
        } else {
            self.webtoon_next_frame(window, cx);
        }
    }

    pub(super) fn webtoon_scroll(&mut self, dy: f32, window: &mut Window, cx: &mut Context<Self>) {
        if dy == 0.0 || self.list.is_empty() {
            return;
        }
        self.wt_offset += dy;
        self.webtoon_normalize();
        if dy > 0.0 {
            // 마지막 페이지 아래로는 내려가지 않는다.
            let viewport_h = self.container().h;
            let mut remaining = -self.wt_offset;
            let mut index = self.wt_anchor;
            while index < self.list.len() && remaining < viewport_h {
                remaining += self.webtoon_size(index).1;
                index += 1;
            }
            if index == self.list.len() && remaining < viewport_h {
                self.wt_offset -= viewport_h - remaining;
                self.webtoon_normalize();
            }
        }
        self.webtoon_sync(window, cx);
        cx.notify();
    }

    /// 로드 전후로 높이가 달라져도 읽던 위치가 유지되게 한다.
    pub(super) fn webtoon_page_arrived(
        &mut self,
        index: usize,
        picture: &Picture,
        _: &mut Context<Self>,
    ) {
        let before = self.webtoon_size(index).1;
        self.wt_dims.insert(index, (picture.width, picture.height));
        if index == self.wt_anchor && before > 0.0 {
            let after = self.webtoon_size(index).1;
            self.wt_offset *= after / before;
        }
    }

    /// 화면 근처 페이지를 요청하고 먼 페이지를 내려놓은 뒤, 화면 가운데 페이지를
    /// 현재 페이지로 삼는다. 현재 페이지가 바뀌어도 전체를 다시 로드하지 않는다.
    pub(super) fn webtoon_sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let settings = &SettingsStore::global(cx).settings;
        self.wt_fit_width = settings.webtoon_fit_width;
        self.webtoon_normalize();
        let viewport_h = self.container().h.max(1.0);

        // 위아래 2화면 안은 로드하고, 4화면 밖은 내려놓는다.
        let near: Vec<usize> = self
            .webtoon_span(viewport_h * 2.0, viewport_h * 2.0)
            .into_iter()
            .map(|(index, _)| index)
            .collect();
        for index in &near {
            self.ensure_page(*index, true, window, cx);
        }
        let keep: HashSet<usize> = self
            .webtoon_span(viewport_h * 4.0, viewport_h * 4.0)
            .into_iter()
            .map(|(index, _)| index)
            .collect();
        self.evict_except(&keep, window, cx);

        let middle = px(viewport_h / 2.0);
        let center = self
            .webtoon_rects()
            .into_iter()
            .find(|(_, bounds)| bounds.bottom() >= middle)
            .map(|(index, _)| index)
            .unwrap_or(self.wt_anchor);
        let changed = center != self.index || self.pending_commit;
        self.index = center;
        self.shown = vec![center];
        let Some(page) = self.pages.get(&center) else {
            return;
        };
        let stale = self
            .picture
            .as_ref()
            .is_none_or(|picture| !Arc::ptr_eq(picture, &page.picture));
        if changed || stale {
            self.pending_commit = false;
            self.error = None;
            self.picture = Some(page.picture.clone());
            self.info = Some(page.info.clone());
            if let Some(info) = &self.info {
                window.set_window_title(&self.display_name(info));
            }
            self.save_progress(cx);
            self.load_info(window, cx);
            self.request_thumbs(window, cx);
            cx.notify();
        }
    }

    // ----- 픽셀 보존 표시 -----

    /// 지금 단일 보기를 최근접 보간으로 그려야 하는지(SPEC §18). 축소에서는 항상 부드럽게.
    fn wants_pixelated(&self, cx: &App) -> bool {
        if self.view_mode(cx) != ViewMode::Single || self.zoom < 1.0 {
            return false;
        }
        if self
            .picture
            .as_ref()
            .is_none_or(|picture| picture.is_vector)
        {
            return false;
        }
        let settings = &SettingsStore::global(cx).settings;
        match settings.image_scaling_mode {
            ImageScalingMode::Smooth => false,
            ImageScalingMode::Pixelated => true,
            ImageScalingMode::Auto => {
                self.zoom >= 2.0
                    || (settings.auto_detect_pixel_art && self.pixel_hint == Some(true))
            }
        }
    }

    /// 자동 모드의 1x~2x 확대에서만 픽셀 아트 분석을 요청한다. 그 밖에서는 결과가
    /// 판정에 쓰이지 않는다.
    pub(super) fn request_pixel_hint(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let settings = &SettingsStore::global(cx).settings;
        if self.pixel_hint.is_some()
            || self.pixel_hint_pending
            || self.view_mode(cx) != ViewMode::Single
            || settings.image_scaling_mode != ImageScalingMode::Auto
            || !settings.auto_detect_pixel_art
            || !(1.0..2.0).contains(&self.zoom)
        {
            return;
        }
        let Some(info) = &self.info else {
            return;
        };
        self.pixel_hint_pending = true;
        let path = PathBuf::from(&info.file_path);
        let generation = self.list_gen;
        let index = self.index;
        let task = cx.background_spawn(async move { araview_core::pixel_art::detect_file(&path) });
        cx.spawn_in(window, async move |this, cx| {
            let detection = task.await;
            this.update(cx, |this, cx| {
                this.pixel_hint_pending = false;
                if this.list_gen == generation && this.index == index {
                    this.pixel_hint = Some(matches!(
                        detection.classification,
                        PixelArtClassification::PixelArt
                    ));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// 보이는 영역만 최근접 보간으로 확대한 래스터를 만든다. GPUI 샘플러는 선형
    /// 보간뿐이라 기기 픽셀 크기로 미리 확대해 1:1로 그린다.
    fn pixel_view(
        &mut self,
        dest: Bounds<Pixels>,
        scale_factor: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<(Arc<RenderImage>, Bounds<Pixels>)> {
        let picture = self.picture.clone()?;
        let level = &picture.levels[0];
        let container = self.container();
        let visible = dest.intersect(&rect(0.0, 0.0, container.w, container.h));
        let width = (f32::from(visible.size.width) * scale_factor).round() as u32;
        let height = (f32::from(visible.size.height) * scale_factor).round() as u32;
        if width == 0 || height == 0 {
            return None;
        }
        let frame = self
            .frame
            .min(level.tiles[0].image.frame_count().saturating_sub(1));
        let key = PixelKey {
            picture: Arc::as_ptr(&picture) as usize,
            frame,
            zoom: self.zoom.to_bits(),
            x: f32::from(dest.origin.x).to_bits(),
            y: f32::from(dest.origin.y).to_bits(),
            width,
            height,
            scale: scale_factor.to_bits(),
        };
        if let Some(cached) = &self.pixel_view
            && cached.key == key
        {
            return Some((cached.image.clone(), cached.rect));
        }
        // 원본은 타일 격자(행 우선)로 나뉘어 있을 수 있어, 출력 픽셀마다 타일을 찾아 읽는다.
        let tile_side = crate::picture::TILE_SIDE as usize;
        let (src_w, src_h) = (level.width as usize, level.height as usize);
        let tile_columns = src_w.div_ceil(tile_side);
        let sources: Vec<(&[u8], usize)> = level
            .tiles
            .iter()
            .map(|tile| Some((tile.image.as_bytes(frame)?, tile.w as usize)))
            .collect::<Option<_>>()?;
        let step = 1.0 / (self.zoom * scale_factor);
        let start_x = f32::from(visible.origin.x - dest.origin.x) / self.zoom;
        let start_y = f32::from(visible.origin.y - dest.origin.y) / self.zoom;
        // 열마다 (타일 열, 타일 안 바이트 오프셋)을 미리 구한다.
        let columns: Vec<(usize, usize)> = (0..width)
            .map(|x| {
                let src_x = ((start_x + (x as f32 + 0.5) * step) as usize).min(src_w - 1);
                (src_x / tile_side, (src_x % tile_side) * 4)
            })
            .collect();
        let mut buffer = vec![0u8; width as usize * height as usize * 4];
        for (y, row) in buffer.chunks_exact_mut(width as usize * 4).enumerate() {
            let src_y = ((start_y + (y as f32 + 0.5) * step) as usize).min(src_h - 1);
            let (tile_row, local_y) = (src_y / tile_side, src_y % tile_side);
            for (pixel, (tile_column, offset)) in
                row.as_chunks_mut::<4>().0.iter_mut().zip(&columns)
            {
                let (bytes, tile_w) = sources[tile_row * tile_columns + tile_column];
                let at = local_y * tile_w * 4 + offset;
                pixel.copy_from_slice(&bytes[at..at + 4]);
            }
        }
        // 원본이 이미 BGRA라 채널을 바꾸지 않고 그대로 옮긴다.
        let frames: SmallVec<[Frame; 1]> =
            SmallVec::from_elem(Frame::new(RgbaImage::from_raw(width, height, buffer)?), 1);
        let image = Arc::new(RenderImage::new(frames));
        if let Some(previous) = self.pixel_view.replace(PixelView {
            key,
            image: image.clone(),
            rect: visible,
        }) {
            cx.drop_image(previous.image, Some(window));
        }
        Some((image, visible))
    }

    pub(super) fn drop_pixel_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(previous) = self.pixel_view.take() {
            cx.drop_image(previous.image, Some(window));
        }
    }

    // ----- 렌더 -----

    pub(super) fn render_viewport(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let settings = SettingsStore::global(cx).settings.clone();
        let mode = self.view_mode(cx);
        let background = settings.viewer_background;
        let fill = match background {
            ViewerBackground::Theme | ViewerBackground::Checker => cx.theme().background,
            ViewerBackground::Black => gpui_kit::black(),
            ViewerBackground::White => gpui_kit::white(),
        };
        let checker = (background == ViewerBackground::Checker).then(|| {
            (
                Hsla::from(rgb(super::CHECKER_BASE)),
                Hsla::from(rgb(super::CHECKER_ALT)),
            )
        });
        let scale_factor = window.scale_factor();
        let container = self.container();

        let mut items: Vec<PaintItem> = Vec::new();
        let mut placeholders: Vec<Bounds<Pixels>> = Vec::new();
        let mut boundaries: Vec<Pixels> = Vec::new();
        let mut pixel: Option<(Arc<RenderImage>, Bounds<Pixels>)> = None;
        match mode {
            ViewMode::Single => {
                if let Some(picture) = self.picture.clone() {
                    let (dw, dh) = (
                        picture.width as f32 * self.zoom,
                        picture.height as f32 * self.zoom,
                    );
                    let dest = rect(
                        container.w / 2.0 + self.position.x - dw / 2.0,
                        container.h / 2.0 + self.position.y - dh / 2.0,
                        dw,
                        dh,
                    );
                    if self.wants_pixelated(cx) {
                        pixel = self.pixel_view(dest, scale_factor, window, cx);
                    }
                    if pixel.is_none() {
                        self.drop_pixel_view(window, cx);
                    }
                    items.push(PaintItem {
                        picture,
                        rect: dest,
                        frame: self.frame,
                    });
                }
            }
            ViewMode::LeftToRight | ViewMode::RightToLeft => {
                self.drop_pixel_view(window, cx);
                items.extend(
                    self.dual_rects(cx)
                        .into_iter()
                        .map(|(_, picture, rect)| PaintItem {
                            picture,
                            rect,
                            frame: 0,
                        }),
                );
            }
            ViewMode::Webtoon => {
                self.drop_pixel_view(window, cx);
                for (index, bounds) in self.webtoon_rects() {
                    if settings.webtoon_page_boundaries && index > 0 {
                        boundaries.push(bounds.top());
                    }
                    match self.pages.get(&index) {
                        Some(page) => items.push(PaintItem {
                            picture: page.picture.clone(),
                            rect: bounds,
                            frame: 0,
                        }),
                        None => placeholders.push(bounds),
                    }
                }
            }
        }

        let viewport = self.viewport.clone();
        let entity = cx.entity();
        let placeholder_fill = cx.theme().muted;
        let boundary_fill = cx.theme().border;
        let drawing = canvas(
            move |bounds, window, cx| {
                if viewport.get() != bounds {
                    viewport.set(bounds);
                    // prepaint 중에는 엔티티를 고칠 수 없어 프레임 뒤로 미룬다.
                    let handle = window.window_handle();
                    cx.defer(move |cx| {
                        handle
                            .update(cx, |_, window, cx| {
                                entity.update(cx, |this, cx| this.viewport_resized(window, cx));
                            })
                            .ok();
                    });
                }
            },
            move |bounds, _, window, _| {
                let shift = |local: Bounds<Pixels>| {
                    Bounds::new(
                        point(
                            bounds.origin.x + local.origin.x,
                            bounds.origin.y + local.origin.y,
                        ),
                        local.size,
                    )
                };
                for area in &placeholders {
                    window.paint_quad(gpui_kit::fill(
                        shift(*area).intersect(&bounds),
                        placeholder_fill,
                    ));
                }
                for item in &items {
                    let dest = shift(item.rect);
                    if let Some((base, alt)) = checker {
                        paint_checker(window, bounds.intersect(&dest), base, alt);
                    }
                    if pixel.is_some() {
                        continue;
                    }
                    let device_scale =
                        f32::from(dest.size.width) / item.picture.width as f32 * scale_factor;
                    let level = item.picture.level_for(device_scale);
                    let sx = dest.size.width / level.width as f32;
                    let sy = dest.size.height / level.height as f32;
                    for tile in &level.tiles {
                        let tile_bounds = Bounds::new(
                            point(
                                dest.origin.x + sx * tile.x as f32,
                                dest.origin.y + sy * tile.y as f32,
                            ),
                            size(sx * tile.w as f32, sy * tile.h as f32),
                        );
                        if !tile_bounds.intersects(&bounds) {
                            continue;
                        }
                        let frame = item.frame.min(tile.image.frame_count().saturating_sub(1));
                        if let Err(error) = window.paint_image(
                            bounds,
                            tile_bounds,
                            Corners::default(),
                            tile.image.clone(),
                            frame,
                            false,
                        ) {
                            log::warn!("[paint] image tile failed: {error}");
                        }
                    }
                }
                if let Some((image, local)) = &pixel {
                    let dest = shift(*local);
                    if let Err(error) = window.paint_image(
                        bounds,
                        dest,
                        Corners::default(),
                        image.clone(),
                        0,
                        false,
                    ) {
                        log::warn!("[paint] pixel view failed: {error}");
                    }
                }
                for y in &boundaries {
                    let line = Bounds::new(
                        point(bounds.origin.x, bounds.origin.y + *y),
                        size(bounds.size.width, px(1.)),
                    );
                    window.paint_quad(gpui_kit::fill(line.intersect(&bounds), boundary_fill));
                }
            },
        )
        .size_full();

        let waiting = self.picture.is_none() && (self.opening || self.pending_commit);
        // 이미 그림이 떠 있을 때는 새 페이지나 파일이 늦어질 때만 모서리에 작게 알린다.
        let busy = !waiting
            && (self.opening_visible()
                || (self.pending_commit
                    && self.nav_started.elapsed() >= super::pages::PREVIOUS_HOLD)
                || (mode == ViewMode::Webtoon && !self.page_loading.is_empty()));
        // 디코드가 길어지면 이미 받아 둔 썸네일을 저해상 미리보기로 먼저 깐다.
        let preview_thumb = (mode == ViewMode::Single
            && self.pending_commit
            && self.nav_started.elapsed() >= super::pages::PREVIOUS_HOLD)
            .then(|| self.list.get(self.index))
            .flatten()
            .and_then(|item| match self.thumbs.get(item) {
                Some(super::ThumbState::Ready(path)) => Some(path.clone()),
                _ => None,
            });
        let preview = (mode != ViewMode::Webtoon)
            .then(|| self.pages.get(&self.index))
            .flatten()
            .filter(|page| page.archive_preview && self.shown.contains(&self.index))
            .and_then(|_| self.list.get(self.index))
            .map(|path| {
                std::path::Path::new(&**path)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.to_string())
            });
        let progress = (mode == ViewMode::Webtoon
            && (settings.webtoon_show_progress || settings.webtoon_thumbnail_jump))
            .then(|| {
                let total = self.list.len().max(1);
                let percent = ((self.index + 1) * 100 / total).min(100);
                (
                    settings.webtoon_show_progress.then(|| {
                        t_with(
                            "viewer.webtoon.position",
                            &[
                                ("current", &(self.index + 1)),
                                ("total", &total),
                                ("percent", &percent),
                            ],
                        )
                    }),
                    settings.webtoon_thumbnail_jump,
                )
            });

        div()
            .id("viewport")
            .relative()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .bg(fill)
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.drag = None),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.drag = None),
            )
            .on_scroll_wheel(cx.listener(Self::on_wheel))
            .child(drawing)
            .when_some(preview_thumb, |area, path| {
                area.child(
                    div()
                        .absolute()
                        .inset_0()
                        .bg(fill)
                        .child(img(path).size_full().object_fit(ObjectFit::Contain)),
                )
            })
            .when(waiting, |area| {
                area.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(super::loading_badge(t("viewer.loading"), cx)),
                )
            })
            .when(busy, |area| {
                area.child(
                    div()
                        .absolute()
                        .top_3()
                        .right_3()
                        .child(super::loading_badge(t("viewer.loading"), cx)),
                )
            })
            .when_some(preview, |area, name| {
                area.child(
                    div()
                        .absolute()
                        .top_3()
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(
                            h_flex()
                                .gap_3()
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().popover)
                                .text_sm()
                                .child(t_with("viewer.archivePreview.hint", &[("name", &name)]))
                                .child(
                                    Button::new("open-archive")
                                        .primary()
                                        .small()
                                        .icon(IconName::BookOpen)
                                        .label(t("viewer.archivePreview.open"))
                                        .tooltip(t("viewer.archivePreview.openTitle"))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.open_previewed_archive(window, cx)
                                        })),
                                ),
                        ),
                )
            })
            .when_some(progress, |area, (label, jump)| {
                area.child(
                    h_flex()
                        .absolute()
                        .bottom_3()
                        .right_3()
                        .gap_2()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().popover)
                        .text_xs()
                        .when_some(label, |row, text| row.child(text))
                        .when(jump, |row| {
                            row.child(
                                Button::new("webtoon-grid")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::LayoutGrid)
                                    .tooltip(t("viewer.webtoon.openThumbnails"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.toggle_grid(window, cx)
                                    })),
                            )
                        }),
                )
            })
    }
}
