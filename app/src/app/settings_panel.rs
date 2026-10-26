//! 설정 다이얼로그(SPEC §9.5, §13, §14.3, §20.2).

use crate::toast::{Toast, WindowToast};
use araview_core::cache::{CacheCategory, CacheScope, CacheStats};
use araview_core::file_assoc::FileAssociation;
use araview_core::thumb_shell::PsdThumbStatus;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants as _};
use gpui_kit::component::group_box::{GroupBox, GroupBoxVariants as _};
use gpui_kit::component::kbd::Kbd;
use gpui_kit::component::select::{SearchableVec, Select, SelectEvent, SelectState};
use gpui_kit::component::skeleton::Skeleton;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IndexPath, Selectable as _, Sizable as _, StyledExt as _,
    WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use super::{AraView, format_bytes};
use crate::dialog::{ALERT_HEIGHT, centered_margin_top};
use crate::i18n::{Language, t, t_with};
use crate::keys;
use crate::settings::{
    CacheMode, CacheStorageMode, DockPosition, DockThumbSize, ImageScalingMode, MaxResolution,
    MouseAction, Settings, SettingsStore, SortKey, ThemeChoice, ViewMode, ViewerBackground,
    WheelAction, WheelMode, default_shortcuts,
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

/// 선택형 설정 묶음. `options` 순서가 드롭다운 순서이며 인덱스로 값을 되찾는다.
struct ChoiceDef<'a, T> {
    current: T,
    options: &'a [(&'static str, T)],
    get: fn(&Settings) -> T,
    set: fn(&mut Settings, T),
    effect: Effect,
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
    /// 선택형 설정의 드롭다운 상태. 라벨이 `SharedString`이라 모든 항목이 같은
    /// 델리게이트를 쓰며, 인덱스로 설정값을 되찾는다.
    selects: HashMap<&'static str, Entity<SelectState<SearchableVec<SharedString>>>>,
    select_labels: HashMap<&'static str, Vec<SharedString>>,
    select_subs: Vec<Subscription>,
}

pub(super) const TABS: &[&str] = &[
    "settings.tabs.general",
    "settings.tabs.view",
    "settings.tabs.list",
    "settings.tabs.performance",
    "settings.tabs.shortcuts",
    // 확장자 연결과 PSD 썸네일은 Windows 셸 연동이라 다른 OS에서는 탭을 두지 않는다.
    #[cfg(windows)]
    "settings.tabs.extensions",
];

