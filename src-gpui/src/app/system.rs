//! 설정 반영, 항상 위, 업데이트 확인, 라이선스, 두 번째 실행 전달(SPEC §4.3, §20).

use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use crate::toast::{Toast, WindowToast};
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::theme::{Theme, ThemeMode};
use gpui_kit::*;

use super::AraView;
use super::settings_panel::{Effect, SettingsPanel};
use crate::dialog::{
    SETTINGS_HEIGHT, SETTINGS_TITLE_HEIGHT, SETTINGS_WIDTH, centered, centered_margin_top,
};
use crate::i18n::{self, t};
use crate::platform;
use crate::settings::{SettingsStore, ThemeChoice, WindowState};

/// 두 번째 실행이 넘긴 경로를 확인하는 주기.
const INSTANCE_POLL: Duration = Duration::from_millis(150);

pub fn apply_theme(choice: ThemeChoice, window: &mut Window, cx: &mut App) {
    match choice {
        ThemeChoice::System => Theme::sync_system_appearance(Some(window), cx),
        ThemeChoice::Light => Theme::change(ThemeMode::Light, Some(window), cx),
        ThemeChoice::Dark => Theme::change(ThemeMode::Dark, Some(window), cx),
    }
    if platform::high_contrast() {
        raise_contrast(cx);
        window.refresh();
    }
}

/// OS 대비 테마가 켜져 있을 때 글자·테두리 대비를 올린다. 별도 토글은 없다(SPEC §3.3).
pub(super) fn raise_contrast(cx: &mut App) {
    let theme = Theme::global_mut(cx);
    let ink = if theme.is_dark() {
        gpui_kit::white()
    } else {
        gpui_kit::black()
    };
    theme.foreground = ink;
    theme.muted_foreground = ink.opacity(0.85);
    theme.border = ink.opacity(0.8);
    theme.input = ink.opacity(0.8);
    theme.ring = ink;
}

impl AraView {
    pub(super) fn settings_changed(
        &mut self,
        effect: Effect,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match effect {
            Effect::Redraw => {
                self.sync_pages(window, cx);
                self.request_thumbs(window, cx);
            }
            Effect::Language => {
                i18n::set_language(SettingsStore::global(cx).settings.language);
                if self.info.is_none() {
                    window.set_window_title(&t("app.title"));
                }
            }
            Effect::Theme => apply_theme(SettingsStore::global(cx).theme, window, cx),
            Effect::Relist => self.refresh_listing(window, cx),
            Effect::Reload => self.reload(window, cx),
            Effect::Layout => {
                // 설정에서 보기 모드를 직접 골랐으므로 아카이브의 자동 결정을 해제한다.
                self.comic_mode = None;
                self.view_mode_changed(window, cx);
            }
            Effect::AlwaysOnTop => self.apply_always_on_top(window, cx),
        }
        cx.notify();
    }

    /// 정렬·하위 폴더 설정이 바뀌면 폴더 목록을 다시 읽고 보던 파일 위치를 복원한다.
    /// 아카이브 모드에서는 하지 않는다.
    fn refresh_listing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.archive.is_some() {
            return;
        }
        // sidecar처럼 표시 경로가 원본과 다를 수 있어 목록의 원본 경로를 쓴다.
        if let Some(path) = self.list.get(self.index) {
            self.open_path(path.to_string(), window, cx);
        }
    }

    /// 실행 중에 OS 대비 설정이 바뀌었으면 테마를 다시 적용한다. 설정 앱에서 바꾸고
    /// 돌아오는 창 활성화 시점에 확인한다.
    pub(super) fn recheck_contrast(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = platform::high_contrast();
        if now != self.high_contrast {
            self.high_contrast = now;
            apply_theme(SettingsStore::global(cx).theme, window, cx);
            cx.notify();
        }
    }

    pub(super) fn cache_cleared(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.thumbs.clear();
        self.thumbs_pending.clear();
        self.reload(window, cx);
        self.request_thumbs(window, cx);
    }

    pub(super) fn apply_always_on_top(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let on_top = SettingsStore::global(cx).settings.always_on_top;
        if !platform::set_always_on_top(window, on_top) {
            window.toast(Toast::error(t("toast.alwaysOnTop.fail")), cx);
        }
    }

    pub(super) fn toggle_always_on_top(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        SettingsStore::update(cx, |settings| {
            settings.always_on_top = !settings.always_on_top
        });
        self.apply_always_on_top(window, cx);
        cx.notify();
    }

    pub(super) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        let panel = cx.new(|cx| SettingsPanel::new(view, cx));
        let outside_view = cx.entity();
        window.open_dialog(cx, move |dialog, window, _| {
            let panel = panel.clone();
            let outside_view = outside_view.clone();
            // 배경을 끈 다이얼로그라 바깥 클릭 닫기는 설정 영역 밖 클릭으로 직접 처리한다.
            // 그 위에 확인 다이얼로그가 떠 있으면 그쪽 클릭이므로 건드리지 않는다.
            centered(dialog, window, SETTINGS_HEIGHT)
                .title(t("settings.title"))
                .width(px(SETTINGS_WIDTH))
                .child(
                    div()
                        .on_mouse_down_out({
                            let panel = panel.clone();
                            move |event, window, cx| {
                                if panel.read(cx).has_nested_dialog() {
                                    return;
                                }
                                // 제목 줄은 다이얼로그 안쪽이지만 감싼 영역 밖이다.
                                let width = f32::from(window.viewport_size().width);
                                let left = (width - SETTINGS_WIDTH) / 2.0;
                                let top = f32::from(centered_margin_top(window, SETTINGS_HEIGHT));
                                let (x, y) =
                                    (f32::from(event.position.x), f32::from(event.position.y));
                                if (left..left + SETTINGS_WIDTH).contains(&x)
                                    && (top..top + SETTINGS_TITLE_HEIGHT).contains(&y)
                                {
                                    return;
                                }
                                window.close_dialog(cx);
                                outside_view.update(cx, |this, cx| this.focus.focus(window, cx));
                            }
                        })
                        .child(panel),
                )
        });
    }

    /// 두 번째 실행이 넘기는 경로를 받아 기존 창에서 연다.
    pub(super) fn listen_for_instances(
        &mut self,
        receiver: Receiver<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(INSTANCE_POLL).await;
                let path = match receiver.try_recv() {
                    Ok(path) => path,
                    Err(TryRecvError::Empty) => continue,
                    Err(TryRecvError::Disconnected) => break,
                };
                let alive = this.update_in(cx, |this, window, cx| {
                    // 개발 빌드는 `action:<id>` 인자로 동작을 실행해 런타임 검증에 쓴다.
                    if cfg!(debug_assertions)
                        && let Some(action) = path.strip_prefix("action:")
                    {
                        this.run_action(action, window, cx);
                        return;
                    }
                    window.activate_window();
                    if !path.is_empty() {
                        this.open_path(path, window, cx);
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// 창 위치와 크기를 기억해 다음 실행에서 복원한다.
    pub(super) fn remember_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (bounds, maximized) = match window.window_bounds() {
            WindowBounds::Windowed(bounds) => (bounds, false),
            WindowBounds::Maximized(bounds) => (bounds, true),
            // 전체화면은 표시 상태일 뿐이라 복원 크기만 기억한다.
            WindowBounds::Fullscreen(bounds) => (bounds, false),
        };
        SettingsStore::set_window(
            cx,
            WindowState {
                x: bounds.origin.x.into(),
                y: bounds.origin.y.into(),
                width: bounds.size.width.into(),
                height: bounds.size.height.into(),
                maximized,
            },
        );
    }
}
