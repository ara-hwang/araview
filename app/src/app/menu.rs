//! 동작 실행, 컨텍스트 메뉴, 명령 팔레트, 표지 지정(SPEC §8, §14).

use crate::toast::{Toast, WindowToast};
use gpui_kit::component::command::{Command, CommandGroup, CommandItem, CommandState};
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::component::{ActiveTheme as _, WindowExt as _, v_flex};
use gpui_kit::*;

use araview_core::ops;

use super::AraView;
use super::ocr::OcrOutcome;
use super::settings_panel::Effect;
use super::viewport::WT_KEY_SCROLL;
use crate::dialog::{BackdropClick, centered, watch_backdrop_click};
use crate::geometry::{PAN_STEP, ZOOM_STEP};
use crate::i18n::{t, t_en};
use crate::keys;
use crate::layout;
use crate::platform;
use crate::settings::{FitMode, ImageScalingMode, SettingsStore, ViewMode};

/// 단축키가 없는 팔레트 전용 명령.
const OPEN_SETTINGS: &str = "openSettings";
const CHECK_UPDATES: &str = "checkUpdates";
const TOGGLE_DOCK: &str = "toggleDock";
const TOGGLE_COVER: &str = "toggleCover";

/// 우클릭 메뉴의 이미지 표시 하위 메뉴 항목. 설정 패널과 같은 라벨을 쓴다.
const SCALING_MODES: &[(&str, ImageScalingMode)] = &[
    ("settings.imageRendering.auto", ImageScalingMode::Auto),
    ("settings.imageRendering.smooth", ImageScalingMode::Smooth),
    (
        "settings.imageRendering.pixelated",
        ImageScalingMode::Pixelated,
    ),
];

/// 팔레트 그룹 순서: file → navigate → view → display → system.
const PALETTE_GROUPS: &[(&str, &[&str])] = &[
    (
        "palette.group.file",
        &[
            "openFile",
            "closeImage",
            "copyImage",
            "copyPath",
            "ocr",
            "ocrHighlight",
            "renameFile",
            "trashFile",
            "revealInExplorer",
            "openExternal",
            TOGGLE_COVER,
        ],
    ),
    (
        "palette.group.navigate",
        &[
            "navigatePrev",
            "navigateNext",
            "jumpPrev10",
            "jumpNext10",
            "jumpFirst",
            "jumpLast",
            "toggleGrid",
        ],
    ),
    (
        "palette.group.view",
        &[
            "zoomIn",
            "zoomOut",
            "resetView",
            "fitWidth",
            "fitHeight",
            "fitScreen",
            "rotateCW",
            "rotateCCW",
            "flipH",
            "flipV",
            "toggleGifPlayback",
            "gifPrevFrame",
            "gifNextFrame",
            "videoSeekBack",
            "videoSeekFwd",
            "videoSpeedDown",
            "videoSpeedUp",
            "videoMute",
        ],
    ),
    (
        "palette.group.display",
        &[
            "toggleExif",
            TOGGLE_DOCK,
            "cycleBackground",
            "toggleFullscreen",
            "toggleAlwaysOnTop",
        ],
    ),
    ("palette.group.system", &[OPEN_SETTINGS, CHECK_UPDATES]),
];

fn command_label_key(id: &str) -> &'static str {
    match id {
        OPEN_SETTINGS => "palette.settings",
        CHECK_UPDATES => "palette.checkUpdates",
        TOGGLE_DOCK => "menu.toggleDock",
        TOGGLE_COVER => "menu.toggleCover",
        other => keys::label_key(other).unwrap_or("palette.title"),
    }
}

