//! 설정 다이얼로그(SPEC §9.5, §13, §14.3, §20.2).

use crate::toast::{Toast, WindowToast};
use araview_core::cache::{CacheCategory, CacheScope, CacheStats};
use araview_core::file_assoc::FileAssociation;
use araview_core::thumb_shell::PsdThumbStatus;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::{DialogDescription, DialogFooter};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _, StyledExt as _,
    WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::cell::Cell;
use std::rc::Rc;

use super::{AraView, format_bytes};
use crate::dialog::{ALERT_HEIGHT, centered};
use crate::i18n::{Language, t, t_with};
use crate::keys;
use crate::settings::{
    CacheMode, CacheStorageMode, DockPosition, DockThumbSize, ImageScalingMode, MaxResolution,
    MouseAction, Settings, SettingsStore, SortKey, ThemeChoice, ViewMode, ViewerBackground,
    WheelAction, default_shortcuts,
};
use crate::smooth::SmoothScroll;

/// 설정 변경 뒤 뷰어가 따라 해야 할 일.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Effect {
    Redraw,
    Language,
    Theme,
    /// 폴더 목록을 다시 읽는다(정렬·하위 폴더).
    Relist,
    /// 페이지를 다시 디코드한다(해상도·표시 정책).
    Reload,
    /// 보기 모드나 양쪽 배치가 바뀌었다.
    Layout,
    AlwaysOnTop,
}

#[derive(Default)]
enum Loadable<T> {
    #[default]
    Idle,
    Loading,
    Ready(T),
    Failed,
}

/// 설정 위에 쌓인 확인 다이얼로그가 살아 있는 동안 개수를 센다. 다이얼로그 빌더 클로저가
/// 이 값을 쥐고 있다가 닫히며 클로저가 버려질 때 줄어든다(Esc로 닫혀도 같다).
pub(super) struct NestedGuard(Rc<Cell<usize>>);

impl Drop for NestedGuard {
    fn drop(&mut self) {
        self.0.set(self.0.get().saturating_sub(1));
    }
}

pub(super) struct SettingsPanel {
    view: WeakEntity<AraView>,
    nested: Rc<Cell<usize>>,
    tab: usize,
    focus: FocusHandle,
    scroll: SmoothScroll,
    /// 키 입력을 기다리는 단축키 동작 ID.
    pub(super) capturing: Option<String>,
    cache: Loadable<CacheStats>,
    associations: Loadable<Vec<FileAssociation>>,
    psd: Loadable<PsdThumbStatus>,
}

const TABS: &[&str] = &[
    "settings.tabs.general",
    "settings.tabs.view",
    "settings.tabs.list",
    "settings.tabs.performance",
    "settings.tabs.shortcuts",
    "settings.tabs.extensions",
];

fn setting_row(
    title: SharedString,
    description: Option<SharedString>,
    control: impl IntoElement,
    cx: &App,
) -> Div {
    h_flex()
        .w_full()
        .gap_4()
        .items_start()
        .justify_between()
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(div().text_sm().child(title))
                .when_some(description, |column, text| {
                    column.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .whitespace_normal()
                            .child(text),
                    )
                }),
        )
        // 선택지가 많은 컨트롤이 제목을 짓누르지 않도록 폭을 제한해 줄바꿈시킨다.
        .child(div().flex_none().max_w(relative(0.62)).child(control))
}

fn heading(title: SharedString) -> Div {
    div().pt_2().text_sm().font_semibold().child(title)
}

impl SettingsPanel {
    pub(super) fn new(view: WeakEntity<AraView>, cx: &mut Context<Self>) -> Self {
        Self {
            view,
            nested: Rc::default(),
            tab: 0,
            focus: cx.focus_handle(),
            scroll: SmoothScroll::default(),
            capturing: None,
            cache: Loadable::Idle,
            associations: Loadable::Idle,
            psd: Loadable::Idle,
        }
    }

    /// 설정 위에 확인 다이얼로그가 떠 있는지. 떠 있으면 설정 바깥 클릭으로 닫지 않는다.
    pub(super) fn has_nested_dialog(&self) -> bool {
        self.nested.get() > 0
    }

    fn nested_guard(&self) -> Rc<NestedGuard> {
        self.nested.set(self.nested.get() + 1);
        Rc::new(NestedGuard(self.nested.clone()))
    }

    /// 설정을 고치고 저장한 뒤 뷰어에 알린다.
    fn apply(
        &mut self,
        edit: impl FnOnce(&mut Settings),
        effect: Effect,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        SettingsStore::update(cx, edit);
        self.view
            .update(cx, |view, cx| view.settings_changed(effect, window, cx))
            .ok();
        cx.notify();
    }

