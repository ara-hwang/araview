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
mod ocr;
mod pages;
mod settings_panel;
pub mod system;
mod update;
mod viewport;

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use crate::smooth::{Glide, SmoothScroll};
use crate::toast::{Toast, WindowToast};
use araview_core::app_error::{AppError, ErrorCode};
use araview_core::comic_info::ComicInfo;
use araview_core::file_availability::FileAvailability;
use araview_core::file_identity::FileIdentity;
use araview_core::image::{ImageInfo, is_archive_file};
use araview_core::ops::{self, DirListOptions, DirSortKey};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::empty::{
    Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyMediaVariant, EmptyTitle,
};
use gpui_kit::component::menu::ContextMenuExt as _;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, Selectable as _, Sizable as _, TitleBar,
    VirtualListScrollHandle, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::assets::Ph;
use crate::geometry::Offset;
use crate::i18n::{t, t_with};
use crate::keys;
use crate::picture::{Orientation, Picture};
use crate::platform;
use crate::settings::{DockPosition, FitMode, MouseAction, SettingsStore, SortKey, ViewMode};

/// 체커 배경은 두 테마 모두 20px 타일, 흰 바탕에 `#c7c7c7`로 고정한다(DESIGN.md).
const CHECKER_CELL: f32 = 20.0;
const CHECKER_BASE: u32 = 0xffffff;
const CHECKER_ALT: u32 = 0xc7c7c7;
/// 이 폭(논리 px) 이상이면 보기 모드/변형/상태 버튼에 라벨을 붙인다. 라벨이 붙은 채로
/// 캡션 버튼 3개가 온전히 보이는 최소 폭을 개발 앱에서 한국어·영어 모두 재어 보니 약
/// 1487px로 같았고, 여기에 약 30px 여유를 더했다. 헤드리스 테스트의 텍스트 폭은 실제와
/// 달라 라벨 경계의 근거로 쓰지 못하므로 값을 바꾸면 개발 앱에서 폭별로 캡처해 확인한다.
const HEADER_EXTRA_LABELS_MIN_WIDTH: f32 = 1520.0;
/// 좁은 창에서 우선 숨길 도구 그룹의 경계(논리 px). 그룹이 보일 때 우측 메뉴와 그립이
/// Windows 캡션 버튼 3개(34px씩 102px) 앞에 들어가는 최소 폭에, 버튼 사이 간격이 늘어난
/// 몫과 약 30px 여유를 더한 값이다.
/// `header_never_overlaps_caption_controls` 테스트가 500~2000px 전체에서 이를 지킨다.
const HEADER_FIT_CONTROLS_MIN_WIDTH: f32 = 690.0;
const HEADER_VIEW_MODES_MIN_WIDTH: f32 = 840.0;
const HEADER_TRANSFORM_CONTROLS_MIN_WIDTH: f32 = 1010.0;
/// 타이틀바 높이(논리 px). 32px 도구 버튼 위아래로 4px씩 남아야 그룹이 눌리지 않는다.
const HEADER_HEIGHT: f32 = 40.0;
/// 그룹 안 도구 버튼 사이의 간격(논리 px). 그룹과 그룹 사이는 이보다 넓은 `gap_2`다.
const HEADER_BUTTON_GAP: f32 = 4.0;
/// 숨긴 상단바는 창 가장자리를 정밀하게 겨냥하지 않아도 다시 열 수 있다(논리 px).
const PEEK_TOP_EDGE: f32 = 24.0;
const PEEK_BOTTOM_EDGE: f32 = 6.0;
const PEEK_TOP_KEEP: f32 = HEADER_HEIGHT + 8.0;
const PEEK_BOTTOM_KEEP: f32 = 150.0;
/// 홈 최근 목록 카드의 가로 폭(논리 px). 560px 목록에서 카드 두 장과 다음 카드
/// 일부가 보여 스크롤할 거리를 암시한다.
const RECENT_CARD_WIDTH: f32 = 240.0;

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
    /// 열 때 캡처한 파일 identity(SPEC §8). 이어보기 기록 저장에 함께 쓴다.
    identity: Option<FileIdentity>,
    availability: Vec<FileAvailability>,
    /// 폴더 목록을 만든 기준 경로(처음 연 파일, 부모 폴더 기준). 아카이브는 `None`.
    root: Option<Arc<str>>,
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
    /// 폴더 목록을 만든 기준 경로. 정렬·하위 폴더 설정이 바뀌면 이 경로로 목록을
    /// 다시 읽는다(§5.2). 보던 파일로 다시 열면 하위 폴더 파일에서는 목록이 그
    /// 폴더로 좁아진다.
    list_root: Option<Arc<str>>,
    /// 목록 새로고침 순번. 늦게 끝난 새로고침 결과를 버린다.
    relist_seq: u64,
    comic: pages::Comic,
    /// 아카이브 세션 보기 모드. 열 때 자동으로 정하고, 보는 동안 직접 고르면
    /// 세션에만 바뀐다. 일반 이미지는 설정값을 그대로 쓴다(§15).
    comic_mode: Option<ViewMode>,
    /// 열 때 캡처한 아카이브 파일 identity. 이어보기 기록에 함께 저장한다(§8).
    archive_identity: Option<FileIdentity>,
    /// 이어보기를 끈 채 연 아카이브에서 첫 페이지를 벗어나기 전인지. 그동안은
    /// 열기만으로 저장된 위치를 덮지 않도록 진행 상황을 저장하지 않는다.
    progress_held: bool,

    /// 인덱스별 디코드 상태(대기·디코드 중·완료·실패)와 대기열. full 디코드는
    /// 상한까지 병렬로 돌리고 우선순위로 고른다(§9.1).
    pages: pages::PageSlots<pages::Page>,
    /// 바이트 예산 때문에 내려놓은 경계. 그보다 먼 페이지는 다시 미리 로드하지 않는다(§9.1).
    budget_reach: Option<pages::BudgetReach>,
    /// 가로가 더 긴 페이지. 로드하면서 알게 되고, 페이지를 내려놓아도 남긴다.
    wide: HashSet<usize>,
    /// 지금 화면에 올라가 있는 인덱스.
    shown: Vec<usize>,
    /// 화면 이동 상태: 커밋 대기, 뒤로 넘기기 대기, 이동 시작 시각과 방향.
    nav: pages::Nav,
    open_seq: u64,
    /// 파일 열기를 시작한 시각. 값이 있으면 목록을 읽는 중이다.
    opening: Option<Instant>,
    /// 에러 카드에 보일 열기 실패 또는 페이지 실패.
    error: Option<pages::ViewError>,

    /// 기준 그림과 그 정보(단일 보기의 표시 대상, 정보 패널·파일 작업의 대상).
    info: Option<ImageInfo>,
    picture: Option<Arc<Picture>>,
    orientation: Orientation,
    oriented: Option<Arc<Picture>>,
    orient_seq: u64,
    zoom: f32,
    zoom_animation: Option<viewport::ZoomAnimation>,
    zoom_generation: u64,
    position: Offset,
    /// 맞춤 상태(크기 대기 / 맞춤 고정 / 수동 줌). 수동 줌을 하면 리사이즈해도 줌을 유지한다.
    fit: viewport::Fit,
    /// 읽기 영역의 창 좌표 경계. 캔버스 prepaint가 갱신한다.
    viewport: Rc<Cell<Bounds<Pixels>>>,
    drag: Option<DragState>,
    /// 애니메이션(GIF/APNG 등) 재생 상태: 현재 프레임, 재생 여부와 프레임 시작 시각, 반복 횟수.
    anim: viewport::AnimPlayback,
    /// 애니메이션(GIF/APNG 등) 프레임 진행 슬라이더.
    anim_timeline: Option<viewport::AnimTimeline>,
    /// 마지막 마우스·키보드 조작 시각. 재생 중 하단 컨트롤 자동 숨김에 쓴다.
    overlay_active: Instant,

    pixel_view: Option<viewport::PixelView>,
    /// 창의 기기 배율. 그림 사각형을 기기 픽셀에 맞출 때 쓴다(§18). 렌더마다 갱신한다.
    device_scale: f32,
    /// 프레임 페이싱 통계(디버그 빌드 전용). 제어 채널 벤치가 연속 동작의
    /// 프레임 간격을 잰다(yosh의 flipbench와 같은 용도).
    #[cfg(debug_assertions)]
    frame_stats: FrameStats,

    /// 웹툰 스크롤 위치: 화면 맨 위에 걸친 페이지와 그 안에서 내려간 거리.
    wt_anchor: usize,
    wt_offset: f32,
    /// 부드러운 스크롤이 아직 반영하지 못한 남은 거리와 프레임 예약 상태.
    wt_glide: Glide,
    wt_zoom: f32,
    wt_fit_width: bool,
    wt_dims: HashMap<usize, (u32, u32)>,

    thumbs: HashMap<Arc<str>, ThumbState>,
    thumbs_pending: HashSet<Arc<str>>,
    /// 썸네일 그리드. 닫아도 필터 입력과 스크롤 상태를 남겨 둔다(`GridState::open`).
    grid: Option<grid::GridState>,
    /// 우클릭한 지점의 페이지(표지 지정 대상).
    context_page: Option<usize>,

    info_open: bool,
    home_scroll: SmoothScroll,
    /// 홈 최근 목록(가로 스크롤)의 감속 스크롤.
    home_recent_scroll: SmoothScroll,
    info_scroll: SmoothScroll,
    /// 이미지 목록(도크) 스크롤과, 마지막으로 따라간 (현재 인덱스, 목록 길이).
    dock_virtual: VirtualListScrollHandle,
    dock_scroll: SmoothScroll,
    dock_followed: std::cell::Cell<Option<(usize, usize, bool, bool)>>,
    info_data: info_panel::InfoData,
    info_seq: u64,
    /// 글자 인식 작업, 마지막 강조 결과, 강조 표시와 글자 선택(SPEC §11.5).
    ocr: ocr::OcrState,
    /// 업데이트 확인·다운로드 상태(SPEC §20.1).
    update_status: update::UpdateStatus,
    peek_top: bool,
    peek_bottom: bool,
    high_contrast: bool,
}