impl AraView {
    /// 동작 ID를 실행한다. 단축키, 컨텍스트 메뉴, 명령 팔레트가 모두 여기로 온다.
    pub(super) fn run_action(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let webtoon = self.view_mode(cx) == ViewMode::Webtoon;
        match id {
            "navigatePrev" => self.navigate(false, window, cx),
            "navigateNext" => self.navigate(true, window, cx),
            "jumpPrev10" => self.jump_by(-10, window, cx),
            "jumpNext10" => self.jump_by(10, window, cx),
            "jumpFirst" => self.jump_to(0, window, cx),
            "jumpLast" => self.jump_to(self.list.len().saturating_sub(1), window, cx),
            "panLeft" => self.pan_by(PAN_STEP, 0.0, window, cx),
            "panRight" => self.pan_by(-PAN_STEP, 0.0, window, cx),
            "panUp" if webtoon => self.webtoon_scroll_smooth(-WT_KEY_SCROLL, window, cx),
            "panDown" if webtoon => self.webtoon_scroll_smooth(WT_KEY_SCROLL, window, cx),
            "panUp" => self.pan_by(0.0, PAN_STEP, window, cx),
            "panDown" => self.pan_by(0.0, -PAN_STEP, window, cx),
            "zoomIn" => self.zoom_by(ZOOM_STEP, None, cx),
            "zoomOut" => self.zoom_by(1.0 / ZOOM_STEP, None, cx),
            "resetView" => self.reset_view(window, cx),
            "fitWidth" => self.set_fit_mode(FitMode::Width, cx),
            "fitHeight" => self.set_fit_mode(FitMode::Height, cx),
            "fitScreen" => self.set_fit_mode(FitMode::Screen, cx),
            "openFile" => self.prompt_open(window, cx),
            "closeImage" => self.close_image(window, cx),
            "toggleExif" => self.toggle_info(window, cx),
            "rotateCW" => self.rotate(90, window, cx),
            "rotateCCW" => self.rotate(270, window, cx),
            "flipH" => self.flip(true, window, cx),
            "flipV" => self.flip(false, window, cx),
            "toggleFullscreen" => window.toggle_fullscreen(),
            "toggleAlwaysOnTop" => self.toggle_always_on_top(window, cx),
            "copyImage" => self.copy_image(window, cx),
            "trashFile" => self.confirm_trash(window, cx),
            "revealInExplorer" => self.reveal(window, cx),
            "openExternal" => self.open_external(window, cx),
            "cycleBackground" => {
                SettingsStore::update(cx, |settings| {
                    settings.viewer_background = settings.viewer_background.next();
                });
                cx.notify();
            }
            "renameFile" => self.open_rename_dialog(window, cx),
            "copyPath" => self.copy_path(window, cx),
            "ocr" => self.recognize_text(OcrOutcome::Copy, window, cx),
            "ocrHighlight" => self.toggle_text_highlight(window, cx),
            "togglePalette" => self.open_palette(window, cx),
            "toggleGrid" => self.toggle_grid(window, cx),
            "toggleGifPlayback" => self.toggle_playback(cx),
            "gifPrevFrame" => self.step_frame(-1, cx),
            "gifNextFrame" => self.step_frame(1, cx),
            "videoSeekBack" => self.video_seek_by(-10_000, cx),
            "videoSeekFwd" => self.video_seek_by(10_000, cx),
            "videoSpeedDown" => self.video_step_speed(-1, cx),
            "videoSpeedUp" => self.video_step_speed(1, cx),
            "videoMute" => self.toggle_video_mute(cx),
            OPEN_SETTINGS => self.open_settings(window, cx),
            CHECK_UPDATES => self.check_updates(window, cx),
            TOGGLE_DOCK => {
                SettingsStore::update(cx, |settings| {
                    settings.dock_visible = !settings.dock_visible;
                });
                cx.notify();
            }
            TOGGLE_COVER => self.toggle_cover(self.index, window, cx),
            "installUpdate" if cfg!(debug_assertions) => self.install_latest_update(window, cx),
            "viewSingle" => self.set_view_mode(ViewMode::Single, window, cx),
            "viewLtr" => self.set_view_mode(ViewMode::LeftToRight, window, cx),
            "viewRtl" => self.set_view_mode(ViewMode::RightToLeft, window, cx),
            "viewWebtoon" => self.set_view_mode(ViewMode::Webtoon, window, cx),
            _ => {}
        }
    }