    /// 여러 값 중 하나를 고르는 버튼 묶음.
    fn choice<T: Copy + PartialEq + 'static>(
        &self,
        id: &'static str,
        current: T,
        options: &[(&'static str, T)],
        set: fn(&mut Settings, T),
        effect: Effect,
        cx: &mut Context<Self>,
    ) -> Div {
        h_flex()
            .gap_1()
            .flex_wrap()
            .justify_end()
            .children(options.iter().enumerate().map(|(position, (key, value))| {
                let value = *value;
                let button = Button::new((id, position)).small().label(t(key));
                // 고른 값은 채운 버튼으로 구분한다.
                let button = if current == value {
                    button.primary()
                } else {
                    button.outline()
                };
                button.on_click(cx.listener(move |this, _, window, cx| {
                    this.apply(|settings| set(settings, value), effect, window, cx)
                }))
            }))
    }

    fn toggle(
        &self,
        id: &'static str,
        checked: bool,
        set: fn(&mut Settings, bool),
        effect: Effect,
        cx: &mut Context<Self>,
    ) -> Switch {
        Switch::new(id).checked(checked).on_change(cx.listener(
            move |this, value: &bool, window, cx| {
                let value = *value;
                this.apply(|settings| set(settings, value), effect, window, cx)
            },
        ))
    }

    /// 항목에 연결된 단축키를 배지로 보여준다. 재할당하면 바로 따라간다.
    fn with_shortcut(&self, title: SharedString, action: &str, cx: &App) -> SharedString {
        match SettingsStore::global(cx).settings.shortcuts.get(action) {
            Some(spec) if !spec.is_empty() => format!("{title}  [{spec}]").into(),
            _ => title,
        }
    }

    // ----- 탭 -----

    fn general_tab(&mut self, cx: &mut Context<Self>) -> Div {
        let store = SettingsStore::global(cx);
        let s = store.settings.clone();
        let theme = store.theme;
        v_flex()
            .gap_4()
            .child(setting_row(
                t("settings.language.title"),
                Some(t("settings.language.desc")),
                self.choice(
                    "language",
                    s.language,
                    &[
                        ("settings.language.ko", Language::Ko),
                        ("settings.language.en", Language::En),
                    ],
                    |settings, value| settings.language = value,
                    Effect::Language,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.theme.title"),
                Some(t("settings.theme.desc")),
                h_flex().gap_1().children(
                    [
                        ("settings.theme.system", ThemeChoice::System),
                        ("settings.theme.light", ThemeChoice::Light),
                        ("settings.theme.dark", ThemeChoice::Dark),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(position, (key, value))| {
                        let button = Button::new(("theme", position)).small().label(t(key));
                        let button = if theme == value {
                            button.primary()
                        } else {
                            button.outline()
                        };
                        button.on_click(cx.listener(move |this, _, window, cx| {
                            SettingsStore::set_theme(cx, value);
                            this.apply(|_| {}, Effect::Theme, window, cx);
                        }))
                    }),
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.nav.title"),
                Some(t("settings.nav.desc")),
                self.choice(
                    "loop",
                    s.loop_navigation,
                    &[("settings.nav.stop", false), ("settings.nav.loop", true)],
                    |settings, value| settings.loop_navigation = value,
                    Effect::Redraw,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.skipBroken.title"),
                Some(t("settings.skipBroken.desc")),
                self.choice(
                    "skip-broken",
                    s.skip_broken_files,
                    &[
                        ("settings.skipBroken.off", false),
                        ("settings.skipBroken.on", true),
                    ],
                    |settings, value| settings.skip_broken_files = value,
                    Effect::Redraw,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.startup.title"),
                Some(t("settings.startup.desc")),
                self.choice(
                    "startup",
                    s.auto_open_last_file,
                    &[
                        ("settings.startup.home", false),
                        ("settings.startup.last", true),
                    ],
                    |settings, value| settings.auto_open_last_file = value,
                    Effect::Redraw,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.recent.title"),
                Some(t("settings.recent.desc")),
                self.choice(
                    "recent",
                    s.record_recent_files,
                    &[("settings.recent.on", true), ("settings.recent.off", false)],
                    |settings, value| settings.record_recent_files = value,
                    Effect::Redraw,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                self.with_shortcut(t("settings.window.alwaysOnTop"), "toggleAlwaysOnTop", cx),
                Some(t("settings.window.desc")),
                self.toggle(
                    "always-on-top",
                    s.always_on_top,
                    |settings, value| settings.always_on_top = value,
                    Effect::AlwaysOnTop,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.update.title"),
                Some(t_with(
                    "settings.update.current",
                    &[("version", &env!("CARGO_PKG_VERSION"))],
                )),
                Button::new("check-update")
                    .small()
                    .outline()
                    .label(t("settings.update.check"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.view
                            .update(cx, |view, cx| view.check_updates(window, cx))
                            .ok();
                    })),
                cx,
            ))
            .child(setting_row(
                t("settings.licenses.title"),
                Some(t("settings.licenses.desc")),
                Button::new("licenses")
                    .small()
                    .outline()
                    .label(t("settings.licenses.open"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.view
                            .update(cx, |view, cx| view.open_licenses(window, cx))
                            .ok();
                    })),
                cx,
            ))
    }

    fn view_tab(&mut self, cx: &mut Context<Self>) -> Div {
        let s = SettingsStore::global(cx).settings.clone();
        v_flex()
            .gap_4()
            .child(setting_row(
                t("settings.view.title"),
                Some(t("settings.view.desc")),
                self.choice(
                    "view-mode",
                    s.view_mode,
                    &[
                        ("settings.view.single", ViewMode::Single),
                        ("settings.view.ltr", ViewMode::LeftToRight),
                        ("settings.view.rtl", ViewMode::RightToLeft),
                        ("settings.view.webtoon", ViewMode::Webtoon),
                    ],
                    |settings, value| settings.view_mode = value,
                    Effect::Layout,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.imageRendering.title"),
                Some(t("settings.imageRendering.desc")),
                self.choice(
                    "scaling",
                    s.image_scaling_mode,
                    &[
                        ("settings.imageRendering.auto", ImageScalingMode::Auto),
                        ("settings.imageRendering.smooth", ImageScalingMode::Smooth),
                        (
                            "settings.imageRendering.pixelated",
                            ImageScalingMode::Pixelated,
                        ),
                    ],
                    |settings, value| settings.image_scaling_mode = value,
                    Effect::Reload,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.imageRendering.autoDetect"),
                None,
                self.toggle(
                    "auto-detect",
                    s.auto_detect_pixel_art,
                    |settings, value| settings.auto_detect_pixel_art = value,
                    Effect::Reload,
                    cx,
                ),
                cx,
            ))
            .child(heading(t("settings.webtoon.title")))
            .child(setting_row(
                t("settings.webtoon.pageBoundaries"),
                Some(t("settings.webtoon.desc")),
                self.toggle(
                    "wt-boundaries",
                    s.webtoon_page_boundaries,
                    |settings, value| settings.webtoon_page_boundaries = value,
                    Effect::Redraw,
                    cx,
                ),
                cx,
            ))
            .children(
                [
                    (
                        "wt-fit",
                        "settings.webtoon.fitWidth",
                        s.webtoon_fit_width,
                        (|settings, value| settings.webtoon_fit_width = value)
                            as fn(&mut Settings, bool),
                        Effect::Redraw,
                    ),
                    (
                        "wt-progress",
                        "settings.webtoon.showProgress",
                        s.webtoon_show_progress,
                        |settings, value| settings.webtoon_show_progress = value,
                        Effect::Redraw,
                    ),
                    (
                        "wt-jump",
                        "settings.webtoon.thumbnailJump",
                        s.webtoon_thumbnail_jump,
                        |settings, value| settings.webtoon_thumbnail_jump = value,
                        Effect::Redraw,
                    ),
                ]
                .into_iter()
                .map(|(id, key, checked, set, effect)| {
                    setting_row(t(key), None, self.toggle(id, checked, set, effect, cx), cx)
                })
                .collect::<Vec<_>>(),
            )
            .child(heading(t("settings.reading.title")))
            .children(
                [
                    (
                        "resume",
                        "settings.reading.resume",
                        s.resume_reading,
                        (|settings, value| settings.resume_reading = value)
                            as fn(&mut Settings, bool),
                        Effect::Redraw,
                    ),
                    (
                        "comic-auto",
                        "settings.reading.comicAutoDual",
                        s.comic_auto_dual_view,
                        |settings, value| settings.comic_auto_dual_view = value,
                        Effect::Redraw,
                    ),
                    (
                        "cover-alone",
                        "settings.reading.coverAlone",
                        s.show_cover_alone,
                        |settings, value| settings.show_cover_alone = value,
                        Effect::Layout,
                    ),
                    (
                        "wide-alone",
                        "settings.reading.wideAlone",
                        s.show_wide_page_alone,
                        |settings, value| settings.show_wide_page_alone = value,
                        Effect::Layout,
                    ),
                    (
                        "comic-info",
                        "settings.reading.showComicInfo",
                        s.show_comic_info,
                        |settings, value| settings.show_comic_info = value,
                        Effect::Redraw,
                    ),
                ]
                .into_iter()
                .map(|(id, key, checked, set, effect)| {
                    setting_row(t(key), None, self.toggle(id, checked, set, effect, cx), cx)
                })
                .collect::<Vec<_>>(),
            )
            .child(heading(t("settings.dock.title")))
            .child(setting_row(
                t("settings.dock.visible"),
                Some(t("settings.dock.desc")),
                self.toggle(
                    "dock-visible",
                    s.dock_visible,
                    |settings, value| settings.dock_visible = value,
                    Effect::Redraw,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.dock.position"),
                None,
                self.choice(
                    "dock-position",
                    s.dock_position,
                    &[
                        ("settings.dock.top", DockPosition::Top),
                        ("settings.dock.bottom", DockPosition::Bottom),
                        ("settings.dock.left", DockPosition::Left),
                        ("settings.dock.right", DockPosition::Right),
                    ],
                    |settings, value| settings.dock_position = value,
                    Effect::Redraw,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.dock.thumbSize"),
                None,
                self.choice(
                    "dock-size",
                    s.dock_thumb_size,
                    &[
                        ("settings.dock.thumbS", DockThumbSize::S),
                        ("settings.dock.thumbM", DockThumbSize::M),
                        ("settings.dock.thumbL", DockThumbSize::L),
                    ],
                    |settings, value| settings.dock_thumb_size = value,
                    Effect::Redraw,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.dock.showName"),
                None,
                self.toggle(
                    "dock-name",
                    s.dock_show_name,
                    |settings, value| settings.dock_show_name = value,
                    Effect::Redraw,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.dock.showIndex"),
                None,
                self.toggle(
                    "dock-index",
                    s.dock_show_index,
                    |settings, value| settings.dock_show_index = value,
                    Effect::Redraw,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                self.with_shortcut(t("settings.bg.title"), "cycleBackground", cx),
                Some(t("settings.bg.desc")),
                self.choice(
                    "background",
                    s.viewer_background,
                    &[
                        ("settings.bg.theme", ViewerBackground::Theme),
                        ("settings.bg.black", ViewerBackground::Black),
                        ("settings.bg.white", ViewerBackground::White),
                        ("settings.bg.checker", ViewerBackground::Checker),
                    ],
                    |settings, value| settings.viewer_background = value,
                    Effect::Redraw,
                    cx,
                ),
                cx,
            ))
            .child(heading(t("settings.present.title")))
            .child(setting_row(
                t("settings.present.autohide"),
                Some(t("settings.present.desc")),
                self.toggle(
                    "autohide",
                    s.auto_hide_ui,
                    |settings, value| settings.auto_hide_ui = value,
                    Effect::Redraw,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.present.hideMenuBar"),
                None,
                self.toggle(
                    "hide-menu-bar",
                    s.menu_bar_hidden,
                    |settings, value| settings.menu_bar_hidden = value,
                    Effect::Redraw,
                    cx,
                ),
                cx,
            ))
    }

    fn list_tab(&mut self, cx: &mut Context<Self>) -> Div {
        let s = SettingsStore::global(cx).settings.clone();
        v_flex()
            .gap_4()
            .child(setting_row(
                t("settings.sort.title"),
                Some(t("settings.sort.desc")),
                self.choice(
                    "sort-key",
                    s.sort_key,
                    &[
                        ("settings.sort.name", SortKey::Name),
                        ("settings.sort.date", SortKey::Date),
                        ("settings.sort.size", SortKey::Size),
                    ],
                    |settings, value| settings.sort_key = value,
                    Effect::Relist,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.sort.title"),
                None,
                self.choice(
                    "sort-order",
                    s.sort_descending,
                    &[
                        ("settings.sort.asc", false),
                        ("settings.sort.descOrder", true),
                    ],
                    |settings, value| settings.sort_descending = value,
                    Effect::Relist,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.sort.subfolders"),
                None,
                self.toggle(
                    "subfolders",
                    s.include_subfolders,
                    |settings, value| settings.include_subfolders = value,
                    Effect::Relist,
                    cx,
                ),
                cx,
            ))
    }

    fn load_cache_stats(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cache = Loadable::Loading;
        let task = cx.background_spawn(async { araview_core::cache::get_cache_stats() });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.cache = match result {
                    Ok(stats) => Loadable::Ready(stats),
                    Err(_) => Loadable::Failed,
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn clear_cache(&mut self, scope: CacheScope, window: &mut Window, cx: &mut Context<Self>) {
        let task = cx.background_spawn(async move { araview_core::cache::clear_cache(scope) });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| match result {
                Ok(cleared) => {
                    let key = if cleared.removed_file_count == 0
                        && cleared.stats.protected_file_count > 0
                    {
                        "toast.cache.protectedOnly"
                    } else if cleared.failed_file_count > 0 {
                        "toast.cache.partial"
                    } else {
                        "toast.cache.cleared"
                    };
                    window.toast(
                        Toast::success(t_with(
                            key,
                            &[("size", &format_bytes(cleared.removed_bytes))],
                        )),
                        cx,
                    );
                    this.cache = Loadable::Ready(cleared.stats);
                    // 지운 썸네일과 파생 이미지를 다음 탐색에서 다시 만들게 한다.
                    this.view
                        .update(cx, |view, cx| view.cache_cleared(window, cx))
                        .ok();
                    cx.notify();
                }
                Err(error) => {
                    window.toast(Toast::error(error.message), cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn confirm_clear_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let guard = self.nested_guard();
        let panel = cx.entity();
        // `AlertDialog`에는 상단 오프셋이 없어 가운데 배치가 안 되니,
        // 같은 모양의 `Dialog`로 쌓는다(버튼 생김새·동작은 경고창과 같다).
        window.open_dialog(cx, move |dialog, window, _cx| {
            let _guard = &guard;
            let panel = panel.clone();
            centered(dialog, window, ALERT_HEIGHT)
                .close_button(false)
                .overlay_closable(false)
                .title(t("confirm.cache.clearAll.title"))
                .child(DialogDescription::new().child(t("confirm.cache.clearAll.message")))
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("clear-cache-cancel")
                                .label(t("dialog.cancel"))
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("clear-cache-ok")
                                .danger()
                                .label(t("settings.cacheManagement.clearAll"))
                                .on_click(move |_, window, cx| {
                                    panel.update(cx, |this, cx| {
                                        this.clear_cache(CacheScope::All, window, cx)
                                    });
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
    }

    fn cache_section(&mut self, cx: &mut Context<Self>) -> Div {
        let muted = cx.theme().muted_foreground;
        let body: AnyElement = match &self.cache {
            Loadable::Idle | Loadable::Loading => div()
                .text_xs()
                .text_color(muted)
                .child(t("settings.cacheManagement.loading"))
                .into_any_element(),
            Loadable::Failed => v_flex()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .child(t("settings.cacheManagement.errorTitle")),
                )
                .child(
                    Button::new("cache-retry")
                        .small()
                        .outline()
                        .label(t("settings.cacheManagement.retry"))
                        .on_click(
                            cx.listener(|this, _, window, cx| this.load_cache_stats(window, cx)),
                        ),
                )
                .into_any_element(),
            Loadable::Ready(stats) if stats.file_count == 0 => v_flex()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .child(t("settings.cacheManagement.emptyTitle")),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child(t("settings.cacheManagement.emptyDesc")),
                )
                .into_any_element(),
            Loadable::Ready(stats) => {
                let mode_key = if !stats.persistent_available {
                    "settings.cacheManagement.persistentUnavailable"
                } else if matches!(
                    stats.storage_mode,
                    araview_core::process_temp::CacheStorageMode::Persistent
                ) {
                    "settings.cacheManagement.persistentActive"
                } else {
                    "settings.cacheManagement.temporaryActive"
                };
                v_flex()
                    .gap_2()
                    .child(div().text_xs().text_color(muted).child(t(mode_key)))
                    .child(
                        h_flex()
                            .justify_between()
                            .text_sm()
                            .child(t("settings.cacheManagement.total"))
                            .child(format!(
                                "{} · {} · {}",
                                format_bytes(stats.total_bytes),
                                t_with(
                                    "settings.cacheManagement.files",
                                    &[("count", &stats.file_count)]
                                ),
                                t_with(
                                    "settings.cacheManagement.protected",
                                    &[("count", &stats.protected_file_count)]
                                ),
                            )),
                    )
                    .children(stats.categories.iter().map(|category| {
                        let (name_key, scope) = match category.key {
                            CacheCategory::Thumbnails => ("thumbnails", CacheScope::Thumbnails),
                            CacheCategory::Converted => ("converted", CacheScope::Converted),
                            CacheCategory::Scaled => ("scaled", CacheScope::Scaled),
                            CacheCategory::Archives => ("archives", CacheScope::Archives),
                            CacheCategory::Other => ("other", CacheScope::Other),
                        };
                        let name = t(&format!("settings.cacheManagement.categories.{name_key}"));
                        h_flex()
                            .justify_between()
                            .items_center()
                            .text_sm()
                            .child(v_flex().child(name.clone()).child(
                                div().text_xs().text_color(muted).child(t_with(
                                    "settings.cacheManagement.categorySize",
                                    &[
                                        ("size", &format_bytes(category.bytes)),
                                        ("count", &category.file_count),
                                    ],
                                )),
                            ))
                            .child(
                                Button::new(("cache-clear", name_key.len() + scope as usize * 16))
                                    .small()
                                    .outline()
                                    .label(t("settings.cacheManagement.clear"))
                                    .tooltip(t_with(
                                        "settings.cacheManagement.clearCategory",
                                        &[("name", &name)],
                                    ))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.clear_cache(scope, window, cx)
                                    })),
                            )
                    }))
                    .child(
                        h_flex().justify_end().child(
                            Button::new("cache-clear-all")
                                .small()
                                .danger()
                                .label(t("settings.cacheManagement.clearAll"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.confirm_clear_all(window, cx)
                                })),
                        ),
                    )
                    .into_any_element()
            }
        };
        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(heading(t("settings.cacheManagement.title")))
                    .child(
                        Button::new("cache-refresh")
                            .small()
                            .ghost()
                            .label(t("settings.cacheManagement.refresh"))
                            .on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.load_cache_stats(window, cx)
                                }),
                            ),
                    ),
            )
            .child(body)
    }

    fn performance_tab(&mut self, cx: &mut Context<Self>) -> Div {
        let s = SettingsStore::global(cx).settings.clone();
        v_flex()
            .gap_4()
            .child(setting_row(
                t("settings.cache.title"),
                Some(t("settings.cache.desc")),
                self.choice(
                    "cache-mode",
                    s.cache_mode,
                    &[
                        ("settings.cache.off", CacheMode::Off),
                        ("settings.cache.nearby", CacheMode::Nearby),
                        ("settings.cache.extended", CacheMode::Extended),
                        ("settings.cache.gb1", CacheMode::Memory1Gb),
                        ("settings.cache.gb2", CacheMode::Memory2Gb),
                    ],
                    |settings, value| settings.cache_mode = value,
                    Effect::Redraw,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.cacheStorage.persistent"),
                Some(t("settings.cacheStorage.desc")),
                Switch::new("cache-storage")
                    .checked(s.cache_storage_mode == CacheStorageMode::Persistent)
                    .on_change(cx.listener(|this, value: &bool, window, cx| {
                        let mode = if *value {
                            CacheStorageMode::Persistent
                        } else {
                            CacheStorageMode::Temporary
                        };
                        this.apply(
                            |settings| settings.cache_storage_mode = mode,
                            Effect::Redraw,
                            window,
                            cx,
                        );
                        // 저장 방식은 다음 실행부터 적용된다.
                        window.toast(Toast::info(t("toast.cache.storagePending")), cx);
                    })),
                cx,
            ))
            .child(setting_row(
                t("settings.resolution.title"),
                Some(t("settings.resolution.desc")),
                self.choice(
                    "resolution",
                    s.max_resolution,
                    &[
                        ("settings.resolution.original", MaxResolution::Original),
                        ("settings.resolution.4k", MaxResolution::FourK),
                        ("settings.resolution.1080p", MaxResolution::FullHd),
                    ],
                    |settings, value| settings.max_resolution = value,
                    Effect::Reload,
                    cx,
                ),
                cx,
            ))
            .child(self.cache_section(cx))
    }

    // ----- 단축키 -----

    fn set_shortcut(
        &mut self,
        action: String,
        spec: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.capturing = None;
        self.apply(
            move |settings| {
                // 같은 키를 쓰던 다른 동작은 해제한다.
                if let Some(other) =
                    keys::conflict(&settings.shortcuts, &action, &spec).map(str::to_owned)
                {
                    settings.shortcuts.insert(other, String::new());
                }
                settings.shortcuts.insert(action, spec);
            },
            Effect::Redraw,
            window,
            cx,
        );
    }

    pub(super) fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(action) = self.capturing.clone() else {
            return;
        };
        cx.stop_propagation();
        if event.keystroke.key == "escape" {
            self.capturing = None;
            cx.notify();
            return;
        }
        if event.keystroke.key == "tab" {
            window.toast(Toast::warning(t("settings.shortcuts.reserved")), cx);
            return;
        }
        let Some(spec) = keys::spec_from_keystroke(&event.keystroke) else {
            // 수식키만 눌린 동안은 계속 기다린다.
            return;
        };
        let shortcuts = SettingsStore::global(cx).settings.shortcuts.clone();
        let Some(other) = keys::conflict(&shortcuts, &action, &spec).map(str::to_owned) else {
            self.set_shortcut(action, spec, window, cx);
            return;
        };
        // 중복 바인딩은 교체 여부를 묻는다.
        let other_label = keys::label_key(&other).map(t).unwrap_or_default();
        let guard = self.nested_guard();
        let panel = cx.entity();
        let pressed = spec.clone();
        window.open_dialog(cx, move |dialog, window, _cx| {
            let _guard = &guard;
            let panel = panel.clone();
            let action = action.clone();
            let spec = spec.clone();
            centered(dialog, window, ALERT_HEIGHT)
                .close_button(false)
                .overlay_closable(false)
                .title(t("settings.shortcuts.conflictTitle"))
                .child(DialogDescription::new().child(t_with(
                    "settings.shortcuts.conflictDesc",
                    &[("key", &pressed), ("action", &other_label)],
                )))
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("shortcut-conflict-cancel")
                                .label(t("settings.shortcuts.cancel"))
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("shortcut-conflict-ok")
                                .primary()
                                .label(t("settings.shortcuts.replace"))
                                .on_click(move |_, window, cx| {
                                    panel.update(cx, |this, cx| {
                                        this.set_shortcut(action.clone(), spec.clone(), window, cx)
                                    });
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
    }

    fn shortcuts_tab(&mut self, cx: &mut Context<Self>) -> Div {
        let s = SettingsStore::global(cx).settings.clone();
        let defaults = default_shortcuts();
        let muted = cx.theme().muted_foreground;
        let capturing = self.capturing.clone();
        let hint_key = if capturing.is_some() {
            "settings.shortcuts.capturing"
        } else {
            "settings.shortcuts.captureHint"
        };

        let wheel_options: [(&'static str, WheelAction); 5] = [
            ("settings.wheel.action.prev", WheelAction::Prev),
            ("settings.wheel.action.next", WheelAction::Next),
            ("settings.wheel.action.zoomIn", WheelAction::ZoomIn),
            ("settings.wheel.action.zoomOut", WheelAction::ZoomOut),
            ("settings.wheel.action.none", WheelAction::None),
        ];
        type WheelSet = fn(&mut Settings, WheelAction);
        let wheel_slots: [(&'static str, &'static str, WheelAction, WheelSet); 8] = [
            ("w-up", "wheelUp", s.wheel.wheel_up, |s, v| {
                s.wheel.wheel_up = v
            }),
            ("w-down", "wheelDown", s.wheel.wheel_down, |s, v| {
                s.wheel.wheel_down = v
            }),
            ("w-c-up", "ctrl+wheelUp", s.wheel.ctrl_wheel_up, |s, v| {
                s.wheel.ctrl_wheel_up = v
            }),
            (
                "w-c-down",
                "ctrl+wheelDown",
                s.wheel.ctrl_wheel_down,
                |s, v| s.wheel.ctrl_wheel_down = v,
            ),
            ("w-s-up", "shift+wheelUp", s.wheel.shift_wheel_up, |s, v| {
                s.wheel.shift_wheel_up = v
            }),
            (
                "w-s-down",
                "shift+wheelDown",
                s.wheel.shift_wheel_down,
                |s, v| s.wheel.shift_wheel_down = v,
            ),
            ("w-a-up", "alt+wheelUp", s.wheel.alt_wheel_up, |s, v| {
                s.wheel.alt_wheel_up = v
            }),
            (
                "w-a-down",
                "alt+wheelDown",
                s.wheel.alt_wheel_down,
                |s, v| s.wheel.alt_wheel_down = v,
            ),
        ];

        let click_options: [(&'static str, MouseAction); 7] = [
            ("settings.mouse.action.prev", MouseAction::Prev),
            ("settings.mouse.action.next", MouseAction::Next),
            ("settings.mouse.action.zoomIn", MouseAction::ZoomIn),
            ("settings.mouse.action.zoomOut", MouseAction::ZoomOut),
            (
                "settings.mouse.action.toggleFullscreen",
                MouseAction::ToggleFullscreen,
            ),
            (
                "settings.mouse.action.contextMenu",
                MouseAction::ContextMenu,
            ),
            ("settings.mouse.action.none", MouseAction::None),
        ];
        // 왼쪽 드래그는 pan 또는 none만 허용한다.
        let drag_options: [(&'static str, MouseAction); 2] = [
            ("settings.mouse.action.pan", MouseAction::Pan),
            ("settings.mouse.action.none", MouseAction::None),
        ];

        v_flex()
            .gap_4()
            .child(heading(t("settings.shortcuts.title")))
            .child(div().text_xs().text_color(muted).child(t(hint_key)))
            .children(
                keys::ACTION_LABELS
                    .iter()
                    .enumerate()
                    .map(|(position, (id, key))| {
                        let spec = s.shortcuts.get(*id).cloned().unwrap_or_default();
                        let waiting = capturing.as_deref() == Some(*id);
                        let label = if waiting {
                            t("settings.shortcuts.capturing")
                        } else if spec.is_empty() {
                            t("settings.shortcuts.unbound")
                        } else {
                            spec.clone().into()
                        };
                        let default_spec = defaults.get(*id).cloned().unwrap_or_default();
                        let action = (*id).to_owned();
                        h_flex()
                            .gap_2()
                            .items_center()
                            .justify_between()
                            .child(div().flex_1().text_sm().child(t(key)))
                            .child(
                                Button::new(("shortcut", position))
                                    .small()
                                    .outline()
                                    .label(label)
                                    .selected(waiting)
                                    .on_click(cx.listener({
                                        let action = action.clone();
                                        move |this, _, window, cx| {
                                            this.capturing = Some(action.clone());
                                            this.focus.focus(window, cx);
                                            cx.notify();
                                        }
                                    })),
                            )
                            .child(
                                Button::new(("shortcut-clear", position))
                                    .xsmall()
                                    .ghost()
                                    .label(t("settings.shortcuts.clear"))
                                    .on_click(cx.listener({
                                        let action = action.clone();
                                        move |this, _, window, cx| {
                                            this.set_shortcut(
                                                action.clone(),
                                                String::new(),
                                                window,
                                                cx,
                                            )
                                        }
                                    })),
                            )
                            .child(
                                Button::new(("shortcut-restore", position))
                                    .xsmall()
                                    .ghost()
                                    .label(t("settings.shortcuts.restore"))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.set_shortcut(
                                            action.clone(),
                                            default_spec.clone(),
                                            window,
                                            cx,
                                        )
                                    })),
                            )
                    }),
            )
            .child(heading(t("settings.wheel.title")))
            .children(wheel_slots.into_iter().map(|(id, slot, current, set)| {
                setting_row(
                    t(&format!("settings.wheel.slot.{slot}")),
                    None,
                    self.choice(id, current, &wheel_options, set, Effect::Redraw, cx),
                    cx,
                )
            }))
            .child(heading(t("settings.mouse.title")))
            .child(setting_row(
                t("settings.mouse.trigger.leftDrag"),
                None,
                self.choice(
                    "m-drag",
                    s.mouse.left_drag,
                    &drag_options,
                    |settings, value| settings.mouse.left_drag = value,
                    Effect::Redraw,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.mouse.trigger.middleClick"),
                None,
                self.choice(
                    "m-middle",
                    s.mouse.middle_click,
                    &click_options,
                    |settings, value| settings.mouse.middle_click = value,
                    Effect::Redraw,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.mouse.trigger.doubleClick"),
                None,
                self.choice(
                    "m-double",
                    s.mouse.double_click,
                    &click_options,
                    |settings, value| settings.mouse.double_click = value,
                    Effect::Redraw,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.mouse.trigger.rightClick"),
                (s.mouse.right_click != MouseAction::ContextMenu)
                    .then(|| t("settings.mouse.rightClickWarning")),
                self.choice(
                    "m-right",
                    s.mouse.right_click,
                    &click_options,
                    |settings, value| settings.mouse.right_click = value,
                    Effect::Redraw,
                    cx,
                ),
                cx,
            ))
            .child(
                h_flex().justify_end().child(
                    Button::new("shortcuts-reset")
                        .small()
                        .outline()
                        .label(t("settings.shortcuts.resetAll"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.apply(
                                |settings| {
                                    let defaults = Settings::default();
                                    settings.shortcuts = defaults.shortcuts;
                                    settings.wheel = defaults.wheel;
                                    settings.mouse = defaults.mouse;
                                },
                                Effect::Redraw,
                                window,
                                cx,
                            )
                        })),
                ),
            )
    }

    // ----- 확장자 -----

    fn load_extensions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.associations = Loadable::Loading;
        self.psd = Loadable::Loading;
        let task = cx.background_spawn(async {
            (
                araview_core::file_assoc::list_associations(),
                araview_core::thumb_shell::status(),
            )
        });
        cx.spawn_in(window, async move |this, cx| {
            let (associations, psd) = task.await;
            this.update_in(cx, |this, window, cx| {
                this.associations = match associations {
                    Ok(list) => Loadable::Ready(list),
                    Err(error) => {
                        window.toast(
                            Toast::error(error.message).title(t("toast.assoc.loadFail")),
                            cx,
                        );
                        Loadable::Failed
                    }
                };
                this.psd = match psd {
                    Ok(status) => Loadable::Ready(status),
                    Err(_) => Loadable::Failed,
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn set_association(
        &mut self,
        extension: String,
        associate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let hwnd = crate::platform::hwnd(window);
        let task = cx.background_spawn(async move {
            araview_core::file_assoc::set_association(&extension, associate, hwnd)
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| match result {
                Ok(updated) => {
                    if let Loadable::Ready(list) = &mut this.associations
                        && let Some(slot) = list
                            .iter_mut()
                            .find(|item| item.extension == updated.extension)
                    {
                        *slot = updated;
                    }
                    cx.notify();
                }
                Err(error) => window.toast(
                    Toast::error(error.message).title(t("toast.assoc.openFail")),
                    cx,
                ),
            })
            .ok();
        })
        .detach();
    }

    fn set_psd_thumbnails(&mut self, enable: bool, window: &mut Window, cx: &mut Context<Self>) {
        let task = cx.background_spawn(async move {
            if enable {
                araview_core::thumb_shell::register()
            } else {
                araview_core::thumb_shell::unregister()
            }
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| match result {
                Ok(status) => {
                    let key = if status.registered {
                        "toast.thumb.doneEnabled"
                    } else {
                        "toast.thumb.doneDisabled"
                    };
                    window.toast(Toast::success(t(key)), cx);
                    this.psd = Loadable::Ready(status);
                    cx.notify();
                }
                Err(error) => {
                    let key = if enable {
                        "toast.thumb.registerFail"
                    } else {
                        "toast.thumb.unregisterFail"
                    };
                    window.toast(Toast::error(error.message).title(t(key)), cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn extensions_tab(&mut self, cx: &mut Context<Self>) -> Div {
        let muted = cx.theme().muted_foreground;
        let associations: AnyElement = match &self.associations {
            Loadable::Ready(list) => v_flex()
                .gap_2()
                .children(list.iter().enumerate().map(|(position, item)| {
                    let extension = item.extension.clone();
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(
                            v_flex()
                                .child(div().text_sm().child(format!(".{}", item.extension)))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(muted)
                                        .child(t(&format!("ext.{}", item.extension))),
                                ),
                        )
                        .child(
                            Switch::new(("assoc", position))
                                .checked(item.associated)
                                .on_change(cx.listener(move |this, value: &bool, window, cx| {
                                    this.set_association(extension.clone(), *value, window, cx)
                                })),
                        )
                }))
                .into_any_element(),
            Loadable::Failed => div()
                .text_xs()
                .text_color(muted)
                .child(t("toast.assoc.loadFail"))
                .into_any_element(),
            _ => div()
                .text_xs()
                .text_color(muted)
                .child(t("settings.ext.loading"))
                .into_any_element(),
        };
        let psd: AnyElement = match &self.psd {
            Loadable::Ready(status) => {
                let dll_exists = status.dll_exists;
                v_flex()
                    .gap_1()
                    .child(setting_row(
                        t("settings.thumb.label"),
                        Some(t("settings.thumb.desc")),
                        Switch::new("psd-thumb")
                            .checked(status.registered)
                            .disabled(!dll_exists && !status.registered)
                            .on_change(cx.listener(|this, value: &bool, window, cx| {
                                this.set_psd_thumbnails(*value, window, cx)
                            })),
                        cx,
                    ))
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .whitespace_normal()
                            .child(t(if dll_exists {
                                "settings.thumb.note"
                            } else {
                                "settings.thumb.dllMissing"
                            })),
                    )
                    .into_any_element()
            }
            Loadable::Failed => div()
                .text_xs()
                .text_color(muted)
                .child(t("toast.thumb.loadFail"))
                .into_any_element(),
            _ => div().into_any_element(),
        };
        v_flex()
            .gap_4()
            .child(heading(t("settings.ext.title")))
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .whitespace_normal()
                    .child(t("settings.ext.desc")),
            )
            .child(
                h_flex().child(
                    Button::new("default-apps")
                        .small()
                        .outline()
                        .label(t("settings.ext.openSettings"))
                        .on_click(|_, window, cx| {
                            if let Err(error) =
                                araview_core::file_assoc::open_default_apps_settings()
                            {
                                window.toast(
                                    Toast::error(error.message)
                                        .title(t("toast.assoc.settingsFail")),
                                    cx,
                                );
                            }
                        }),
                ),
            )
            .child(associations)
            .child(heading(t("settings.thumb.title")))
            .child(psd)
    }

    pub(super) fn select_tab(&mut self, tab: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.tab = tab;
        self.capturing = None;
        self.scroll.reset();
        match tab {
            3 => self.load_cache_stats(window, cx),
            5 => self.load_extensions(window, cx),
            _ => {}
        }
        cx.notify();
    }

    fn confirm_reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let guard = self.nested_guard();
        let panel = cx.entity();
        window.open_dialog(cx, move |dialog, window, _cx| {
            let _guard = &guard;
            let panel = panel.clone();
            centered(dialog, window, ALERT_HEIGHT)
                .close_button(false)
                .overlay_closable(false)
                .title(t("confirm.resetSettings.title"))
                .child(DialogDescription::new().child(t("confirm.resetSettings.message")))
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("reset-settings-cancel")
                                .label(t("dialog.cancel"))
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("reset-settings-ok")
                                .danger()
                                .label(t("settings.reset"))
                                .on_click(move |_, window, cx| {
                                    panel.update(cx, |this, cx| {
                                        this.apply(
                                            |settings| {
                                                // 언어 설정은 유지한다.
                                                let language = settings.language;
                                                *settings = Settings::default();
                                                settings.language = language;
                                            },
                                            Effect::Reload,
                                            window,
                                            cx,
                                        );
                                        this.view
                                            .update(cx, |view, cx| {
                                                view.settings_changed(
                                                    Effect::AlwaysOnTop,
                                                    window,
                                                    cx,
                                                );
                                                view.settings_changed(Effect::Relist, window, cx);
                                            })
                                            .ok();
                                        window.toast(
                                            Toast::success(t("toast.settings.resetDone")),
                                            cx,
                                        );
                                    });
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
    }
}

impl Render for SettingsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.tab {
            0 => self.general_tab(cx),
            1 => self.view_tab(cx),
            2 => self.list_tab(cx),
            3 => self.performance_tab(cx),
            4 => self.shortcuts_tab(cx),
            _ => self.extensions_tab(cx),
        };
        v_flex()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .w_full()
            .h(px(520.))
            .gap_3()
            .child(
                TabBar::new("settings-tabs")
                    .underline()
                    .selected_index(self.tab)
                    .on_click(cx.listener(|this, index: &usize, window, cx| {
                        this.select_tab(*index, window, cx)
                    }))
                    .children(TABS.iter().map(|key| Tab::new().label(t(key)))),
            )
            .child(
                self.scroll
                    .area("settings-body", div().pr_3().child(body))
                    .flex_1()
                    .min_h_0(),
            )
            .child(
                h_flex()
                    .justify_between()
                    .child(
                        Button::new("settings-reset")
                            .outline()
                            .label(t("settings.reset"))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.confirm_reset(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("settings-done")
                            .primary()
                            .label(t("settings.confirm"))
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    ),
            )
    }
}
