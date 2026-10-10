//! 메인 창 뷰: 홈, 뷰어, 헤더, 상태바와 전체 조립.

// 제어 브리지는 개발 빌드에만 들어간다.
#[cfg(debug_assertions)]
mod bridge;
mod dock;
mod file_ops;
mod grid;
mod info_panel;
mod licenses;
mod menu;
mod pages;
mod settings_panel;
pub mod system;
#[cfg(test)]
mod tests;
mod update;
mod viewport;

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use crate::smooth::SmoothScroll;
use crate::toast::{Toast, WindowToast};
use araview_core::app_error::{AppError, ErrorCode};
use araview_core::comic_info::ComicInfo;
use araview_core::file_availability::FileAvailability;
use araview_core::image::{ImageInfo, is_archive_file};
use araview_core::ops::{self, DirListOptions, DirSortKey};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::ContextMenuExt as _;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{
    ActiveTheme as _, Icon, Selectable as _, Sizable as _, TitleBar, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::geometry::Offset;
use crate::i18n::{t, t_with};
use crate::keys;
use crate::picture::{Orientation, Picture};
use crate::settings::{DockPosition, FitMode, MouseAction, SettingsStore, SortKey, ViewMode};

/// 체커 배경은 두 테마 모두 20px 타일, 흰 바탕에 `#c7c7c7`로 고정한다(DESIGN.md).
const CHECKER_CELL: f32 = 20.0;
const CHECKER_BASE: u32 = 0xffffff;
const CHECKER_ALT: u32 = 0xc7c7c7;
/// 애니메이션 컨트롤은 클러스터 폭(`GIF_CLUSTER_BASE_WIDTH` + 카운터)을 더한 폭 이상일 때만
/// 보인다(SPEC §7.5). 이 값은 클러스터를 뺀 헤더가 캡션 버튼 앞에 들어가는 폭이다.
const GIF_CONTROLS_MIN_WIDTH: f32 = 950.0;
/// 이 폭(논리 px) 이상이면 헤더 기본 버튼에 라벨을 붙인다.
/// 캡처로 잰 최소 필요 폭(1313px)에 여유를 더한 값이다.
const HEADER_LABELS_MIN_WIDTH: f32 = 1340.0;
/// 이 폭 이상이면 보기 모드/변형/상태 버튼에도 라벨을 붙인다(최소 필요 1824px).
const HEADER_EXTRA_LABELS_MIN_WIDTH: f32 = 1850.0;
/// 좁은 창에서 우선 숨길 도구 그룹의 경계(논리 px). 그룹이 보일 때 우측 메뉴와 그립이
/// Windows 캡션 버튼 3개(34px씩 102px) 앞에 들어가는 최소 폭에 약 30px 여유를 더한 값이다.
/// `header_never_overlaps_caption_controls` 테스트가 500~2000px 전체에서 이를 지킨다.
const HEADER_FIT_CONTROLS_MIN_WIDTH: f32 = 670.0;
const HEADER_VIEW_MODES_MIN_WIDTH: f32 = 810.0;
const HEADER_TRANSFORM_CONTROLS_MIN_WIDTH: f32 = 980.0;
/// 애니메이션 클러스터가 차지하는 폭(버튼 3개 + 카운터 + 간격).
const GIF_CLUSTER_BASE_WIDTH: f32 = 112.0;
/// 숨긴 크롬을 다시 보여주는 가장자리 영역(px).
const PEEK_EDGE: f32 = 6.0;
const PEEK_TOP_KEEP: f32 = 48.0;
const PEEK_BOTTOM_KEEP: f32 = 150.0;

enum ThumbState {
    Ready(PathBuf),
    Failed,
}

struct DragState {
    start: Point<Pixels>,
    origin: Offset,
    last_y: Pixels,
}

struct Listing {
    items: Vec<Arc<str>>,
    index: usize,
    /// 이어보기 기록에서 시작했는지.
    resumed: bool,
    archive: Option<Arc<str>>,
    availability: Vec<FileAvailability>,
    /// 아카이브의 ComicInfo.xml. 파싱 실패는 로드를 막지 않고 패널에 에러로만 남긴다.
    comic: Result<Option<ComicInfo>, AppError>,
}

pub struct AraView {
    focus: FocusHandle,
    /// 폴더 모드는 파일 경로, 아카이브 모드는 엔트리 이름.
    list: Arc<Vec<Arc<str>>>,
    availability: Vec<FileAvailability>,
    /// 목록이 바뀔 때마다 올린다. 늦게 도착한 비동기 결과를 버리는 데 쓴다.
    list_gen: u64,
    /// 현재 화면의 시작 인덱스.
    index: usize,
    archive: Option<Arc<str>>,
    comic: Option<ComicInfo>,
    comic_failed: bool,
    /// 아카이브를 열 때 자동으로 정한 보기 모드. 직접 고르면 해제된다.
    comic_mode: Option<ViewMode>,

    pages: HashMap<usize, pages::Page>,
    page_loading: HashSet<usize>,
    page_failed: HashMap<usize, AppError>,
    /// 가로가 더 긴 페이지. 로드하면서 알게 된다.
    wide: HashSet<usize>,
    /// 지금 화면에 올라가 있는 인덱스.
    shown: Vec<usize>,
    /// 이동한 화면이 아직 올라가지 않았는지.
    pending_commit: bool,
    last_forward: bool,
    skip_budget: usize,
    /// 마지막 이동을 시작한 시각. 이전 화면을 얼마나 붙잡았는지 잰다.
    nav_started: Instant,
    open_seq: u64,
    opening: bool,
    opening_started: Instant,
    error: Option<AppError>,

    /// 기준 그림과 그 정보(단일 보기의 표시 대상, 정보 패널·파일 작업의 대상).
    info: Option<ImageInfo>,
    picture: Option<Arc<Picture>>,
    orientation: Orientation,
    oriented: Option<Arc<Picture>>,
    orient_seq: u64,
    zoom: f32,
    position: Offset,
    /// 맞춤 잠금. 수동 줌을 하면 풀려 리사이즈해도 줌을 유지한다.
    fit_locked: bool,
    /// 읽기 영역의 창 좌표 경계. 캔버스 prepaint가 갱신한다.
    viewport: Rc<Cell<Bounds<Pixels>>>,
    drag: Option<DragState>,
    playing: bool,
    frame: usize,
    frame_started: Instant,
    /// 끝까지 재생한 횟수. 유한 반복 애니메이션을 마지막 회차에서 멈추는 데 쓴다.
    loops_done: u32,
    /// 뒤로 넘기기 전에 도착할 자리의 페이지 크기를 확인하는 중인지.
    pending_back: bool,
    pixel_hint: Option<bool>,
    pixel_hint_pending: bool,
    pixel_view: Option<viewport::PixelView>,

    /// 웹툰 스크롤 위치: 화면 맨 위에 걸친 페이지와 그 안에서 내려간 거리.
    wt_anchor: usize,
    wt_offset: f32,
    /// 부드러운 스크롤이 아직 반영하지 못한 남은 거리(px).
    wt_pending: f32,
    wt_animating: bool,
    wt_frame: Instant,
    wt_zoom: f32,
    wt_fit_width: bool,
    wt_dims: HashMap<usize, (u32, u32)>,

    thumbs: HashMap<Arc<str>, ThumbState>,
    thumbs_pending: HashSet<Arc<str>>,
    grid: Option<grid::GridState>,
    grid_open: bool,
    /// 우클릭한 지점의 페이지(표지 지정 대상).
    context_page: Option<usize>,

    info_open: bool,
    home_scroll: SmoothScroll,
    info_scroll: SmoothScroll,
    info_data: info_panel::InfoData,
    info_seq: u64,
    update_checking: bool,
    peek_top: bool,
    peek_bottom: bool,
    high_contrast: bool,
}

impl AraView {
    pub fn new(
        initial: Option<String>,
        instances: Option<Receiver<String>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let mut this = Self {
            focus,
            list: Arc::new(Vec::new()),
            availability: Vec::new(),
            list_gen: 0,
            index: 0,
            archive: None,
            comic: None,
            comic_failed: false,
            comic_mode: None,
            pages: HashMap::new(),
            page_loading: HashSet::new(),
            page_failed: HashMap::new(),
            wide: HashSet::new(),
            shown: Vec::new(),
            pending_commit: false,
            last_forward: true,
            skip_budget: 0,
            nav_started: Instant::now(),
            open_seq: 0,
            opening: false,
            opening_started: Instant::now(),
            error: None,
            info: None,
            picture: None,
            orientation: Orientation::default(),
            oriented: None,
            orient_seq: 0,
            zoom: 1.0,
            position: Offset::default(),
            fit_locked: true,
            viewport: Rc::new(Cell::new(Bounds::default())),
            drag: None,
            playing: false,
            frame: 0,
            frame_started: Instant::now(),
            loops_done: 0,
            pending_back: false,
            pixel_hint: None,
            pixel_hint_pending: false,
            pixel_view: None,
            wt_anchor: 0,
            wt_offset: 0.0,
            wt_pending: 0.0,
            wt_animating: false,
            wt_frame: Instant::now(),
            wt_zoom: 1.0,
            wt_fit_width: false,
            wt_dims: HashMap::new(),
            thumbs: HashMap::new(),
            thumbs_pending: HashSet::new(),
            grid: None,
            grid_open: false,
            context_page: None,
            info_open: false,
            home_scroll: SmoothScroll::default(),
            info_scroll: SmoothScroll::default(),
            info_data: info_panel::InfoData::default(),
            info_seq: 0,
            update_checking: false,
            peek_top: false,
            peek_bottom: false,
            high_contrast: crate::platform::high_contrast(),
        };
        system::apply_theme(SettingsStore::global(cx).theme, window, cx);
        if SettingsStore::global(cx).settings.always_on_top {
            this.apply_always_on_top(window, cx);
        }
        cx.observe_window_bounds(window, |this, window, cx| this.remember_window(window, cx))
            .detach();
        cx.observe_window_activation(window, |this, window, cx| this.recheck_contrast(window, cx))
            .detach();
        window.on_window_should_close(cx, |_, cx| {
            SettingsStore::flush(cx);
            true
        });
        if let Some(receiver) = instances {
            this.listen_for_instances(receiver, window, cx);
        }
        // 개발 빌드는 실행 중인 창을 에이전트가 직접 조작할 수 있게 제어 통로를 연다.
        #[cfg(all(debug_assertions, not(test)))]
        bridge::listen(crate::IDENTIFIER, cx.entity(), window, cx);
        let store = SettingsStore::global(cx);
        let auto_open = (store.settings.auto_open_last_file && store.settings.record_recent_files)
            .then(|| store.recent_files.first().cloned())
            .flatten();
        if let Some(path) = initial.or(auto_open) {
            this.open_path(path, window, cx);
        }
        this
    }

    /// 이미지 필터가 걸린 파일 선택 창을 연다. GPUI의 경로 선택 창에는 형식 필터가
    /// 없어 네이티브 대화상자를 직접 쓴다.
    fn prompt_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dialog = rfd::AsyncFileDialog::new()
            .add_filter(
                t("picker.images").to_string(),
                araview_core::image::SUPPORTED_EXTENSIONS,
            )
            .set_parent(&*window)
            .pick_file();
        cx.spawn_in(window, async move |this, cx| {
            let file = dialog.await?;
            let path = file.path().to_string_lossy().into_owned();
            this.update_in(cx, |this, window, cx| this.open_path(path, window, cx))
                .ok()
        })
        .detach();
    }

    /// 드롭된 경로들을 해석해 이름순 첫 항목을 연다(SPEC §4.2).
    fn open_dropped(&mut self, paths: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        let task = cx.background_spawn(async move {
            let mut resolved: Vec<String> = paths
                .iter()
                .filter_map(|path| {
                    let text = path.to_string_lossy();
                    if is_archive_file(path) {
                        Some(text.into_owned())
                    } else {
                        ops::resolve_dropped_path_impl(&text).ok()
                    }
                })
                .collect();
            resolved.sort_by_cached_key(|path| araview_core::natural_sort::natural_key(path));
            resolved
        });
        cx.spawn_in(window, async move |this, cx| {
            let resolved = task.await;
            this.update_in(cx, |this, window, cx| {
                let count = resolved.len();
                let Some(first) = resolved.into_iter().next() else {
                    window.toast(Toast::error(t("toast.drop.fail")), cx);
                    return;
                };
                if count > 1 {
                    window.toast(
                        Toast::info(t_with("toast.drop.firstOf", &[("count", &count)])),
                        cx,
                    );
                }
                this.open_path(first, window, cx);
            })
            .ok();
        })
        .detach();
    }

    fn list_options(cx: &App) -> DirListOptions {
        let settings = &SettingsStore::global(cx).settings;
        DirListOptions {
            sort_key: match settings.sort_key {
                SortKey::Name => DirSortKey::Name,
                SortKey::Date => DirSortKey::Date,
                SortKey::Size => DirSortKey::Size,
            },
            descending: settings.sort_descending,
            recursive: settings.include_subfolders,
        }
    }

    /// 화면에 보여줄 파일명. 아카이브 엔트리는 추출 파일명이 아니라 엔트리 이름을 쓴다.
    fn display_name(&self, info: &ImageInfo) -> String {
        match (&self.archive, self.list.get(self.index)) {
            (Some(_), Some(entry)) => entry.rsplit(['/', '\\']).next().unwrap_or(entry).to_owned(),
            _ => info.file_name.clone(),
        }
    }

    /// 헤더 버튼과 `1/2/3` 단축키: 맞춤 모드를 기억하고 바로 적용한다.
    fn set_fit_mode(&mut self, mode: FitMode, cx: &mut Context<Self>) {
        SettingsStore::update(cx, |settings| settings.fit_mode = mode);
        self.fit_locked = true;
        self.apply_fit(cx);
        cx.notify();
    }

    /// 파일 작업 대상: 아카이브 모드면 아카이브 자체, 아니면 연 원본 파일.
    fn target_path(&self) -> Option<String> {
        match &self.archive {
            Some(archive) => Some(archive.to_string()),
            None => self.info.as_ref().map(|info| info.source_path.clone()),
        }
    }

    fn copy_path(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.target_path() else {
            window.toast(Toast::warning(t("toast.path.empty")), cx);
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(path));
        window.toast(Toast::success(t("toast.path.done")), cx);
    }

    fn copy_image(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(info) = &self.info else {
            window.toast(Toast::warning(t("toast.copy.empty")), cx);
            return;
        };
        let path = PathBuf::from(&info.file_path);
        let task = cx.background_spawn(async move {
            let png = araview_core::clipboard_png::export_clipboard_png(&path)?;
            std::fs::read(&png.file_path)
                .map_err(|e| AppError::io("Failed to read clipboard png", e, ErrorCode::Unknown))
        });
        cx.spawn_in(window, async move |_, cx| {
            let result = task.await;
            cx.update(|window, cx| match result {
                Ok(bytes) => {
                    cx.write_to_clipboard(ClipboardItem::new_image(&Image::from_bytes(
                        ImageFormat::Png,
                        bytes,
                    )));
                    window.toast(Toast::success(t("toast.copy.done")), cx);
                }
                Err(error) => {
                    window.toast(Toast::error(error.message).title(t("toast.copy.fail")), cx)
                }
            })
            .ok();
        })
        .detach();
    }

    fn reveal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.target_path() {
            Some(path) => cx.reveal_path(Path::new(&path)),
            None => window.toast(Toast::warning(t("toast.reveal.empty")), cx),
        }
    }

    fn open_external(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.target_path() {
            Some(path) => cx.open_with_system(Path::new(&path)),
            None => window.toast(Toast::warning(t("toast.external.empty")), cx),
        }
    }

    fn render_home(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let store = SettingsStore::global(cx);
        let recent: Vec<String> = if store.settings.record_recent_files {
            store.recent_files.clone()
        } else {
            Vec::new()
        };
        let muted = cx.theme().muted_foreground;
        // 최근 파일이 많아 창보다 커져도 잘리지 않게 스크롤 영역 안에 둔다.
        // 내용이 작을 때는 가운데에 놓인다.
        self.home_scroll.area(
            "home",
            v_flex()
                .w_full()
                .min_h_full()
                .items_center()
                .justify_center()
                .gap_4()
                .p_6()
                .child(Icon::new(IconName::Image).size_12().text_color(muted))
                .child(div().text_sm().text_color(muted).child(t("home.emptyDesc")))
                .child(
                    Button::new("home-open")
                        .primary()
                        .icon(IconName::FolderOpen)
                        .label(t("home.openFile"))
                        .on_click(cx.listener(|this, _, window, cx| this.prompt_open(window, cx))),
                )
                .when(!recent.is_empty(), |home| {
                    home.child(
                        v_flex()
                            .w_full()
                            .max_w(px(560.))
                            .gap_1()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div().text_sm().child(t_with(
                                            "home.recent",
                                            &[("count", &recent.len())],
                                        )),
                                    )
                                    .child(
                                        Button::new("recent-clear")
                                            .ghost()
                                            .xsmall()
                                            .label(t("home.clearAll"))
                                            .on_click(cx.listener(|_, _, _, cx| {
                                                SettingsStore::clear_recent(cx);
                                                cx.notify();
                                            })),
                                    ),
                            )
                            .children(recent.into_iter().enumerate().map(|(ix, path)| {
                                let file = Path::new(&path);
                                let name = file
                                    .file_name()
                                    .map(|name| name.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| path.clone());
                                let parent = file
                                    .parent()
                                    .map(|dir| dir.to_string_lossy().into_owned())
                                    .unwrap_or_default();
                                let icon = if is_archive_file(file) {
                                    IconName::BookOpen
                                } else {
                                    IconName::Image
                                };
                                // 아카이브는 읽기 진도를 함께 보여준다.
                                let progress = SettingsStore::global(cx)
                                    .archive_progress(&path)
                                    .map(|progress| {
                                        t_with(
                                            "home.card.progress",
                                            &[
                                                ("index", &(progress.index + 1)),
                                                ("total", &progress.total),
                                            ],
                                        )
                                    });
                                let removed = path.clone();
                                h_flex()
                                    .w_full()
                                    .gap_1()
                                    .items_center()
                                    .child(
                                        h_flex()
                                            .id(("recent", ix))
                                            .flex_1()
                                            .min_w_0()
                                            .gap_3()
                                            .px_2()
                                            .py_1()
                                            .rounded_md()
                                            .cursor_pointer()
                                            .hover(|style| style.bg(cx.theme().muted))
                                            .child(Icon::new(icon).text_color(muted))
                                            .child(
                                                v_flex()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .overflow_hidden()
                                                            .text_ellipsis()
                                                            .whitespace_nowrap()
                                                            .child(name),
                                                    )
                                                    .when_some(progress, |column, text| {
                                                        column.child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(muted)
                                                                .child(text),
                                                        )
                                                    })
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(muted)
                                                            .overflow_hidden()
                                                            .text_ellipsis()
                                                            .whitespace_nowrap()
                                                            .child(parent),
                                                    ),
                                            )
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.open_path(path.clone(), window, cx)
                                            })),
                                    )
                                    .child(
                                        Button::new(("recent-remove", ix))
                                            .ghost()
                                            .xsmall()
                                            .icon(IconName::X)
                                            .tooltip(t("home.card.remove"))
                                            .on_click(cx.listener(move |_, _, _, cx| {
                                                SettingsStore::remove_recent(cx, &removed);
                                                cx.notify();
                                            })),
                                    )
                            })),
                    )
                }),
        )
    }

    fn render_error(&self, error: &AppError, cx: &mut Context<Self>) -> impl IntoElement {
        let kind = match error.code {
            ErrorCode::NotFound => "notFound",
            ErrorCode::Permission => "permission",
            ErrorCode::Unsupported => "unsupported",
            ErrorCode::Corrupt | ErrorCode::TooLarge => "corrupt",
            _ => "unknown",
        };
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_3()
            .p_6()
            .child(
                Icon::new(IconName::TriangleAlert)
                    .size_8()
                    .text_color(cx.theme().danger),
            )
            .child(div().text_base().child(t(&format!("error.{kind}.title"))))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t(&format!("error.{kind}.hint"))),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("error-retry")
                            .primary()
                            .label(t("viewer.error.retry"))
                            .on_click(cx.listener(|this, _, window, cx| this.reload(window, cx))),
                    )
                    .child(
                        Button::new("error-home")
                            .label(t("viewer.error.home"))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.close_image(window, cx)),
                            ),
                    ),
            )
    }

    fn render_status(&self, info: &ImageInfo, cx: &mut Context<Self>) -> impl IntoElement {
        let dimensions = self
            .picture
            .as_ref()
            .map(|picture| format!("{} × {}", picture.width, picture.height));
        h_flex()
            .w_full()
            .flex_none()
            .h(px(24.))
            .px_3()
            .gap_4()
            .items_center()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .border_t_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(self.display_name(info)),
            )
            .when(self.list.len() > 1, |row| {
                row.child(format!("{} / {}", self.index + 1, self.list.len()))
            })
            .when_some(dimensions, |row, text| row.child(text))
            .child(format!("{}%", self.zoom_percent(cx)))
            .child(format_bytes(info.file_size))
    }

    // ----- 헤더 -----

    fn icon_button(
        id: &'static str,
        icon: IconName,
        tooltip_key: &'static str,
        action: &'static str,
        cx: &mut Context<Self>,
    ) -> Button {
        Button::new(id)
            .ghost()
            .icon(icon)
            .tooltip(t(tooltip_key))
            .on_click(cx.listener(move |this, _, window, cx| this.run_action(action, window, cx)))
    }

    fn render_header(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let settings = SettingsStore::global(cx).settings.clone();
        let has_image = self.picture.is_some();
        let mode = self.view_mode(cx);
        let single = mode == ViewMode::Single;
        let animated = self.has_animation(cx);
        let frames = self
            .picture
            .as_ref()
            .map(|picture| picture.frame_count())
            .unwrap_or(1);
        // 프레임 카운터는 자릿수가 바뀌어도 옆 버튼이 밀리지 않게 폭을 고정한다.
        let counter_width = 16.0 + 14.0 * (frames.to_string().len() as f32);
        // 라벨은 실제로 들어갈 폭이 되면 바로 붙인다. 애니메이션 클러스터가
        // 보이면 그 폭만큼 자리를 비워 두고 판단한다.
        let width = f32::from(window.viewport_size().width);
        let gif_cluster_width = GIF_CLUSTER_BASE_WIDTH + counter_width;
        let show_gif_controls = animated && width >= GIF_CONTROLS_MIN_WIDTH + gif_cluster_width;
        let gif_width = if show_gif_controls {
            gif_cluster_width
        } else {
            0.0
        };
        let wide_labels = width >= HEADER_LABELS_MIN_WIDTH + gif_width;
        let extra_labels = width >= HEADER_EXTRA_LABELS_MIN_WIDTH + gif_width;
        // Below the compact toolbar's natural width, hide secondary groups in order;
        // keep the left/right menus and native caption controls outside the shrinkable area.
        let show_zoom_controls = width >= HEADER_FIT_CONTROLS_MIN_WIDTH;
        let show_fit_controls = single && show_zoom_controls;
        let show_view_modes = width >= HEADER_VIEW_MODES_MIN_WIDTH;
        let show_transform_controls = single && width >= HEADER_TRANSFORM_CONTROLS_MIN_WIDTH;
        // 창 드래그 가능 표시 그립(Tauri 시절 DotsNine 자리). 빈 div라
        // 타이틀바 드래그 영역에 포함돼 그립을 잡고 창을 옮길 수 있다.
        let grip_dot = cx.theme().muted_foreground;
        let grip = div()
            .flex()
            .flex_col()
            .h_full()
            .justify_center()
            .items_center()
            .gap(px(2.))
            .px(px(10.))
            .children((0..3).map(|_| {
                div()
                    .flex()
                    .flex_row()
                    .gap(px(2.))
                    .children((0..3).map(|_| div().size(px(2.)).rounded(px(1.)).bg(grip_dot)))
            }));
        // 타이틀 바 본문은 창 드래그 영역이다. 버튼 묶음이 마우스를 막지 않으면
        // Windows가 클릭을 캡션 드래그로 처리해 버튼이 눌리지 않는다.
        let group = || h_flex().gap_0p5().items_center().occlude().flex_none();
        let mode_button = |id: &'static str, icon: IconName, key: &'static str, value: ViewMode| {
            Button::new(id)
                .ghost()
                .icon(icon)
                .tooltip(t(key))
                .selected(mode == value)
                .when(extra_labels, |button| button.label(t(key)))
        };

        TitleBar::new().h(px(36.)).child(
            h_flex()
                .w_full()
                .min_w_0()
                .gap_2()
                .pr_2()
                .items_center()
                .child(
                    group()
                        .debug_selector(|| "header-left-menu".to_owned())
                        .child(
                            Button::new("home")
                                .ghost()
                                .icon(IconName::House)
                                .tooltip(t("header.home"))
                                .when(wide_labels, |button| button.label(t("header.home")))
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.close_image(window, cx)),
                                ),
                        )
                        .child(
                            Button::new("open")
                                .ghost()
                                .icon(IconName::FolderOpen)
                                .tooltip(t("header.open"))
                                .when(wide_labels, |button| button.label(t("header.open")))
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.prompt_open(window, cx)),
                                ),
                        ),
                )
                .when(has_image, |row| {
                    row.when(show_fit_controls, |row| {
                        row.child(
                            group()
                                .child(
                                    Self::icon_button(
                                        "fit-width",
                                        IconName::MoveHorizontal,
                                        "header.fitWidth",
                                        "fitWidth",
                                        cx,
                                    )
                                    .when(wide_labels, |button| button.label(t("header.fitWidth")))
                                    .selected(
                                        self.fit_locked && settings.fit_mode == FitMode::Width,
                                    ),
                                )
                                .child(
                                    Self::icon_button(
                                        "fit-height",
                                        IconName::MoveVertical,
                                        "header.fitHeight",
                                        "fitHeight",
                                        cx,
                                    )
                                    .when(wide_labels, |button| button.label(t("header.fitHeight")))
                                    .selected(
                                        self.fit_locked && settings.fit_mode == FitMode::Height,
                                    ),
                                )
                                .child(
                                    Self::icon_button(
                                        "fit-screen",
                                        IconName::Maximize,
                                        "header.fitScreen",
                                        "fitScreen",
                                        cx,
                                    )
                                    .when(wide_labels, |button| button.label(t("header.fitScreen")))
                                    .selected(
                                        self.fit_locked && settings.fit_mode == FitMode::Screen,
                                    ),
                                ),
                        )
                    })
                    .when(show_zoom_controls, |row| {
                        row.child(
                            group()
                                .debug_selector(|| "header-zoom".to_owned())
                                .child(
                                    Self::icon_button(
                                        "zoom-out",
                                        IconName::ZoomOut,
                                        "header.zoomOut",
                                        "zoomOut",
                                        cx,
                                    )
                                    .when(wide_labels, |button| button.label(t("header.zoomOut"))),
                                )
                                .child(
                                    div()
                                        .w(px(48.))
                                        .text_xs()
                                        .text_center()
                                        .child(format!("{}%", self.zoom_percent(cx))),
                                )
                                .child(
                                    Self::icon_button(
                                        "zoom-in",
                                        IconName::ZoomIn,
                                        "header.zoomIn",
                                        "zoomIn",
                                        cx,
                                    )
                                    .when(wide_labels, |button| button.label(t("header.zoomIn"))),
                                ),
                        )
                    })
                    .when(show_view_modes, |row| {
                        row.child(
                            group()
                                .debug_selector(|| "header-view-modes".to_owned())
                                .child(
                                    mode_button(
                                        "view-single",
                                        IconName::Image,
                                        "header.viewSingle",
                                        ViewMode::Single,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, window, cx| {
                                            this.set_view_mode(ViewMode::Single, window, cx)
                                        },
                                    )),
                                )
                                .child(
                                    mode_button(
                                        "view-ltr",
                                        IconName::BookOpen,
                                        "header.viewLtr",
                                        ViewMode::LeftToRight,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, window, cx| {
                                            this.set_view_mode(ViewMode::LeftToRight, window, cx)
                                        },
                                    )),
                                )
                                .child(
                                    mode_button(
                                        "view-rtl",
                                        IconName::BookOpenText,
                                        "header.viewRtl",
                                        ViewMode::RightToLeft,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, window, cx| {
                                            this.set_view_mode(ViewMode::RightToLeft, window, cx)
                                        },
                                    )),
                                )
                                .child(
                                    mode_button(
                                        "view-webtoon",
                                        IconName::GalleryVertical,
                                        "header.viewWebtoon",
                                        ViewMode::Webtoon,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, window, cx| {
                                            this.set_view_mode(ViewMode::Webtoon, window, cx)
                                        },
                                    )),
                                ),
                        )
                    })
                    .when(show_transform_controls, |row| {
                        row.child(
                            group()
                                .debug_selector(|| "header-transform".to_owned())
                                .child(
                                    Self::icon_button(
                                        "rotate-ccw",
                                        IconName::RotateCcw,
                                        "header.rotateCcw",
                                        "rotateCCW",
                                        cx,
                                    )
                                    .when(extra_labels, |button| {
                                        button.label(t("header.rotateCcw"))
                                    }),
                                )
                                .child(
                                    Self::icon_button(
                                        "rotate-cw",
                                        IconName::RotateCw,
                                        "header.rotateCw",
                                        "rotateCW",
                                        cx,
                                    )
                                    .when(extra_labels, |button| {
                                        button.label(t("header.rotateCw"))
                                    }),
                                )
                                .child(
                                    Self::icon_button(
                                        "flip-h",
                                        IconName::FlipHorizontal2,
                                        "header.flipH",
                                        "flipH",
                                        cx,
                                    )
                                    .when(extra_labels, |button| button.label(t("header.flipH")))
                                    .selected(self.orientation.flip_h),
                                )
                                .child(
                                    Self::icon_button(
                                        "flip-v",
                                        IconName::FlipVertical2,
                                        "header.flipV",
                                        "flipV",
                                        cx,
                                    )
                                    .when(extra_labels, |button| button.label(t("header.flipV")))
                                    .selected(self.orientation.flip_v),
                                ),
                        )
                    })
                    .when(show_gif_controls, |row| {
                        row.child(
                            group()
                                .debug_selector(|| "header-gif-controls".to_owned())
                                .child(Self::icon_button(
                                    "gif-prev",
                                    IconName::SkipBack,
                                    "menu.gifPrevFrame",
                                    "gifPrevFrame",
                                    cx,
                                ))
                                .child(Self::icon_button(
                                    "gif-play",
                                    if self.playing {
                                        IconName::Pause
                                    } else {
                                        IconName::Play
                                    },
                                    if self.playing {
                                        "menu.gifPause"
                                    } else {
                                        "menu.gifPlay"
                                    },
                                    "toggleGifPlayback",
                                    cx,
                                ))
                                .child(Self::icon_button(
                                    "gif-next",
                                    IconName::SkipForward,
                                    "menu.gifNextFrame",
                                    "gifNextFrame",
                                    cx,
                                ))
                                .child(
                                    div()
                                        .w(px(counter_width))
                                        .text_xs()
                                        .text_center()
                                        .child(format!("{}/{}", self.frame + 1, frames)),
                                ),
                        )
                    })
                    .child(
                        group()
                            .debug_selector(|| "header-center-info".to_owned())
                            .child(
                                Self::icon_button(
                                    "info",
                                    IconName::Info,
                                    "header.info",
                                    "toggleExif",
                                    cx,
                                )
                                .when(wide_labels, |button| button.label(t("header.info")))
                                .selected(self.info_open),
                            ),
                    )
                })
                .child(div().flex_1().min_w_0())
                .child(
                    group()
                        .debug_selector(|| "header-right-menu".to_owned())
                        .child(
                            Self::icon_button(
                                "palette",
                                IconName::Search,
                                "palette.open",
                                "togglePalette",
                                cx,
                            )
                            .when(wide_labels, |button| button.label(t("palette.open"))),
                        )
                        .child(
                            Self::icon_button(
                                "settings",
                                IconName::Settings,
                                "header.settings",
                                "openSettings",
                                cx,
                            )
                            .when(wide_labels, |button| button.label(t("header.settings"))),
                        )
                        .child(
                            Self::icon_button(
                                "on-top",
                                if settings.always_on_top {
                                    IconName::Pin
                                } else {
                                    IconName::PinOff
                                },
                                "header.alwaysOnTop",
                                "toggleAlwaysOnTop",
                                cx,
                            )
                            .when(extra_labels, |button| button.label(t("header.onTop")))
                            .selected(settings.always_on_top),
                        )
                        .when(has_image, |row| {
                            row.child(
                                Button::new("hide-bar")
                                    .ghost()
                                    .icon(if settings.menu_bar_hidden {
                                        IconName::Eye
                                    } else {
                                        IconName::EyeOff
                                    })
                                    .tooltip(t(if settings.menu_bar_hidden {
                                        "header.showMenuBar"
                                    } else {
                                        "header.hideMenuBar"
                                    }))
                                    .when(extra_labels, |button| {
                                        button.label(t(if settings.menu_bar_hidden {
                                            "header.showBar"
                                        } else {
                                            "header.hideBar"
                                        }))
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        SettingsStore::update(cx, |settings| {
                                            settings.menu_bar_hidden = !settings.menu_bar_hidden
                                        });
                                        this.peek_top = false;
                                        cx.notify();
                                    })),
                            )
                        }),
                )
                .child(
                    grip.debug_selector(|| "header-grip".to_owned())
                        .flex_shrink_0(),
                ),
        )
    }

    // ----- 입력 -----

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.grid_open {
            self.on_grid_key(event, window, cx);
            return;
        }
        // 입력에 포커스가 있으면 뷰어 단축키를 막는다.
        if window.has_focused_input(cx) {
            return;
        }
        let action = keys::action_for(
            &SettingsStore::global(cx).settings.shortcuts,
            &event.keystroke,
        )
        .map(str::to_owned);
        if let Some(action) = action {
            cx.stop_propagation();
            self.run_action(&action, window, cx);
        }
    }

    /// 숨긴 크롬을 가장자리 호버로 잠깐 보여준다.
    fn on_root_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let height = f32::from(window.viewport_size().height);
        let y = f32::from(event.position.y);
        let top = y < PEEK_EDGE || (self.peek_top && y < PEEK_TOP_KEEP);
        let bottom = y > height - PEEK_EDGE || (self.peek_bottom && y > height - PEEK_BOTTOM_KEEP);
        if top != self.peek_top || bottom != self.peek_bottom {
            self.peek_top = top;
            self.peek_bottom = bottom;
            cx.notify();
        }
    }
}

