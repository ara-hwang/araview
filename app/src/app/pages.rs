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
use araview_core::comic_info::ComicInfo;
use araview_core::file_availability::FileAvailability;
use araview_core::file_identity::FileIdentity;
use araview_core::image::{ImageInfo, is_archive_file};
use araview_core::ops;
use gpui_kit::*;

use super::{AraView, Listing};

mod state;
use crate::geometry::Offset;
use crate::i18n::{t, t_with};
use crate::layout::{self, CoverLayout, Solo};
use crate::picture::{self, Orientation, Picture};
use crate::settings::{ArchiveProgress, CacheMax, CacheMode, SettingsStore, ViewMode};
pub(super) use state::{Nav, PageSlots, ViewError};

pub(super) enum Page {
    Preview {
        picture: Arc<Picture>,
        bytes: u64,
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

    pub fn is_archive_preview(&self) -> bool {
        matches!(
            self,
            Self::Full {
                archive_preview: true,
                ..
            }
        )
    }
}

/// 새 이미지가 디코드될 때까지 이전 이미지를 붙잡는 최대 시간(SPEC §3.3).
/// 열기가 이 시간보다 길어지면 로딩 표시를 보인다.
pub(super) const LOADING_DELAY: std::time::Duration = std::time::Duration::from_millis(200);
pub(super) const PREVIOUS_HOLD: std::time::Duration = std::time::Duration::from_millis(300);

/// 캐시 모드별 프리페치 거리(SPEC §9.1). 폴더 전체 모드는 목록 길이를 쓴다.
fn prefetch_distance(mode: CacheMode, total: usize) -> usize {
    match mode {
        CacheMode::Off => 0,
        CacheMode::Nearby => 1,
        CacheMode::Extended => 3,
        CacheMode::Folder | CacheMode::Memory1Gb | CacheMode::Memory2Gb => total,
    }
}

/// 페이지 캐시 예산: 항목 수와 디코드 바이트의 이중 상한(SPEC §9.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CacheBudget {
    pub items: usize,
    pub bytes: u64,
}

/// 폴더 전체 모드에서 한 번에 대기시키는 프리페치 요청 수. full 디코드는 한 번에
/// 하나만 돌므로 더 많이 쌓아도 이득이 없고, 긴 목록에서 대기열이 넘치지 않게 한다.
const FOLDER_PREFETCH_BATCH: usize = 8;

