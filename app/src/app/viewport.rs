//! 읽기 영역: 단일·양쪽·웹툰 배치, 줌/팬, 휠·마우스 입력, 픽셀 보존 표시
//! (SPEC §6, §7, §18).

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState, SliderValue};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use image::{Frame, RgbaImage};
use smallvec::{SmallVec, smallvec};

use super::{AraView, DragState, paint_checker};
use crate::assets::Ph;
use crate::geometry::{
    self, MAX_ZOOM, MAX_ZOOM_VECTOR, Offset, Size2, ZOOM_STEP, clamp_position, position_bounds,
    zoom_about, zoom_for_fit,
};
use araview_core::ocr::OcrRect;

use crate::i18n::{t, t_with};
use crate::layout;
use crate::picture::{Picture, choose_level};
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

pub(super) struct ZoomAnimation {
    target: f32,
    anchor: Offset,
    mode: ViewMode,
    index: usize,
    list_gen: u64,
    last_frame: Instant,
}

fn zoom_step(current: f32, target: f32, elapsed: Duration) -> f32 {
    let fraction = 1.0 - (-elapsed.as_secs_f32().min(0.05) / 0.08).exp();
    let next = current + (target - current) * fraction;
    if (target - next).abs() <= target * 0.001 {
        target
    } else {
        next
    }
}

/// 단일 보기의 맞춤 상태. 시작값은 `Pending`이다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Fit {
    /// 맞춤을 따르지만 읽기 영역 크기를 아직 재지 못해 맞춤 배율이 확정되지 않았다.
    /// 캔버스가 크기를 재면 `apply_fit`으로 바로 확정한다. 재기 전에 그리면 100% 기본
    /// 배율로 잠깐 실제 크기가 표시되는 문제가 있었으므로, 이 동안에는 그림을 비둔다.
    Pending,
    /// 맞춤 고정. 창 크기가 바뀌면 다시 맞춘다.
    Locked,
    /// 수동 줌. 창 크기가 바뀌어도 배율을 유지한다.
    Manual,
}

impl Fit {
    /// 맞춤(헤더의 맞춤 버튼 선택 표시, 창 크기 변경 시 다시 맞춤)을 따르는지.
    pub fn follows_window(self) -> bool {
        self != Self::Manual
    }

    /// 맞춤 고정으로 돌아간다(새 그림, 회전, 맞춤 모드 선택). 크기를 아직 재지
    /// 못했으면 그대로 기다린다.
    pub fn lock(self) -> Self {
        match self {
            Self::Pending => Self::Pending,
            Self::Locked | Self::Manual => Self::Locked,
        }
    }

    /// 맞춤 배율을 정했다. 수동 줌은 그대로 둔다.
    fn measured(self) -> Self {
        match self {
            Self::Pending | Self::Locked => Self::Locked,
            Self::Manual => Self::Manual,
        }
    }

    /// 읽기 영역을 재기 전이라 맞춤 배율을 정하지 못했다. 수동 줌은 그대로 둔다.
    fn unmeasured(self) -> Self {
        match self {
            Self::Pending | Self::Locked => Self::Pending,
            Self::Manual => Self::Manual,
        }
    }
}

/// 애니메이션(GIF/APNG 등) 재생 상태. 프레임 시작 시각은 재생 중에만 있으므로
/// 정지 상태에서 낡은 시작 시각이 남는 조합이 없다.
#[derive(Debug, Clone, Default)]
pub(super) struct AnimPlayback {
    /// 보이는 프레임 번호.
    frame: usize,
    /// 재생 중이면 현재 프레임을 보이기 시작한 시각, 정지면 `None`.
    frame_started: Option<Instant>,
    /// 끝까지 재생한 횟수. 유한 반복 애니메이션을 마지막 회차에서 멈추는 데 쓴다.
    /// 끝까지 재생해 멈춘 뒤에도 남는다(진단 상태에 보인다).
    loops_done: u32,
}

impl AnimPlayback {
    pub(super) fn is_playing(&self) -> bool {
        self.frame_started.is_some()
    }

    pub(super) fn frame(&self) -> usize {
        self.frame
    }

    pub(super) fn loops_done(&self) -> u32 {
        self.loops_done
    }

    /// 새 그림을 기준으로 처음부터 시작한다. `play`면 재생, 아니면 첫 프레임에서 정지.
    pub(super) fn restart(&mut self, play: bool, now: Instant) {
        self.frame = 0;
        self.loops_done = 0;
        self.frame_started = play.then_some(now);
    }

    /// 정지하고 첫 프레임으로 돌린다. 반복 횟수는 그대로 둔다.
    pub(super) fn stop_at_first_frame(&mut self) {
        self.frame = 0;
        self.frame_started = None;
    }

    /// 재생/정지를 바꾼다. 반복 횟수를 새로 센다. 재생으로 바뀌면 현재 프레임부터
    /// 지연 시간을 다시 센다.
    pub(super) fn toggle(&mut self, now: Instant) {
        self.loops_done = 0;
        self.frame_started = if self.is_playing() { None } else { Some(now) };
    }

    /// 정지하고 `delta`만큼 프레임을 옮긴다(앞뒤 끝에서 감김).
    pub(super) fn step(&mut self, delta: isize, count: usize) {
        if count == 0 {
            return;
        }
        self.frame_started = None;
        self.frame = (self.frame as isize + delta).rem_euclid(count as isize) as usize;
    }

    /// 지정한 프레임으로 옮긴다. 재생 중이면 그 프레임부터 지연 시간을 다시 센다.
    pub(super) fn jump_to(&mut self, frame: usize, now: Instant) {
        self.frame = frame;
        if self.is_playing() {
            self.frame_started = Some(now);
        }
    }

    /// 같은 애니메이션을 다시 불러온 뒤(회전 등) 첫 프레임으로 돌린다. 재생 상태는 유지한다.
    pub(super) fn rewind(&mut self, now: Instant) {
        self.jump_to(0, now);
    }

    /// 경과 시간에 맞춰 프레임을 넘긴다. 계속 재생하면 `true`(다음 프레임을 예약),
    /// 정지 중이거나 유한 반복의 마지막 회차 마지막 프레임에서 멈췄으면 `false`다.
    pub(super) fn advance(
        &mut self,
        delays_ms: &[u32],
        loop_count: Option<u32>,
        now: Instant,
    ) -> bool {
        let Some(mut started) = self.frame_started else {
            return false;
        };
        let count = delays_ms.len();
        if count == 0 {
            return false;
        }
        let mut elapsed = now.saturating_duration_since(started).as_millis() as u64;
        let mut playing = true;
        loop {
            let delay = u64::from(delays_ms[self.frame % count]);
            if elapsed < delay {
                break;
            }
            if self.frame + 1 == count {
                self.loops_done += 1;
                // 유한 반복은 마지막 회차의 마지막 프레임에서 멈춘다.
                if loop_count.is_some_and(|loops| self.loops_done >= loops) {
                    playing = false;
                    break;
                }
            }
            elapsed -= delay;
            self.frame = (self.frame + 1) % count;
            started += Duration::from_millis(delay);
        }
        self.frame_started = playing.then_some(started);
        playing
    }
}

