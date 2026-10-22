//! 설정 반영, 항상 위, 업데이트 확인, 라이선스, 두 번째 실행 전달(SPEC §4.3, §20).

use std::rc::Rc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use crate::toast::{Toast, WindowToast};
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::theme::{Theme, ThemeMode};
use gpui_kit::*;

use super::AraView;
use super::settings_panel::{Effect, SettingsPanel};
use crate::dialog::{
    BackdropClick, SETTINGS_HEIGHT, SETTINGS_TITLE_HEIGHT, SETTINGS_WIDTH, centered,
    centered_margin_top, watch_backdrop_click,
};
use crate::i18n::{self, t};
use crate::platform;
use crate::settings::{SettingsStore, ThemeChoice, WindowState};

/// 두 번째 실행이 넘긴 경로를 확인하는 주기.
const INSTANCE_POLL: Duration = Duration::from_millis(150);

/// 다이얼로그가 뒤 화면을 덮는 검정 불투명도. 라이트 테마 기본값(5%)이 너무 옅어
/// 다크 기본값과 같은 세기로 맞춘다(SPEC §17).
const DIALOG_DIM_ALPHA: f32 = 0.2;
/// `DIALOG_DIM_ALPHA`의 `#RRGGBBAA` 표기. 테마 설정은 문자열 색만 받는다.
const DIALOG_DIM_HEX: &str = "#00000033";

pub fn apply_theme(choice: ThemeChoice, window: &mut Window, cx: &mut App) {
    match choice {
        ThemeChoice::System => Theme::sync_system_appearance(Some(window), cx),
        ThemeChoice::Light => Theme::change(ThemeMode::Light, Some(window), cx),
        ThemeChoice::Dark => Theme::change(ThemeMode::Dark, Some(window), cx),
    }
    // 다이얼로그 딤은 모드와 무관하게 같은 세기로 둔다. 모드를 바꾸면 팔레트가 테마
    // 설정에서 다시 만들어지므로 설정 자체를 고쳐 두고, 지금 팔레트에도 바로 반영한다.
    let theme = Theme::global_mut(cx);
    for config in [&mut theme.light_theme, &mut theme.dark_theme] {
        Rc::make_mut(config).colors.overlay = Some(DIALOG_DIM_HEX.into());
    }
    theme.overlay = gpui_kit::black().opacity(DIALOG_DIM_ALPHA);
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
        // 지원하지 않는 OS에서는 단축키(`T`)도 아무 일을 하지 않는다.
        if !platform::ALWAYS_ON_TOP_SUPPORTED {
            return;
        }
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
        // 배경 누름 판정은 설정 창이 사는 동안 유지해야 한다(다이얼로그 빌더는 다시 돈다).
        let backdrop = BackdropClick::default();
        window.open_dialog(cx, move |dialog, window, _| {
            let panel = panel.clone();
            let outside_view = outside_view.clone();
            let backdrop = backdrop.clone();
            // 다이얼로그 배경은 클릭으로 닫히지 않으므로(`centered`의 `overlay_closable(false)`)
            // 바깥 클릭 닫기는 설정 영역 밖에서 직접 처리한다. 누름에서는 닫지 않는다:
            // 배경이 캡션 판정이라 그 누름이 곧 창 이동 드래그의 시작일 수 있다.
            // 그 위에 확인 다이얼로그가 떠 있으면 그쪽 클릭이므로 건드리지 않는다.
            centered(dialog, window, SETTINGS_HEIGHT)
                .title(t("settings.title"))
                .width(px(SETTINGS_WIDTH))
                .child({
                    let close_settings = {
                        let outside_view = outside_view.clone();
                        move |window: &mut Window, cx: &mut App| {
                            window.close_dialog(cx);
                            outside_view.update(cx, |this, cx| this.focus.focus(window, cx));
                        }
                    };
                    let press_backdrop = backdrop.clone();
                    let release_backdrop = backdrop.clone();
                    div()
                        .on_mouse_down_out({
                            let panel = panel.clone();
                            let close_settings = close_settings.clone();
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
                                if !press_backdrop
                                    .press(event.position, platform::window_origin(window))
                                {
                                    return;
                                }
                                watch_backdrop_click(
                                    press_backdrop.clone(),
                                    window,
                                    cx,
                                    close_settings.clone(),
                                );
                            }
                        })
                        .on_mouse_up_out(MouseButton::Left, move |event, window, cx| {
                            if release_backdrop
                                .release(Some(event.position), platform::window_origin(window))
                            {
                                close_settings(window, cx);
                            }
                        })
                        .child(panel)
                })
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