fn cache_budget(mode: CacheMode, max: CacheMax) -> CacheBudget {
    match mode {
        CacheMode::Off => CacheBudget {
            items: 1,
            bytes: u64::MAX,
        },
        CacheMode::Nearby => CacheBudget {
            items: 24,
            bytes: max.bytes(),
        },
        CacheMode::Extended => CacheBudget {
            items: 64,
            bytes: max.bytes(),
        },
        // 거리 창 대신 바이트 상한이 한도를 정한다.
        CacheMode::Folder | CacheMode::Memory1Gb | CacheMode::Memory2Gb => CacheBudget {
            items: usize::MAX,
            bytes: max.bytes(),
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

/// 예산 때문에 내려놓은 페이지 가운데 지금 위치에서 가장 가까운 거리. 같은 위치,
/// 같은 바이트 예산에서는 이 거리 이상인 페이지를 다시 미리 로드하지 않는다. 그렇지
/// 않으면 예산보다 큰 목록에서 경계 밖 페이지를 로드하고 내려놓기를 끝없이
/// 되풀이한다(SPEC §9.1 "상한에 닿으면 더 요청하지 않고"). 위치나 예산이 바뀌면
/// 효력을 잃는다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct BudgetReach {
    anchor: usize,
    budget: u64,
    distance: usize,
}

impl BudgetReach {
    /// 이번 정리(`evicted`)를 반영한 경계. 같은 위치·예산의 이전 경계와는 더 가까운 쪽을 남긴다.
    fn after_evict(
        previous: Option<Self>,
        evicted: &[usize],
        anchor: usize,
        budget: u64,
    ) -> Option<Self> {
        let previous = previous.filter(|reach| reach.anchor == anchor && reach.budget == budget);
        let Some(distance) = evicted.iter().map(|index| index.abs_diff(anchor)).min() else {
            return previous;
        };
        let distance = previous.map_or(distance, |reach| reach.distance.min(distance));
        Some(Self {
            anchor,
            budget,
            distance,
        })
    }

    /// `index`가 예산 경계 밖이라 미리 로드하지 않아야 하는지.
    fn blocks(&self, anchor: usize, budget: u64, index: usize) -> bool {
        self.anchor == anchor && self.budget == budget && index.abs_diff(anchor) >= self.distance
    }
}

/// 아카이브의 ComicInfo.xml 상태(SPEC §8, §10).
#[derive(Debug)]
pub(super) enum Comic {
    /// 아카이브가 아니거나 메타데이터가 없다.
    Absent,
    Parsed(Box<ComicInfo>),
    /// 파싱에 실패했다. 정보 패널에 에러 문구만 남기고 표지 판정은 기본값을 쓴다.
    Failed,
}

impl Comic {
    pub(super) fn from_listing(result: Result<Option<ComicInfo>, AppError>) -> Self {
        match result {
            Ok(Some(comic)) => Self::Parsed(Box::new(comic)),
            Ok(None) => Self::Absent,
            Err(_) => Self::Failed,
        }
    }

    pub(super) fn info(&self) -> Option<&ComicInfo> {
        match self {
            Self::Parsed(comic) => Some(comic.as_ref()),
            Self::Absent | Self::Failed => None,
        }
    }

    pub(super) fn failed(&self) -> bool {
        matches!(self, Self::Failed)
    }
}

impl AraView {
    /// 화면에 적용되는 보기 모드. 아카이브는 세션 모드(`comic_mode`), 일반
    /// 이미지는 설정값을 쓴다.
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
        let comic = self.archive.as_ref().and(self.comic.info());
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
        // 로드에 실패한 페이지도 혼자 한 화면이다. 짝을 지은 채 두면 손상 파일을
        // 건너뛸 때 화면이 한 장만 밀려 앞뒤 화면이 같은 페이지를 겹쳐 보여준다.
        solo.extend(self.pages.failed_indices());
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
        self.opening = Some(Instant::now());
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
                    root: None,
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
                    root: Some(Arc::from(source.as_str())),
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
                this.opening = None;
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
                        this.error = Some(ViewError::Open { path, error });
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
        self.list_root = listing.root;
        self.archive_identity = listing.identity;
        self.comic = Comic::from_listing(listing.comic);
        let settings = &SettingsStore::global(cx).settings;
        self.progress_held = self.archive.is_some() && !settings.resume_reading;
        // 아카이브 세션 보기 모드를 새로 정한다. 목록이 바뀔 때마다 내려놓으므로
        // 다른 목록으로 가면 세션 모드도 사라지고, 일반 이미지는 설정값으로
        // 돌아간다(§15).
        self.comic_mode = self.archive.as_ref().and_then(|_| {
            layout::comic_view_mode(
                settings.view_mode,
                settings.comic_auto_dual_view,
                self.comic.info(),
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
    /// `availability`는 새 목록과 같은 순서의 파일 판별값이다. 비었으면 클라우드
    /// 자동 내려받기 억제가 풀린다(SPEC §9.6).
    pub(super) fn replace_list(
        &mut self,
        items: Vec<Arc<str>>,
        availability: Vec<FileAvailability>,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.clear_pages(window, cx);
        self.list = Arc::new(items);
        self.availability = availability;
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
        for page in self.pages.clear() {
            Self::drop_picture(page.picture(), window, cx);
        }
        if let Some(oriented) = self.oriented.take() {
            Self::drop_picture(&oriented, window, cx);
        }
        self.wide.clear();
        self.shown.clear();
        self.picture = None;
        self.info = None;
        self.orientation = Orientation::default();
        // 진행 중인 회전 결과가 새 목록의 화면을 덮지 않게 한다.
        self.orient_seq += 1;
        self.nav.reset();
        self.budget_reach = None;
        self.wt_dims.clear();
        self.wt_anchor = 0;
        self.wt_offset = 0.0;
        self.wt_glide.discard();
    }

    pub(super) fn close_image(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // 고른 글자가 있으면 첫 입력은 선택만 푼다.
        if self.ocr_select.is_some() {
            self.clear_ocr_selection(cx);
            return;
        }
        self.open_seq += 1;
        self.opening = None;
        self.error = None;
        self.clear_pages(window, cx);
        self.list = Arc::new(Vec::new());
        self.archive = None;
        self.list_root = None;
        self.archive_identity = None;
        self.comic = Comic::Absent;
        self.comic_mode = None;
        self.index = 0;
        self.thumbs.clear();
        self.thumbs_pending.clear();
        self.hide_grid();
        window.set_window_title(&t("app.title"));
        cx.notify();
    }

    /// 파일이나 아카이브를 여는 데 시간이 걸려 로딩 표시를 보여야 하는지.
    pub(super) fn opening_visible(&self) -> bool {
        self.opening
            .is_some_and(|started| started.elapsed() >= LOADING_DELAY)
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
        self.nav.restart();
        self.error = None;
        self.sync_pages(window, cx);
    }

    /// `index`를 화면 시작으로 삼아 이동한다.
    pub(super) fn go_to(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.list.len() {
            return;
        }
        self.index = index;
        // 앞 장 로드를 기다리던 뒤로 넘기기는 다른 이동이 오면 버린다(`Nav::start`).
        // 실패한 페이지의 에러 카드는 다음 화면이 커밋될 때까지 남긴다
        // (`commit_current`가 지운다). 여기서 지우면 그 사이 홈이 깜빡인다.
        self.nav.start(self.list.len());
        // 이전 화면 유지 시간이 지나면 미리보기로 바꿔 그리도록 한 번 깨운다.
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(PREVIOUS_HOLD).await;
            this.update(cx, |_, cx| cx.notify()).ok();
        })
        .detach();
        if self.view_mode(cx) == ViewMode::Webtoon {
            reset_webtoon_scroll_for_navigation(
                &mut self.wt_anchor,
                &mut self.wt_offset,
                &mut self.wt_glide,
                index,
            );
        }
        self.sync_pages(window, cx);
        self.request_thumbs(window, cx);
        cx.notify();
    }

    /// 이전/다음. 양쪽 보기는 화면 단위, 웹툰은 이미지 단위 스크롤 이동이다.
    pub(super) fn navigate(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let total = self.list.len();
        let looped = SettingsStore::global(cx).settings.loop_navigation;
        self.nav.set_forward(forward);
        if !forward && self.view_mode(cx).is_dual() && self.check_previous_pages(window, cx) {
            // 앞의 두 장이 넓은 페이지인지 알아야 건너뛰지 않는다. 로드가 끝나면 이어서 넘긴다.
            self.nav.wait_back();
            return;
        }
        self.nav.cancel_back();
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
            if self.wt_dims.contains_key(&index) || self.pages.is_failed(index) {
                continue;
            }
            self.ensure_page(index, false, false, window, cx);
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
        self.nav.set_forward(offset >= 0);
        self.jump_to(target, window, cx);
    }

    /// 임의 인덱스로 점프한다. 양쪽 보기에서는 쌍 시작으로 스냅한다.
    pub(super) fn jump_to(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.list.len() {
            return;
        }
        // 점프는 기다리던 뒤로 넘기기를 대신한다. 같은 화면이라 이동하지 않아도 버린다.
        self.nav.cancel_back();
        let target = if self.view_mode(cx).is_dual() {
            layout::pair_start(index, self.list.len(), &self.solo(cx), None)
        } else {
            index
        };
        if target != self.index || self.shown.is_empty() {
            self.go_to(target, window, cx);
        }
    }

    /// 헤더·설정·단축키에서 보기 모드를 직접 고른다. 아카이브를 보는 동안에는
    /// 설정값을 바꾸지 않고 세션 보기 모드(`comic_mode`)에만 적용해, 일반
    /// 이미지는 아카이브를 열기 전 설정 그대로 열린다(§15).
    pub(super) fn set_view_mode(
        &mut self,
        mode: ViewMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.archive.is_some() {
            // 아카이브의 자동 결정을 내려놓고 고른 모드를 세션에 반영한다.
            if self.comic_mode == Some(mode) {
                return;
            }
            self.comic_mode = Some(mode);
            self.view_mode_changed(window, cx);
            return;
        }
        if SettingsStore::global(cx).settings.view_mode == mode {
            return;
        }
        SettingsStore::update(cx, |settings| settings.view_mode = mode);
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
            self.ensure_page(*index, true, false, window, cx);
        }
        self.try_commit(window, cx);

        let (cache_mode, cache_max) = {
            let settings = &SettingsStore::global(cx).settings;
            (settings.cache_mode, settings.cache_max)
        };
        let total = self.list.len();
        let base = prefetch_distance(cache_mode, total);
        let budget = cache_budget(cache_mode, cache_max);
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
        self.budget_reach =
            BudgetReach::after_evict(self.budget_reach, &evicted, self.index, budget.bytes);
        if let Some(reach) = self.budget_reach {
            // 경계 밖으로 이미 대기시킨 미리 로드도 시작 전에 버린다. 도착하면 곧바로
            // 내려놓을 페이지다.
            self.cancel_queued_beyond(reach, budget.bytes);
        }
        if base > 0 {
            let start = self.index.saturating_sub(before);
            let end = (self.index + after).min(total - 1);
            let mut candidates: Vec<usize> = (start..=end).collect();
            if cache_mode.covers_folder() {
                // 긴 목록에서도 지금 위치와 가까운 페이지부터 예산을 채운다.
                candidates.sort_by_key(|index| (index.abs_diff(self.index), *index));
            }
            // 대기·진행 중인 요청도 묶음 수에 센다. 세지 않으면 동기화할 때마다 묶음만큼
            // 더 쌓여, 디코드가 도착하기 전에 목록 전체가 대기열에 들어간다.
            let mut prefetched = 0;
            for index in candidates {
                if cache_mode.covers_folder() && prefetched >= FOLDER_PREFETCH_BATCH {
                    break;
                }
                if self.cache_bytes() >= budget.bytes {
                    break;
                }
                // 미리 로드는 자동 내려받기이므로, 자동 내려받기가 꺼진 채로는 클라우드
                // 전용(온디맨드) 파일을 건드리지 않는다(§9.6).
                if self.hold_cloud(index, cx)
                    || self
                        .budget_reach
                        .is_some_and(|reach| reach.blocks(self.index, budget.bytes, index))
                    || self.pages.get(index).is_some()
                    || self.pages.is_failed(index)
                {
                    continue;
                }
                if self.pages.is_loading(index) {
                    prefetched += 1;
                    continue;
                }
                self.ensure_page(index, false, true, window, cx);
                prefetched += 1;
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
            .iter()
            .map(|(index, _)| index)
            .filter(|index| !keep.contains(index))
            .collect();
        for index in &stale {
            if let Some(page) = self.pages.evict(*index) {
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
        self.pages.retain_failed(|index| keep.contains(&index));

        // 항목 수와 바이트 예산이 넘으면 가장 먼 페이지부터 내려놓는다(§9.1).
        let (cache_mode, cache_max) = {
            let settings = &SettingsStore::global(cx).settings;
            (settings.cache_mode, settings.cache_max)
        };
        let budget = cache_budget(cache_mode, cache_max);
        let bytes: HashMap<usize, u64> = self
            .pages
            .iter()
            .map(|(index, page)| (index, page.bytes()))
            .collect();
        let plan = evict_plan(&bytes, self.index, protect, budget.items, budget.bytes);
        for index in &plan {
            let Some(page) = self.pages.evict(*index) else {
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
        self.pages.iter().map(|(_, page)| page.bytes()).sum()
    }

    pub(super) fn current_page_is_full(&self) -> bool {
        self.pages.ready(self.index).is_some()
    }

    /// 프리뷰는 화면에만 올린다. ImageInfo 기반 동작은 full 디코드가 끝날 때까지
    /// 비활성으로 두고, 늦게 끝난 프리뷰가 full 결과를 덮지 않도록 Page 상태도 유지한다.
    fn show_preview_current(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index != self.index || !self.nav.is_pending() || self.view_mode(cx) != ViewMode::Single {
            return;
        }
        let Some(Page::Preview { picture, .. }) = self.pages.get(index) else {
            return;
        };
        let picture = picture.clone();
        if let Some(oriented) = self.oriented.take() {
            Self::drop_picture(&oriented, window, cx);
        }
        if let Some(previous) = self.picture.replace(picture.clone()) {
            let cached = self
                .pages
                .iter()
                .any(|(_, page)| Arc::ptr_eq(page.picture(), &previous));
            if !cached {
                Self::drop_picture(&previous, window, cx);
            }
        }
        self.info = None;
        self.orientation = Orientation::default();
        self.orient_seq += 1;
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
        self.fit = self.fit.lock();
        self.position = Offset::default();
        self.apply_fit(cx);
    }

    /// 페이지가 캐시에 없으면 백그라운드에서 디코드한다. `protect`는 표시용 추출물을
    /// 캐시 정리에서 보호할지이며, 프리페치는 보호 슬롯을 쓰지 않는다. `prefetch`는
    /// 지금 화면이 아니라 미리 로드라는 뜻이고, 클라우드 전용 파일은 설정에 따라
    /// 자동으로 내려받지 않는다(SPEC §9.6).
    pub(super) fn ensure_page(
        &mut self,
        index: usize,
        protect: bool,
        prefetch: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.list.len() {
            return;
        }
        if prefetch && self.hold_cloud(index, cx) {
            return;
        }
        // 이미 끝났거나 대기·진행 중이거나 실패한 인덱스는 다시 넣지 않는다.
        if self.pages.request(index, protect, prefetch) {
            self.pump_decode(window, cx);
        }
    }

    /// 자동 내려받기가 꺼져 있을 때 클라우드 전용(온디맨드) 파일을 미리 읽지 않는지.
    /// 판별값은 목록을 읽을 때 파일을 건드리지 않는 속성으로 정리된 `availability`다.
    pub(super) fn hold_cloud(&self, index: usize, cx: &App) -> bool {
        holds_cloud(
            &self.availability,
            index,
            SettingsStore::global(cx).settings.cloud_auto_download,
        )
    }

    /// 이 인덱스의 파일을 실제로 읽었다. 폴더 목록이 클라우드 전용으로 판별해 둔
    /// 값을 local로 되돌려, 도크·그리드의 구름 아이콘과 썸네일 요청이 실제 상태를
    /// 따르게 한다(`availability`는 폴더 목록 길이와 같을 때만 의미가 있다, SPEC §9.6).
    fn mark_local(&mut self, index: usize) {
        if self.availability.len() == self.list.len() && self.list.get(index).is_some() {
            self.availability[index] = FileAvailability::Local;
        }
    }

    /// 실행 중인 full 디코드가 없으면 대기열에서 하나를 골라 시작한다. 고르는 중에
    /// 범위를 벗어난 항목과, 자동 내려받기가 꺼진 뒤에는 대기 중인 클라우드 전용
    /// 프리페치도 버린다(§9.6). 이미 시작한 디코드는 끝까지 돈다.
    fn pump_decode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let anchor = self.index;
        let (list, availability) = (self.list.clone(), &self.availability);
        let auto_download = SettingsStore::global(cx).settings.cloud_auto_download;
        let Some(job) = self.pages.start_next(anchor, |job| {
            let held = job.prefetch && holds_cloud(availability, job.index, auto_download);
            !held && job.index < list.len()
        }) else {
            return;
        };
        let (index, protect, seq) = (job.index, job.protect, job.seq);
        let item = list[index].clone();
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
                    if this.list_gen != generation
                        || this.index != index
                        || !this.nav.is_pending()
                        || this.view_mode(cx) != ViewMode::Single
                    {
                        Self::drop_picture(&picture, window, cx);
                        return;
                    }
                    let bytes = picture::retained_bytes(&picture);
                    let preview = Page::Preview {
                        picture: picture.clone(),
                        bytes,
                    };
                    // 같은 요청의 full 디코드가 아직 돌 때만 받는다. full이 먼저 왔으면
                    // 늦은 미리보기가 full 결과를 덮지 않게 버린다.
                    let previous = match this.pages.offer_preview(index, seq, preview) {
                        Ok(previous) => previous,
                        Err(_) => {
                            Self::drop_picture(&picture, window, cx);
                            return;
                        }
                    };
                    // 미리보기 디코드도 파일을 실제로 읽은 결과다(§9.6).
                    this.mark_local(index);
                    if let Some(previous) = previous {
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
                if let Ok(page) = &result {
                    // 표시든 미리 로드든 파일을 실제로 읽었다. 이제 local이다(§9.6).
                    this.mark_local(index);
                    if page.picture().width > page.picture().height && !page.is_archive_preview() {
                        this.wide.insert(index);
                    }
                    this.webtoon_page_arrived(index, page.picture(), cx);
                }
                // full 결과로 바꾸거나 실패로 기록하고, 들고 있던 미리보기를 내려놓는다.
                if let Some(previous) = this.pages.finish(index, result) {
                    let displayed = this
                        .picture
                        .as_ref()
                        .is_some_and(|shown| Arc::ptr_eq(shown, previous.picture()));
                    if !displayed {
                        Self::drop_picture(previous.picture(), window, cx);
                    }
                }
                if this.nav.awaiting_back() {
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

    /// 바이트 예산 경계 밖의 대기 중인 미리 로드를 시작 전에 취소한다(§9.1).
    fn cancel_queued_beyond(&mut self, reach: BudgetReach, budget: u64) {
        let anchor = self.index;
        self.pages
            .cancel_queued(|job| job.prefetch && reach.blocks(anchor, budget, job.index));
    }

    /// 범위 밖으로 밀려난 대기 요청을 시작 전에 취소한다. 이미 시작한 디코드는
    /// 끝까지 돌고 결과는 캐시 정리가 맡는다.
    fn cancel_queued_outside(&mut self, keep: &HashSet<usize>) {
        self.pages.cancel_queued(|job| !keep.contains(&job.index));
    }

    /// 이동한 화면의 페이지가 모두 도착했으면 화면을 바꾼다.
    fn try_commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.nav.is_pending() {
            return;
        }
        let screen = self.screen_at(self.index, cx);
        if screen.iter().any(|index| self.pages.is_loading(*index)) {
            return;
        }
        if let Some(error) = self.pages.failed(self.index).cloned() {
            self.current_failed(error, window, cx);
            return;
        }
        if self.pages.ready(self.index).is_none() {
            return;
        }
        self.nav.commit();
        // 로드 실패 페이지는 화면에서 뺀다.
        self.shown = screen
            .into_iter()
            .filter(|index| self.pages.get(*index).is_some())
            .collect();
        self.commit_current(window, cx);
    }

    fn current_failed(&mut self, error: AppError, window: &mut Window, cx: &mut Context<Self>) {
        let skip = SettingsStore::global(cx).settings.skip_broken_files;
        if skip && self.nav.take_skip() {
            let total = self.list.len();
            let looped = SettingsStore::global(cx).settings.loop_navigation;
            // 양쪽 보기는 화면 단위로 건너뛴다. 실패한 페이지는 단독 화면이므로 짝이
            // 있던 페이지를 잃지 않고, 화면 시작이 아닌 자리로 옮겨 짝이 어긋나지도 않는다.
            let next = if self.view_mode(cx).is_dual() {
                layout::dual_step(
                    self.index,
                    total,
                    looped,
                    self.nav.forward(),
                    &self.solo(cx),
                )
            } else {
                layout::step_index(self.index, total, 1, looped, self.nav.forward())
            };
            if let Some(next) = next.filter(|next| !self.pages.is_failed(*next)) {
                window.toast(Toast::warning(t("toast.load.skipped")), cx);
                self.index = next;
                self.sync_pages(window, cx);
                return;
            }
        }
        self.nav.commit();
        // 이전 이미지를 남겨 두면 에러 카드가 가려지고 상태바가 두 파일을 섞어
        // 보여준다. 화면을 비워 재시도·홈 경로가 있는 에러 카드를 띄운다(§3.2).
        self.clear_shown(window, cx);
        window.toast(
            Toast::error(error.message.clone()).title(t("toast.load.imageFail")),
            cx,
        );
        self.error = Some(ViewError::Page(error));
        cx.notify();
    }

    /// 지금 화면의 그림과 보기 상태를 내려놓는다. 페이지 캐시는 그대로 둔다.
    fn clear_shown(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(oriented) = self.oriented.take() {
            Self::drop_picture(&oriented, window, cx);
        }
        if let Some(previous) = self.picture.take() {
            let cached = self
                .pages
                .iter()
                .any(|(_, page)| Arc::ptr_eq(page.picture(), &previous));
            if !cached {
                Self::drop_picture(&previous, window, cx);
            }
        }
        self.info = None;
        self.shown.clear();
        self.orientation = Orientation::default();
        self.orient_seq += 1;
        self.playing = false;
        self.frame = 0;
        self.info_seq += 1;
        self.info_data = Default::default();
        self.ocr_shown = None;
        self.ocr_select = None;
        self.ocr_drag = None;
        self.close_video();
        self.drop_pixel_view(window, cx);
        window.set_window_title(&t("app.title"));
    }

    /// 현재 인덱스의 페이지를 기준 그림으로 삼고 보기 상태를 초기화한다.
    pub(super) fn commit_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(page) = self.pages.ready(self.index) else {
            return;
        };
        let Some(info) = page.info().cloned() else {
            return;
        };
        let picture = page.picture().clone();
        self.error = None;
        self.orientation = Orientation::default();
        // 이전 페이지에서 시작한 회전 결과가 도착해도 새 페이지를 덮지 않게 한다.
        self.orient_seq += 1;
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
                .iter()
                .any(|(_, page)| Arc::ptr_eq(page.picture(), &previous));
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
        self.fit = self.fit.lock();
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
                this.fit = this.fit.lock();
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
        self.fit = self.fit.lock();
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
            .get(self.index)
            .is_some_and(Page::is_archive_preview);
        if let Some(path) = self.list.get(self.index).filter(|_| previewing) {
            self.open_path(path.to_string(), window, cx);
        }
    }
}

/// 자동 내려받기가 꺼져 있을 때 `index`가 클라우드 전용(온디맨드) 파일이라 미리 읽지 않는지.
fn holds_cloud(availability: &[FileAvailability], index: usize, auto_download: bool) -> bool {
    !auto_download && matches!(availability.get(index), Some(FileAvailability::CloudOnly))
}

fn reset_webtoon_scroll_for_navigation(
    anchor: &mut usize,
    offset: &mut f32,
    glide: &mut crate::smooth::Glide,
    index: usize,
) {
    *anchor = index;
    *offset = 0.0;
    // 이미 예약된 프레임이 스스로 끝나므로 남은 이동 거리만 버린다.
    glide.discard();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `use gpui_kit::*`가 gpui의 `test` 매크로를 끌어오므로 빌트인 `#[test]`를
    /// 전체 경로로 고른다.
    #[::core::prelude::v1::test]
    fn webtoon_navigation_discards_pending_smooth_scroll() {
        let (mut anchor, mut offset) = (2, 120.0);
        let mut glide = crate::smooth::Glide::default();
        glide.push(350.0, Instant::now());
        reset_webtoon_scroll_for_navigation(&mut anchor, &mut offset, &mut glide, 8);
        assert_eq!((anchor, offset), (8, 0.0));
        assert_eq!(glide.step(Instant::now()), 0.0);
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

    /// 예산 표(SPEC §9.1)와 코드가 같다. 바이트 상한은 전략과 따로 고른다.
    #[::core::prelude::v1::test]
    fn cache_budgets_match_the_mode_table() {
        assert_eq!(cache_budget(CacheMode::Off, CacheMax::Gb1).items, 1);
        assert_eq!(cache_budget(CacheMode::Nearby, CacheMax::Gb1).items, 24);
        assert_eq!(cache_budget(CacheMode::Extended, CacheMax::Gb1).items, 64);
        assert_eq!(
            cache_budget(CacheMode::Folder, CacheMax::Gb1).items,
            usize::MAX
        );
        assert_eq!(cache_budget(CacheMode::Off, CacheMax::Gb2).bytes, u64::MAX);
        assert_eq!(
            cache_budget(CacheMode::Nearby, CacheMax::Gb1).bytes,
            1024 * MB
        );
        assert_eq!(
            cache_budget(CacheMode::Nearby, CacheMax::Mb512).bytes,
            512 * MB
        );
        assert_eq!(
            cache_budget(CacheMode::Extended, CacheMax::Gb2).bytes,
            2048 * MB
        );
        assert_eq!(
            cache_budget(CacheMode::Folder, CacheMax::Mb512).bytes,
            512 * MB
        );
    }

    /// 폴더 전체 모드의 프리페치 거리는 목록 전체다.
    #[::core::prelude::v1::test]
    fn folder_prefetch_covers_the_whole_list() {
        assert_eq!(prefetch_distance(CacheMode::Off, 100), 0);
        assert_eq!(prefetch_distance(CacheMode::Nearby, 100), 1);
        assert_eq!(prefetch_distance(CacheMode::Extended, 100), 3);
        assert_eq!(prefetch_distance(CacheMode::Folder, 100), 100);
    }

    const MB: u64 = 1024 * 1024;

    /// 예산보다 큰 목록을 폴더 전체 모드로 미리 로드하는 흐름. 경계 밖 페이지를
    /// 내려놓은 뒤에는 같은 위치에서 그보다 먼 페이지를 다시 요청하지 않아야 한다.
    fn simulate_folder_prefetch(remember: bool) -> Vec<usize> {
        let (anchor, budget) = (0, 350);
        let protect = HashSet::from([anchor]);
        let mut cache: HashMap<usize, u64> = HashMap::new();
        let mut reach: Option<BudgetReach> = None;
        let mut requested = Vec::new();
        for _ in 0..40 {
            if cache.values().sum::<u64>() >= budget {
                break;
            }
            let next = (0..10).find(|index| {
                !cache.contains_key(index)
                    && !reach.is_some_and(|reach| reach.blocks(anchor, budget, *index))
            });
            let Some(next) = next else {
                break;
            };
            requested.push(next);
            cache.insert(next, 100);
            let evicted = evict_plan(&cache, anchor, &protect, usize::MAX, budget);
            for index in &evicted {
                cache.remove(index);
            }
            if remember {
                reach = BudgetReach::after_evict(reach, &evicted, anchor, budget);
            }
        }
        requested
    }

    #[::core::prelude::v1::test]
    fn folder_prefetch_stops_after_the_budget_evicts() {
        // 4번째 페이지(인덱스 3)가 예산을 넘겨 내려가면 그 뒤로는 더 요청하지 않는다.
        assert_eq!(simulate_folder_prefetch(true), vec![0, 1, 2, 3]);
        // 경계를 기억하지 않으면 3을 로드하고 내려놓기를 되풀이한다(#166 회귀).
        assert!(simulate_folder_prefetch(false).len() > 10);
    }

    #[::core::prelude::v1::test]
    fn budget_reach_resets_when_position_or_budget_changes() {
        let reach = BudgetReach::after_evict(None, &[7, 9], 2, 100).unwrap();
        assert!(reach.blocks(2, 100, 7));
        assert!(!reach.blocks(2, 100, 0));
        assert!(!reach.blocks(3, 100, 9));
        assert!(!reach.blocks(2, 200, 9));
        // 같은 위치에서 더 가까운 페이지를 내려놓으면 경계가 당겨진다.
        let closer = BudgetReach::after_evict(Some(reach), &[5], 2, 100).unwrap();
        assert!(closer.blocks(2, 100, 5));
        // 이번 정리가 없으면 이전 경계를 유지하고, 위치가 바뀌었으면 버린다.
        assert_eq!(
            BudgetReach::after_evict(Some(reach), &[], 2, 100),
            Some(reach)
        );
        assert_eq!(BudgetReach::after_evict(Some(reach), &[], 4, 100), None);
    }

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

    #[::core::prelude::v1::test]
    fn comic_metadata_is_absent_parsed_or_failed() {
        let parsed = Comic::from_listing(Ok(Some(ComicInfo::default())));
        assert!(parsed.info().is_some() && !parsed.failed());
        let absent = Comic::from_listing(Ok(None));
        assert!(absent.info().is_none() && !absent.failed());
        let failed = Comic::from_listing(Err(AppError::not_found("bad xml")));
        assert!(failed.info().is_none() && failed.failed());
    }
}
