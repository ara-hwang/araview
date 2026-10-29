//! 목록 열기, 페이지 캐시와 프리페치, 이동(SPEC §4, §5, §6, §9.1).
//!
//! 페이지는 인덱스별로 디코드해 캐시에 두고, 화면에 올릴 페이지가 모두 준비되면
//! 한 번에 교체한다. 그 전까지는 이전 화면을 유지한다(SPEC §3.3).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use crate::toast::{Toast, WindowToast, push_timed_notification};
use araview_core::app_error::AppError;
use araview_core::file_identity::FileIdentity;
use araview_core::image::{ImageInfo, is_archive_file};
use araview_core::ops;
use gpui_kit::*;

use super::{AraView, Listing};
use crate::geometry::Offset;
use crate::i18n::{t, t_with};
use crate::layout::{self, CoverLayout, Solo};
use crate::picture::{self, Orientation, Picture};
use crate::settings::{ArchiveProgress, CacheMode, SettingsStore, ViewMode};

pub(super) enum Page {
    Preview {
        picture: Arc<Picture>,
        bytes: u64,
        sequence: u64,
    },
    Full {
        info: ImageInfo,
        picture: Arc<Picture>,
        /// 디코드 결과의 보존 바이트(타일·밉 단계·애니메이션 프레임 합계). 바이트
        /// 예산이 이 값으로 페이지를 내려놓는다(SPEC §9.1).
        bytes: u64,
        /// 폴더 목록의 아카이브를 첫 페이지로 미리 보여주는 중인지.
        archive_preview: bool,
    },
}

impl Page {
    pub fn picture(&self) -> &Arc<Picture> {
        match self {
            Self::Preview { picture, .. } | Self::Full { picture, .. } => picture,
        }
    }

    pub fn info(&self) -> Option<&ImageInfo> {
        match self {
            Self::Preview { .. } => None,
            Self::Full { info, .. } => Some(info),
        }
    }

    pub fn info_mut(&mut self) -> Option<&mut ImageInfo> {
        match self {
            Self::Preview { .. } => None,
            Self::Full { info, .. } => Some(info),
        }
    }

    pub fn bytes(&self) -> u64 {
        match self {
            Self::Preview { bytes, .. } | Self::Full { bytes, .. } => *bytes,
        }
    }

    pub fn is_full(&self) -> bool {
        matches!(self, Self::Full { .. })
    }

    pub fn preview_sequence(&self) -> Option<u64> {
        match self {
            Self::Preview { sequence, .. } => Some(*sequence),
            Self::Full { .. } => None,
        }
    }

    pub fn is_archive_preview(&self) -> bool {
        matches!(self, Self::Full { archive_preview: true, .. })
    }
}

/// 대기 중인 full 디코드 요청.
pub(super) struct DecodeJob {
    index: usize,
    protect: bool,
    seq: u64,
}

/// 대기 중인 요청 가운데 다음에 돌릴 것. 현재 화면(`protect`)이 먼저이고,
/// 같으면 현재 위치에서 가까운 순, 그다음 요청 순이다.
fn preview_is_stale(is_full_page: bool, previous_preview: Option<u64>, sequence: u64) -> bool {
    is_full_page || previous_preview.is_some_and(|previous| previous >= sequence)
}

fn next_decode_job(queue: &[DecodeJob], anchor: usize) -> Option<usize> {
    queue
        .iter()
        .enumerate()
        .min_by_key(|(_, job)| (!job.protect, job.index.abs_diff(anchor), job.seq))
        .map(|(position, _)| position)
}

/// 새 이미지가 디코드될 때까지 이전 이미지를 붙잡는 최대 시간(SPEC §3.3).
/// 열기가 이 시간보다 길어지면 로딩 표시를 보인다.
pub(super) const LOADING_DELAY: std::time::Duration = std::time::Duration::from_millis(200);
pub(super) const PREVIOUS_HOLD: std::time::Duration = std::time::Duration::from_millis(300);

/// 캐시 모드별 프리페치 거리(SPEC §9.1).
fn prefetch_distance(mode: CacheMode) -> usize {
    match mode {
        CacheMode::Off => 0,
        CacheMode::Nearby => 1,
        CacheMode::Memory1Gb => 2,
        CacheMode::Extended | CacheMode::Memory2Gb => 3,
    }
}

/// 페이지 캐시 예산: 항목 수와 디코드 바이트의 이중 상한(SPEC §9.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CacheBudget {
    pub items: usize,
    pub bytes: u64,
}

/// `nearby`/`extended`의 기본 바이트 상한. 거리 창이 좁아 평소에는 바이트
/// 예산에 닿지 않지만, 밉 단계와 프레임이 붙은 큰 이미지가 창을 채울 때를
/// 제한한다(SPEC §9.1).
const NEARBY_BYTES: u64 = 512 * 1024 * 1024;
const EXTENDED_BYTES: u64 = 1024 * 1024 * 1024;

const GIB: u64 = 1024 * 1024 * 1024;