/// 프레임 페이싱 통계(디버그 빌드 전용). 제어 채널 벤치가 연속 동작의 프레임
/// 간격을 재는 데 쓴다: 평균은 전체 속도, 최댓값은 히치의 상한을 보여준다.
/// 릴리스 빌드에는 통로가 없다.
#[derive(Clone, Copy, Default, Debug)]
struct FrameStats {
    /// 간격을 잰 프레임 수.
    intervals: u64,
    /// 간격 누적(ms).
    total_ms: f32,
    /// 마지막 간격(ms).
    last_ms: f32,
    /// 리셋 이후 최대 간격(ms).
    max_ms: f32,
    /// 마지막 render 시작 시각.
    prev_start: Option<Instant>,
}

impl FrameStats {
    /// render 시작을 기록한다. 이전 시작과의 간격이 프레임 간격이다.
    fn record(&mut self, now: Instant) {
        if let Some(prev) = self.prev_start {
            let ms = (now - prev).as_secs_f32() * 1000.0;
            self.intervals += 1;
            self.total_ms += ms;
            self.last_ms = ms;
            if ms > self.max_ms {
                self.max_ms = ms;
            }
        }
        self.prev_start = Some(now);
    }

    /// 누적 통계를 지워 다음 측정을 새로 시작한다.
    fn reset(&mut self) {
        *self = Self::default();
    }
}

