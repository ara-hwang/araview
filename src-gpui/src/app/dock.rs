//! 이미지 목록 도크와 썸네일 요청(SPEC §3.2, §9.3).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use araview_core::file_availability::FileAvailability;
use araview_core::image::is_archive_file;
use araview_core::ops;
use araview_core::thumbnail::BatchThumb;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{AraView, ThumbState};
use crate::assets::Ph;
use crate::geometry::Offset;
use crate::i18n::{t, t_with};
use crate::settings::{DockPosition, DockThumbSize, SettingsStore, ViewMode, WheelAction};

/// 썸네일 생성 요청 한 변(px). 코어 캐시의 기본 크기와 같다.
const THUMB_MAX_SIDE: u32 = 256;
/// 한 번에 요청하는 썸네일 수. 끝난 청크부터 그린다.
const THUMB_CHUNK: usize = 8;
const DOCK_GAP: f32 = 6.0;
const DOCK_PADDING: f32 = 8.0;
/// 도크 양 끝 버튼들이 차지하는 길이(px).
const DOCK_CONTROLS: f32 = 200.0;
const LABEL_HEIGHT: f32 = 16.0;

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

    /// 현재 항목을 가운데 두고 도크 길이에 들어가는 인덱스 범위.
    fn dock_range(&self, window: &Window, cx: &App) -> std::ops::Range<usize> {
        let settings = &SettingsStore::global(cx).settings;
        let viewport = window.viewport_size();
        let length = if settings.dock_position.is_horizontal() {
            // 정보 패널이 열려 있으면 도크가 쓸 수 있는 폭이 그만큼 줄어든다.
            let panel = if self.info_open {
                super::info_panel::PANEL_WIDTH
            } else {
                0.0
            };
            f32::from(viewport.width) - panel
        } else {
            f32::from(viewport.height) - 120.0
        };
        let labels = usize::from(settings.dock_show_name) + usize::from(settings.dock_show_index);
        let cell = settings.dock_thumb_size.side()
            + if settings.dock_position.is_horizontal() {
                0.0
            } else {
                labels as f32 * LABEL_HEIGHT
            };
        let capacity = ((length - DOCK_PADDING * 2.0 - DOCK_CONTROLS) / (cell + DOCK_GAP))
            .floor()
            .max(1.0) as usize;
        let total = self.list.len();
        if total <= capacity {
            return 0..total;
        }
        let start = self
            .index
            .saturating_sub(capacity / 2)
            .min(total - capacity);
        start..start + capacity
    }

    fn set_dock(
        &mut self,
        edit: impl FnOnce(&mut crate::settings::Settings),
        cx: &mut Context<Self>,
    ) {
        SettingsStore::update(cx, edit);
        cx.notify();
    }

    fn dock_cell(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement + use<> {
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
        v_flex()
            .id(("thumb", index))
            .flex_none()
            .w(px(side))
            .gap_0p5()
            .items_center()
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
                        .w_full()
                        .text_xs()
                        .text_color(muted)
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(name.clone()),
                )
            })
    }

    fn on_dock_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delta = event.delta.pixel_delta(window.line_height()).y;
        if delta == px(0.) {
            return;
        }
        // 웹툰 모드에서도 도크 휠은 이동으로 동작한다.
        let (action, _) = Self::wheel_action(event, delta > px(0.), cx);
        if action != WheelAction::None {
            cx.stop_propagation();
            self.run_wheel_action(action, Offset::default(), window, cx);
        }
    }

    pub(super) fn render_dock(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let settings = SettingsStore::global(cx).settings.clone();
        let position = settings.dock_position;
        let horizontal = position.is_horizontal();
        let border = cx.theme().border;
        let base = div()
            .id("dock")
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .gap(px(DOCK_GAP))
            .bg(cx.theme().background)
            .border_color(border)
            .map(|dock| match position {
                DockPosition::Top => dock.w_full().border_b_1(),
                DockPosition::Bottom => dock.w_full().border_t_1(),
                DockPosition::Left => dock.h_full().flex_col().border_r_1(),
                DockPosition::Right => dock.h_full().flex_col().border_l_1(),
            });

        if !settings.dock_visible {
            // 접힌 상태는 얇은 엣지 바로 남겨 다시 펼칠 수 있게 한다.
            return base
                .p_0p5()
                .child(
                    Button::new("dock-show")
                        .ghost()
                        .xsmall()
                        .icon(Ph::CaretUp)
                        .tooltip(t("viewer.nav.dockShow"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.set_dock(|settings| settings.dock_visible = true, cx)
                        })),
                )
                .into_any_element();
        }

        // 우→좌 양쪽 보기의 가로 도크는 읽는 방향을 따라 첫 장을 오른쪽에 둔다.
        let reversed = horizontal && self.view_mode(cx) == ViewMode::RightToLeft;
        let range = self.dock_range(window, cx);
        let mut cells: Vec<AnyElement> = range
            .map(|index| self.dock_cell(index, cx).into_any_element())
            .collect();
        if reversed {
            cells.reverse();
        }
        let prev = Button::new("dock-prev")
            .ghost()
            .small()
            .icon(if horizontal {
                if reversed {
                    Ph::CaretRight
                } else {
                    Ph::CaretLeft
                }
            } else {
                Ph::CaretUp
            })
            .tooltip(t("viewer.nav.prev"))
            .on_click(cx.listener(|this, _, window, cx| this.navigate(false, window, cx)));
        let next = Button::new("dock-next")
            .ghost()
            .small()
            .icon(if horizontal {
                if reversed {
                    Ph::CaretLeft
                } else {
                    Ph::CaretRight
                }
            } else {
                Ph::CaretDown
            })
            .tooltip(t("viewer.nav.next"))
            .on_click(cx.listener(|this, _, window, cx| this.navigate(true, window, cx)));
        let (first, last) = if reversed { (next, prev) } else { (prev, next) };

        let view = cx.entity();
        let options = Button::new("dock-menu")
            .ghost()
            .small()
            .icon(Ph::DotsThree)
            .tooltip(t("viewer.nav.dockMenu"))
            .dropdown_menu(move |menu, window, cx| {
                let settings = SettingsStore::global(cx).settings.clone();
                let mut menu = menu;
                for (key, value) in [
                    ("settings.view.dockTop", DockPosition::Top),
                    ("settings.view.dockBottom", DockPosition::Bottom),
                    ("settings.view.dockLeft", DockPosition::Left),
                    ("settings.view.dockRight", DockPosition::Right),
                ] {
                    menu = menu.item(
                        PopupMenuItem::new(t(key))
                            .checked(settings.dock_position == value)
                            .on_click(window.listener_for(&view, move |this, _, _, cx| {
                                this.set_dock(|settings| settings.dock_position = value, cx)
                            })),
                    );
                }
                menu = menu.separator();
                for (key, value) in [
                    ("settings.view.dockSizeS", DockThumbSize::S),
                    ("settings.view.dockSizeM", DockThumbSize::M),
                    ("settings.view.dockSizeL", DockThumbSize::L),
                ] {
                    menu = menu.item(
                        PopupMenuItem::new(t(key))
                            .checked(settings.dock_thumb_size == value)
                            .on_click(window.listener_for(&view, move |this, _, window, cx| {
                                this.set_dock(|settings| settings.dock_thumb_size = value, cx);
                                this.request_thumbs(window, cx);
                            })),
                    );
                }
                menu.separator()
                    .item(
                        PopupMenuItem::new(t("settings.view.dockShowName"))
                            .checked(settings.dock_show_name)
                            .on_click(window.listener_for(&view, |this, _, _, cx| {
                                this.set_dock(
                                    |settings| settings.dock_show_name = !settings.dock_show_name,
                                    cx,
                                )
                            })),
                    )
                    .item(
                        PopupMenuItem::new(t("settings.view.dockShowIndex"))
                            .checked(settings.dock_show_index)
                            .on_click(window.listener_for(&view, |this, _, _, cx| {
                                this.set_dock(
                                    |settings| settings.dock_show_index = !settings.dock_show_index,
                                    cx,
                                )
                            })),
                    )
            });
        let grid = Button::new("dock-grid")
            .ghost()
            .small()
            .icon(Ph::SquaresFour)
            .tooltip(t("viewer.nav.gridTitle"))
            .on_click(cx.listener(|this, _, window, cx| this.toggle_grid(window, cx)));
        let collapse = Button::new("dock-hide")
            .ghost()
            .small()
            .icon(Ph::CaretDown)
            .tooltip(t("viewer.nav.dockHide"))
            .on_click(cx.listener(|this, _, _, cx| {
                this.set_dock(|settings| settings.dock_visible = false, cx)
            }));
        let controls: AnyElement = if horizontal {
            h_flex()
                .gap_0p5()
                .child(grid)
                .child(options)
                .child(collapse)
                .into_any_element()
        } else {
            v_flex()
                .gap_0p5()
                .child(grid)
                .child(options)
                .child(collapse)
                .into_any_element()
        };

        base.p(px(DOCK_PADDING))
            .on_scroll_wheel(cx.listener(Self::on_dock_wheel))
            .child(first)
            .children(cells)
            .child(last)
            .child(controls)
            .into_any_element()
    }
}