fn cache_budget(mode: CacheMode) -> CacheBudget {
    match mode {
        CacheMode::Off => CacheBudget {
            items: 1,
            bytes: u64::MAX,
        },
        CacheMode::Nearby => CacheBudget {
            items: 24,
            bytes: NEARBY_BYTES,
        },
        CacheMode::Extended => CacheBudget {
            items: 64,
            bytes: EXTENDED_BYTES,
        },
        // 거리 창과 바이트 예산이 한도를 대신한다.
        CacheMode::Memory1Gb => CacheBudget {
            items: usize::MAX,
            bytes: GIB,
        },
        CacheMode::Memory2Gb => CacheBudget {
            items: usize::MAX,
            bytes: 2 * GIB,
        },
    }
}

/// 항목 수와 바이트 예산을 넘겼을 때 내려놓을 페이지 고르기. 현재 위치에서
/// 가장 먼 페이지부터 고르고, 현재 화면·표시 페이지(`protected`)는 예산을
/// 넘겨도 남겨 둔다(들여다보는 화면은 예산에 내어놓지 않는다). 거리가 같으면
/// 더 큰 인덱스(뒤쪽)를 먼저 놓아 결정적으로 움직인다.
fn evict_plan(
    bytes: &HashMap<usize, u64>,
    anchor: usize,
    protected: &HashSet<usize>,
    item_limit: usize,
    byte_limit: u64,
) -> Vec<usize> {
    let mut plan = Vec::new();
    let mut remaining = bytes.clone();
    loop {
        let total: u64 = remaining.values().sum();
        if remaining.len() <= item_limit && total <= byte_limit {
            break;
        }
        let farthest = remaining
            .keys()
            .copied()
            .filter(|index| !protected.contains(index))
            .max_by_key(|index| (index.abs_diff(anchor), *index));
        let Some(farthest) = farthest else {
            // 보호 대상만 남았으면 예산을 넘어도 화면을 유지한다(§9.1).
            break;
        };
        remaining.remove(&farthest);
        plan.push(farthest);
    }
    plan
}

impl AraView {
    /// 화면에 적용되는 보기 모드. 아카이브는 자동 결정(`comic_mode`)이 설정보다 우선한다.
    pub(super) fn view_mode(&self, cx: &App) -> ViewMode {
        let base = SettingsStore::global(cx).settings.view_mode;
        if self.archive.is_some() {
            self.comic_mode.unwrap_or(base)
        } else {
            base
        }
    }

    /// 표지 배치. 폴더 목록에는 ComicInfo를 적용하지 않는다.
    pub(super) fn cover(&self, cx: &App) -> CoverLayout {
        let default_alone = SettingsStore::global(cx).settings.show_cover_alone;
        let comic = self.archive.as_ref().and(self.comic.as_ref());
        layout::cover_layout(comic, self.list.len(), default_alone)
    }

    /// 양쪽 보기에서 혼자 한 화면을 쓰는 인덱스.
    pub(super) fn solo(&self, cx: &App) -> Solo {
        let mut solo: Solo = self.cover(cx).pages().collect();
        if self.archive.is_none() {
            solo.extend(
                self.list
                    .iter()
                    .enumerate()
                    .filter(|(_, path)| is_archive_file(Path::new(&***path)))
                    .map(|(index, _)| index),
            );
        }
        if SettingsStore::global(cx).settings.show_wide_page_alone {
            solo.extend(self.wide.iter().copied());
        }
        solo
    }

    /// `index`에서 시작하는 화면에 함께 보일 인덱스.
    fn screen_at(&self, index: usize, cx: &App) -> Vec<usize> {
        if self.view_mode(cx).is_dual() {
            let looped = SettingsStore::global(cx).settings.loop_navigation;
            layout::screen_indices(index, self.list.len(), looped, &self.solo(cx))
        } else if index < self.list.len() {
            vec![index]
        } else {
            Vec::new()
        }
    }

