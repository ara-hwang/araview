//! 목록 열기, 페이지 캐시와 프리페치, 이동(SPEC §4, §5, §6, §9.1).
//!
//! 페이지는 인덱스별로 디코드해 캐시에 두고, 화면에 올릴 페이지가 모두 준비되면
//! 한 번에 교체한다. 그 전까지는 이전 화면을 유지한다(SPEC §3.3).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use araview_core::app_error::AppError;
use araview_core::image::{ImageInfo, is_archive_file};
use araview_core::ops;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::notification::Notification;
use gpui_kit::*;

use super::{AraView, Listing};
use crate::geometry::Offset;
use crate::i18n::{t, t_with};
use crate::layout::{self, CoverLayout, Solo};
use crate::picture::{self, Orientation, Picture};
use crate::settings::{ArchiveProgress, CacheMode, SettingsStore, ViewMode};

pub(super) struct Page {
    pub info: ImageInfo,
    pub picture: Arc<Picture>,
    /// 폴더 목록의 아카이브를 첫 페이지로 미리 보여주는 중인지.
    pub archive_preview: bool,
}

/// 새 이미지가 디코드될 때까지 이전 이미지를 붙잡는 최대 시간(SPEC §3.3).
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
        self.error = None;
        cx.notify();
        let source = path.clone();
        let task = cx.background_spawn(async move {
            let file = Path::new(&source);
            if is_archive_file(file) {
                let images = ops::get_archive_images_impl(file)?;
                let items: Vec<Arc<str>> = images
                    .images
                    .iter()
                    .map(|name| Arc::from(&**name))
                    .collect();
                // 저장된 엔트리가 목록에 그대로 있을 때만 이어 본다.
                let resumed = resume
                    .and_then(|progress| items.iter().position(|name| **name == *progress.entry));
                Ok::<_, AppError>(Listing {
                    items,
                    index: resumed.unwrap_or(0),
                    resumed: resumed.is_some_and(|index| index > 0),
                    archive: Some(Arc::from(source.as_str())),
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
                        window.push_notification(
                            Notification::error(error.message.clone()).title(t(key)),
                            cx,
                        );
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
        self.comic_failed = listing.comic.is_err();
        self.comic = listing.comic.ok().flatten();
        let settings = &SettingsStore::global(cx).settings;
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
            window.push_notification(
                Notification::info(t_with(
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

    fn drop_picture(picture: &Picture, window: &mut Window, cx: &mut Context<Self>) {
        for image in picture.images() {
            cx.drop_image(image.clone(), Some(window));
        }
    }

    /// 페이지 캐시와 표시 상태를 비우고 텍스처를 아틀라스에서 내린다.
    pub(super) fn clear_pages(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.list_gen += 1;
        for (_, page) in std::mem::take(&mut self.pages) {
            Self::drop_picture(&page.picture, window, cx);
        }
        if let Some(oriented) = self.oriented.take() {
            Self::drop_picture(&oriented, window, cx);
        }
        self.page_loading.clear();
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
    }

    pub(super) fn close_image(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_seq += 1;
        self.opening = false;
        self.error = None;
        self.clear_pages(window, cx);
        self.list = Arc::new(Vec::new());
        self.archive = None;
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

        let base = prefetch_distance(SettingsStore::global(cx).settings.cache_mode);
        let total = self.list.len();
        let (before, after) = if mode.is_dual() {
            (base + 1, base + 2)
        } else {
            (base, base)
        };
        let keep: HashSet<usize> = (self.index.saturating_sub(before + 1)
            ..=(self.index + after + 1).min(total - 1))
            .chain(screen.iter().copied())
            .chain(self.shown.iter().copied())
            .collect();
        if base > 0 {
            for index in self.index.saturating_sub(before)..=(self.index + after).min(total - 1) {
                self.ensure_page(index, false, window, cx);
            }
        }
        self.evict_except(&keep, window, cx);
    }

    pub(super) fn evict_except(
        &mut self,
        keep: &HashSet<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let stale: Vec<usize> = self
            .pages
            .keys()
            .copied()
            .filter(|index| !keep.contains(index))
            .collect();
        for index in stale {
            if let Some(page) = self.pages.remove(&index) {
                // 단일 보기에서 표시 중인 그림은 `picture`가 같은 Arc를 들고 있다.
                let displayed = self
                    .picture
                    .as_ref()
                    .is_some_and(|shown| Arc::ptr_eq(shown, &page.picture));
                if !displayed {
                    Self::drop_picture(&page.picture, window, cx);
                }
            }
        }
        self.page_failed.retain(|index, _| keep.contains(index));
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
        if self.pages.contains_key(&index)
            || self.page_loading.contains(&index)
            || self.page_failed.contains_key(&index)
        {
            return;
        }
        let Some(item) = self.list.get(index).cloned() else {
            return;
        };
        self.page_loading.insert(index);
        let generation = self.list_gen;
        let settings = &SettingsStore::global(cx).settings;
        let max_side = settings.max_resolution.max_side();
        let scaling = Some(settings.image_scaling_mode.as_str().to_owned());
        let auto_detect = Some(settings.auto_detect_pixel_art);
        let archive = self.archive.clone();
        let task = cx.background_spawn(async move {
            let (info, archive_preview) = match &archive {
                Some(archive) => (
                    ops::load_archive_image_blocking(
                        archive,
                        &item,
                        max_side,
                        Some(protect),
                        scaling,
                        auto_detect,
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
                            auto_detect,
                        )?,
                        true,
                    )
                }
                None => (
                    ops::load_image_blocking(&item, max_side, scaling, auto_detect)?,
                    false,
                ),
            };
            let picture = picture::load(Path::new(&info.file_path), Orientation::default())?;
            Ok::<_, AppError>(Page {
                info,
                picture: Arc::new(picture),
                archive_preview,
            })
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                if this.list_gen != generation {
                    return;
                }
                this.page_loading.remove(&index);
                match result {
                    Ok(page) => {
                        if page.picture.width > page.picture.height && !page.archive_preview {
                            this.wide.insert(index);
                        }
                        this.webtoon_page_arrived(index, &page.picture, cx);
                        this.pages.insert(index, page);
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
                cx.notify();
            })
            .ok();
        })
        .detach();
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
        if !self.pages.contains_key(&self.index) {
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
                window.push_notification(Notification::warning(t("toast.load.skipped")), cx);
                self.index = next;
                self.sync_pages(window, cx);
                return;
            }
        }
        self.pending_commit = false;
        window.push_notification(
            Notification::error(error.message.clone()).title(t("toast.load.imageFail")),
            cx,
        );
        self.error = Some(error);
        cx.notify();
    }

    /// 현재 인덱스의 페이지를 기준 그림으로 삼고 보기 상태를 초기화한다.
    pub(super) fn commit_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(page) = self.pages.get(&self.index) else {
            return;
        };
        let info = page.info.clone();
        let picture = page.picture.clone();
        self.error = None;
        self.orientation = Orientation::default();
        if let Some(oriented) = self.oriented.take() {
            Self::drop_picture(&oriented, window, cx);
        }
        self.playing = picture.is_animated() && !self.view_mode(cx).is_dual();
        self.frame = 0;
        self.loops_done = 0;
        self.frame_started = Instant::now();
        self.picture = Some(picture);
        self.info = Some(info);
        if let Some(info) = &self.info {
            window.set_window_title(&self.display_name(info));
        }
        self.fit_locked = true;
        self.position = Offset::default();
        self.apply_fit(cx);
        self.save_progress(cx);
        self.load_info(window, cx);
        self.pixel_hint = None;
        self.request_pixel_hint(window, cx);
        cx.notify();
    }

    pub(super) fn save_progress(&mut self, cx: &mut Context<Self>) {
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
                },
            );
        }
    }

    /// 회전/반전이 바뀌면 같은 렌더 바이트를 새 방향으로 다시 만든다(단일 보기).
    fn reorient(&mut self, orientation: Orientation, window: &mut Window, cx: &mut Context<Self>) {
        let Some(info) = &self.info else {
            return;
        };
        self.orientation = orientation;
        self.orient_seq += 1;
        let seq = self.orient_seq;
        let generation = self.list_gen;
        let path = PathBuf::from(&info.file_path);
        let task = cx.background_spawn(async move { picture::load(&path, orientation) });
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
            .is_some_and(|page| page.archive_preview);
        if let Some(path) = self.list.get(self.index).filter(|_| previewing) {
            self.open_path(path.to_string(), window, cx);
        }
    }
}
