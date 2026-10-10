//! 썸네일 그리드 오버레이(SPEC §3.2, §14.1).

use araview_core::file_availability::FileAvailability;
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{AraView, ThumbState};
use crate::assets::Ph;
use crate::i18n::{t, t_with};

const CELL: f32 = 148.0;
const GAP: f32 = 10.0;
const LABEL: f32 = 20.0;
const PADDING: f32 = 16.0;

pub(super) struct GridState {
    filter: Entity<InputState>,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
    /// 필터된 목록 안에서의 키보드 선택 위치.
    selected: usize,
    _subscription: Subscription,
}

fn file_label(item: &str) -> &str {
    item.rsplit(['/', '\\']).next().unwrap_or(item)
}

impl AraView {
    pub(super) fn toggle_grid(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.grid_open {
            self.close_grid(window, cx);
            return;
        }
        if self.list.len() < 2 {
            return;
        }
        let grid = match self.grid.take() {
            Some(grid) => grid,
            None => {
                let filter =
                    cx.new(|cx| InputState::new(window, cx).placeholder(t("viewer.grid.filter")));
                let subscription =
                    cx.subscribe_in(&filter, window, |this, _, event, window, cx| match event {
                        InputEvent::Change => {
                            if let Some(grid) = &mut this.grid {
                                grid.selected = 0;
                                grid.scroll.scroll_to_item(0, ScrollStrategy::Top);
                            }
                            cx.notify();
                        }
                        InputEvent::PressEnter { .. } => this.grid_confirm(window, cx),
                        _ => {}
                    });
                GridState {
                    filter,
                    focus: cx.focus_handle(),
                    scroll: UniformListScrollHandle::new(),
                    selected: 0,
                    _subscription: subscription,
                }
            }
        };
        grid.filter
            .update(cx, |state, cx| state.set_value("", window, cx));
        grid.focus.focus(window, cx);
        self.grid = Some(grid);
        self.grid_open = true;
        // 열 때 현재 이미지를 선택하고 가운데 둔다.
        let columns = self.grid_columns(window);
        if let Some(grid) = &mut self.grid {
            grid.selected = self.index;
            grid.scroll
                .scroll_to_item(self.index / columns, ScrollStrategy::Center);
        }
        cx.notify();
    }

    pub(super) fn close_grid(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.grid_open = false;
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn grid_columns(&self, window: &Window) -> usize {
        let width = f32::from(window.viewport_size().width) - PADDING * 2.0;
        ((width + GAP) / (CELL + GAP)).floor().max(1.0) as usize
    }

    /// 파일명 필터를 통과한 목록 인덱스.
    fn grid_items(&self, cx: &App) -> Vec<usize> {
        let query = self
            .grid
            .as_ref()
            .map(|grid| grid.filter.read(cx).value().to_lowercase())
            .unwrap_or_default();
        if query.is_empty() {
            return (0..self.list.len()).collect();
        }
        self.list
            .iter()
            .enumerate()
            .filter(|(_, item)| file_label(item).to_lowercase().contains(&query))
            .map(|(index, _)| index)
            .collect()
    }

    fn grid_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let items = self.grid_items(cx);
        let Some(target) = self
            .grid
            .as_ref()
            .and_then(|grid| items.get(grid.selected))
            .copied()
        else {
            return;
        };
        self.close_grid(window, cx);
        self.jump_to(target, window, cx);
    }