/// 표시용 파일명에서 제어문자와 bidi 제어를 치환한다. 파일 조작에는 원본 이름을
/// 그대로 쓰고, 화면·창 제목에만 이 결과를 쓴다.
fn display_safe_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_control()
                || matches!(
                    c,
                    '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
                )
            {
                '\u{FFFD}'
            } else {
                c
            }
        })
        .collect()
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
        // 도크는 VirtualList가 핸들을 추적하고, 같은 핸들을 SmoothScroll이 움직인다.
        let dock_virtual = VirtualListScrollHandle::new();
        let dock_scroll = SmoothScroll::from_handle(dock_virtual.base_handle().clone());
        let mut this = Self {
            focus,
            list: Arc::new(Vec::new()),
            availability: Vec::new(),
            list_gen: 0,
            index: 0,
            archive: None,
            comic: pages::Comic::Absent,
            comic_mode: None,
            list_root: None,
            relist_seq: 0,
            archive_identity: None,
            progress_held: false,
            pages: pages::PageSlots::default(),
            budget_reach: None,
            wide: HashSet::new(),
            shown: Vec::new(),
            nav: pages::Nav::default(),
            open_seq: 0,
            opening: None,
            error: None,
            info: None,
            picture: None,
            orientation: Orientation::default(),
            oriented: None,
            orient_seq: 0,
            zoom: 1.0,
            zoom_animation: None,
            zoom_generation: 0,
            position: Offset::default(),
            fit: viewport::Fit::Pending,
            viewport: Rc::new(Cell::new(Bounds::default())),
            drag: None,
            anim: viewport::AnimPlayback::default(),
            anim_timeline: None,
            overlay_active: Instant::now(),

            pixel_view: None,
            device_scale: 1.0,
            #[cfg(debug_assertions)]
            frame_stats: FrameStats::default(),
            wt_anchor: 0,
            wt_offset: 0.0,
            wt_glide: Glide::default(),
            wt_zoom: 1.0,
            wt_fit_width: false,
            wt_dims: HashMap::new(),
            thumbs: HashMap::new(),
            thumbs_pending: HashSet::new(),
            grid: None,
            context_page: None,
            info_open: false,
            home_scroll: SmoothScroll::default(),
            home_recent_scroll: SmoothScroll::default(),
            info_scroll: SmoothScroll::default(),
            dock_virtual,
            dock_scroll,
            dock_followed: std::cell::Cell::new(None),
            info_data: info_panel::InfoData::default(),
            info_seq: 0,
            ocr: Default::default(),
            update_status: Default::default(),
            peek_top: false,
            peek_bottom: false,
            high_contrast: crate::platform::high_contrast(),
        };
        system::apply_theme(SettingsStore::global(cx).theme, window, cx);
        if platform::ALWAYS_ON_TOP_SUPPORTED && SettingsStore::global(cx).settings.always_on_top {
            this.apply_always_on_top(window, cx);
        }
        cx.observe_window_bounds(window, |this, window, cx| this.remember_window(window, cx))
            .detach();
        cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() && (this.peek_top || this.peek_bottom) {
                this.peek_top = false;
                this.peek_bottom = false;
                cx.notify();
            }
            this.system_appearance_changed(window, cx);
        })
        .detach();
        // OS 밝기(라이트·다크)가 실행 중에 바뀌면 테마 설정이 '시스템'일 때
        // 즉시 따라간다(SPEC §3.3). Windows는 시스템 테마가 바뀌면 창에 바로 알려 준다.
        cx.observe_window_appearance(window, |this, window, cx| {
            this.system_appearance_changed(window, cx)
        })
        .detach();
        window.on_window_should_close(cx, |_, cx| {
            SettingsStore::flush(cx);
            true
        });
        if let Some(receiver) = instances {
            this.listen_for_instances(receiver, window, cx);
        }
        // 개발 빌드는 실행 중인 창을 에이전트가 직접 조작할 수 있게 제어 통로를 연다.
        #[cfg(debug_assertions)]
        bridge::listen(crate::IDENTIFIER, cx.entity(), window, cx);
        let store = SettingsStore::global(cx);
        let auto_open = (store.settings.auto_open_last_file && store.settings.record_recent_files)
            .then(|| store.recent_files.first().cloned())
            .flatten();
        if let Some(path) = initial.or(auto_open) {
            this.open_path(path, window, cx);
        }
        this.maybe_auto_check_updates(window, cx);
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
            (Some(_), Some(entry)) => {
                display_safe_name(entry.rsplit(['/', '\\']).next().unwrap_or(entry))
            }
            _ => display_safe_name(&info.file_name),
        }
    }

    /// 헤더 버튼과 `1/2/3` 단축키: 맞춤 모드를 기억하고 바로 적용한다.
    fn set_fit_mode(&mut self, mode: FitMode, cx: &mut Context<Self>) {
        SettingsStore::update(cx, |settings| settings.fit_mode = mode);
        self.fit = self.fit.lock();
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
        if !self.current_page_is_full() {
            window.toast(Toast::warning(t("toast.copy.empty")), cx);
            return;
        }
        // 이미지 위에서 고른 글자가 있으면 이미지 대신 그 글자를 복사한다.
        if let Some(text) = self.ocr_selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            window.toast(Toast::success(t("toast.ocr.selectionCopied")), cx);
            return;
        }
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
                // Empty는 기본으로 flex_1이라 남은 세로 공간을 다 차지해 최근
                // 목록을 창 아래로 민다. 늘어나지 않게 감싸 버튼 카드와 목록이
                // 함께 가운데에 놓이게 한다.
                .child(
                    v_flex().items_center().child(
                        Empty::new().content(
                            EmptyContent::new().child(
                                Button::new("home-open")
                                    .primary()
                                    .icon(Ph::FolderOpen)
                                    .label(t("home.openFile"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.prompt_open(window, cx)
                                    })),
                            ),
                        ),
                    ),
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
                            // 최근 목록은 가로 카드 줄이다. 카드가 넘치면
                            // 가로로 스크롤하며, 세로 휠도 가로 이동으로 받는다.
                            .child(
                                self.home_recent_scroll.h_area(
                                    "home-recent",
                                    h_flex().gap_2().pt_1().pb(px(14.)).children(
                                        recent
                                            .into_iter()
                                            .enumerate()
                                            .map(|(ix, path)| self.recent_card(ix, &path, cx)),
                                    ),
                                ),
                            ),
                    )
                }),
        )
    }

    /// 홈 최근 목록의 카드 한 장. 파일명·상위 폴더·(아카이브면) 읽기 진도를 담는다.
    /// 파일명과 폴더는 한 줄에만 놓고 전체 경로는 툴팁으로 내준다(DESIGN.md). 삭제
    /// 버튼은 열기 영역과 형제로 둔다. 버튼은 눌림이 바깥 카드로 번지지 않게 막지
    /// 않으므로(gpui-kit Button), 카드에 `on_click`을 걸면 X를 눌러도 파일이 열린다.
    fn recent_card(&self, ix: usize, path: &str, cx: &mut Context<Self>) -> Stateful<Div> {
        let file = Path::new(path);
        let name = file
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_owned());
        let parent = file
            .parent()
            .map(|dir| dir.to_string_lossy().into_owned())
            .unwrap_or_default();
        let icon = if is_archive_file(file) {
            Ph::FileArchive
        } else {
            Ph::FileImage
        };
        // 아카이브는 읽기 진도를 함께 보여준다. 경로만 같다고
        // 믿지 않아 identity가 지금 파일과 다르면(§8) 진도를
        // 숨긴다(§12).
        let progress = SettingsStore::global(cx)
            .archive_progress(path)
            .filter(|progress| progress.matches_identity(FileIdentity::capture(file)))
            .map(|progress| {
                t_with(
                    "home.card.progress",
                    &[("index", &(progress.index + 1)), ("total", &progress.total)],
                )
            });
        let muted = cx.theme().muted_foreground;
        let opened = path.to_owned();
        let removed = path.to_owned();
        let full = path.to_owned();
        h_flex()
            .id(("recent", ix))
            .flex_none()
            .w(px(RECENT_CARD_WIDTH))
            .items_start()
            .gap_1()
            .px_2()
            .py_2()
            .rounded_md()
            .hover(|style| style.bg(cx.theme().muted))
            .tooltip(move |window, cx| {
                gpui_kit::component::tooltip::Tooltip::new(full.clone()).build(window, cx)
            })
            .child(
                // 파일명·폴더·진도를 여는 영역. 삭제 버튼과 형제다.
                v_flex()
                    .id(("recent-open", ix))
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .cursor_pointer()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(Icon::new(icon).flex_none().text_color(muted))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_sm()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .child(name),
                            ),
                    )
                    .child(
                        div()
                            .w_full()
                            .text_xs()
                            .text_color(muted)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(parent),
                    )
                    .when_some(progress, |column, text| {
                        column.child(Tag::secondary().small().child(text))
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_path(opened.clone(), window, cx)
                    })),
            )
            .child(
                Button::new(("recent-remove", ix))
                    .ghost()
                    .xsmall()
                    .icon(Ph::X)
                    .tooltip(t("home.card.remove"))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        SettingsStore::remove_recent(cx, &removed);
                        cx.notify();
                    })),
            )
    }

    fn render_error(&self, error: pages::ViewError, cx: &mut Context<Self>) -> impl IntoElement {
        let kind = match error.error().code {
            ErrorCode::NotFound => "notFound",
            ErrorCode::Permission => "permission",
            ErrorCode::Unsupported => "unsupported",
            ErrorCode::Corrupt | ErrorCode::TooLarge => "corrupt",
            _ => "unknown",
        };
        Empty::new()
            .header(
                EmptyHeader::new()
                    .media(
                        EmptyMedia::new()
                            .with_variant(EmptyMediaVariant::Icon)
                            .child(Icon::new(Ph::WarningCircle).text_color(cx.theme().danger)),
                    )
                    .title(EmptyTitle::new().child(t(&format!("error.{kind}.title"))))
                    .description(EmptyDescription::new().child(t(&format!("error.{kind}.hint")))),
            )
            .content(
                EmptyContent::new()
                    .flex_row()
                    .justify_center()
                    .child(
                        Button::new("error-retry")
                            .primary()
                            .label(t("viewer.error.retry"))
                            .on_click(cx.listener(move |this, _, window, cx| match &error {
                                // 열기 실패는 같은 경로를 다시 연다. 목록이 비어 있으면
                                // 다시 디코드할 페이지가 없어 아무 일도 일어나지 않았다.
                                pages::ViewError::Open { path, .. } => {
                                    this.open_path(path.clone(), window, cx)
                                }
                                pages::ViewError::Page(_) => this.reload(window, cx),
                            })),
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
            .h(px(28.))
            .px_3()
            .gap_4()
            .items_center()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .border_t_1()
            .border_color(cx.theme().border)
            // 이미지 목록이 있을 때 도크 컨트롤을 파일명 왼쪽에 둔다.
            .when(self.list.len() > 1 || self.has_animation(cx), |row| {
                row.child(self.render_dock_controls(cx))
            })
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
        icon: Ph,
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
        let (fit_mode, always_on_top, menu_bar_hidden) = {
            let settings = &SettingsStore::global(cx).settings;
            (
                settings.fit_mode,
                settings.always_on_top,
                settings.menu_bar_hidden,
            )
        };
        let has_image = self.picture.is_some();
        let mode = self.view_mode(cx);
        let single = mode == ViewMode::Single;
        let width = f32::from(window.viewport_size().width);
        let extra_labels = width >= HEADER_EXTRA_LABELS_MIN_WIDTH;
        // Below the compact toolbar's natural width, hide secondary groups in order;
        // keep the left/right menus and native caption controls outside the shrinkable area.
        let show_zoom_controls = width >= HEADER_FIT_CONTROLS_MIN_WIDTH;
        // 양쪽 보기의 배율은 화면 맞춤 대비 상대값이라 맞춤과 회전/반전은 화면을 바꾸는
        // 기능을 하지 않는다. 그래도 버튼을 숨기면 모드를 바꿀 때 다른 버튼이 옮겨 가므로
        // 보이는 채로 두고 비활성화만 한다.
        let show_fit_controls = show_zoom_controls;
        let show_view_modes = width >= HEADER_VIEW_MODES_MIN_WIDTH;
        let show_transform_controls = width >= HEADER_TRANSFORM_CONTROLS_MIN_WIDTH;
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
        // 그룹 안 버튼이 맞붙어 보이지 않게 간격을 두고, 창 드래그 영역에서는 뺀다.
        let group = || {
            h_flex()
                .gap(px(HEADER_BUTTON_GAP))
                .items_center()
                .occlude()
                .flex_none()
        };
        let mode_button = |id: &'static str, icon: Ph, key: &'static str, value: ViewMode| {
            Button::new(id)
                .ghost()
                .icon(icon)
                .tooltip(t(key))
                .selected(mode == value)
                .when(extra_labels, |button| button.label(t(key)))
        };

        TitleBar::new().h(px(HEADER_HEIGHT)).child(
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
                                .icon(Ph::House)
                                .tooltip(t("header.home"))
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.close_image(window, cx)),
                                ),
                        )
                        .child(
                            Button::new("open")
                                .ghost()
                                .icon(Ph::FolderOpen)
                                .tooltip(t("header.open"))
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
                                        Ph::ArrowsHorizontal,
                                        "header.fitWidth",
                                        "fitWidth",
                                        cx,
                                    )
                                    .disabled(!single)
                                    .selected(
                                        self.fit.follows_window() && fit_mode == FitMode::Width,
                                    ),
                                )
                                .child(
                                    Self::icon_button(
                                        "fit-height",
                                        Ph::ArrowsVertical,
                                        "header.fitHeight",
                                        "fitHeight",
                                        cx,
                                    )
                                    .disabled(!single)
                                    .selected(
                                        self.fit.follows_window() && fit_mode == FitMode::Height,
                                    ),
                                )
                                .child(
                                    Self::icon_button(
                                        "fit-screen",
                                        Ph::ArrowsOut,
                                        "header.fitScreen",
                                        "fitScreen",
                                        cx,
                                    )
                                    .disabled(!single)
                                    .selected(
                                        self.fit.follows_window() && fit_mode == FitMode::Screen,
                                    ),
                                ),
                        )
                    })
                    .when(show_zoom_controls, |row| {
                        row.child(
                            group()
                                .debug_selector(|| "header-zoom".to_owned())
                                .child(Self::icon_button(
                                    "zoom-out",
                                    Ph::MagnifyingGlassMinus,
                                    "header.zoomOut",
                                    "zoomOut",
                                    cx,
                                ))
                                .child(
                                    div()
                                        .w(px(48.))
                                        .text_xs()
                                        .text_center()
                                        .child(format!("{}%", self.zoom_percent(cx))),
                                )
                                .child(Self::icon_button(
                                    "zoom-in",
                                    Ph::MagnifyingGlassPlus,
                                    "header.zoomIn",
                                    "zoomIn",
                                    cx,
                                )),
                        )
                    })
                    .when(show_view_modes, |row| {
                        row.child(
                            group()
                                .debug_selector(|| "header-view-modes".to_owned())
                                .child(
                                    mode_button(
                                        "view-single",
                                        Ph::Image,
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
                                        Ph::BookOpenText,
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
                                        Ph::BookOpenTextMirrored,
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
                                        Ph::ArrowsDownUp,
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
                                        Ph::ArrowCounterClockwise,
                                        "header.rotateCcw",
                                        "rotateCCW",
                                        cx,
                                    )
                                    .disabled(!single)
                                    .when(extra_labels, |button| {
                                        button.label(t("header.rotateCcw"))
                                    }),
                                )
                                .child(
                                    Self::icon_button(
                                        "rotate-cw",
                                        Ph::ArrowClockwise,
                                        "header.rotateCw",
                                        "rotateCW",
                                        cx,
                                    )
                                    .disabled(!single)
                                    .when(extra_labels, |button| {
                                        button.label(t("header.rotateCw"))
                                    }),
                                )
                                .child(
                                    Self::icon_button(
                                        "flip-h",
                                        Ph::FlipHorizontal,
                                        "header.flipH",
                                        "flipH",
                                        cx,
                                    )
                                    .disabled(!single)
                                    .when(extra_labels, |button| button.label(t("header.flipH")))
                                    .selected(self.orientation.flip_h),
                                )
                                .child(
                                    Self::icon_button(
                                        "flip-v",
                                        Ph::FlipVertical,
                                        "header.flipV",
                                        "flipV",
                                        cx,
                                    )
                                    .disabled(!single)
                                    .when(extra_labels, |button| button.label(t("header.flipV")))
                                    .selected(self.orientation.flip_v),
                                ),
                        )
                    })
                    .child(
                        group()
                            .debug_selector(|| "header-center-info".to_owned())
                            .when(platform::OCR_SUPPORTED, |row| {
                                row.child(
                                    Self::icon_button(
                                        "ocr-highlight",
                                        Ph::Scan,
                                        "header.ocrHighlight",
                                        "ocrHighlight",
                                        cx,
                                    )
                                    .selected(self.ocr_visible()),
                                )
                            }),
                    )
                })
                .child(div().flex_1().min_w_0())
                .child(
                    group()
                        .debug_selector(|| "header-right-menu".to_owned())
                        .when(has_image, |row| {
                            row.child(
                                Self::icon_button(
                                    "info",
                                    Ph::Info,
                                    "header.info",
                                    "toggleExif",
                                    cx,
                                )
                                .selected(self.info_open),
                            )
                        })
                        .child(Self::icon_button(
                            "palette",
                            Ph::MagnifyingGlass,
                            "palette.open",
                            "togglePalette",
                            cx,
                        ))
                        .child(Self::icon_button(
                            "settings",
                            Ph::Gear,
                            "header.settings",
                            "openSettings",
                            cx,
                        ))
                        .when(platform::ALWAYS_ON_TOP_SUPPORTED, |row| {
                            row.child(
                                Self::icon_button(
                                    "on-top",
                                    if always_on_top {
                                        Ph::PushPinFill
                                    } else {
                                        Ph::PushPin
                                    },
                                    "header.alwaysOnTop",
                                    "toggleAlwaysOnTop",
                                    cx,
                                )
                                .when(extra_labels, |button| button.label(t("header.onTop")))
                                .selected(always_on_top),
                            )
                        })
                        .when(has_image, |row| {
                            row.child(
                                Button::new("hide-bar")
                                    .ghost()
                                    .icon(if menu_bar_hidden {
                                        Ph::CaretDown
                                    } else {
                                        Ph::CaretUp
                                    })
                                    .tooltip(t(if menu_bar_hidden {
                                        "header.showMenuBar"
                                    } else {
                                        "header.hideMenuBar"
                                    }))
                                    .when(extra_labels, |button| {
                                        button.label(t(if menu_bar_hidden {
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
        if self.grid_open() {
            self.on_grid_key(event, window, cx);
            return;
        }
        // 다이얼로그가 열려 있으면 뷰어 단축키를 막는다(SPEC §14.3).
        if window.has_active_dialog(cx) {
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
        let settings = &SettingsStore::global(cx).settings;
        let reading = self.picture.is_some() || !self.shown.is_empty();
        let top = reading
            && (settings.menu_bar_hidden || settings.auto_hide_ui)
            && peek_at_edge(y, self.peek_top, PEEK_TOP_EDGE, PEEK_TOP_KEEP);
        let bottom = reading
            && settings.auto_hide_ui
            && peek_at_edge(
                height - y,
                self.peek_bottom,
                PEEK_BOTTOM_EDGE,
                PEEK_BOTTOM_KEEP,
            );
        if top != self.peek_top || bottom != self.peek_bottom {
            self.peek_top = top;
            self.peek_bottom = bottom;
            cx.notify();
        }
    }
}

impl Render for AraView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(debug_assertions)]
        self.frame_stats.record(Instant::now());
        self.advance_animation(window);
        // 매 프레임 `Settings` 전체(BTreeMap 44쌍 포함)를 클론하지 않고 렌더에
        // 쓰는 `Copy` 필드만 복사한다.
        let (auto_hide_ui, menu_bar_hidden, dock_position, dock_visible, context_menu) = {
            let settings = &SettingsStore::global(cx).settings;
            (
                settings.auto_hide_ui,
                settings.menu_bar_hidden,
                settings.dock_position,
                settings.dock_visible,
                settings.mouse.right_click == MouseAction::ContextMenu,
            )
        };
        let reading = self.picture.is_some() || !self.shown.is_empty();
        // 크롬 숨김은 이미지를 보고 있을 때만 적용한다. 홈에서는 항상 보인다.
        let hide_all = reading && auto_hide_ui;
        let hide_header = reading && (menu_bar_hidden || auto_hide_ui);
        // 숨김을 끄거나 홈으로 돌아온 뒤에는 이전 호버 상태를 재사용하지 않는다.
        self.peek_top &= hide_header;
        self.peek_bottom &= hide_all;
        let animated = self.has_animation(cx);
        // 애니메이션 컨트롤이 하단 독으로 이동했으므로, 애니메이션 이미지에서는
        // 목록이 하나뿐이어도 독을 보여 컨트롤을 노출한다.
        let show_dock =
            reading && (self.list.len() > 1 || animated) && (!hide_all || self.peek_bottom);
        let show_status = reading && (!hide_all || self.peek_bottom);

        let header = (!hide_header || self.peek_top)
            .then(|| self.render_header(window, cx).into_any_element());
        let (flow_header, peek_header) = if hide_header {
            (None, header)
        } else {
            (header, None)
        };
        // 접힌 도크는 그리지 않는다. 다시 펼치는 버튼은 상태바에만 둔다.
        let dock = (show_dock && dock_visible).then(|| self.render_dock(window, cx));
        let status = self
            .info
            .clone()
            .filter(|_| show_status)
            .map(|info| self.render_status(&info, cx).into_any_element());
        let panel =
            (reading && self.info_open).then(|| self.render_info_panel(cx).into_any_element());
        let grid = self.grid_open().then(|| self.render_grid(window, cx));

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
            v_flex()
                .flex_1()
                .min_h_0()
                .child(self.render_error(error, cx))
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

fn peek_at_edge(distance: f32, open: bool, trigger: f32, keep: f32) -> bool {
    distance >= 0.0 && distance < if open { keep } else { trigger }
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

#[cfg(test)]
mod peek_tests {
    use super::{
        HEADER_HEIGHT, PEEK_BOTTOM_EDGE, PEEK_BOTTOM_KEEP, PEEK_TOP_EDGE, PEEK_TOP_KEEP,
        peek_at_edge,
    };

    #[test]
    fn top_peek_opens_across_the_expanded_band() {
        for y in [0.0, 6.0, 12.0, 20.0, 23.9] {
            assert!(peek_at_edge(y, false, PEEK_TOP_EDGE, PEEK_TOP_KEEP));
        }
        assert!(!peek_at_edge(24.0, false, PEEK_TOP_EDGE, PEEK_TOP_KEEP));
    }

    #[test]
    fn top_peek_stays_open_over_the_header_and_closes_beyond_it() {
        for y in [24.0, HEADER_HEIGHT - 1.0, HEADER_HEIGHT + 7.0] {
            assert!(peek_at_edge(y, true, PEEK_TOP_EDGE, PEEK_TOP_KEEP));
        }
        assert!(!peek_at_edge(48.0, true, PEEK_TOP_EDGE, PEEK_TOP_KEEP));
        assert!(!peek_at_edge(40.0, false, PEEK_TOP_EDGE, PEEK_TOP_KEEP));
    }

    #[test]
    fn bottom_peek_keeps_its_existing_trigger_and_hold_band() {
        assert!(peek_at_edge(5.0, false, PEEK_BOTTOM_EDGE, PEEK_BOTTOM_KEEP));
        assert!(!peek_at_edge(
            6.0,
            false,
            PEEK_BOTTOM_EDGE,
            PEEK_BOTTOM_KEEP
        ));
        assert!(peek_at_edge(
            149.0,
            true,
            PEEK_BOTTOM_EDGE,
            PEEK_BOTTOM_KEEP
        ));
        assert!(!peek_at_edge(
            150.0,
            true,
            PEEK_BOTTOM_EDGE,
            PEEK_BOTTOM_KEEP
        ));
    }

    #[test]
    fn pointer_outside_the_window_does_not_keep_peek_open() {
        for open in [false, true] {
            assert!(!peek_at_edge(-1.0, open, PEEK_TOP_EDGE, PEEK_TOP_KEEP));
            assert!(!peek_at_edge(
                -1.0,
                open,
                PEEK_BOTTOM_EDGE,
                PEEK_BOTTOM_KEEP
            ));
        }
    }
}
