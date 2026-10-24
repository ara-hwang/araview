//! 이미지 목록 도크와 썸네일 요청(SPEC §3.2, §9.3).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use araview_core::file_availability::FileAvailability;
use araview_core::image::is_archive_file;
use araview_core::ops;
use araview_core::thumbnail::BatchThumb;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{
    ActiveTheme as _, Icon, Sizable as _, h_flex, h_virtual_list, v_flex, v_virtual_list,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{AraView, ThumbState};
use crate::assets::Ph;
use crate::i18n::{t, t_with};
use crate::settings::{DockPosition, SettingsStore, ViewMode};

/// 썸네일 생성 요청 한 변(px). 코어 캐시의 기본 크기와 같다.
const THUMB_MAX_SIDE: u32 = 256;
/// 한 번에 요청하는 썸네일 수. 끝난 청크부터 그린다.
const THUMB_CHUNK: usize = 8;
const DOCK_GAP: f32 = 6.0;
const DOCK_PADDING: f32 = 8.0;
const LABEL_HEIGHT: f32 = 16.0;
/// 라벨과 썸네일 사이 간격(`gap_0p5`).
const LABEL_GAP: f32 = 2.0;
/// 보이는 범위 앞뒤로 더 그리는 칸 수.
const OVERSCAN: usize = 3;
/// 현재 항목으로 따라갈 때 이 화면 수보다 멀면 애니메이션 없이 옮긴다.
const FOLLOW_ANIMATE_SCREENS: f32 = 3.0;

fn file_label(item: &str) -> String {
    item.rsplit(['/', '\\']).next().unwrap_or(item).to_owned()
}

impl AraView {
    /// 썸네일을 요청한다. 이미 받았거나 요청 중인 항목은 건너뛰고, 8개 청크로
    /// 나눠 보내 끝난 청크부터 그린다.
    pub(super) fn request_thumbs_for(
        &mut self,
        indices: impl IntoIterator<Item = usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let wanted: Vec<Arc<str>> = indices
            .into_iter()
            .filter_map(|index| self.list.get(index))
            .filter(|item| !self.thumbs.contains_key(*item) && !self.thumbs_pending.contains(*item))
            .cloned()
            .collect();
        for chunk in wanted.chunks(THUMB_CHUNK) {
            let chunk = chunk.to_vec();
            self.thumbs_pending.extend(chunk.iter().cloned());
            let archive = self.archive.clone();
            let generation = self.list_gen;
            let task = cx.background_spawn(async move {
                let names: Vec<String> = chunk.iter().map(|item| item.to_string()).collect();
                let thumbs: Vec<BatchThumb> = match &archive {
                    Some(archive) => ops::generate_archive_thumbnails_batch_impl(
                        Path::new(&**archive),
                        &names,
                        THUMB_MAX_SIDE,
                    )
                    .unwrap_or_default(),
                    None => {
                        let (archives, images): (Vec<String>, Vec<String>) = names
                            .into_iter()
                            .partition(|name| is_archive_file(Path::new(name)));
                        let mut out = araview_core::thumbnail::generate_thumbnails_batch(
                            &images,
                            THUMB_MAX_SIDE,
                        );
                        out.extend(ops::generate_archive_file_thumbnails_batch_impl(
                            &archives,
                            THUMB_MAX_SIDE,
                        ));
                        out
                    }
                };
                (chunk, thumbs)
            });
            cx.spawn_in(window, async move |this, cx| {
                let (chunk, thumbs) = task.await;
                this.update(cx, |this, cx| {
                    for item in &chunk {
                        this.thumbs_pending.remove(item);
                    }
                    // 응답이 오기 전에 다른 목록으로 바뀌었으면 버린다.
                    if this.list_gen != generation {
                        return;
                    }
                    let mut by_source: HashMap<String, Option<PathBuf>> = thumbs
                        .into_iter()
                        .map(|item| (item.source, item.thumb.map(|thumb| thumb.file_path.into())))
                        .collect();
                    for item in chunk {
                        let state = match by_source.remove(&*item).flatten() {
                            Some(path) => ThumbState::Ready(path),
                            None => ThumbState::Failed,
                        };
                        this.thumbs.insert(item, state);
                    }
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
    }

    /// 도크에 보이는 범위의 썸네일을 요청한다.
    pub(super) fn request_thumbs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.list.len() < 2 {
            return;
        }
        let range = self.dock_range(window, cx);
        self.request_thumbs_for(range, window, cx);
    }

    /// 실패한 썸네일을 다시 요청한다.
    pub(super) fn retry_thumb(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(item) = self.list.get(index).cloned() {
            self.thumbs.remove(&item);
            self.request_thumbs_for([index], window, cx);
        }
    }

    /// 스크롤 방향 길이(도크 안쪽)와 칸 하나가 차지하는 길이(간격 포함).
    fn dock_metrics(&self, window: &Window, cx: &App) -> (f32, f32) {
        let settings = &SettingsStore::global(cx).settings;
        let viewport = window.viewport_size();
        // 읽기 영역의 실제 크기가 있으면 그 값을 쓴다(정보 패널과 도크 위치 반영).
        // 첫 프레임처럼 아직 없으면 창 크기로 어림한다.
        let bounds = self.viewport.get();
        let length = if settings.dock_position.is_horizontal() {
            if f32::from(bounds.size.width) > 0.0 {
                f32::from(bounds.size.width)
            } else {
                let panel = if self.info_open {
                    super::info_panel::PANEL_WIDTH
                } else {
                    0.0
                };
                f32::from(viewport.width) - panel
            }
        } else if f32::from(bounds.size.height) > 0.0 {
            f32::from(bounds.size.height)
        } else {
            f32::from(viewport.height) - 120.0
        };
        (length, self.dock_cell_metrics(cx).0)
    }

    /// 칸의 축 방향 크기(간격 포함)와 교차축 내용 크기.
    fn dock_cell_metrics(&self, cx: &App) -> (f32, f32) {
        let settings = &SettingsStore::global(cx).settings;
        let side = settings.dock_thumb_size.side();
        let labels = usize::from(settings.dock_show_name) + usize::from(settings.dock_show_index);
        let labels_height = labels as f32 * (LABEL_HEIGHT + LABEL_GAP);
        if settings.dock_position.is_horizontal() {
            (side + DOCK_GAP, side + labels_height)
        } else {
            (side + labels_height + DOCK_GAP, side)
        }
    }

    /// 우→좌 양쪽 보기의 가로 도크는 읽는 방향을 따라 첫 장을 오른쪽에 둔다.
    fn dock_reversed(&self, cx: &App) -> bool {
        SettingsStore::global(cx)
            .settings
            .dock_position
            .is_horizontal()
            && self.view_mode(cx) == ViewMode::RightToLeft
    }

    /// 현재 스크롤 위치에서 보이는(앞뒤 여유 포함) 인덱스 범위.
    fn dock_range(&self, window: &Window, cx: &App) -> std::ops::Range<usize> {
        let total = self.list.len();
        let (length, cell) = self.dock_metrics(window, cx);
        let horizontal = SettingsStore::global(cx)
            .settings
            .dock_position
            .is_horizontal();
        let offset = self.dock_scroll.handle().offset();
        let scrolled = -f32::from(if horizontal { offset.x } else { offset.y });
        let first = ((scrolled - DOCK_PADDING).max(0.0) / cell).floor() as usize;
        let visible = (length / cell).ceil() as usize + 1;
        let slots = first.saturating_sub(OVERSCAN)..(first + visible + OVERSCAN).min(total);
        if slots.start >= slots.end {
            return 0..0;
        }
        if self.dock_reversed(cx) {
            (total - slots.end)..(total - slots.start)
        } else {
            slots
        }
    }

    /// 현재 항목이 바뀌면 도크를 그 항목이 가운데 오도록 스크롤한다.
    fn follow_current(&self, window: &mut Window, cx: &App) {
        let horizontal = SettingsStore::global(cx)
            .settings
            .dock_position
            .is_horizontal();
        let reversed = self.dock_reversed(cx);
        // 축이나 읽는 방향이 바뀌면 이전 오프셋은 의미가 없으므로 다시 가운데로 맞춘다.
        let key = (self.index, self.list.len(), horizontal, reversed);
        if self.dock_followed.get() == Some(key) {
            return;
        }
        let first = match self.dock_followed.replace(Some(key)) {
            None => true,
            Some((_, _, was_horizontal, was_reversed)) => {
                was_horizontal != horizontal || was_reversed != reversed
            }
        };
        let total = self.list.len();
        if total == 0 {
            return;
        }
        let (length, cell) = self.dock_metrics(window, cx);
        let slot = if reversed {
            total - 1 - self.index.min(total - 1)
        } else {
            self.index
        };
        let content = total as f32 * cell + DOCK_PADDING * 2.0;
        let max = (content - length).max(0.0);
        let target =
            (DOCK_PADDING + slot as f32 * cell + cell / 2.0 - length / 2.0).clamp(0.0, max);
        let offset = self.dock_scroll.handle().offset();
        let current = -f32::from(if horizontal { offset.x } else { offset.y });
        if first || (target - current).abs() > length * FOLLOW_ANIMATE_SCREENS {
            // 첫 배치나 먼 이동은 바로 옮긴다. 사이 칸마다 썸네일을 요청하지 않게 한다.
            let next = if horizontal {
                point(px(-target), offset.y)
            } else {
                point(offset.x, px(-target))
            };
            self.dock_scroll.handle().set_offset(next);
        } else {
            self.dock_scroll.animate_to(-target, horizontal, window);
        }
    }

    fn set_dock(
        &mut self,
        edit: impl FnOnce(&mut crate::settings::Settings),
        cx: &mut Context<Self>,
    ) {
        SettingsStore::update(cx, edit);
        cx.notify();
    }

    fn dock_cell(
        &self,
        index: usize,
        horizontal: bool,
        cell: f32,
        cross: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let settings = &SettingsStore::global(cx).settings;
        let side = settings.dock_thumb_size.side();
        let (show_name, show_index) = (settings.dock_show_name, settings.dock_show_index);
        let item = &self.list[index];
        let name = file_label(item);
        let thumb = match self.thumbs.get(item) {
            Some(ThumbState::Ready(path)) => Some(path.clone()),
            _ => None,
        };
        let failed = matches!(self.thumbs.get(item), Some(ThumbState::Failed));
        let cloud_only = matches!(
            self.availability.get(index),
            Some(FileAvailability::CloudOnly)
        );
        let current = self.shown.contains(&index) || index == self.index;
        let tooltip_key = if failed {
            "viewer.nav.thumbError"
        } else if cloud_only {
            "viewer.nav.cloudOnly"
        } else {
            "viewer.nav.thumb"
        };
        let tooltip = t_with(tooltip_key, &[("index", &(index + 1)), ("name", &name)]);
        let muted = cx.theme().muted_foreground;
        // VirtualList가 축 방향 크기를 정하고, 교차축은 측정값을 쓴다. 간격은
        // 아이템 크기에 포함되므로 내용은 그만큼 비워 둔다.
        let (width, height) = if horizontal {
            (cell, cross)
        } else {
            (cross, cell)
        };
        v_flex()
            .id(("thumb", index))
            .w(px(width))
            .h(px(height))
            .gap(px(LABEL_GAP))
            .when(horizontal, |cell| cell.items_start().justify_center())
            .when(!horizontal, |cell| cell.items_center())
            .cursor_pointer()
            .tooltip(move |window, cx| {
                gpui_kit::component::tooltip::Tooltip::new(tooltip.clone()).build(window, cx)
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                if failed {
                    this.retry_thumb(index, window, cx);
                }
                this.jump_to(index, window, cx)
            }))
            .child(
                div()
                    .relative()
                    .size(px(side))
                    .rounded_sm()
                    .overflow_hidden()
                    .border_2()
                    .border_color(if current {
                        cx.theme().primary
                    } else {
                        cx.theme().border
                    })
                    .bg(cx.theme().muted)
                    .flex()
                    .items_center()
                    .justify_center()
                    .map(|cell| match thumb {
                        Some(path) => {
                            cell.child(img(path).size_full().object_fit(ObjectFit::Cover))
                        }
                        None if failed => {
                            cell.child(Icon::new(Ph::WarningCircle).small().text_color(muted))
                        }
                        None => cell,
                    })
                    .when(cloud_only, |cell| {
                        cell.child(
                            div()
                                .absolute()
                                .top_0p5()
                                .right_0p5()
                                .child(Icon::new(Ph::Cloud).xsmall().text_color(muted)),
                        )
                    }),
            )
            .when(show_index, |cell| {
                cell.child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child((index + 1).to_string()),
                )
            })
            .when(show_name, |cell| {
                cell.child(
                    div()
                        .w(px(side))
                        .text_xs()
                        .text_color(muted)
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(name.clone()),
                )
            })
    }

    /// VirtualList가 보이는 슬롯 범위를 받아 데이터 인덱스로 되돌리고 그린다.
    fn dock_slice(
        &mut self,
        range: std::ops::Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let (cell, cross) = self.dock_cell_metrics(cx);
        let horizontal = SettingsStore::global(cx)
            .settings
            .dock_position
            .is_horizontal();
        let reversed = self.dock_reversed(cx);
        let total = self.list.len();
        if total == 0 {
            return Vec::new();
        }
        let to_index = |slot: usize| {
            let slot = slot.min(total - 1);
            if reversed { total - 1 - slot } else { slot }
        };
        // 보이는 범위 앞뒤로 여유를 두고 썸네일을 요청한다(이미 받았거나 요청 중인 칸은 건너뜀).
        let start = range.start.saturating_sub(OVERSCAN);
        let end = (range.end + OVERSCAN).min(total);
        let wanted: Vec<usize> = (start..end).map(to_index).collect();
        self.request_thumbs_for(wanted, window, cx);
        range
            .map(|slot| {
                self.dock_cell(to_index(slot), horizontal, cell, cross, cx)
                    .into_any_element()
            })
            .collect()
    }

    pub(super) fn render_dock(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let settings = SettingsStore::global(cx).settings.clone();
        let position = settings.dock_position;
        let horizontal = position.is_horizontal();
        let border = cx.theme().border;
        let (cell, cross) = self.dock_cell_metrics(cx);
        let total = self.list.len();
        let base = div()
            .id("dock")
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .bg(cx.theme().background)
            .border_color(border)
            .map(|dock| match position {
                DockPosition::Top => dock.w_full().border_b_1(),
                DockPosition::Bottom => dock.w_full().border_t_1(),
                DockPosition::Left => dock.h_full().flex_col().border_r_1(),
                DockPosition::Right => dock.h_full().flex_col().border_l_1(),
            });

        self.follow_current(window, cx);
        // 내용이 화면보다 짧으면 기존 도크처럼 가운데에 둔다.
        let (length, _) = self.dock_metrics(window, cx);
        let content = total as f32 * cell + DOCK_PADDING * 2.0;
        let lead = ((length - content) / 2.0).max(0.0);
        let sizes: Rc<Vec<Size<Pixels>>> = Rc::new(
            (0..total)
                .map(|_| {
                    if horizontal {
                        size(px(cell), px(0.))
                    } else {
                        size(px(0.), px(cell))
                    }
                })
                .collect(),
        );
        let list = if horizontal {
            h_virtual_list(
                cx.entity(),
                "dock-list",
                sizes.clone(),
                move |this, range, window, cx| this.dock_slice(range, window, cx),
            )
        } else {
            v_virtual_list(
                cx.entity(),
                "dock-list",
                sizes,
                move |this, range, window, cx| this.dock_slice(range, window, cx),
            )
        }
        .track_scroll(&self.dock_virtual)
        .p(px(DOCK_PADDING))
        .when(horizontal, |list| list.pl(px(DOCK_PADDING + lead)))
        .when(!horizontal, |list| list.pt(px(DOCK_PADDING + lead)))
        // 휠은 바깥 SmoothScroll이 처리하고, 목록 자체의 기본 스크롤은 끈다.
        .overflow_hidden();
        let scroller = div()
            .id("dock-scroller")
            .relative()
            .map(|scroller| {
                if horizontal {
                    scroller.w_full().h(px(cross + DOCK_PADDING * 2.0))
                } else {
                    scroller.h_full().w(px(cross + DOCK_PADDING * 2.0))
                }
            })
            .on_scroll_wheel(self.dock_scroll.wheel_handler(horizontal))
            .when(horizontal, |scroller| {
                scroller.horizontal_scrollbar(&self.dock_virtual)
            })
            .when(!horizontal, |scroller| {
                scroller.vertical_scrollbar(&self.dock_virtual)
            })
            .child(list);
        base.child(scroller).into_any_element()
    }

    /// 상태바 파일명 왼쪽에 두는 도크 컨트롤: 목록 접기·펼치기, 그리드 토글.
    /// 이미지 목록 세부 설정은 설정 다이얼로그에서만 바꾼다.
    pub(super) fn render_dock_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        let settings = SettingsStore::global(cx).settings.clone();
        let grid = Button::new("dock-grid")
            .ghost()
            .small()
            .icon(Ph::SquaresFour)
            .tooltip(t("viewer.nav.gridTitle"))
            .on_click(cx.listener(|this, _, window, cx| this.toggle_grid(window, cx)));
        // 접기 버튼은 상태바에서 접힌 도크까지 되돌린다.
        let (fold_icon, fold_label) = if settings.dock_visible {
            (Ph::CaretDown, t("viewer.nav.dockHide"))
        } else {
            (Ph::CaretUp, t("viewer.nav.dockShow"))
        };
        let collapse = Button::new("dock-hide")
            .ghost()
            .small()
            .icon(fold_icon)
            .tooltip(fold_label)
            .on_click(cx.listener(|this, _, _, cx| {
                this.set_dock(
                    |settings| settings.dock_visible = !settings.dock_visible,
                    cx,
                )
            }));
        h_flex()
            .gap_0p5()
            .flex_none()
            .child(collapse)
            .child(grid)
            .into_any_element()
    }
}