    /// 그리드가 열려 있는 동안의 키 입력. 뷰어 단축키는 동작하지 않는다.
    pub(super) fn on_grid_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();
        let in_filter = self
            .grid
            .as_ref()
            .is_some_and(|grid| !grid.focus.is_focused(window));
        if key == "escape" {
            self.close_grid(window, cx);
            return;
        }
        if in_filter && !matches!(key, "up" | "down") {
            // 필터 입력 중에는 글자와 좌우 이동을 입력에 맡긴다.
            return;
        }
        let columns = self.grid_columns(window) as isize;
        let count = self.grid_items(cx).len() as isize;
        if count == 0 {
            return;
        }
        let page = columns * 4;
        let Some(grid) = &mut self.grid else {
            return;
        };
        let current = grid.selected as isize;
        let next = match key {
            "left" => current - 1,
            "right" => current + 1,
            "up" => current - columns,
            "down" => current + columns,
            "home" => 0,
            "end" => count - 1,
            "pageup" => current - page,
            "pagedown" => current + page,
            "enter" => {
                self.grid_confirm(window, cx);
                return;
            }
            "g" if !event.keystroke.modifiers.modified() => {
                self.close_grid(window, cx);
                return;
            }
            _ => return,
        };
        let next = next.clamp(0, count - 1) as usize;
        grid.selected = next;
        grid.scroll
            .scroll_to_item(next / columns as usize, ScrollStrategy::Nearest);
        cx.stop_propagation();
        cx.notify();
    }

    fn grid_cell(&self, position: usize, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let item = &self.list[index];
        let name = file_label(item).to_owned();
        let state = self.thumbs.get(item);
        let thumb = match state {
            Some(ThumbState::Ready(path)) => Some(path.clone()),
            _ => None,
        };
        let failed = matches!(state, Some(ThumbState::Failed));
        let cloud_only = matches!(
            self.availability.get(index),
            Some(FileAvailability::CloudOnly)
        );
        let current = self.shown.contains(&index) || index == self.index;
        let selected = self
            .grid
            .as_ref()
            .is_some_and(|grid| grid.selected == position);
        let muted = cx.theme().muted_foreground;
        v_flex()
            .id(("grid-cell", index))
            .w(px(CELL))
            .flex_none()
            .gap_1()
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, window, cx| {
                if failed {
                    this.retry_thumb(index, window, cx);
                    return;
                }
                this.close_grid(window, cx);
                this.jump_to(index, window, cx);
            }))
            .child(
                div()
                    .relative()
                    .size(px(CELL))
                    .rounded_md()
                    .overflow_hidden()
                    .border_2()
                    // 키보드 선택은 현재 페이지 표시와 다른 색으로 보여준다.
                    .border_color(if selected {
                        cx.theme().ring
                    } else if current {
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
                            cell.child(img(path).size_full().object_fit(ObjectFit::Contain))
                        }
                        None if failed => cell.child(
                            v_flex()
                                .items_center()
                                .gap_1()
                                .text_xs()
                                .text_color(muted)
                                .child(Icon::new(Ph::WarningCircle))
                                .child(t("viewer.grid.failed")),
                        ),
                        None => cell,
                    })
                    .when(cloud_only, |cell| {
                        cell.child(
                            div()
                                .absolute()
                                .top_1()
                                .right_1()
                                .child(Icon::new(Ph::Cloud).small().text_color(muted)),
                        )
                    }),
            )
            .child(
                div()
                    .h(px(LABEL))
                    .text_xs()
                    .text_color(muted)
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(format!("{}. {name}", index + 1)),
            )
            .test_support()
            .into_any_element()
    }

    pub(super) fn render_grid(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(grid) = &self.grid else {
            return div().into_any_element();
        };
        let items = self.grid_items(cx);
        let columns = self.grid_columns(window);
        let rows = items.len().div_ceil(columns);
        let total = self.list.len();
        let empty_key = if total == 0 {
            "viewer.grid.empty"
        } else {
            "viewer.grid.noResults"
        };
        let list = uniform_list(
            "grid-rows",
            rows,
            cx.processor(move |this, range: std::ops::Range<usize>, window, cx| {
                let items = this.grid_items(cx);
                let columns = this.grid_columns(window);
                let first = range.start * columns;
                let last = (range.end * columns).min(items.len());
                // 보이는 행의 썸네일만 요청한다.
                let visible: Vec<usize> = items[first.min(last)..last].to_vec();
                this.request_thumbs_for(visible, window, cx);
                range
                    .map(|row| {
                        let start = row * columns;
                        let end = (start + columns).min(items.len());
                        h_flex()
                            .gap(px(GAP))
                            .pb(px(GAP))
                            .children(
                                (start..end)
                                    .map(|position| this.grid_cell(position, items[position], cx)),
                            )
                            .into_any_element()
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .track_scroll(&grid.scroll)
        .flex_1()
        .min_h_0();

        v_flex()
            .id("grid")
            .absolute()
            .inset_0()
            .occlude()
            .track_focus(&grid.focus)
            .bg(cx.theme().background)
            .p(px(PADDING))
            .gap_3()
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(div().text_sm().child(t("viewer.grid.title")))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(t_with(
                                "viewer.grid.count",
                                &[("current", &(self.index + 1)), ("total", &total)],
                            )),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .w(px(240.))
                            .child(Input::new(&grid.filter).cleanable(true)),
                    )
                    .child(
                        Button::new("grid-close")
                            .ghost()
                            .small()
                            .icon(Ph::X)
                            .tooltip(t("viewer.grid.close"))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.close_grid(window, cx)),
                            ),
                    ),
            )
            .map(|grid| {
                if items.is_empty() {
                    grid.child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(t(empty_key)),
                    )
                } else {
                    grid.child(list)
                }
            })
            .into_any_element()
    }
}