/// 애니메이션 프레임 진행 슬라이더. 프레임 수가 바뀌면 다시 만든다.
pub(super) struct AnimTimeline {
    frames: usize,
    slider: Entity<SliderState>,
    _sub: Subscription,
}

/// 재생 중 조작이 없으면 하단 컨트롤을 숨기기까지의 시간.
const OVERLAY_HIDE_AFTER: Duration = Duration::from_secs(3);

/// 하단 재생 컨트롤을 보일지. 정지 중에는 항상 보이고, 재생 중에는 마지막
/// 조작 뒤 `OVERLAY_HIDE_AFTER`가 지나면 숨겨 화면을 가리지 않는다.
fn overlay_visible(playing: bool, idle: Duration) -> bool {
    !playing || idle < OVERLAY_HIDE_AFTER
}

/// 프레임 카운터가 표시하는 자릿수(4자리). 이보다 큰 값은 앞자리를 줄임표로
/// 대체해 어떤 애니메이션이든 같은 폭을 차지하게 한다.
const FRAME_COUNTER_MODULUS: u64 = 10_000;
/// 앞자리 생략 표시(공백 또는 `…`)를 포함한 슬롯 폭(px).
const FRAME_COUNTER_WIDTH: f32 = 100.0;

/// 프레임 카운터. 4자리까지는 그대로 보여주고, 그보다 큰 값은 앞자리를
/// 줄임표로 대체해 항상 같은 폭(11자)을 차지하게 한다.
fn format_frame_counter(current: u64, total: u64) -> String {
    let current = current.min(total);
    if total >= FRAME_COUNTER_MODULUS {
        format!(
            "…{:04}/…{:04}",
            current % FRAME_COUNTER_MODULUS,
            total % FRAME_COUNTER_MODULUS
        )
    } else {
        format!(" {current:04}/ {total:04}")
    }
}

/// 읽기 값의 숫자 폭을 고정한다(DESIGN "Readout").
fn tabular_figures() -> FontFeatures {
    static FIGURES: std::sync::LazyLock<FontFeatures> =
        std::sync::LazyLock::new(|| FontFeatures(Arc::new(vec![("tnum".into(), 1)])));
    FIGURES.clone()
}

fn slider_ms(event: &SliderEvent) -> u64 {
    match event {
        SliderEvent::Change(SliderValue::Single(value))
        | SliderEvent::Release(SliderValue::Single(value)) => (*value).max(0.0) as u64,
        SliderEvent::Change(SliderValue::Range(start, _))
        | SliderEvent::Release(SliderValue::Range(start, _)) => (*start).max(0.0) as u64,
    }
}
/// 자동 표시 모드가 픽셀 보존으로 바뀌는 확대 배율(SPEC §18).
const AUTO_PIXELATED_ZOOM: f32 = 2.0;
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

/// 레벨의 타일을 실제 좌표로 찾는 격자. 정지 이미지는 `TILE_SIDE` 격자로 나뉘지만
/// 애니메이션은 타일 하나(한 변 최대 16384px)라, 한 변 크기를 가정하지 않고 각 타일의
/// 시작 좌표로 열과 행을 나눈다.
struct TileGrid {
    /// 열 시작 x(오름차순).
    xs: Vec<u32>,
    /// 행 시작 y(오름차순).
    ys: Vec<u32>,
    /// 행 우선 칸마다 그 칸을 덮는 타일 번호.
    cells: Vec<Option<usize>>,
}

impl TileGrid {
    fn new(origins: &[(u32, u32)]) -> Self {
        let mut xs: Vec<u32> = origins.iter().map(|(x, _)| *x).collect();
        let mut ys: Vec<u32> = origins.iter().map(|(_, y)| *y).collect();
        xs.sort_unstable();
        xs.dedup();
        ys.sort_unstable();
        ys.dedup();
        let mut cells = vec![None; xs.len() * ys.len()];
        for (tile, (x, y)) in origins.iter().enumerate() {
            let column = xs.partition_point(|start| start < x);
            let row = ys.partition_point(|start| start < y);
            cells[row * xs.len() + column] = Some(tile);
        }
        Self { xs, ys, cells }
    }

    /// `x`가 속한 열과 그 열 안의 x.
    fn column(&self, x: u32) -> (usize, u32) {
        let column = self
            .xs
            .partition_point(|start| *start <= x)
            .saturating_sub(1);
        (column, x - self.xs[column])
    }

    /// `y`가 속한 행과 그 행 안의 y.
    fn row(&self, y: u32) -> (usize, u32) {
        let row = self
            .ys
            .partition_point(|start| *start <= y)
            .saturating_sub(1);
        (row, y - self.ys[row])
    }

    fn tile(&self, column: usize, row: usize) -> Option<usize> {
        self.cells
            .get(row * self.xs.len() + column)
            .copied()
            .flatten()
    }
}

/// 글자 강조의 (줄 사각형, 글자 사각형) 목록.
type OcrBoxes = (Vec<Bounds<Pixels>>, Vec<Bounds<Pixels>>);