    /// 지금 실행할 수 있는 명령인지(SPEC §14.4).
    fn command_available(&self, id: &str, cx: &App) -> bool {
        let has_image = self.picture.is_some();
        match id {
            "openFile" | "togglePalette" | "toggleFullscreen" | OPEN_SETTINGS | CHECK_UPDATES => {
                true
            }
            "toggleAlwaysOnTop" => platform::ALWAYS_ON_TOP_SUPPORTED,
            "ocr" | "ocrHighlight" => {
                has_image && self.current_page_is_full() && platform::OCR_SUPPORTED
            }
            "navigatePrev" | "navigateNext" | "jumpPrev10" | "jumpNext10" | "jumpFirst"
            | "jumpLast" | "toggleGrid" | TOGGLE_DOCK => self.list.len() > 1,
            "toggleGifPlayback" | "gifPrevFrame" | "gifNextFrame" => {
                self.has_animation(cx) || self.has_video(cx)
            }
            "videoSeekBack" | "videoSeekFwd" | "videoSpeedDown" | "videoSpeedUp" | "videoMute" => {
                self.has_video(cx)
            }
            TOGGLE_COVER => has_image && self.archive.is_some(),
            _ => has_image,
        }
    }

    /// 뷰어 본문 우클릭 메뉴. 아카이브 안에서는 끝에 표지 지정이 붙는다.
    pub(super) fn build_context_menu(
        view: &Entity<Self>,
        menu: PopupMenu,
        window: &mut Window,
        cx: &mut Context<PopupMenu>,
    ) -> PopupMenu {
        let item = |id: &'static str, window: &mut Window| {
            PopupMenuItem::new(t(command_label_key(id))).on_click(
                window.listener_for(view, move |this, _, window, cx| {
                    this.run_action(id, window, cx)
                }),
            )
        };
        let mut menu = menu
            .item(item("openFile", window))
            .item(item("closeImage", window))
            .separator()
            .item(item("zoomIn", window))
            .item(item("zoomOut", window))
            .item(item("resetView", window))
            .item(item("fitWidth", window))
            .item(item("fitHeight", window))
            .item(item("fitScreen", window))
            .separator()
            .item(item("rotateCW", window))
            .item(item("rotateCCW", window))
            .item(item("flipH", window))
            .item(item("flipV", window))
            .separator()
            .submenu(t("settings.imageRendering.title"), window, cx, {
                let view = view.clone();
                move |submenu, window, cx| {
                    let current = SettingsStore::global(cx).settings.image_scaling_mode;
                    SCALING_MODES.iter().fold(submenu, |submenu, &(key, mode)| {
                        submenu.item(
                            PopupMenuItem::new(t(key))
                                .checked(current == mode)
                                .on_click(window.listener_for(
                                    &view,
                                    move |this, _, window, cx| {
                                        this.set_image_scaling_mode(mode, window, cx)
                                    },
                                )),
                        )
                    })
                }
            })
            .item(item("toggleExif", window))
            .item(item("toggleGrid", window))
            .item(item("toggleFullscreen", window))
            .separator()
            .item(item("copyImage", window))
            .item(item("copyPath", window));
        if platform::OCR_SUPPORTED {
            menu = menu
                .item(item("ocr", window))
                .item(item("ocrHighlight", window));
        }
        let mut menu = menu
            .item(item("revealInExplorer", window))
            .item(item("openExternal", window))
            .item(item("renameFile", window))
            .item(item("trashFile", window));
        let this = view.read(cx);
        if this.archive.is_some()
            && let Some(page) = this.context_page
        {
            let is_cover = this.cover(cx).pages().contains(&page);
            let key = if is_cover {
                "menu.unsetCover"
            } else {
                "menu.setCover"
            };
            menu = menu.separator().item(PopupMenuItem::new(t(key)).on_click(
                window.listener_for(view, move |this, _, window, cx| {
                    this.toggle_cover(page, window, cx)
                }),
            ));
        }
        menu
    }

    /// 이미지 표시 방식을 바꾼다(SPEC §18). 설정 패널의 같은 항목과 같은 값을 저장한다.
    fn set_image_scaling_mode(
        &mut self,
        mode: ImageScalingMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let settings = &SettingsStore::global(cx).settings;
        let previous = settings.image_scaling_mode;
        if previous == mode {
            return;
        }
        let capped = settings.max_resolution.max_side().is_some();
        SettingsStore::update(cx, |settings| settings.image_scaling_mode = mode);
        // 해상도 상한 사본의 저장 형식(JPEG/PNG)이 바뀔 때만 다시 디코드한다. 다시
        // 디코드하면 배율이 처음으로 돌아가므로, 나머지 경우는 다시 그리기만 한다.
        let sidecar_changed = capped
            && ops::parse_image_scaling_mode(Some(previous.as_str().to_owned()))
                != ops::parse_image_scaling_mode(Some(mode.as_str().to_owned()));
        let effect = if sidecar_changed {
            Effect::Reload
        } else {
            Effect::Redraw
        };
        self.settings_changed(effect, window, cx);
    }

    /// `index` 페이지의 표지 지정을 뒤집어 ComicInfo.xml에 쓴다.
    pub(super) fn toggle_cover(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(archive) = self.archive.clone() else {
            return;
        };
        let current: Vec<usize> = self.cover(cx).pages().collect();
        let was_cover = current.contains(&index);
        let next: Vec<u32> = layout::toggle_cover_page(&current, index)
            .into_iter()
            .filter_map(|page| u32::try_from(page).ok())
            .collect();
        let generation = self.list_gen;
        let task =
            cx.background_spawn(async move { ops::set_comic_cover_pages_impl(&archive, &next) });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                if this.list_gen != generation {
                    return;
                }
                match result {
                    Ok(comic) => {
                        this.comic = Some(comic);
                        this.comic_failed = false;
                        let key = if was_cover {
                            "toast.cover.unset"
                        } else {
                            "toast.cover.set"
                        };
                        window.toast(Toast::success(t(key)), cx);
                        // 배치가 바뀌었으므로 그 페이지의 쌍 시작으로 다시 맞춘다.
                        this.index = index;
                        this.view_mode_changed(window, cx);
                    }
                    Err(error) => {
                        window.toast(Toast::error(error.message).title(t("toast.cover.fail")), cx)
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // 그룹별로 지금 쓸 수 있는 명령만 남긴다. 확정 시 인덱스로 ID를 되찾는다.
        let mut groups: Vec<(&'static str, Vec<&'static str>)> = PALETTE_GROUPS
            .iter()
            .map(|(label, ids)| {
                (
                    *label,
                    ids.iter()
                        .copied()
                        .filter(|id| self.command_available(id, cx))
                        .collect::<Vec<_>>(),
                )
            })
            .filter(|(_, ids)| !ids.is_empty())
            .collect();
        // 최근에 실행한 명령을 맨 위 그룹으로 보여준다.
        let recent: Vec<&'static str> = SettingsStore::global(cx)
            .palette_mru
            .iter()
            .filter_map(|id| {
                groups
                    .iter()
                    .flat_map(|(_, ids)| ids.iter().copied())
                    .find(|known| *known == id.as_str())
            })
            .collect();
        if !recent.is_empty() {
            groups.insert(0, ("palette.recent", recent));
        }
        let shortcuts = SettingsStore::global(cx).settings.shortcuts.clone();
        let state = cx.new(|cx| CommandState::new(window, cx));
        let view = cx.entity();
        let search = state.clone();
        // 배경 누름 판정은 팔레트가 사는 동안 유지해야 한다(다이얼로그 빌더는 다시 돈다).
        let backdrop = BackdropClick::default();
        window.open_dialog(cx, move |dialog, _window, _| {
            let state = state.clone();
            let groups = groups.clone();
            let shortcuts = shortcuts.clone();
            let view = view.clone();
            let backdrop = backdrop.clone();
            centered(dialog)
                .close_button(false)
                .p_0()
                .content(move |content, _, _| {
                    let confirm_groups = groups.clone();
                    let view = view.clone();
                    let mut command = Command::new(&state)
                        .bordered(false)
                        .placeholder(t("palette.search"));
                    for (label, ids) in &groups {
                        let mut group = CommandGroup::new().label(t(label));
                        for id in ids {
                            let key = command_label_key(id);
                            let hint = shortcuts
                                .get(*id)
                                .filter(|spec| !spec.is_empty())
                                .map(|spec| format!("  ({spec})"))
                                .unwrap_or_default();
                            // 한국어 UI에서도 영문 별칭으로 검색된다.
                            group = group.item(
                                CommandItem::new()
                                    .label(format!("{}{hint}", t(key)))
                                    .keywords([t_en(key).to_string(), (*id).to_owned()]),
                            );
                        }
                        command = command.group(group);
                    }
                    let outside_view = view.clone();
                    // 다이얼로그 배경은 클릭으로 닫히지 않으므로(`centered`의
                    // `overlay_closable(false)`), 바깥 클릭 닫기는 팔레트 영역 밖에서
                    // 직접 처리한다. 누름에서는 닫지 않는다: 배경이 캡션 판정이라 그 누름이
                    // 곧 창 이동 드래그의 시작일 수 있다.
                    let close_palette = {
                        let outside_view = outside_view.clone();
                        move |window: &mut Window, cx: &mut App| {
                            window.close_dialog(cx);
                            outside_view.update(cx, |this, cx| this.focus.focus(window, cx));
                        }
                    };
                    let press_backdrop = backdrop.clone();
                    let release_backdrop = backdrop.clone();
                    content.child(
                        div()
                            .on_mouse_down_out({
                                let close_palette = close_palette.clone();
                                move |event, window, cx| {
                                    if !press_backdrop
                                        .press(event.position, platform::window_origin(window))
                                    {
                                        return;
                                    }
                                    watch_backdrop_click(
                                        press_backdrop.clone(),
                                        window,
                                        cx,
                                        close_palette.clone(),
                                    );
                                }
                            })
                            .on_mouse_up_out(MouseButton::Left, move |event, window, cx| {
                                if release_backdrop
                                    .release(Some(event.position), platform::window_origin(window))
                                {
                                    close_palette(window, cx);
                                }
                            })
                            .child(
                                command
                                    .empty(|_, _, cx| {
                                        v_flex()
                                            .items_center()
                                            .gap_1()
                                            .py_4()
                                            .child(t("palette.noResult"))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(t("palette.noResultHint")),
                                            )
                                    })
                                    .on_confirm(move |index, window, cx| {
                                        let Some(id) = confirm_groups
                                            .get(index.section)
                                            .and_then(|(_, ids)| ids.get(index.row))
                                            .copied()
                                        else {
                                            return;
                                        };
                                        window.close_dialog(cx);
                                        SettingsStore::push_palette_mru(cx, id);
                                        view.update(cx, |this, cx| {
                                            this.focus.focus(window, cx);
                                            this.run_action(id, window, cx);
                                        });
                                    }),
                            ),
                    )
                })
        });
        // 열자마자 검색어를 칠 수 있게 검색 입력에 포커스를 준다.
        search.update(cx, |state, cx| state.focus(window, cx));
    }
}
