//! 휴지통 이동과 이름 변경(SPEC §11.1, §11.2).

use std::sync::Arc;

use crate::toast::{Toast, WindowToast};
use araview_core::app_error::AppError;
use araview_core::ops;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::{DialogDescription, DialogFooter};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{WindowExt as _, v_flex};
use gpui_kit::*;

use super::AraView;
use crate::dialog::{ALERT_HEIGHT, RENAME_HEIGHT, centered_margin_top};
use crate::i18n::{t, t_with};
use crate::settings::SettingsStore;

impl AraView {
    /// 확인 다이얼로그를 거쳐 현재 파일을 OS 휴지통으로 보낸다.
    pub(super) fn confirm_trash(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.archive.is_some() {
            window.toast(Toast::warning(t("toast.trash.noArchive")), cx);
            return;
        }
        let Some(info) = &self.info else {
            window.toast(Toast::warning(t("toast.trash.empty")), cx);
            return;
        };
        let name = info.file_name.clone();
        let path = info.source_path.clone();
        let entity = cx.entity();
        // `AlertDialog`에는 상단 오프셋이 없어 가운데 배치가 안 되니,
        // 같은 모양의 `Dialog`로 쌓는다(버튼 생김새·동작은 경고창과 같다).
        window.open_dialog(cx, move |dialog, window, _cx| {
            let entity = entity.clone();
            let path = path.clone();
            dialog
                .margin_top(centered_margin_top(window, ALERT_HEIGHT))
                .close_button(false)
                .overlay_closable(false)
                .title(t("confirm.trash.title"))
                .child(
                    DialogDescription::new()
                        .child(t_with("confirm.trash.message", &[("name", &name)])),
                )
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("trash-cancel")
                                .label(t("dialog.cancel"))
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("trash-ok")
                                .danger()
                                .label(t("menu.trash"))
                                .on_click(move |_, window, cx| {
                                    entity.update(cx, |this, cx| {
                                        this.trash(path.clone(), window, cx)
                                    });
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
    }

    fn trash(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        let target = path.clone();
        let task = cx.background_spawn(async move { ops::trash_file_impl(&target) });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| match result {
                Ok(()) => this.after_trash(&path, window, cx),
                Err(error) => {
                    window.toast(Toast::error(error.message).title(t("toast.trash.fail")), cx)
                }
            })
            .ok();
        })
        .detach();
    }

    /// 목록과 최근 파일에서 지운 항목을 빼고, 남은 목록의 같은 자리(끝이면 앞)를 연다.
    fn after_trash(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        SettingsStore::remove_recent(cx, path);
        window.toast(Toast::success(t("toast.trash.done")), cx);
        let mut items = (*self.list).clone();
        let removed = items
            .iter()
            .position(|item| **item == *path)
            .unwrap_or(self.index.min(items.len().saturating_sub(1)));
        if removed < items.len() {
            let gone = items.remove(removed);
            self.thumbs.remove(&gone);
        }
        if items.is_empty() {
            self.close_image(window, cx);
            return;
        }
        let next = removed.min(items.len() - 1);
        self.replace_list(items, next, window, cx);
    }

    pub(super) fn open_rename_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.archive.is_some() {
            window.toast(Toast::warning(t("toast.rename.noArchive")), cx);
            return;
        }
        let Some(info) = &self.info else {
            window.toast(Toast::warning(t("toast.rename.empty")), cx);
            return;
        };
        let old_path = info.source_path.clone();
        let input = cx.new(|cx| InputState::new(window, cx).default_value(info.file_name.clone()));
        let entity = cx.entity();
        // Enter로도 제출한다. 다이얼로그가 닫히면 입력 엔티티와 함께 구독도 사라진다.
        let submit_on_enter = {
            let entity = entity.clone();
            let old_path = old_path.clone();
            window.subscribe(&input, cx, move |input, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    let name = input.read(cx).value().to_string();
                    entity.update(cx, |this, cx| {
                        this.rename(old_path.clone(), name, window, cx);
                    });
                }
            })
        };
        submit_on_enter.detach();
        let name_input = input.clone();
        window.open_dialog(cx, move |dialog, window, _| {
            let entity = entity.clone();
            let input = input.clone();
            let old_path = old_path.clone();
            dialog
                .margin_top(centered_margin_top(window, RENAME_HEIGHT))
                .title(t("dialog.rename.title"))
                .child(
                    v_flex()
                        .gap_2()
                        .child(div().text_sm().child(t("dialog.rename.desc")))
                        .child(Input::new(&input)),
                )
                .footer(
                    DialogFooter::new()
                        .gap_2()
                        .child(
                            Button::new("rename-cancel")
                                .outline()
                                .label(t("dialog.cancel"))
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("rename-submit")
                                .primary()
                                .label(t("dialog.rename.submit"))
                                .on_click(move |_, window, cx| {
                                    let name = input.read(cx).value().to_string();
                                    entity.update(cx, |this, cx| {
                                        this.rename(old_path.clone(), name, window, cx);
                                    });
                                }),
                        ),
                )
        });
        // 바로 고쳐 쓸 수 있게 이름 입력에 포커스를 준다.
        name_input.read(cx).focus_handle(cx).focus(window, cx);
    }

    fn rename(
        &mut self,
        old_path: String,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if name.trim().is_empty() {
            window.toast(Toast::warning(t("toast.rename.blank")), cx);
            return;
        }
        let settings = &SettingsStore::global(cx).settings;
        let max_side = settings.max_resolution.max_side();
        let mode = ops::parse_image_scaling_mode(
            Some(settings.image_scaling_mode.as_str().to_owned()),
            Some(settings.auto_detect_pixel_art),
        );
        let source = old_path.clone();
        let task = cx.background_spawn(async move {
            ops::rename_file_impl_with_mode(&source, &name, max_side, mode)
        });
        cx.spawn_in(window, async move |this, cx| {
            let result: Result<_, AppError> = task.await;
            this.update_in(cx, |this, window, cx| match result {
                Ok(info) => {
                    window.close_dialog(cx);
                    let new_path: Arc<str> = Arc::from(info.source_path.as_str());
                    let mut items = (*this.list).clone();
                    if let Some(slot) = items.iter_mut().find(|item| ***item == *old_path) {
                        if let Some(thumb) = this.thumbs.remove(slot) {
                            this.thumbs.insert(new_path.clone(), thumb);
                        }
                        *slot = new_path;
                    }
                    this.list = Arc::new(items);
                    SettingsStore::replace_recent(cx, &old_path, &info.source_path);
                    window.set_window_title(&info.file_name);
                    // 표시 경로가 원본 자체였다면 이름과 함께 바뀌었으므로 정보만 교체한다.
                    this.info = Some(info);
                    window.toast(Toast::success(t("toast.rename.done")), cx);
                    cx.notify();
                }
                Err(error) => window.toast(
                    Toast::error(error.message).title(t("toast.rename.fail")),
                    cx,
                ),
            })
            .ok();
        })
        .detach();
    }
}