/// 페이지 사각형을 기기 픽셀 격자에 맞춘다. GPUI 샘플러는 밉맵 없는 선형 보간뿐이라
/// 사각형이 반쪽 픽셀에 걸리면 1:1 배율에서도 각 행·열이 50/50 블렌딩돼 흐려진다
/// (yosh가 페이지 넘김 경로에서 정수 배치로 막는 문제). 가장자리 둘을 각각 스냅해
/// 크기 오차가 1기기픽셀 이하가 되게 한다. 웹툰(스크롤)은 부드러움을 위해 쓰지 않는다.
fn snap_to_device_pixels(bounds: Bounds<Pixels>, scale: f32) -> Bounds<Pixels> {
    if scale <= 0.0 || !scale.is_finite() {
        return bounds;
    }
    let snap = |v: Pixels| Pixels::from((f32::from(v) * scale).round() / scale);
    let x0 = snap(bounds.origin.x);
    let y0 = snap(bounds.origin.y);
    let x1 = snap(bounds.origin.x + bounds.size.width);
    let y1 = snap(bounds.origin.y + bounds.size.height);
    rect(
        f32::from(x0),
        f32::from(y0),
        f32::from(x1) - f32::from(x0),
        f32::from(y1) - f32::from(y0),
    )
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
        self.zoom_animation = None;
        self.position = Offset::default();
        if self.view_mode(cx) != ViewMode::Single {
            // 양쪽 보기의 배율은 화면 맞춤 대비 상대값이다.
            self.zoom = 1.0;
            self.fit = self.fit.measured();
            return;
        }
        let Some(image) = self.image_size() else {
            return;
        };
        let container = self.container();
        if container.is_empty() {
            // 캔버스가 읽기 영역을 아직 재기 전은 맞춤 배율을 정할 수 없다.
            // 기본 배율(100%)로 그리면 실제 크기가 한순간 보이므로, 크기를
            // 재면 바로 적용하도록 기다린다.
            self.fit = self.fit.unmeasured();
            return;
        }
        let mode = SettingsStore::global(cx).settings.fit_mode;
        self.zoom = zoom_for_fit(mode, container, image);
        self.fit = self.fit.measured();
    }

    pub(super) fn zoom_by(
        &mut self,
        factor: f32,
        anchor: Option<Offset>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mode = self.view_mode(cx);
        let current = if mode == ViewMode::Webtoon {
            self.wt_zoom
        } else {
            self.zoom
        };
        let target = self
            .zoom_animation
            .as_ref()
            .filter(|animation| {
                animation.mode == mode
                    && animation.index == self.index
                    && animation.list_gen == self.list_gen
            })
            .map_or(current, |animation| animation.target);
        let (min, max) = match mode {
            ViewMode::Webtoon => (RELATIVE_ZOOM_MIN, RELATIVE_ZOOM_MAX),
            ViewMode::LeftToRight | ViewMode::RightToLeft => (1.0, RELATIVE_ZOOM_MAX),
            ViewMode::Single => {
                let Some(image) = self.image_size() else {
                    return;
                };
                let container = self.container();
                if container.is_empty() {
                    return;
                }
                let max = if self
                    .picture
                    .as_ref()
                    .is_some_and(|picture| picture.is_vector)
                {
                    MAX_ZOOM_VECTOR
                } else {
                    MAX_ZOOM
                };
                (geometry::min_zoom(container, image).min(target), max)
            }
        };
        let next = (target * factor).clamp(min, max);
        if next == target {
            return;
        }
        if mode == ViewMode::Single {
            self.fit = Fit::Manual;
        }
        let anchor = anchor.unwrap_or_default();
        if platform::reduced_motion() {
            self.zoom_animation = None;
            self.apply_zoom(next, anchor, cx);
            return;
        }
        let running = self.zoom_animation.is_some();
        self.zoom_animation = Some(ZoomAnimation {
            target: next,
            anchor,
            mode,
            index: self.index,
            list_gen: self.list_gen,
            last_frame: self
                .zoom_animation
                .as_ref()
                .map_or_else(Instant::now, |animation| animation.last_frame),
        });
        if !running {
            self.zoom_generation = self.zoom_generation.wrapping_add(1);
            self.zoom_next_frame(window, cx);
        }
    }

    fn zoom_next_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        let generation = self.zoom_generation;
        window.on_next_frame(move |window, cx| {
            view.update(cx, |this, cx| {
                if this.zoom_generation == generation {
                    this.zoom_frame(window, cx);
                }
            });
        });
    }

    fn zoom_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(animation) = self.zoom_animation.take() else {
            return;
        };
        if animation.mode != self.view_mode(cx)
            || animation.index != self.index
            || animation.list_gen != self.list_gen
        {
            return;
        }
        let current = if animation.mode == ViewMode::Webtoon {
            self.wt_zoom
        } else {
            self.zoom
        };
        let now = Instant::now();
        let next = if platform::reduced_motion() {
            animation.target
        } else {
            zoom_step(
                current,
                animation.target,
                now.duration_since(animation.last_frame),
            )
        };
        self.apply_zoom(next, animation.anchor, cx);
        if next != animation.target {
            self.zoom_animation = Some(ZoomAnimation {
                last_frame: now,
                ..animation
            });
            self.zoom_next_frame(window, cx);
        }
    }

    fn apply_zoom(&mut self, next: f32, anchor: Offset, cx: &mut Context<Self>) {
        match self.view_mode(cx) {
            ViewMode::Webtoon => {
                if next != self.wt_zoom {
                    // 페이지 높이가 같은 비율로 바뀌므로 읽던 위치도 함께 옮긴다.
                    self.wt_offset *= next / self.wt_zoom;
                    self.wt_zoom = next;
                    cx.notify();
                }
            }
            ViewMode::LeftToRight | ViewMode::RightToLeft => {
                if next != self.zoom {
                    let spread = self.spread_size(cx);
                    self.position = zoom_about(
                        self.container(),
                        spread,
                        self.zoom,
                        next,
                        self.position,
                        anchor,
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
                if container.is_empty() {
                    return;
                }
                if next == self.zoom {
                    return;
                }
                self.position =
                    zoom_about(container, image, self.zoom, next, self.position, anchor);
                self.zoom = next;
                self.fit = Fit::Manual;
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
            ViewMode::Single if self.fit.follows_window() => self.apply_fit(cx),
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

    /// 한 장 보기에서 그림이 놓인 사각형(읽기 영역 왼쪽 위 기준).
    /// 읽기 영역을 아직 재기 전에는 `None`으로 그리지 않는다. 빈 컨테이너
    /// 기준 사각형은 기본 배율(100%)의 실제 크기로 그려지기 때문이다.
    fn single_dest(&self) -> Option<Bounds<Pixels>> {
        let picture = self.picture.as_ref()?;
        let container = self.container();
        if container.is_empty() {
            return None;
        }
        let (dw, dh) = (
            picture.width as f32 * self.zoom,
            picture.height as f32 * self.zoom,
        );
        Some(snap_to_device_pixels(
            rect(
                container.w / 2.0 + self.position.x - dw / 2.0,
                container.h / 2.0 + self.position.y - dh / 2.0,
                dw,
                dh,
            ),
            self.device_scale,
        ))
    }

    /// 글자 강조의 줄과 글자 사각형(읽기 영역 기준). 강조가 안 보이는 상태면 `None`.
    fn ocr_boxes(&self, cx: &App) -> Option<OcrBoxes> {
        if self.view_mode(cx) != ViewMode::Single {
            return None;
        }
        let highlight = self.ocr_current()?;
        let dest = self.single_dest()?;
        let (x, y) = (f32::from(dest.origin.x), f32::from(dest.origin.y));
        let (w, h) = (f32::from(dest.size.width), f32::from(dest.size.height));
        let place = |r: OcrRect| rect(x + r.x * w, y + r.y * h, r.width * w, r.height * h);
        Some((
            highlight
                .line_rects(self.orientation)
                .into_iter()
                .map(place)
                .collect(),
            highlight
                .glyph_rects(self.orientation)
                .into_iter()
                .map(place)
                .collect(),
        ))
    }

    /// 창 좌표 `position`에서 가장 가까운 글자 번호. 글자 사이나 바깥을 끌어도 가까운 글자를
    /// 잡아 웹의 글자 선택처럼 끝이 끊기지 않게 한다.
    pub(super) fn ocr_glyph_at(&self, position: Point<Pixels>, cx: &App) -> Option<usize> {
        let (_, glyphs) = self.ocr_boxes(cx)?;
        let origin = self.viewport.get().origin;
        let (x, y) = (
            f32::from(position.x - origin.x),
            f32::from(position.y - origin.y),
        );
        glyphs
            .iter()
            .enumerate()
            .map(|(index, b)| {
                let (left, top) = (f32::from(b.origin.x), f32::from(b.origin.y));
                let (right, bottom) = (
                    left + f32::from(b.size.width),
                    top + f32::from(b.size.height),
                );
                let dx = (left - x).max(x - right).max(0.0);
                let dy = (top - y).max(y - bottom).max(0.0);
                (index, dx * dx + dy * dy)
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(index, _)| index)
    }

    /// `position`이 글자 자리(글자나 줄 사각형) 안이면 가장 가까운 글자 번호. 글자 사이의
    /// 틈을 눌러도 선택이 시작되게 줄 사각형까지 본다.
    fn ocr_hit(&self, position: Point<Pixels>, cx: &App) -> Option<usize> {
        let (lines, glyphs) = self.ocr_boxes(cx)?;
        let origin = self.viewport.get().origin;
        let local = point(position.x - origin.x, position.y - origin.y);
        lines
            .iter()
            .chain(glyphs.iter())
            .any(|b| b.contains(&local))
            .then(|| self.ocr_glyph_at(position, cx))
            .flatten()
    }

    /// 글자 강조와 선택 표시. 줄마다 글자 커서를 가진 바탕을 깔고, 고른 글자는 같은 줄끼리
    /// 이어 한 칸으로 칠한다.
    fn render_ocr_layer(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some((lines, glyphs)) = self.ocr_boxes(cx) else {
            return Vec::new();
        };
        let line_fill = cx.theme().warning.opacity(0.4);
        // 테마 색은 어두운 테마에서 거의 흰색이라 글자를 가린다. 웹의 선택색처럼 푸른색으로 고정한다.
        let picked_fill = hsla(212.0 / 360.0, 0.9, 0.55, 0.45);
        let picked = self.ocr.selection().map(|(a, b)| (a.min(b), a.max(b)));
        let place = |b: Bounds<Pixels>| {
            div()
                .absolute()
                .left(b.origin.x)
                .top(b.origin.y)
                .w(b.size.width)
                .h(b.size.height)
        };
        let mut layer = Vec::with_capacity(lines.len() + 4);
        for b in lines {
            layer.push(place(b).bg(line_fill).cursor_text().into_any_element());
        }
        if let (Some((low, high)), Some(highlight)) = (picked, self.ocr_current()) {
            let mut run: Option<(usize, Bounds<Pixels>)> = None;
            for (index, bounds) in glyphs
                .iter()
                .copied()
                .enumerate()
                .skip(low)
                .take(high + 1 - low)
            {
                let line = highlight.glyph_line(index);
                let Some(line) = line else { continue };
                run = match run {
                    Some((current, joined)) if current == line => {
                        Some((line, joined.union(&bounds)))
                    }
                    Some((_, joined)) => {
                        layer.push(place(joined).bg(picked_fill).into_any_element());
                        Some((line, bounds))
                    }
                    None => Some((line, bounds)),
                };
            }
            if let Some((_, joined)) = run {
                layer.push(place(joined).bg(picked_fill).into_any_element());
            }
        }
        layer
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
            WheelAction::ZoomIn => self.zoom_by(ZOOM_STEP, Some(anchor), window, cx),
            WheelAction::ZoomOut => self.zoom_by(1.0 / ZOOM_STEP, Some(anchor), window, cx),
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
            MouseAction::ZoomIn => self.zoom_by(ZOOM_STEP, None, window, cx),
            MouseAction::ZoomOut => self.zoom_by(1.0 / ZOOM_STEP, None, window, cx),
            MouseAction::ToggleFullscreen => window.toggle_fullscreen(),
            // 메뉴는 읽기 영역 요소가 띄운다. 여기서는 표지 지정 대상만 기억한다.
            MouseAction::ContextMenu => self.context_page = self.page_at(position, cx),
            MouseAction::Pan | MouseAction::None => {}
        }
    }

    /// 휠 이벤트를 설정된 동작으로 바꾼다. 수식키 없는 휠만 `plain`이다.
    pub(super) fn wheel_action(event: &ScrollEvent, up: bool, cx: &App) -> (WheelAction, bool) {
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

    fn on_wheel(&mut self, event: &ScrollEvent, window: &mut Window, cx: &mut Context<Self>) {
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
        self.bump_overlay();
        self.focus.focus(window, cx);
        // 글자 위를 누르면 선택을 시작하고, 이미지 이동이나 더블클릭 동작은 하지 않는다.
        if event.button == MouseButton::Left
            && let Some(glyph) = self.ocr_hit(event.position, cx)
        {
            self.ocr_press(glyph, event.click_count, event.position, window, cx);
            return;
        }
        // 글자가 아닌 곳을 누르면 고른 글자를 푼다.
        self.clear_ocr_selection(cx);
        let mouse = SettingsStore::global(cx).settings.mouse;
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
        self.bump_overlay();
        if self.ocr.dragging() {
            if event.pressed_button == Some(MouseButton::Left) {
                if let Some(glyph) = self.ocr_glyph_at(event.position, cx) {
                    self.ocr_extend(glyph, event.position, cx);
                }
                return;
            }
            self.ocr.end_drag();
        }
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
        if !self.anim.is_playing() || !picture.is_animated() {
            return;
        }
        if self
            .anim
            .advance(&picture.delays_ms, picture.loop_count, Instant::now())
        {
            window.request_animation_frame();
        }
    }

    pub(super) fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        if !self.has_animation(cx) {
            return;
        }
        self.anim.toggle(Instant::now());
        cx.notify();
    }

    pub(super) fn step_frame(&mut self, delta: isize, cx: &mut Context<Self>) {
        if !self.has_animation(cx) {
            return;
        }
        let Some(picture) = &self.picture else {
            return;
        };
        let count = picture.frame_count();
        self.anim.step(delta, count);
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
                    .get(*index)
                    .map(|page| (*index, page.picture().clone()))
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
                // 가운데 이음매가 반쪽 픽셀에 걸리면 두 페이지가 서로를 흐리므로
                // 사각형을 기기 픽셀에 맞춘다(§18).
                (
                    index,
                    picture,
                    snap_to_device_pixels(rect(x, center_y - dh / 2.0, dw, dh), self.device_scale),
                )
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
            .get(index)
            .map(|page| (page.picture().width, page.picture().height))
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
        if self.wt_glide.push(dy, Instant::now()) {
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
        let step = self.wt_glide.step(Instant::now());
        let before = (self.wt_anchor, self.wt_offset);
        self.webtoon_scroll(step, window, cx);
        // 끝에 닿아 더 움직이지 못하면 남은 거리를 버린다.
        let moved = (self.wt_anchor, self.wt_offset) != before;
        if self.wt_glide.end_frame(moved) {
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

        // 위아래 2화면 안은 로드하고, 4화면 밖은 내려놓는다. 스크롤로 곧바로
        // 보게 될 페이지도 지금 화면에 속한다: 설정과 관계없이 내려받는다(SPEC §9.6).
        let near: Vec<usize> = self
            .webtoon_span(viewport_h * 2.0, viewport_h * 2.0)
            .into_iter()
            .map(|(index, _)| index)
            .collect();
        for index in &near {
            self.ensure_page(*index, true, false, window, cx);
        }
        let keep: HashSet<usize> = self
            .webtoon_span(viewport_h * 4.0, viewport_h * 4.0)
            .into_iter()
            .map(|(index, _)| index)
            .collect();
        // 화면에 걸친 페이지는 예산이 넘어도 유지하고, 넘는 만큼 먼 페이지부터 내려놓는다.
        let protect: HashSet<usize> = self.shown.iter().copied().collect();
        self.evict_except(&keep, &protect, window, cx);

        let middle = px(viewport_h / 2.0);
        let center = self
            .webtoon_rects()
            .into_iter()
            .find(|(_, bounds)| bounds.bottom() >= middle)
            .map(|(index, _)| index)
            .unwrap_or(self.wt_anchor);
        let changed = center != self.index || self.nav.is_pending();
        self.index = center;
        self.shown = vec![center];
        let Some(page) = self.pages.get(center) else {
            return;
        };
        if !page.is_full() {
            return;
        }
        let stale = self
            .picture
            .as_ref()
            .is_none_or(|picture| !Arc::ptr_eq(picture, page.picture()));
        if changed || stale {
            self.nav.commit();
            self.error = None;
            self.picture = Some(page.picture().clone());
            self.info = page.info().cloned();
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
        if !self.current_page_is_full() || self.view_mode(cx) != ViewMode::Single || self.zoom < 1.0
        {
            return false;
        }
        if self
            .picture
            .as_ref()
            .is_none_or(|picture| picture.is_vector)
        {
            return false;
        }
        match SettingsStore::global(cx).settings.image_scaling_mode {
            ImageScalingMode::Smooth => false,
            ImageScalingMode::Pixelated => true,
            ImageScalingMode::Auto => self.zoom >= AUTO_PIXELATED_ZOOM,
        }
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
        if !self.current_page_is_full() {
            return None;
        }
        let picture = self.picture.clone()?;
        let levels = picture.levels();
        let level = &levels[0];
        let container = self.container();
        let visible = dest.intersect(&rect(0.0, 0.0, container.w, container.h));
        let width = (f32::from(visible.size.width) * scale_factor).round() as u32;
        let height = (f32::from(visible.size.height) * scale_factor).round() as u32;
        if width == 0 || height == 0 {
            return None;
        }
        let frame = self
            .anim
            .frame()
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
        // 원본은 타일로 나뉘어 있을 수 있어, 출력 픽셀마다 타일을 실제 좌표로 찾아
        // 읽는다. 정지 이미지는 8192px 격자, 애니메이션은 큰 타일 한 장이다.
        let (src_w, src_h) = (level.width, level.height);
        if src_w == 0 || src_h == 0 || picture.width == 0 || picture.height == 0 {
            return None;
        }
        let origins: Vec<(u32, u32)> = level.tiles.iter().map(|tile| (tile.x, tile.y)).collect();
        let grid = TileGrid::new(&origins);
        let sources: Vec<(&[u8], usize)> = level
            .tiles
            .iter()
            .map(|tile| Some((tile.image.as_bytes(frame)?, tile.w as usize)))
            .collect::<Option<_>>()?;
        // 배율과 위치는 원본(`Picture`) 크기 기준이다. 아틀라스 한계로 줄여 둔
        // 애니메이션은 레벨이 더 작으므로 그 비율로 좌표를 옮긴다.
        let ratio_x = src_w as f32 / picture.width as f32;
        let ratio_y = src_h as f32 / picture.height as f32;
        let step_x = ratio_x / (self.zoom * scale_factor);
        let step_y = ratio_y / (self.zoom * scale_factor);
        let start_x = f32::from(visible.origin.x - dest.origin.x) / self.zoom * ratio_x;
        let start_y = f32::from(visible.origin.y - dest.origin.y) / self.zoom * ratio_y;
        // 열마다 (타일 열, 타일 안 바이트 오프셋)을 미리 구한다.
        let columns: Vec<(usize, usize)> = (0..width)
            .map(|x| {
                let src_x = ((start_x + (x as f32 + 0.5) * step_x) as u32).min(src_w - 1);
                let (column, local_x) = grid.column(src_x);
                (column, local_x as usize * 4)
            })
            .collect();
        let row_bytes = width as usize * 4;
        let mut buffer = vec![0u8; row_bytes * height as usize];
        let mut previous_row = None;
        for y in 0..height as usize {
            let src_y = ((start_y + (y as f32 + 0.5) * step_y) as u32).min(src_h - 1);
            // 확대하면 여러 출력 행이 같은 원본 행을 가리킨다. 이미 만든 행을
            // 복사해 픽셀 단위 타일 조회를 반복하지 않는다.
            if previous_row == Some(src_y) {
                buffer.copy_within((y - 1) * row_bytes..y * row_bytes, y * row_bytes);
                continue;
            }
            previous_row = Some(src_y);
            let (tile_row, local_y) = grid.row(src_y);
            let row = &mut buffer[y * row_bytes..(y + 1) * row_bytes];
            for (pixel, (tile_column, offset)) in
                row.as_chunks_mut::<4>().0.iter_mut().zip(&columns)
            {
                let Some(tile) = grid.tile(*tile_column, tile_row) else {
                    continue;
                };
                let (bytes, tile_w) = sources[tile];
                let at = local_y as usize * tile_w * 4 + offset;
                if let Some(source) = bytes.get(at..at + 4) {
                    pixel.copy_from_slice(source);
                }
            }
        }
        // 원본이 이미 BGRA라 채널을 바꾸지 않고 그대로 옮긴다.
        let frames: SmallVec<[Frame; 1]> =
            smallvec![Frame::new(RgbaImage::from_raw(width, height, buffer)?)];
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

    /// 조작이 있었음을 기록해 숨겨진 재생 컨트롤을 다시 보이게 한다.
    pub(super) fn bump_overlay(&mut self) {
        self.overlay_active = Instant::now();
    }

    /// 애니메이션 프레임 슬라이더를 현재 그림에 맞춰 만들고 위치를 동기화한다.
    fn sync_anim_timeline(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let frames = if self.has_animation(cx) {
            self.picture
                .as_ref()
                .map_or(0, |picture| picture.frame_count())
        } else {
            0
        };
        if frames < 2 {
            self.anim_timeline = None;
            return;
        }
        if self
            .anim_timeline
            .as_ref()
            .is_none_or(|timeline| timeline.frames != frames)
        {
            let slider = cx.new(|_| {
                SliderState::new()
                    .min(0.0)
                    .max((frames - 1) as f32)
                    .step(1.0)
            });
            let sub = cx.subscribe_in(
                &slider,
                window,
                move |this, _, event: &SliderEvent, _, cx| {
                    // 재생 중이면 끌어 놓은 프레임부터 지연 시간을 다시 센다.
                    this.anim
                        .jump_to((slider_ms(event) as usize).min(frames - 1), Instant::now());
                    this.bump_overlay();
                    cx.notify();
                },
            );
            self.anim_timeline = Some(AnimTimeline {
                frames,
                slider,
                _sub: sub,
            });
            self.bump_overlay();
        }
        if let Some(timeline) = &self.anim_timeline {
            let value = SliderValue::Single(self.anim.frame() as f32);
            // 값이 같으면 갱신하지 않는다. `set_value`가 항상 notify하기 때문이다.
            if timeline.slider.read(cx).value() != value {
                timeline.slider.update(cx, |state, cx| {
                    state.set_value(value, window, cx);
                });
            }
        }
    }

    /// 애니메이션 재생 제어를 뷰포트 우하단에 떠 있는 반투명 패널로 그린다.
    pub(super) fn render_animation_overlay(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let frames = self
            .picture
            .as_ref()
            .map(|picture| picture.frame_count())
            .unwrap_or(1);
        let counter_width = FRAME_COUNTER_WIDTH;
        h_flex()
            .w_full()
            .gap_2()
            .px_2()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover.opacity(0.75))
            .text_xs()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_this, _event, _window, cx| {
                    cx.stop_propagation();
                }),
            )
            .debug_selector(|| "dock-gif-controls".to_owned())
            .child(
                Button::new("gif-prev")
                    .ghost()
                    .xsmall()
                    .icon(Ph::CaretLeft)
                    .tooltip(t("menu.gifPrevFrame"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.run_action("gifPrevFrame", window, cx)
                    })),
            )
            .child(
                Button::new("gif-play")
                    .ghost()
                    .xsmall()
                    .icon(if self.anim.is_playing() {
                        Ph::Pause
                    } else {
                        Ph::Play
                    })
                    .tooltip(t(if self.anim.is_playing() {
                        "menu.gifPause"
                    } else {
                        "menu.gifPlay"
                    }))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.run_action("toggleGifPlayback", window, cx)
                    })),
            )
            .child(
                Button::new("gif-next")
                    .ghost()
                    .xsmall()
                    .icon(Ph::CaretRight)
                    .tooltip(t("menu.gifNextFrame"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.run_action("gifNextFrame", window, cx)
                    })),
            )
            .child(match &self.anim_timeline {
                Some(timeline) => div()
                    .flex_1()
                    .min_w(px(120.))
                    .child(Slider::new(&timeline.slider).horizontal())
                    .into_any_element(),
                None => div().flex_1().into_any_element(),
            })
            .child(
                div()
                    .w(px(counter_width))
                    .text_right()
                    .whitespace_nowrap()
                    .font_features(tabular_figures())
                    .child(format_frame_counter(
                        (self.anim.frame() + 1) as u64,
                        frames as u64,
                    )),
            )
    }

    /// 읽기 영역 좌우 가장자리의 이전/다음 버튼. 우→좌 양쪽 보기는 읽는 방향대로
    /// 왼쪽이 다음이다.
    fn nav_arrow(&self, left: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let mode = self.view_mode(cx);
        let forward = left == (mode == ViewMode::RightToLeft);
        let looped = SettingsStore::global(cx).settings.loop_navigation;
        // 누르면 실제로 움직이는지로 판정한다. 양쪽 보기의 마지막 화면은 시작 인덱스가
        // 끝이 아니어도 다음 화면이 없다(SPEC §3.2).
        let total = self.list.len();
        let enabled = if mode.is_dual() {
            layout::dual_step(self.index, total, looped, forward, &self.solo(cx)).is_some()
        } else {
            layout::step_index(self.index, total, 1, looped, forward).is_some()
        };
        let (id, icon, label) = match (left, forward) {
            (true, false) => ("nav-left", Ph::CaretLeft, "viewer.nav.prev"),
            (true, true) => ("nav-left", Ph::CaretLeft, "viewer.nav.next"),
            (false, false) => ("nav-right", Ph::CaretRight, "viewer.nav.prev"),
            (false, true) => ("nav-right", Ph::CaretRight, "viewer.nav.next"),
        };
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .when(left, |side| side.left_2())
            .when(!left, |side| side.right_2())
            .flex()
            .items_center()
            // 버튼 위의 클릭이 뷰어의 드래그 이동으로 이어지지 않게 한다.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Button::new(id)
                    .outline()
                    .rounded_full()
                    .icon(icon)
                    .disabled(!enabled)
                    .tooltip(t(label))
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.navigate(forward, window, cx)),
                    ),
            )
    }

    pub(super) fn render_viewport(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        self.sync_anim_timeline(window, cx);
        // 그림 사각형을 기기 픽셀에 맞추는 데 쓴다(§18).
        self.device_scale = window.scale_factor();
        let idle = self.overlay_active.elapsed();
        // 매 프레임 `Settings` 전체를 클론하지 않고 렌더에 쓰는 `Copy` 필드만 복사한다.
        let (background, webtoon_page_boundaries, webtoon_show_progress, webtoon_thumbnail_jump) = {
            let settings = &SettingsStore::global(cx).settings;
            (
                settings.viewer_background,
                settings.webtoon_page_boundaries,
                settings.webtoon_show_progress,
                settings.webtoon_thumbnail_jump,
            )
        };
        let mode = self.view_mode(cx);
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
        let mut items: Vec<PaintItem> = Vec::new();
        let mut placeholders: Vec<Bounds<Pixels>> = Vec::new();
        let mut boundaries: Vec<Pixels> = Vec::new();
        let mut pixel: Option<(Arc<RenderImage>, Bounds<Pixels>)> = None;
        match mode {
            ViewMode::Single => {
                if let (Some(picture), Some(dest)) = (self.picture.clone(), self.single_dest()) {
                    if self.wants_pixelated(cx) {
                        pixel = self.pixel_view(dest, scale_factor, window, cx);
                    }
                    if pixel.is_none() {
                        self.drop_pixel_view(window, cx);
                    }
                    items.push(PaintItem {
                        picture,
                        rect: dest,
                        frame: self.anim.frame(),
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
                    if webtoon_page_boundaries && index > 0 {
                        boundaries.push(bounds.top());
                    }
                    match self.pages.get(index) {
                        Some(page) => items.push(PaintItem {
                            picture: page.picture().clone(),
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
                    let levels = item.picture.levels();
                    let level = choose_level(&levels, item.picture.width, device_scale);
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

        let waiting = self.picture.is_none() && (self.opening.is_some() || self.nav.is_pending());
        // 이미 그림이 떠 있을 때는 새 페이지나 파일이 늦어질 때만 모서리에 작게 알린다.
        let busy = !waiting
            && (self.opening_visible()
                || self.nav.held_longer_than(super::pages::PREVIOUS_HOLD)
                || (mode == ViewMode::Webtoon && self.pages.any_loading()));
        // 디코드가 길어지면 이미 받아 둔 썸네일을 저해상 미리보기로 먼저 깐다.
        let preview_thumb = (mode == ViewMode::Single
            && self.nav.held_longer_than(super::pages::PREVIOUS_HOLD)
            && self.pages.get(self.index).is_none_or(|page| page.is_full()))
        .then(|| self.list.get(self.index))
        .flatten()
        .and_then(|item| match self.thumbs.get(item) {
            Some(super::ThumbState::Ready(path)) => Some(path.clone()),
            _ => None,
        });
        let preview = (mode != ViewMode::Webtoon)
            .then(|| self.pages.get(self.index))
            .flatten()
            .filter(|page| page.is_archive_preview() && self.shown.contains(&self.index))
            .and_then(|_| self.list.get(self.index))
            .map(|path| {
                std::path::Path::new(&**path)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.to_string())
            });
        let progress = (mode == ViewMode::Webtoon
            && (webtoon_show_progress || webtoon_thumbnail_jump))
            .then(|| {
                let total = self.list.len().max(1);
                let percent = ((self.index + 1) * 100 / total).min(100);
                (
                    webtoon_show_progress.then(|| {
                        t_with(
                            "viewer.webtoon.position",
                            &[
                                ("current", &(self.index + 1)),
                                ("total", &total),
                                ("percent", &percent),
                            ],
                        )
                    }),
                    webtoon_thumbnail_jump,
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
                cx.listener(|this, _, _, _| {
                    this.drag = None;
                    this.ocr.end_drag();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.drag = None;
                    this.ocr.end_drag();
                }),
            )
            .on_scroll(cx.listener(Self::on_wheel))
            .child(drawing)
            .children(self.render_ocr_layer(cx))
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
                                        .icon(Ph::BookOpen)
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
                                    .icon(Ph::SquaresFour)
                                    .tooltip(t("viewer.webtoon.openThumbnails"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.toggle_grid(window, cx)
                                    })),
                            )
                        }),
                )
            })
            .when(self.list.len() > 1, |area| {
                area.child(self.nav_arrow(true, cx))
                    .child(self.nav_arrow(false, cx))
            })
            .when(
                self.has_animation(cx) && overlay_visible(self.anim.is_playing(), idle),
                |area| {
                    area.child(
                        div()
                            .absolute()
                            .bottom_3()
                            .left_3()
                            .right_3()
                            .child(self.render_animation_overlay(cx)),
                    )
                },
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AnimPlayback, Fit, OVERLAY_HIDE_AFTER, TileGrid, format_frame_counter, overlay_visible,
        rect, snap_to_device_pixels, zoom_step,
    };
    use std::time::{Duration, Instant};

    /// 1:1에 가까운 배율에서 반쪽 픽셀에 걸린 사각형이 기기 픽셀 격자로 맞는지.
    #[test]
    fn page_rects_snap_to_device_pixel_grid() {
        // 배율 2의 1500px 페이지는 화면에서 반쪽(0.5 논리 px)에 걸릴 수 있다.
        let loose = rect(1151.5, 0.5, 1537.0, 2049.0);
        let snapped = snap_to_device_pixels(loose, 2.0);
        for value in [
            f32::from(snapped.origin.x),
            f32::from(snapped.origin.y),
            f32::from(snapped.origin.x + snapped.size.width),
            f32::from(snapped.origin.y + snapped.size.height),
        ] {
            // 논리 px가 배율로 나누어떨어져야(기기 픽셀 정수) 블렌딩이 없다.
            assert!(
                (value * 2.0).fract() == 0.0,
                "기기 픽셀 격자에 벗어남: {value}"
            );
        }
        // 원래 사각형은 임의 배율을 유지한다(scale 1).
        assert_eq!(snap_to_device_pixels(loose, 1.0), loose);
        // 배율이 0 이하이면 원본을 그대로 둔다.
        assert_eq!(snap_to_device_pixels(loose, 0.0), loose);
        assert_eq!(snap_to_device_pixels(loose, f32::NAN), loose);
    }

    #[test]
    fn snapping_moves_by_less_than_one_device_pixel() {
        let r = rect(10.4, 10.4, 100.2, 200.2);
        let snapped = snap_to_device_pixels(r, 1.0);
        assert!((f32::from(snapped.origin.x) - 10.0).abs() <= 0.5);
        assert!((f32::from(snapped.size.width) - 100.0).abs() <= 1.0);
    }

    #[test]
    fn zoom_eases_toward_target_and_settles() {
        for (start, target) in [(1.0, 2.0), (2.0, 1.0)] {
            let mut value = start;
            for _ in 0..100 {
                let next = zoom_step(value, target, Duration::from_millis(16));
                assert!((next - target).abs() <= (value - target).abs());
                value = next;
            }
            assert_eq!(value, target);
        }
    }

    #[test]
    fn anim_playback_starts_paused_and_toggles() {
        let t0 = Instant::now();
        let mut anim = AnimPlayback::default();
        assert!(!anim.is_playing());
        assert_eq!((anim.frame(), anim.loops_done()), (0, 0));
        // 정지 중에는 시간이 지나도 넘기지 않는다.
        assert!(!anim.advance(&[100, 100], None, t0 + Duration::from_secs(5)));
        assert_eq!(anim.frame(), 0);

        anim.toggle(t0);
        assert!(anim.is_playing());
        assert!(anim.advance(&[100, 100], None, t0 + Duration::from_millis(150)));
        assert_eq!(anim.frame(), 1);
        anim.toggle(t0);
        assert!(!anim.is_playing());
        assert_eq!(anim.frame(), 1, "정지는 현재 프레임에 머문다");
    }

    #[test]
    fn anim_playback_carries_leftover_time_and_loops_forever() {
        let t0 = Instant::now();
        let mut anim = AnimPlayback::default();
        anim.restart(true, t0);
        // 100ms 프레임 3개, 무한 반복. 1050ms면 10프레임을 지나 1번 프레임이다.
        assert!(anim.advance(&[100, 100, 100], None, t0 + Duration::from_millis(1_050)));
        assert_eq!(anim.frame(), 1);
        assert_eq!(anim.loops_done(), 3);
        // 남은 50ms는 이어서 센다. 50ms 더 지나면 다음 프레임이다.
        assert!(anim.advance(&[100, 100, 100], None, t0 + Duration::from_millis(1_100)));
        assert_eq!(anim.frame(), 2);
    }

    #[test]
    fn anim_playback_stops_on_the_last_frame_of_finite_loops() {
        let t0 = Instant::now();
        let mut anim = AnimPlayback::default();
        anim.restart(true, t0);
        // 2회 반복: 마지막 회차의 마지막 프레임에서 멈추고 더는 예약하지 않는다.
        assert!(!anim.advance(&[100, 100], Some(2), t0 + Duration::from_secs(10)));
        assert!(!anim.is_playing());
        assert_eq!(anim.frame(), 1);
        assert_eq!(anim.loops_done(), 2);
        // 끝난 뒤 다시 재생하면 반복 횟수를 새로 센다.
        anim.toggle(t0);
        assert!(anim.is_playing());
        assert_eq!(anim.loops_done(), 0);
    }

    #[test]
    fn anim_playback_step_jump_and_reset() {
        let t0 = Instant::now();
        let mut anim = AnimPlayback::default();
        anim.restart(true, t0);
        // 스텝은 정지시키고 앞뒤 끝에서 감긴다.
        anim.step(-1, 4);
        assert!(!anim.is_playing());
        assert_eq!(anim.frame(), 3);
        anim.step(2, 4);
        assert_eq!(anim.frame(), 1);
        // 정지 중 이동은 재생을 시작하지 않는다.
        anim.jump_to(2, t0);
        assert!(!anim.is_playing());
        assert_eq!(anim.frame(), 2);
        // 재생 중 이동은 그 프레임부터 지연 시간을 다시 센다.
        anim.toggle(t0);
        let t1 = t0 + Duration::from_millis(500);
        anim.jump_to(0, t1);
        assert!(anim.advance(&[100, 100, 100], None, t1 + Duration::from_millis(50)));
        assert_eq!(anim.frame(), 0);
        anim.rewind(t1);
        assert!(anim.is_playing());
        assert_eq!(anim.frame(), 0);
        // 페이지를 닫으면 정지하고 첫 프레임으로 가지만 반복 횟수는 남는다.
        let _ = anim.advance(&[100, 100, 100], None, t1 + Duration::from_millis(350));
        assert_eq!(anim.loops_done(), 1);
        anim.stop_at_first_frame();
        assert!(!anim.is_playing());
        assert_eq!((anim.frame(), anim.loops_done()), (0, 1));
        // 새 그림은 반복 횟수까지 처음부터, 자동 재생이 꺼져 있으면 정지로 시작한다.
        anim.restart(false, t1);
        assert!(!anim.is_playing());
        assert_eq!((anim.frame(), anim.loops_done()), (0, 0));
    }

    #[test]
    fn fit_waits_for_the_viewport_then_locks_until_manual_zoom() {
        // 처음에는 크기를 재기 전이고, 맞춤을 따른다.
        let fit = Fit::Pending;
        assert!(fit.follows_window());
        assert_eq!(fit.lock(), Fit::Pending);
        assert_eq!(fit.unmeasured(), Fit::Pending);
        let fit = fit.measured();
        assert_eq!(fit, Fit::Locked);
        // 읽기 영역을 다시 재지 못하면 크기 대기로 돌아간다.
        assert_eq!(fit.unmeasured(), Fit::Pending);
        // 수동 줌은 리사이즈·맞춤 계산에 풀리지 않고, 맞춤 고정으로만 돌아간다.
        let manual = Fit::Manual;
        assert!(!manual.follows_window());
        assert_eq!(manual.measured(), Fit::Manual);
        assert_eq!(manual.unmeasured(), Fit::Manual);
        assert_eq!(manual.lock(), Fit::Locked);
    }

    #[test]
    fn tile_grid_finds_a_single_wide_animation_tile() {
        // 애니메이션은 9000px 넘는 프레임도 타일 하나다. 8192px 격자로 가정하면
        // 없는 두 번째 타일을 찾아 패닉했다.
        let grid = TileGrid::new(&[(0, 0)]);
        assert_eq!(grid.column(8999), (0, 8999));
        assert_eq!(grid.row(99), (0, 99));
        assert_eq!(grid.tile(0, 0), Some(0));
    }

    #[test]
    fn tile_grid_maps_a_tiled_still_image() {
        // 정지 이미지 10000x9000은 8192px 격자 2x2 타일(행 우선)이다.
        let grid = TileGrid::new(&[(0, 0), (8192, 0), (0, 8192), (8192, 8192)]);
        assert_eq!(grid.column(8191), (0, 8191));
        assert_eq!(grid.column(8192), (1, 0));
        assert_eq!(grid.row(8500), (1, 308));
        assert_eq!(grid.tile(1, 1), Some(3));
        assert_eq!(grid.tile(1, 0), Some(1));
    }

    #[test]
    fn overlay_hides_only_while_playing_and_idle() {
        let long = OVERLAY_HIDE_AFTER + Duration::from_millis(1);
        assert!(overlay_visible(true, Duration::ZERO));
        assert!(overlay_visible(
            true,
            OVERLAY_HIDE_AFTER - Duration::from_millis(1)
        ));
        assert!(!overlay_visible(true, long));
        // 정지 중에는 조작이 없어도 계속 보인다.
        assert!(overlay_visible(false, long));
    }

    #[test]
    fn frame_counter_uses_a_fixed_field() {
        assert_eq!(format_frame_counter(1, 10), " 0001/ 0010");
        assert_eq!(format_frame_counter(999, 1000), " 0999/ 1000");
        assert_eq!(format_frame_counter(9999, 9999), " 9999/ 9999");
    }

    #[test]
    fn frame_counter_omits_leading_digits_past_four() {
        assert_eq!(format_frame_counter(10_000, 10_000), "…0000/…0000");
        assert_eq!(format_frame_counter(12_345, 12_345), "…2345/…2345");
        assert_eq!(format_frame_counter(12_345, 99_999), "…2345/…9999");
    }

    #[test]
    fn frame_counter_clamps_past_the_end() {
        assert_eq!(format_frame_counter(101, 100), " 0100/ 0100");
    }

    #[test]
    fn frame_counter_handles_an_empty_total() {
        assert_eq!(format_frame_counter(0, 0), " 0000/ 0000");
    }
}