fn setting_row(
    title: impl IntoElement,
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
            selects: HashMap::new(),
            select_labels: HashMap::new(),
            select_subs: Vec::new(),
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

    /// 여러 값 중 하나를 고르는 드롭다운(gpui-kit `Select`). 같은 라벨 타입을
    /// 쓰므로 모든 설정이 `SharedString` 델리게이트를 공유하고, 인덱스로 값을
    /// 되찾는다. 현재값이 후보에 없으면(휠 묶음 어긋남) 비어 보인다.
    fn select<T: Copy + PartialEq + 'static>(
        &mut self,
        id: &'static str,
        label: SharedString,
        def: ChoiceDef<'_, T>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let labels: Vec<SharedString> = def.options.iter().map(|(key, _)| t(key)).collect();
        let wanted = def
            .options
            .iter()
            .position(|(_, value)| *value == def.current)
            .map(IndexPath::new);
        if let Some(state) = self.selects.get(id).cloned() {
            let labels_changed = self.select_labels.get(id) != Some(&labels);
            let index_changed = state.read(cx).selected_index(cx) != wanted;
            if labels_changed || index_changed {
                let items = labels.clone();
                state.update(cx, |state, cx| {
                    if labels_changed {
                        state.set_items(SearchableVec::new(items), window, cx);
                    }
                    // 라벨만 바뀌어도 선택 항목(옛 라벨)을 새 라벨로 고친다.
                    if index_changed || labels_changed {
                        state.set_selected_index(wanted, window, cx);
                    }
                });
                if labels_changed {
                    self.select_labels.insert(id, labels);
                }
            }
            return Select::new(&state)
                .id(id)
                .small()
                .w(px(220.))
                .placeholder(t("settings.selectPlaceholder"))
                .accessibility_label(label)
                .into_any_element();
        }
        let state =
            cx.new(|cx| SelectState::new(SearchableVec::new(labels.clone()), wanted, window, cx));
        let values: Vec<T> = def.options.iter().map(|(_, value)| *value).collect();
        let get = def.get;
        let set = def.set;
        let effect = def.effect;
        let sub = cx.subscribe_in(
            &state,
            window,
            move |this, emitter, event: &SelectEvent<SearchableVec<SharedString>>, window, cx| {
                let SelectEvent::Confirm(_) = event;
                let Some(selected) = emitter.read(cx).selected_index(cx) else {
                    return;
                };
                let Some(&value) = values.get(selected.row) else {
                    return;
                };
                if get(&SettingsStore::global(cx).settings) == value {
                    return;
                }
                this.apply(|settings| set(settings, value), effect, window, cx);
            },
        );
        self.select_subs.push(sub);
        self.selects.insert(id, state.clone());
        self.select_labels.insert(id, labels);
        Select::new(&state)
            .id(id)
            .small()
            .w(px(220.))
            .placeholder(t("settings.selectPlaceholder"))
            .accessibility_label(label)
            .into_any_element()
    }

    /// 테마는 `Settings`가 아니라 저장소의 별도 키라 전용 드롭다운을 둔다.
    fn theme_select(
        &mut self,
        label: SharedString,
        current: ThemeChoice,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        const ID: &str = "theme";
        const OPTIONS: &[(&str, ThemeChoice)] = &[
            ("settings.theme.system", ThemeChoice::System),
            ("settings.theme.light", ThemeChoice::Light),
            ("settings.theme.dark", ThemeChoice::Dark),
        ];
        let labels: Vec<SharedString> = OPTIONS.iter().map(|(key, _)| t(key)).collect();
        let wanted = OPTIONS
            .iter()
            .position(|(_, value)| *value == current)
            .map(IndexPath::new);
        if let Some(state) = self.selects.get(ID).cloned() {
            let labels_changed = self.select_labels.get(ID) != Some(&labels);
            let index_changed = state.read(cx).selected_index(cx) != wanted;
            if labels_changed || index_changed {
                let items = labels.clone();
                state.update(cx, |state, cx| {
                    if labels_changed {
                        state.set_items(SearchableVec::new(items), window, cx);
                    }
                    // 라벨만 바뀌어도 선택 항목(옛 라벨)을 새 라벨로 고친다.
                    if index_changed || labels_changed {
                        state.set_selected_index(wanted, window, cx);
                    }
                });
                if labels_changed {
                    self.select_labels.insert(ID, labels);
                }
            }
            return Select::new(&state)
                .id(ID)
                .small()
                .w(px(220.))
                .placeholder(t("settings.selectPlaceholder"))
                .accessibility_label(label)
                .into_any_element();
        }
        let state =
            cx.new(|cx| SelectState::new(SearchableVec::new(labels.clone()), wanted, window, cx));
        let values: Vec<ThemeChoice> = OPTIONS.iter().map(|(_, value)| *value).collect();
        let sub = cx.subscribe_in(
            &state,
            window,
            move |this, emitter, event: &SelectEvent<SearchableVec<SharedString>>, window, cx| {
                let SelectEvent::Confirm(_) = event;
                let Some(selected) = emitter.read(cx).selected_index(cx) else {
                    return;
                };
                let Some(&value) = values.get(selected.row) else {
                    return;
                };
                if SettingsStore::global(cx).theme == value {
                    return;
                }
                SettingsStore::set_theme(cx, value);
                this.apply(|_| {}, Effect::Theme, window, cx);
            },
        );
        self.select_subs.push(sub);
        self.selects.insert(ID, state.clone());
        self.select_labels.insert(ID, labels);
        Select::new(&state)
            .id(ID)
            .small()
            .w(px(220.))
            .placeholder(t("settings.selectPlaceholder"))
            .accessibility_label(label)
            .into_any_element()
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

    /// 항목에 연결된 단축키를 `Kbd` 배지로 함께 보여준다. 재할당하면 바로 따라간다.
    fn with_shortcut(&self, title: SharedString, action: &str, cx: &App) -> AnyElement {
        let spec = SettingsStore::global(cx)
            .settings
            .shortcuts
            .get(action)
            .cloned()
            .unwrap_or_default();
        let kbd = if spec.is_empty() {
            None
        } else {
            keys::kbd_for_spec(&spec)
        };
        h_flex()
            .gap_1p5()
            .items_center()
            .child(div().child(title))
            .when_some(kbd, |row, kbd| row.child(kbd))
            .into_any_element()
    }

    // ----- 탭 -----

    fn general_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let store = SettingsStore::global(cx);
        let s = store.settings.clone();
        let theme = store.theme;
        v_flex()
            .gap_4()
            .child(setting_row(
                t("settings.language.title"),
                Some(t("settings.language.desc")),
                self.select(
                    "language",
                    t("settings.language.title"),
                    ChoiceDef {
                        current: s.language,
                        options: &[
                            ("settings.language.ko", Language::Ko),
                            ("settings.language.en", Language::En),
                        ],
                        get: |settings: &Settings| settings.language,
                        set: |settings, value| settings.language = value,
                        effect: Effect::Language,
                    },
                    window,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.theme.title"),
                Some(t("settings.theme.desc")),
                self.theme_select(t("settings.theme.title"), theme, window, cx),
                cx,
            ))
            .child(setting_row(
                t("settings.nav.title"),
                Some(t("settings.nav.desc")),
                self.select(
                    "loop",
                    t("settings.nav.title"),
                    ChoiceDef {
                        current: s.loop_navigation,
                        options: &[("settings.nav.stop", false), ("settings.nav.loop", true)],
                        get: |settings: &Settings| settings.loop_navigation,
                        set: |settings, value| settings.loop_navigation = value,
                        effect: Effect::Redraw,
                    },
                    window,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.skipBroken.title"),
                Some(t("settings.skipBroken.desc")),
                self.select(
                    "skip-broken",
                    t("settings.skipBroken.title"),
                    ChoiceDef {
                        current: s.skip_broken_files,
                        options: &[
                            ("settings.skipBroken.off", false),
                            ("settings.skipBroken.on", true),
                        ],
                        get: |settings: &Settings| settings.skip_broken_files,
                        set: |settings, value| settings.skip_broken_files = value,
                        effect: Effect::Redraw,
                    },
                    window,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.startup.title"),
                Some(t("settings.startup.desc")),
                self.select(
                    "startup",
                    t("settings.startup.title"),
                    ChoiceDef {
                        current: s.auto_open_last_file,
                        options: &[
                            ("settings.startup.home", false),
                            ("settings.startup.last", true),
                        ],
                        get: |settings: &Settings| settings.auto_open_last_file,
                        set: |settings, value| settings.auto_open_last_file = value,
                        effect: Effect::Redraw,
                    },
                    window,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.recent.title"),
                Some(t("settings.recent.desc")),
                self.select(
                    "recent",
                    t("settings.recent.title"),
                    ChoiceDef {
                        current: s.record_recent_files,
                        options: &[("settings.recent.on", true), ("settings.recent.off", false)],
                        get: |settings: &Settings| settings.record_recent_files,
                        set: |settings, value| settings.record_recent_files = value,
                        effect: Effect::Redraw,
                    },
                    window,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                self.with_shortcut(t("settings.delete.title"), "trashFile", cx),
                Some(t("settings.delete.desc")),
                self.toggle(
                    "trash-no-confirm",
                    s.trash_without_confirm,
                    |settings, value| settings.trash_without_confirm = value,
                    Effect::Redraw,
                    cx,
                ),
                cx,
            ))
            .when(crate::platform::ALWAYS_ON_TOP_SUPPORTED, |section| {
                section.child(setting_row(
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
            })
            .child(setting_row(
                t("settings.update.autoTitle"),
                Some(t("settings.update.autoDesc")),
                self.toggle(
                    "auto-check-updates",
                    s.auto_check_updates,
                    |settings, value| settings.auto_check_updates = value,
                    Effect::Redraw,
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
                if self
                    .view
                    .read_with(cx, |view, _| view.update_checking)
                    .unwrap_or(false)
                {
                    // 새 버전 확인 응답을 기다리는 동안 버튼 대신 로딩 표시를 둔다.
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(Spinner::new().small())
                        .child(div().text_sm().child(t("settings.update.checking")))
                        .into_any_element()
                } else {
                    Button::new("check-update")
                        .small()
                        .outline()
                        .label(t("settings.update.check"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.view
                                .update(cx, |view, cx| view.check_updates(window, cx))
                                .ok();
                            // 확인 상태로 바로 다시 그린다.
                            cx.notify();
                        }))
                        .into_any_element()
                },
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

    fn view_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let s = SettingsStore::global(cx).settings.clone();
        let bg_plain = t("settings.bg.title");
        let bg_title = self.with_shortcut(bg_plain.clone(), "cycleBackground", cx);
        v_flex()
            .gap_4()
            .child(setting_row(
                t("settings.view.title"),
                Some(t("settings.view.desc")),
                self.select(
                    "view-mode",
                    t("settings.view.title"),
                    ChoiceDef {
                        current: s.view_mode,
                        options: &[
                            ("settings.view.single", ViewMode::Single),
                            ("settings.view.ltr", ViewMode::LeftToRight),
                            ("settings.view.rtl", ViewMode::RightToLeft),
                            ("settings.view.webtoon", ViewMode::Webtoon),
                        ],
                        get: |settings: &Settings| settings.view_mode,
                        set: |settings, value| settings.view_mode = value,
                        effect: Effect::Layout,
                    },
                    window,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.imageRendering.title"),
                Some(t("settings.imageRendering.desc")),
                self.select(
                    "scaling",
                    t("settings.imageRendering.title"),
                    ChoiceDef {
                        current: s.image_scaling_mode,
                        options: &[
                            ("settings.imageRendering.auto", ImageScalingMode::Auto),
                            ("settings.imageRendering.smooth", ImageScalingMode::Smooth),
                            (
                                "settings.imageRendering.pixelated",
                                ImageScalingMode::Pixelated,
                            ),
                        ],
                        get: |settings: &Settings| settings.image_scaling_mode,
                        set: |settings, value| settings.image_scaling_mode = value,
                        effect: Effect::Reload,
                    },
                    window,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.wheel.plain.title"),
                Some(t("settings.wheel.plain.desc")),
                self.select(
                    "wheel-mode",
                    t("settings.wheel.plain.title"),
                    ChoiceDef {
                        current: s.wheel.mode(),
                        options: &[
                            ("settings.wheel.plain.navigate", Some(WheelMode::Navigate)),
                            ("settings.wheel.plain.zoom", Some(WheelMode::Zoom)),
                        ],
                        get: |settings: &Settings| settings.wheel.mode(),
                        set: |settings, value| {
                            if let Some(mode) = value {
                                settings.wheel.set_mode(mode);
                            }
                        },
                        effect: Effect::Redraw,
                    },
                    window,
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
                self.select(
                    "dock-position",
                    t("settings.dock.position"),
                    ChoiceDef {
                        current: s.dock_position,
                        options: &[
                            ("settings.dock.top", DockPosition::Top),
                            ("settings.dock.bottom", DockPosition::Bottom),
                            ("settings.dock.left", DockPosition::Left),
                            ("settings.dock.right", DockPosition::Right),
                        ],
                        get: |settings: &Settings| settings.dock_position,
                        set: |settings, value| settings.dock_position = value,
                        effect: Effect::Redraw,
                    },
                    window,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.dock.thumbSize"),
                None,
                self.select(
                    "dock-size",
                    t("settings.dock.thumbSize"),
                    ChoiceDef {
                        current: s.dock_thumb_size,
                        options: &[
                            ("settings.dock.thumbS", DockThumbSize::S),
                            ("settings.dock.thumbM", DockThumbSize::M),
                            ("settings.dock.thumbL", DockThumbSize::L),
                        ],
                        get: |settings: &Settings| settings.dock_thumb_size,
                        set: |settings, value| settings.dock_thumb_size = value,
                        effect: Effect::Redraw,
                    },
                    window,
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
                bg_title,
                Some(t("settings.bg.desc")),
                self.select(
                    "background",
                    bg_plain,
                    ChoiceDef {
                        current: s.viewer_background,
                        options: &[
                            ("settings.bg.theme", ViewerBackground::Theme),
                            ("settings.bg.black", ViewerBackground::Black),
                            ("settings.bg.white", ViewerBackground::White),
                            ("settings.bg.checker", ViewerBackground::Checker),
                        ],
                        get: |settings: &Settings| settings.viewer_background,
                        set: |settings, value| settings.viewer_background = value,
                        effect: Effect::Redraw,
                    },
                    window,
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

    fn list_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let s = SettingsStore::global(cx).settings.clone();
        v_flex()
            .gap_4()
            .child(setting_row(
                t("settings.sort.title"),
                Some(t("settings.sort.desc")),
                self.select(
                    "sort-key",
                    t("settings.sort.title"),
                    ChoiceDef {
                        current: s.sort_key,
                        options: &[
                            ("settings.sort.name", SortKey::Name),
                            ("settings.sort.date", SortKey::Date),
                            ("settings.sort.size", SortKey::Size),
                        ],
                        get: |settings: &Settings| settings.sort_key,
                        set: |settings, value| settings.sort_key = value,
                        effect: Effect::Relist,
                    },
                    window,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.sort.title"),
                None,
                self.select(
                    "sort-order",
                    t("settings.sort.title"),
                    ChoiceDef {
                        current: s.sort_descending,
                        options: &[
                            ("settings.sort.asc", false),
                            ("settings.sort.descOrder", true),
                        ],
                        get: |settings: &Settings| settings.sort_descending,
                        set: |settings, value| settings.sort_descending = value,
                        effect: Effect::Relist,
                    },
                    window,
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
        window.open_alert_dialog(cx, move |alert, window, _cx| {
            let _guard = &guard;
            let panel = panel.clone();
            alert
                .margin_top(centered_margin_top(window, ALERT_HEIGHT))
                .title(t("confirm.cache.clearAll.title"))
                .description(t("confirm.cache.clearAll.message"))
                // Enter로도 지운다. gpui-kit 다이얼로그는 `Enter`를 확인 동작으로
                // 받는다(`Confirm` 액션). 이 창은 확인 동작에 삭제를 건다.
                // 창을 먼저 닫고 지운다: 결과 알림(경고창)이 위에 쌓일 수 있는데,
                // 닫기를 나중에 하면 그 알림이 대신 닫히고 이 창이 남는다.
                .confirm()
                .ok_text(t("settings.cacheManagement.clearAll"))
                .ok_variant(ButtonVariant::Danger)
                .cancel_text(t("dialog.cancel"))
                .on_ok(move |_, window, cx| {
                    window.close_dialog(cx);
                    panel.update(cx, |this, cx| this.clear_cache(CacheScope::All, window, cx));
                    // 창은 위에서 직접 닫았으니 다시 닫지 않게 false를 돌린다.
                    false
                })
        });
    }

    fn cache_section(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let body: AnyElement = match &self.cache {
            Loadable::Idle | Loadable::Loading => v_flex()
                .gap_2()
                .child(Skeleton::new().w(px(220.)))
                .child(Skeleton::new().secondary().w(px(160.)))
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child(t("settings.cacheManagement.loading")),
                )
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
                let persistent_unavailable = !stats.persistent_available;
                let mode_key = if persistent_unavailable {
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
                    // 저장 방식을 쓸 수 없는 상태는 놓치기 쉬운 경고라 Alert로 보여준다.
                    .child(if persistent_unavailable {
                        Alert::warning("cache-storage-note", t(mode_key))
                            .small()
                            .into_any_element()
                    } else {
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(t(mode_key))
                            .into_any_element()
                    })
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
        GroupBox::new()
            .id("cache-management")
            .outline()
            .title(
                h_flex()
                    .w_full()
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
            .into_any_element()
    }

    fn performance_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let s = SettingsStore::global(cx).settings.clone();
        v_flex()
            .gap_4()
            .child(setting_row(
                t("settings.cache.title"),
                Some(t("settings.cache.desc")),
                self.select(
                    "cache-mode",
                    t("settings.cache.title"),
                    ChoiceDef {
                        current: s.cache_mode,
                        options: &[
                            ("settings.cache.off", CacheMode::Off),
                            ("settings.cache.nearby", CacheMode::Nearby),
                            ("settings.cache.extended", CacheMode::Extended),
                            ("settings.cache.gb1", CacheMode::Memory1Gb),
                            ("settings.cache.gb2", CacheMode::Memory2Gb),
                        ],
                        get: |settings: &Settings| settings.cache_mode,
                        set: |settings, value| settings.cache_mode = value,
                        effect: Effect::Redraw,
                    },
                    window,
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
                self.select(
                    "resolution",
                    t("settings.resolution.title"),
                    ChoiceDef {
                        current: s.max_resolution,
                        options: &[
                            ("settings.resolution.original", MaxResolution::Original),
                            ("settings.resolution.4k", MaxResolution::FourK),
                            ("settings.resolution.1080p", MaxResolution::FullHd),
                        ],
                        get: |settings: &Settings| settings.max_resolution,
                        set: |settings, value| settings.max_resolution = value,
                        effect: Effect::Reload,
                    },
                    window,
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
        let pressed_keystroke = keys::keystroke_for_spec(&spec);
        let pressed_label: SharedString = pressed_keystroke
            .as_ref()
            .map(gpui_kit::component::kbd::Kbd::format)
            .unwrap_or_else(|| spec.clone())
            .into();
        let pressed_kbd = pressed_keystroke.clone();
        window.open_alert_dialog(cx, move |alert, window, _cx| {
            let _guard = &guard;
            let panel = panel.clone();
            let cancelled = panel.clone();
            let action = action.clone();
            let spec = spec.clone();
            alert
                .margin_top(centered_margin_top(window, ALERT_HEIGHT))
                .title(t("settings.shortcuts.conflictTitle"))
                .description(t_with(
                    "settings.shortcuts.conflictDesc",
                    &[("key", &pressed_label), ("action", &other_label)],
                ))
                .when_some(pressed_kbd.clone(), |this, stroke| {
                    this.child(
                        gpui_kit::component::h_flex()
                            .w_full()
                            .justify_center()
                            .child(gpui_kit::component::kbd::Kbd::new(stroke)),
                    )
                })
                .confirm()
                .ok_text(t("settings.shortcuts.replace"))
                .cancel_text(t("settings.shortcuts.cancel"))
                // Enter로도 교체한다(확인 동작에 교체를 건다).
                .on_ok(move |_, window, cx| {
                    panel.update(cx, |this, cx| {
                        this.set_shortcut(action.clone(), spec.clone(), window, cx)
                    });
                    true
                })
                .on_cancel(move |_, _, cx| {
                    // 취소하면 캡처도 끝낸다. 남겨 두면 다음 키가 바인딩된다.
                    cancelled.update(cx, |this, cx| {
                        this.capturing = None;
                        cx.notify();
                    });
                    true
                })
        });
    }

    fn shortcuts_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
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
        type WheelGet = fn(&Settings) -> WheelAction;
        let wheel_slots: [(&'static str, &'static str, WheelAction, WheelGet, WheelSet); 8] = [
            (
                "w-up",
                "wheelUp",
                s.wheel.wheel_up,
                (|s: &Settings| s.wheel.wheel_up) as WheelGet,
                (|s, v| s.wheel.wheel_up = v) as WheelSet,
            ),
            (
                "w-down",
                "wheelDown",
                s.wheel.wheel_down,
                (|s: &Settings| s.wheel.wheel_down) as WheelGet,
                (|s, v| s.wheel.wheel_down = v) as WheelSet,
            ),
            (
                "w-c-up",
                "ctrl+wheelUp",
                s.wheel.ctrl_wheel_up,
                (|s: &Settings| s.wheel.ctrl_wheel_up) as WheelGet,
                (|s, v| s.wheel.ctrl_wheel_up = v) as WheelSet,
            ),
            (
                "w-c-down",
                "ctrl+wheelDown",
                s.wheel.ctrl_wheel_down,
                (|s: &Settings| s.wheel.ctrl_wheel_down) as WheelGet,
                (|s, v| s.wheel.ctrl_wheel_down = v) as WheelSet,
            ),
            (
                "w-s-up",
                "shift+wheelUp",
                s.wheel.shift_wheel_up,
                (|s: &Settings| s.wheel.shift_wheel_up) as WheelGet,
                (|s, v| s.wheel.shift_wheel_up = v) as WheelSet,
            ),
            (
                "w-s-down",
                "shift+wheelDown",
                s.wheel.shift_wheel_down,
                (|s: &Settings| s.wheel.shift_wheel_down) as WheelGet,
                (|s, v| s.wheel.shift_wheel_down = v) as WheelSet,
            ),
            (
                "w-a-up",
                "alt+wheelUp",
                s.wheel.alt_wheel_up,
                (|s: &Settings| s.wheel.alt_wheel_up) as WheelGet,
                (|s, v| s.wheel.alt_wheel_up = v) as WheelSet,
            ),
            (
                "w-a-down",
                "alt+wheelDown",
                s.wheel.alt_wheel_down,
                (|s: &Settings| s.wheel.alt_wheel_down) as WheelGet,
                (|s, v| s.wheel.alt_wheel_down = v) as WheelSet,
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
                        let label: SharedString = if waiting {
                            t("settings.shortcuts.capturing")
                        } else if spec.is_empty() {
                            t("settings.shortcuts.unbound")
                        } else {
                            // 표시는 설정 표기가 아니라 다른 곳의 배지와 같은 키 캡션 형식을 쓴다.
                            match keys::keystroke_for_spec(&spec) {
                                Some(stroke) => Kbd::format(&stroke).into(),
                                None => spec.clone().into(),
                            }
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
            .children(
                wheel_slots
                    .into_iter()
                    .map(|(id, slot, current, get, set)| {
                        let title = t(&format!("settings.wheel.slot.{slot}"));
                        setting_row(
                            title.clone(),
                            None,
                            self.select(
                                id,
                                title,
                                ChoiceDef {
                                    current,
                                    options: &wheel_options,
                                    get,
                                    set,
                                    effect: Effect::Redraw,
                                },
                                window,
                                cx,
                            ),
                            cx,
                        )
                    }),
            )
            .child(heading(t("settings.mouse.title")))
            .child(setting_row(
                t("settings.mouse.trigger.leftDrag"),
                None,
                self.select(
                    "m-drag",
                    t("settings.mouse.trigger.leftDrag"),
                    ChoiceDef {
                        current: s.mouse.left_drag,
                        options: &drag_options,
                        get: (|settings: &Settings| settings.mouse.left_drag)
                            as fn(&Settings) -> MouseAction,
                        set: |settings, value| settings.mouse.left_drag = value,
                        effect: Effect::Redraw,
                    },
                    window,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.mouse.trigger.middleClick"),
                None,
                self.select(
                    "m-middle",
                    t("settings.mouse.trigger.middleClick"),
                    ChoiceDef {
                        current: s.mouse.middle_click,
                        options: &click_options,
                        get: (|settings: &Settings| settings.mouse.middle_click)
                            as fn(&Settings) -> MouseAction,
                        set: |settings, value| settings.mouse.middle_click = value,
                        effect: Effect::Redraw,
                    },
                    window,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.mouse.trigger.doubleClick"),
                None,
                self.select(
                    "m-double",
                    t("settings.mouse.trigger.doubleClick"),
                    ChoiceDef {
                        current: s.mouse.double_click,
                        options: &click_options,
                        get: (|settings: &Settings| settings.mouse.double_click)
                            as fn(&Settings) -> MouseAction,
                        set: |settings, value| settings.mouse.double_click = value,
                        effect: Effect::Redraw,
                    },
                    window,
                    cx,
                ),
                cx,
            ))
            .child(setting_row(
                t("settings.mouse.trigger.rightClick"),
                (s.mouse.right_click != MouseAction::ContextMenu)
                    .then(|| t("settings.mouse.rightClickWarning")),
                self.select(
                    "m-right",
                    t("settings.mouse.trigger.rightClick"),
                    ChoiceDef {
                        current: s.mouse.right_click,
                        options: &click_options,
                        get: (|settings: &Settings| settings.mouse.right_click)
                            as fn(&Settings) -> MouseAction,
                        set: |settings, value| settings.mouse.right_click = value,
                        effect: Effect::Redraw,
                    },
                    window,
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
                    .child(if dll_exists {
                        Alert::info("psd-thumb-note", t("settings.thumb.note"))
                            .small()
                            .into_any_element()
                    } else {
                        Alert::warning("psd-thumb-note", t("settings.thumb.dllMissing"))
                            .small()
                            .into_any_element()
                    })
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

    /// 설정을 기본값으로 되돌린다. 언어 설정은 유지한다.
    fn reset_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply(
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
        self.view
            .update(cx, |view, cx| {
                view.settings_changed(Effect::AlwaysOnTop, window, cx);
                view.settings_changed(Effect::Relist, window, cx);
            })
            .ok();
        window.toast(Toast::success(t("toast.settings.resetDone")), cx);
    }

    fn confirm_reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let guard = self.nested_guard();
        let panel = cx.entity();
        window.open_alert_dialog(cx, move |alert, window, _cx| {
            let _guard = &guard;
            let panel = panel.clone();
            alert
                .margin_top(centered_margin_top(window, ALERT_HEIGHT))
                .title(t("confirm.resetSettings.title"))
                .description(t("confirm.resetSettings.message"))
                // Enter로도 초기화한다(확인 동작에 초기화를 건다).
                // 창을 먼저 닫고 초기화한다: 초기화는 알림(경고창)을 띄우는데,
                // 닫기를 나중에 하면 그 알림이 대신 닫히고 이 창이 남는다.
                .confirm()
                .ok_text(t("settings.reset"))
                .ok_variant(ButtonVariant::Danger)
                .cancel_text(t("dialog.cancel"))
                .on_ok(move |_, window, cx| {
                    window.close_dialog(cx);
                    panel.update(cx, |this, cx| {
                        this.reset_settings(window, cx);
                    });
                    // 창은 위에서 직접 닫았으니 다시 닫지 않게 false를 돌린다.
                    false
                })
        });
    }
}

impl Render for SettingsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.tab {
            0 => self.general_tab(window, cx),
            1 => self.view_tab(window, cx),
            2 => self.list_tab(window, cx),
            3 => self.performance_tab(window, cx),
            4 => self.shortcuts_tab(window, cx),
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