    pub fn open_path(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        let options = Self::list_options(cx);
        let resume = {
            let store = SettingsStore::global(cx);
            store
                .settings
                .resume_reading
                .then(|| store.archive_progress(&path).cloned())
                .flatten()
        };
        self.open_seq += 1;
        let seq = self.open_seq;
        self.opening = true;
        self.opening_started = Instant::now();
        self.error = None;
        cx.notify();
        // 금방 끝나는 열기에는 표시가 깜빡이지 않게, 늦어질 때만 보이도록 한 번 깨운다.
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(LOADING_DELAY).await;
            this.update(cx, |_, cx| cx.notify()).ok();
        })
        .detach();
        let source = path.clone();
        let task = cx.background_spawn(async move {
            let file = Path::new(&source);
            if is_archive_file(file) {
                let images = ops::get_archive_images_impl(file)?;
                // 이어보기 기록은 열 때의 identity와 같을 때만 믿는다(§8).
                // 같은 경로의 CBZ가 다른 내용으로 교체되면 첫 페이지에서 연다.
                let identity = FileIdentity::capture(file);
                let items: Vec<Arc<str>> = images
                    .images
                    .iter()
                    .map(|name| Arc::from(&**name))
                    .collect();
                // 저장된 엔트리가 목록에 그대로 있을 때만 이어 본다.
                let resumed = resume
                    .filter(|progress| progress.matches_identity(identity))
                    .and_then(|progress| items.iter().position(|name| **name == *progress.entry));
                Ok::<_, AppError>(Listing {
                    items,
                    index: resumed.unwrap_or(0),
                    resumed: resumed.is_some_and(|index| index > 0),
                    archive: Some(Arc::from(source.as_str())),
                    identity,
                    availability: Vec::new(),
                    comic: ops::get_comic_info_blocking(&source),
                })
            } else {
                let listing = ops::get_directory_images_impl(&source, Some(options))?;
                if listing.images.is_empty() {
                    return Err(AppError::not_found("No images found in directory"));
                }
                Ok(Listing {
                    index: listing.current_index,
                    availability: listing.availability,
                    items: listing.images,
                    resumed: false,
                    archive: None,
                    identity: None,
                    comic: Ok(None),
                })
            }
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                if this.open_seq != seq {
                    return;
                }
                this.opening = false;
                match result {
                    Ok(listing) => {
                        if SettingsStore::global(cx).settings.record_recent_files {
                            SettingsStore::push_recent(cx, &path);
                        }
                        this.commit_listing(listing, window, cx);
                    }
                    Err(error) => {
                        let key = if is_archive_file(Path::new(&path)) {
                            "toast.load.archiveFail"
                        } else {
                            "toast.load.imageFail"
                        };
                        window.toast(Toast::error(error.message.clone()).title(t(key)), cx);
                        this.error = Some(error);
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    fn commit_listing(&mut self, listing: Listing, window: &mut Window, cx: &mut Context<Self>) {
        self.clear_pages(window, cx);
        self.thumbs.clear();
        self.thumbs_pending.clear();
        self.list = Arc::new(listing.items);
        self.availability = listing.availability;
        self.archive = listing.archive;
        self.archive_identity = listing.identity;
        self.comic_failed = listing.comic.is_err();
        self.comic = listing.comic.ok().flatten();
        let settings = &SettingsStore::global(cx).settings;
        self.progress_held = self.archive.is_some() && !settings.resume_reading;
        self.comic_mode = self.archive.as_ref().and_then(|_| {
            layout::comic_view_mode(
                settings.view_mode,
                settings.comic_auto_dual_view,
                self.comic.as_ref(),
            )
        });
        let mut index = listing.index;
        if self.view_mode(cx).is_dual() {
            // 저장 위치가 쌍 중간이면 쌍 시작으로 맞춰 연다.
            index = layout::pair_start(index, self.list.len(), &self.solo(cx), None);
        }
        if listing.resumed {
            let entity = cx.entity();
            push_timed_notification(
                window,
                gpui_kit::component::notification::Notification::info(t_with(
                    "toast.archive.resumed",
                    &[("index", &(index + 1)), ("total", &self.list.len())],
                ))
                .action(move |_, _, _| {
                    let entity = entity.clone();
                    gpui_kit::component::button::Button::new("resume-start-over")
                        .label(t("toast.archive.startOver"))
                        .on_click(move |_, window, cx| {
                            entity.update(cx, |this, cx| this.go_to(0, window, cx));
                        })
                }),
                cx,
            );
        }
        self.go_to(index, window, cx);
    }

    /// 목록을 바꾼다(휴지통 이동 뒤 등). 인덱스가 밀리므로 페이지 캐시를 비운다.
    pub(super) fn replace_list(
        &mut self,
        items: Vec<Arc<str>>,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.clear_pages(window, cx);
        self.list = Arc::new(items);
        // 인덱스가 밀려 파일별 상태 배열이 맞지 않게 되므로 버린다.
        self.availability.clear();
        self.go_to(index, window, cx);
    }

    pub(super) fn drop_picture(picture: &Picture, window: &mut Window, cx: &mut Context<Self>) {
        for image in picture.images() {
            cx.drop_image(image.clone(), Some(window));
        }
    }

    /// 페이지 캐시와 표시 상태를 비우고 텍스처를 아틀라스에서 내린다.
    pub(super) fn clear_pages(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.list_gen += 1;
        for (_, page) in std::mem::take(&mut self.pages) {
            Self::drop_picture(page.picture(), window, cx);
        }
        if let Some(oriented) = self.oriented.take() {
            Self::drop_picture(&oriented, window, cx);
        }
        self.page_loading.clear();
        self.decode_queue.clear();
        self.decode_running = false;
        self.page_failed.clear();
        self.wide.clear();
        self.shown.clear();
        self.picture = None;
        self.info = None;
        self.orientation = Orientation::default();
        self.pending_commit = false;
        self.pending_back = false;
        self.wt_dims.clear();
        self.wt_anchor = 0;
        self.wt_offset = 0.0;
        self.wt_pending = 0.0;
    }

    pub(super) fn close_image(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // 고른 글자가 있으면 첫 입력은 선택만 푼다.
        if self.ocr_select.is_some() {
            self.clear_ocr_selection(cx);
            return;
        }
        self.open_seq += 1;
        self.opening = false;
        self.error = None;
        self.clear_pages(window, cx);
        self.list = Arc::new(Vec::new());
        self.archive = None;
        self.archive_identity = None;
        self.comic = None;
        self.comic_failed = false;
        self.comic_mode = None;
        self.index = 0;
        self.thumbs.clear();
        self.thumbs_pending.clear();
        self.grid_open = false;
        window.set_window_title(&t("app.title"));
        cx.notify();
    }

    /// 파일이나 아카이브를 여는 데 시간이 걸려 로딩 표시를 보여야 하는지.
    pub(super) fn opening_visible(&self) -> bool {
        self.opening && self.opening_started.elapsed() >= LOADING_DELAY
    }

    /// 현재 설정으로 페이지를 다시 디코드한다(재시도, 해상도·표시 정책 변경).
    pub(super) fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.list.is_empty() {
            return;
        }
        let index = self.index;
        let (anchor, offset) = (self.wt_anchor, self.wt_offset);
        self.clear_pages(window, cx);
        self.wt_anchor = anchor;
        self.wt_offset = offset;
        self.index = index;
        self.pending_commit = true;
        self.error = None;
        self.sync_pages(window, cx);
    }

    /// `index`를 화면 시작으로 삼아 이동한다.
    pub(super) fn go_to(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.list.len() {
            return;
        }
        self.index = index;
        self.pending_commit = true;
        self.error = None;
        self.skip_budget = self.list.len();
        self.nav_started = Instant::now();
        // 이전 화면 유지 시간이 지나면 미리보기로 바꿔 그리도록 한 번 깨운다.
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(PREVIOUS_HOLD).await;
            this.update(cx, |_, cx| cx.notify()).ok();
        })
        .detach();
        if self.view_mode(cx) == ViewMode::Webtoon {
            self.wt_anchor = index;
            self.wt_offset = 0.0;
        }
        self.sync_pages(window, cx);
        self.request_thumbs(window, cx);
        cx.notify();
    }

    /// 이전/다음. 양쪽 보기는 화면 단위, 웹툰은 이미지 단위 스크롤 이동이다.
    pub(super) fn navigate(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let total = self.list.len();
        let looped = SettingsStore::global(cx).settings.loop_navigation;
        self.last_forward = forward;
        if !forward && self.view_mode(cx).is_dual() && self.check_previous_pages(window, cx) {
            // 앞의 두 장이 넓은 페이지인지 알아야 건너뛰지 않는다. 로드가 끝나면 이어서 넘긴다.
            self.pending_back = true;
            return;
        }
        self.pending_back = false;
        let next = if self.view_mode(cx).is_dual() {
            layout::dual_step(self.index, total, looped, forward, &self.solo(cx))
        } else {
            layout::step_index(self.index, total, 1, looped, forward)
        };
        if let Some(next) = next {
            self.go_to(next, window, cx);
        }
    }

    /// 이전 화면이 될 앞의 두 장 중 크기를 모르는 페이지를 요청한다. 기다려야 하면 `true`.
    fn check_previous_pages(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let mut waiting = false;
        for index in [self.index.checked_sub(1), self.index.checked_sub(2)]
            .into_iter()
            .flatten()
        {
            if self.wt_dims.contains_key(&index) || self.page_failed.contains_key(&index) {
                continue;
            }
            self.ensure_page(index, false, window, cx);
            waiting = true;
        }
        waiting
    }

    pub(super) fn jump_by(&mut self, offset: isize, window: &mut Window, cx: &mut Context<Self>) {
        if self.list.len() < 2 {
            return;
        }
        let looped = SettingsStore::global(cx).settings.loop_navigation;
        let target = layout::offset_index(self.index, self.list.len(), offset, looped);
        self.last_forward = offset >= 0;
        self.jump_to(target, window, cx);
    }

    /// 임의 인덱스로 점프한다. 양쪽 보기에서는 쌍 시작으로 스냅한다.
    pub(super) fn jump_to(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.list.len() {
            return;
        }
        let target = if self.view_mode(cx).is_dual() {
            layout::pair_start(index, self.list.len(), &self.solo(cx), None)
        } else {
            index
        };
        if target != self.index || self.shown.is_empty() {
            self.go_to(target, window, cx);
        }
    }

    /// 헤더·설정에서 보기 모드를 직접 고른다. 아카이브의 자동 결정은 해제된다.
    pub(super) fn set_view_mode(
        &mut self,
        mode: ViewMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        SettingsStore::update(cx, |settings| settings.view_mode = mode);
        self.comic_mode = None;
        self.view_mode_changed(window, cx);
    }

    /// 보기 모드나 배치 설정이 바뀐 뒤 현재 위치를 새 배치에 맞춘다.
    pub(super) fn view_mode_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.list.is_empty() {
            cx.notify();
            return;
        }
        let index = if self.view_mode(cx).is_dual() {
            layout::pair_start(self.index, self.list.len(), &self.solo(cx), None)
        } else {
            self.index
        };
        self.go_to(index, window, cx);
    }

    /// 지금 필요한 페이지(현재 화면과 프리페치 범위)를 요청하고, 준비됐으면 화면을 바꾼다.
    pub(super) fn sync_pages(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.list.is_empty() {
            return;
        }
        let mode = self.view_mode(cx);
        if mode == ViewMode::Webtoon {
            self.webtoon_sync(window, cx);
            return;
        }
        let screen = self.screen_at(self.index, cx);
        for index in &screen {
            self.ensure_page(*index, true, window, cx);
        }
        self.try_commit(window, cx);

        let cache_mode = SettingsStore::global(cx).settings.cache_mode;
        let base = prefetch_distance(cache_mode);
        let budget = cache_budget(cache_mode);
        let total = self.list.len();
        let (before, after) = if self.view_mode(cx).is_dual() {
            (base + 1, base + 2)
        } else {
            (base, base)
        };
        let keep: HashSet<usize> = (self.index.saturating_sub(before + 1)
            ..=(self.index + after + 1).min(total - 1))
            .chain(screen.iter().copied())
            .chain(self.shown.iter().copied())
            .collect();
        // 예산 때문에 내려놓을 페이지를 선취 요청보다 먼저 판단해, 내려놓은
        // 페이지를 곧바로 다시 디코드하지 않는다(§9.1). 현재 화면과 표시
        // 페이지는 예산이 넘어도 유지한다.
        let protect: HashSet<usize> = screen
            .iter()
            .copied()
            .chain(self.shown.iter().copied())
            .collect();
        self.cancel_queued_outside(&keep);
        let evicted = self.evict_except(&keep, &protect, window, cx);
        if base > 0 && self.cache_bytes() < budget.bytes {
            for index in self.index.saturating_sub(before)..=(self.index + after).min(total - 1) {
                if evicted.contains(&index) {
                    continue;
                }
                self.ensure_page(index, false, window, cx);
            }
        }
    }

    pub(super) fn evict_except(
        &mut self,
        keep: &HashSet<usize>,
        protect: &HashSet<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<usize> {
        let stale: Vec<usize> = self
            .pages
            .keys()
            .copied()
            .filter(|index| !keep.contains(index))
            .collect();
        for index in &stale {
            if let Some(page) = self.pages.remove(index) {
                // 단일 보기에서 표시 중인 그림은 `picture`가 같은 Arc를 들고 있다.
                let displayed = self
                    .picture
                    .as_ref()
                    .is_some_and(|shown| Arc::ptr_eq(shown, page.picture()));
                if !displayed {
                    Self::drop_picture(page.picture(), window, cx);
                }
            }
        }
        self.page_failed.retain(|index, _| keep.contains(index));

        // 항목 수와 바이트 예산이 넘으면 가장 먼 페이지부터 내려놓는다(§9.1).
        let budget = cache_budget(SettingsStore::global(cx).settings.cache_mode);
        let bytes: HashMap<usize, u64> = self
            .pages
            .iter()
            .map(|(index, page)| (*index, page.bytes()))
            .collect();
        let plan = evict_plan(&bytes, self.index, protect, budget.items, budget.bytes);
        for index in &plan {
            let Some(page) = self.pages.remove(index) else {
                continue;
            };
            let displayed = self
                .picture
                .as_ref()
                .is_some_and(|shown| Arc::ptr_eq(shown, page.picture()));
            if !displayed {
                Self::drop_picture(page.picture(), window, cx);
            }
        }
        plan
    }

    /// 페이지 캐시가 지금 들고 있는 디코드 바이트 합계.
    pub(super) fn cache_bytes(&self) -> u64 {
        self.pages.values().map(Page::bytes).sum()
    }

    pub(super) fn current_page_is_full(&self) -> bool {
        self.pages.get(&self.index).is_some_and(Page::is_full)
    }

    /// 프리뷰는 화면에만 올린다. ImageInfo 기반 동작은 full 디코드가 끝날 때까지
    /// 비활성으로 두고, 늦게 끝난 프리뷰가 full 결과를 덮지 않도록 Page 상태도 유지한다.
    fn show_preview_current(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index != self.index || !self.pending_commit || self.view_mode(cx) != ViewMode::Single {
            return;
        }
        let Some(Page::Preview { picture, .. }) = self.pages.get(&index) else {
            return;
        };
        let picture = picture.clone();
        if let Some(oriented) = self.oriented.take() {
            Self::drop_picture(&oriented, window, cx);
        }
        if let Some(previous) = self.picture.replace(picture.clone()) {
            let cached = self
                .pages
                .values()
                .any(|page| Arc::ptr_eq(page.picture(), &previous));
            if !cached {
                Self::drop_picture(&previous, window, cx);
            }
        }
        self.info = None;
        self.orientation = Orientation::default();
        self.playing = false;
        self.frame = 0;
        self.info_seq += 1;
        self.info_data = Default::default();
        self.ocr_shown = None;
        self.ocr_select = None;
        self.ocr_drag = None;
        self.close_video();
        self.drop_pixel_view(window, cx);
        self.shown = vec![index];
        self.fit_locked = true;
        self.position = Offset::default();
        self.apply_fit(cx);
    }

    /// 페이지가 캐시에 없으면 백그라운드에서 디코드한다. `protect`는 표시용 추출물을
    /// 캐시 정리에서 보호할지이며, 프리페치는 보호 슬롯을 쓰지 않는다.
    pub(super) fn ensure_page(
        &mut self,
        index: usize,
        protect: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.pages.get(&index).is_some_and(Page::is_full)
            || self.page_loading.contains(&index)
            || self.page_failed.contains_key(&index)
        {
            return;
        }
        if index >= self.list.len() {
            return;
        }
        self.page_loading.insert(index);
        self.decode_seq += 1;
        self.decode_queue.push(DecodeJob {
            index,
            protect,
            seq: self.decode_seq,
        });
        self.pump_decode(window, cx);
    }

    /// 실행 중인 full 디코드가 없으면 대기열에서 하나를 골라 시작한다.
    fn pump_decode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.decode_running {
            return;
        }
        let Some(position) = next_decode_job(&self.decode_queue, self.index) else {
            return;
        };
        let DecodeJob { index, protect, seq } = self.decode_queue.swap_remove(position);
        let Some(item) = self.list.get(index).cloned() else {
            self.page_loading.remove(&index);
            self.pump_decode(window, cx);
            return;
        };
        self.decode_running = true;
        let generation = self.list_gen;
        let settings = &SettingsStore::global(cx).settings;
        let max_side = settings.max_resolution.max_side();
        let scaling = Some(settings.image_scaling_mode.as_str().to_owned());
        let archive = self.archive.clone();
        let preview_task = if archive.is_none() && self.view_mode(cx) == ViewMode::Single {
            let preview_path = PathBuf::from(&*item);
            let preview_side = max_side.unwrap_or(2048).min(2048);
            Some(cx.background_spawn(async move {
                araview_core::display::decode_preview_for_display(&preview_path, preview_side)
                    .map(|decoded| Arc::new(picture::build(decoded, Orientation::default(), None)))
            }))
        } else {
            None
        };
        let task = cx.background_spawn(async move {
            let (info, archive_preview) = match &archive {
                Some(archive) => (
                    ops::load_archive_image_blocking(
                        archive,
                        &item,
                        max_side,
                        Some(protect),
                        scaling,
                    )?,
                    false,
                ),
                None if is_archive_file(Path::new(&*item)) => {
                    // 폴더 안의 아카이브는 첫 페이지를 미리 보여준다.
                    let entries = ops::get_archive_images_impl(Path::new(&*item))?;
                    let first = entries
                        .images
                        .first()
                        .ok_or_else(|| AppError::not_found("No images found in archive"))?;
                    (
                        ops::load_archive_image_blocking(
                            &item,
                            first,
                            max_side,
                            Some(protect),
                            scaling,
                        )?,
                        true,
                    )
                }
                None => (ops::load_image_blocking(&item, max_side, scaling)?, false),
            };
            let picture = picture::load(
                Path::new(&info.file_path),
                Path::new(&info.source_path),
                Orientation::default(),
            )?;
            let picture = Arc::new(picture);
            let bytes = picture::retained_bytes(&picture);
            Ok::<_, AppError>(Page::Full {
                info,
                picture,
                bytes,
                archive_preview,
            })
        });
        if let Some(preview_task) = preview_task {
            cx.spawn_in(window, async move |this, cx| {
                let Some(picture) = preview_task.await else {
                    return;
                };
                this.update_in(cx, |this, window, cx| {
                    let sequence_is_stale = this
                        .pages
                        .get(&index)
                        .is_some_and(|page| {
                            preview_is_stale(page.is_full(), page.preview_sequence(), seq)
                        });
                    if this.list_gen != generation
                        || this.index != index
                        || !this.pending_commit
                        || this.view_mode(cx) != ViewMode::Single
                        || sequence_is_stale
                    {
                        Self::drop_picture(&picture, window, cx);
                        return;
                    }
                    let bytes = picture::retained_bytes(&picture);
                    let preview = Page::Preview {
                        picture: picture.clone(),
                        bytes,
                        sequence: seq,
                    };
                    if let Some(previous) = this.pages.insert(index, preview) {
                        let displayed = this
                            .picture
                            .as_ref()
                            .is_some_and(|shown| Arc::ptr_eq(shown, previous.picture()));
                        if !displayed {
                            Self::drop_picture(previous.picture(), window, cx);
                        }
                    }
                    this.show_preview_current(index, window, cx);
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                if this.list_gen != generation {
                    return;
                }
                this.decode_running = false;
                this.page_loading.remove(&index);
                match result {
                    Ok(page) => {
                        if page.picture().width > page.picture().height
                            && !page.is_archive_preview()
                        {
                            this.wide.insert(index);
                        }
                        this.webtoon_page_arrived(index, page.picture(), cx);
                        if let Some(previous) = this.pages.insert(index, page) {
                            let displayed = this
                                .picture
                                .as_ref()
                                .is_some_and(|shown| Arc::ptr_eq(shown, previous.picture()));
                            if !displayed {
                                Self::drop_picture(previous.picture(), window, cx);
                            }
                        }
                    }
                    Err(error) => {
                        this.page_failed.insert(index, error);
                    }
                }
                if this.pending_back {
                    // 뒤로 넘기려고 기다리던 페이지가 도착했으면 이어서 넘긴다.
                    this.navigate(false, window, cx);
                }
                this.sync_pages(window, cx);
                this.pump_decode(window, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// 범위 밖으로 밀려난 대기 요청을 시작 전에 취소한다. 이미 시작한 디코드는
    /// 끝까지 돌고 결과는 캐시 정리가 맡는다.
    fn cancel_queued_outside(&mut self, keep: &HashSet<usize>) {
        let page_loading = &mut self.page_loading;
        self.decode_queue.retain(|job| {
            let wanted = keep.contains(&job.index);
            if !wanted {
                page_loading.remove(&job.index);
            }
            wanted
        });
    }

    /// 이동한 화면의 페이지가 모두 도착했으면 화면을 바꾼다.
    fn try_commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.pending_commit {
            return;
        }
        let screen = self.screen_at(self.index, cx);
        if screen.iter().any(|index| self.page_loading.contains(index)) {
            return;
        }
        if let Some(error) = self.page_failed.get(&self.index).cloned() {
            self.current_failed(error, window, cx);
            return;
        }
        if !self.pages.get(&self.index).is_some_and(Page::is_full) {
            return;
        }
        self.pending_commit = false;
        // 로드 실패 페이지는 화면에서 뺀다.
        self.shown = screen
            .into_iter()
            .filter(|index| self.pages.contains_key(index))
            .collect();
        self.commit_current(window, cx);
    }

    fn current_failed(&mut self, error: AppError, window: &mut Window, cx: &mut Context<Self>) {
        let skip = SettingsStore::global(cx).settings.skip_broken_files;
        if skip && self.skip_budget > 0 {
            self.skip_budget -= 1;
            let total = self.list.len();
            let looped = SettingsStore::global(cx).settings.loop_navigation;
            if let Some(next) = layout::step_index(self.index, total, 1, looped, self.last_forward)
                .filter(|next| !self.page_failed.contains_key(next))
            {
                window.toast(Toast::warning(t("toast.load.skipped")), cx);
                self.index = next;
                self.sync_pages(window, cx);
                return;
            }
        }
        self.pending_commit = false;
        window.toast(
            Toast::error(error.message.clone()).title(t("toast.load.imageFail")),
            cx,
        );
        self.error = Some(error);
        cx.notify();
    }

    /// 현재 인덱스의 페이지를 기준 그림으로 삼고 보기 상태를 초기화한다.
    pub(super) fn commit_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(page) = self.pages.get(&self.index).filter(|page| page.is_full()) else {
            return;
        };
        let Some(info) = page.info().cloned() else {
            return;
        };
        let picture = page.picture().clone();
        self.error = None;
        self.orientation = Orientation::default();
        if let Some(oriented) = self.oriented.take() {
            Self::drop_picture(&oriented, window, cx);
        }
        self.playing = picture.is_animated()
            && !self.view_mode(cx).is_dual()
            && !crate::platform::reduced_motion();
        self.frame = 0;
        self.loops_done = 0;
        self.frame_started = Instant::now();
        if let Some(previous) = self.picture.replace(picture.clone()) {
            let cached = self
                .pages
                .values()
                .any(|page| Arc::ptr_eq(page.picture(), &previous));
            if !cached {
                Self::drop_picture(&previous, window, cx);
            }
        } else {
            self.picture = Some(picture);
        }
        self.info = Some(info);
        if self.has_video(cx) {
            self.open_video(window, cx);
        } else {
            self.close_video();
        }
        if let Some(info) = &self.info {
            window.set_window_title(&self.display_name(info));
        }
        self.fit_locked = true;
        self.position = Offset::default();
        self.apply_fit(cx);
        self.save_progress(cx);
        self.load_info(window, cx);
        cx.notify();
    }

    pub(super) fn save_progress(&mut self, cx: &mut Context<Self>) {
        if self.progress_held {
            if self.index == 0 {
                return;
            }
            self.progress_held = false;
        }
        if let Some(archive) = self.archive.clone()
            && let Some(entry) = self.list.get(self.index)
        {
            SettingsStore::set_archive_progress(
                cx,
                &archive,
                ArchiveProgress {
                    entry: entry.to_string(),
                    index: self.index,
                    total: self.list.len(),
                    identity: self.archive_identity,
                },
            );
        }
    }

    /// 회전/반전이 바뀌면 같은 렌더 바이트를 새 방향으로 다시 만든다(단일 보기).
    fn reorient(&mut self, orientation: Orientation, window: &mut Window, cx: &mut Context<Self>) {
        let Some(info) = &self.info else {
            return;
        };
        // 동영상은 다음 프레임부터 새 방향을 적용한다. 정지 상태에서는 다음
        // 프레임이 오지 않으므로 현재 위치를 다시 열어 방향을 즉시 반영한다.
        if self.has_video(cx) {
            self.orientation = orientation;
            self.orient_seq += 1;
            if let Some(player) = &self.video
                && !player.is_playing()
            {
                player.seek(player.position_ms());
            }
            cx.notify();
            return;
        }
        self.orientation = orientation;
        self.orient_seq += 1;
        let seq = self.orient_seq;
        let generation = self.list_gen;
        let path = PathBuf::from(&info.file_path);
        let source = PathBuf::from(&info.source_path);
        let task = cx.background_spawn(async move { picture::load(&path, &source, orientation) });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                if this.orient_seq != seq || this.list_gen != generation {
                    return;
                }
                let Ok(picture) = result else {
                    return;
                };
                let picture = Arc::new(picture);
                if let Some(previous) = this.oriented.replace(picture.clone()) {
                    Self::drop_picture(&previous, window, cx);
                }
                this.picture = Some(picture);
                this.frame = 0;
                this.frame_started = Instant::now();
                this.fit_locked = true;
                this.apply_fit(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn rotate(&mut self, degrees: u16, window: &mut Window, cx: &mut Context<Self>) {
        let mut next = self.orientation;
        next.rotation = (next.rotation + degrees) % 360;
        self.reorient(next, window, cx);
    }

    pub(super) fn flip(&mut self, horizontal: bool, window: &mut Window, cx: &mut Context<Self>) {
        let mut next = self.orientation;
        if horizontal {
            next.flip_h = !next.flip_h;
        } else {
            next.flip_v = !next.flip_v;
        }
        self.reorient(next, window, cx);
    }

    /// `0`: 위치·회전·반전을 초기화하고 자동 맞춤으로 돌아간다.
    pub(super) fn reset_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        SettingsStore::update(cx, |settings| {
            settings.fit_mode = crate::settings::FitMode::Auto;
        });
        self.fit_locked = true;
        self.wt_zoom = 1.0;
        if self.orientation != Orientation::default() {
            self.reorient(Orientation::default(), window, cx);
        } else {
            self.apply_fit(cx);
            cx.notify();
        }
    }

    /// 폴더 미리보기 중인 아카이브를 만화 모드로 연다.
    pub(super) fn open_previewed_archive(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let previewing = self
            .pages
            .get(&self.index)
            .is_some_and(Page::is_archive_preview);
        if let Some(path) = self.list.get(self.index).filter(|_| previewing) {
            self.open_path(path.to_string(), window, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(index: usize, protect: bool, seq: u64) -> DecodeJob {
        DecodeJob {
            index,
            protect,
            seq,
        }
    }

    #[::core::prelude::v1::test]
    fn decode_queue_picks_current_screen_then_nearest_then_oldest() {
        let queue = vec![
            job(9, false, 1),
            job(6, false, 2),
            job(4, false, 3),
            job(5, true, 4),
        ];
        assert_eq!(next_decode_job(&queue, 5), Some(3));
        let queue = vec![job(9, false, 1), job(6, false, 2), job(4, false, 3)];
        // 거리가 같으면 먼저 요청한 것이 앞선다.
        assert_eq!(next_decode_job(&queue, 5), Some(1));
        assert_eq!(next_decode_job(&[], 5), None);
    }

    /// `use gpui_kit::*`가 gpui의 `test` 매크로를 끌어오므로 빌트인 `#[test]`를
    /// 전체 경로로 고른다.
    #[::core::prelude::v1::test]
    fn stale_previews_cannot_replace_full_or_newer_pages() {
        assert!(!preview_is_stale(false, None, 1));
        assert!(!preview_is_stale(false, Some(1), 2));
        assert!(preview_is_stale(false, Some(2), 2));
        assert!(preview_is_stale(true, None, 1));
    }

    #[::core::prelude::v1::test]
    fn byte_budget_evicts_farthest_first() {
        let bytes: HashMap<usize, u64> = (8..=13).map(|index| (index, 10)).collect();
        plan_assert(&bytes, 10, HashSet::new(), usize::MAX, 35, &[13, 12, 8]);
    }

    /// 현재 화면·표시 페이지는 예산을 넘겨도 내려놓지 않는다. 큰 이미지 몇 장으로
    /// 현재 화면만 예산을 초과해도 현재 페이지가 즉시 evict되지 않아야 한다.
    #[::core::prelude::v1::test]
    fn the_current_screen_stays_even_when_the_budget_is_exceeded() {
        let bytes: HashMap<usize, u64> = [(10, 600 << 20), (11, 600 << 20)].into_iter().collect();
        let protected = HashSet::from([10, 11]);
        plan_assert(&bytes, 10, protected, usize::MAX, 512 * MB, &[]);
    }

    /// 항목 상한은 감리 대상을 예외로 하고 가장 먼 것부터 자른다. 동거리
    /// 우선순위가 뒤쪽(큰 인덱스)임은 결정적으로 확인한다.
    #[::core::prelude::v1::test]
    fn item_limit_trims_farthest_first() {
        let bytes: HashMap<usize, u64> = [(3, 1), (4, 1), (5, 1), (6, 1), (20, 1)]
            .into_iter()
            .collect();
        // 20(거리 15)이 먼저, 그다음 6과 4(거리 1, 큰 인덱스 우선), 3은 보호.
        plan_assert(&bytes, 5, HashSet::from([3]), 2, u64::MAX, &[20, 6, 4]);
    }

    /// 보호된 페이지가 멀리 있어도 그 페이지는 예산보다 먼저 남고, 보호 대상만
    /// 남으면 예산을 넘어도 정지한다.
    #[::core::prelude::v1::test]
    fn protected_pages_survive_the_byte_budget() {
        let bytes: HashMap<usize, u64> = [(5, 200 * MB), (6, 200 * MB), (30, 200 * MB)]
            .into_iter()
            .collect();
        // 30(거리 24)을 내려놓아도 400MB로 300MB를 넘지만, 5와 6은 보호 대상이라 남는다.
        plan_assert(
            &bytes,
            6,
            HashSet::from([5, 6]),
            usize::MAX,
            300 * MB,
            &[30],
        );
    }

    /// 예산 표(SPEC §9.1)와 코드가 같다.
    #[::core::prelude::v1::test]
    fn cache_budgets_match_the_mode_table() {
        assert_eq!(cache_budget(CacheMode::Off).items, 1);
        assert_eq!(cache_budget(CacheMode::Nearby).items, 24);
        assert_eq!(cache_budget(CacheMode::Extended).items, 64);
        assert_eq!(cache_budget(CacheMode::Nearby).bytes, 512 * MB);
        assert_eq!(cache_budget(CacheMode::Extended).bytes, 1024 * MB);
        assert_eq!(cache_budget(CacheMode::Memory1Gb).bytes, 1024 * MB);
        assert_eq!(cache_budget(CacheMode::Memory2Gb).bytes, 2048 * MB);
    }

    const MB: u64 = 1024 * 1024;

    fn plan_assert(
        bytes: &HashMap<usize, u64>,
        anchor: usize,
        protected: HashSet<usize>,
        item_limit: usize,
        byte_limit: u64,
        expected: &[usize],
    ) {
        assert_eq!(
            evict_plan(bytes, anchor, &protected, item_limit, byte_limit),
            expected
        );
    }
}