impl Render for AraView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.advance_animation(window);
        let settings = SettingsStore::global(cx).settings.clone();
        let reading = self.picture.is_some() || !self.shown.is_empty();
        // 크롬 숨김은 이미지를 보고 있을 때만 적용한다. 홈에서는 항상 보인다.
        let hide_all = reading && settings.auto_hide_ui;
        let hide_header = reading && (settings.menu_bar_hidden || settings.auto_hide_ui);
        let show_dock = reading && self.list.len() > 1 && (!hide_all || self.peek_bottom);
        let show_status = reading && (!hide_all || self.peek_bottom);
        let dock_position = settings.dock_position;
        let context_menu = settings.mouse.right_click == MouseAction::ContextMenu;

        let header = (!hide_header || self.peek_top)
            .then(|| self.render_header(window, cx).into_any_element());
        let (flow_header, peek_header) = if hide_header {
            (None, header)
        } else {
            (header, None)
        };
        let dock = show_dock.then(|| self.render_dock(window, cx));
        let status = self
            .info
            .clone()
            .filter(|_| show_status)
            .map(|info| self.render_status(&info, cx).into_any_element());
        let panel =
            (reading && self.info_open).then(|| self.render_info_panel(cx).into_any_element());
        let grid = self.grid_open.then(|| self.render_grid(window, cx));

        let body: AnyElement = if reading {
            let view = cx.entity();
            let viewport = self.render_viewport(window, cx);
            let viewport: AnyElement = if context_menu {
                div()
                    .id("viewport-menu")
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(viewport)
                    .context_menu(move |menu, window, cx| {
                        Self::build_context_menu(&view, menu, window, cx)
                    })
                    .into_any_element()
            } else {
                viewport.into_any_element()
            };
            // 도크는 읽기 영역 가장자리에 붙어 영역을 밀어낸다.
            let reading_area = match dock_position {
                DockPosition::Top => v_flex().children(dock).child(viewport),
                DockPosition::Bottom => v_flex().child(viewport).children(dock),
                DockPosition::Left => h_flex().items_stretch().children(dock).child(viewport),
                DockPosition::Right => h_flex().items_stretch().child(viewport).children(dock),
            };
            h_flex()
                .flex_1()
                .min_h_0()
                .items_stretch()
                .child(reading_area.flex_1().min_h_0().min_w_0())
                .children(panel)
                .into_any_element()
        } else if let Some(error) = self.error.clone() {
            div()
                .flex_1()
                .min_h_0()
                .child(self.render_error(&error, cx))
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_h_0()
                .relative()
                .child(self.render_home(cx))
                .when(self.opening_visible(), |home| {
                    home.child(
                        div()
                            .absolute()
                            .top_4()
                            .left_0()
                            .right_0()
                            .flex()
                            .justify_center()
                            .child(loading_badge(t("home.list.loading"), cx)),
                    )
                })
                .into_any_element()
        };

        v_flex()
            .id("araview-root")
            .relative()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .on_mouse_move(cx.listener(Self::on_root_mouse_move))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                this.open_dropped(paths.paths().to_vec(), window, cx)
            }))
            .drag_over::<ExternalPaths>(|style, _, _, cx| {
                style.border_2().border_color(cx.theme().primary)
            })
            .children(flow_header)
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .child(body)
                    .children(grid),
            )
            .children(status)
            // peek 중인 헤더는 읽기 영역 위로 겹쳐 내려온다.
            .children(peek_header.map(|header| {
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .occlude()
                    .bg(cx.theme().background)
                    .child(header)
            }))
    }
}

/// 작은 스피너와 문구로 된 로딩 표시.
fn loading_badge(label: impl Into<SharedString>, cx: &App) -> Div {
    h_flex()
        .gap_2()
        .items_center()
        .px_3()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().popover)
        .text_sm()
        .child(Spinner::new().small())
        .child(label.into())
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// 투명 영역 확인용 체커 배경을 `area` 안에 그린다.
fn paint_checker(window: &mut Window, area: Bounds<Pixels>, base: Hsla, alt: Hsla) {
    if area.size.width <= px(0.) || area.size.height <= px(0.) {
        return;
    }
    window.paint_quad(fill(area, base));
    let cell = px(CHECKER_CELL);
    let columns = (area.size.width / cell).ceil() as usize;
    let rows = (area.size.height / cell).ceil() as usize;
    for row in 0..rows {
        for column in (row % 2..columns).step_by(2) {
            let origin = point(
                area.origin.x + cell * column as f32,
                area.origin.y + cell * row as f32,
            );
            let quad = Bounds::new(origin, size(cell, cell)).intersect(&area);
            window.paint_quad(fill(quad, alt));
        }
    }
}
