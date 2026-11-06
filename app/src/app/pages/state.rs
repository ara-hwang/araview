//! 페이지 디코드·화면 이동·파일 열기의 상태 타입(SPEC §3.3, §4, §5.5, §9.1).
//!
//! 예전에는 한 인덱스의 디코드 상태가 컬렉션 다섯 개(`pages`, `page_loading`,
//! `page_failed`, `decode_queue`, `decode_running`)에 흩어져 있었고, 이동 상태와 열기
//! 상태도 bool 여러 개로 나뉘어 있었다. 여기서는 각각을 한 타입으로 묶고 전이를
//! 메서드로만 하게 해, 불가능한 조합(예: 대기열에 있으면서 실패, 실행 중인데 어느
//! 인덱스인지 모름)을 타입이 막는다. gpui에 기대지 않으므로 단위 테스트로 전이를 확인한다.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use araview_core::app_error::AppError;

/// 대기 중인 full 디코드 요청.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::app) struct DecodeJob {
    pub index: usize,
    pub protect: bool,
    /// 미리 로드(자동 내려받기) 요청인지. 클라우드 전용 파일은 설정에 따라
    /// 대기열에서 버릴 수 있다(SPEC §9.6).
    pub prefetch: bool,
    pub seq: u64,
}

/// 대기 중인 요청 가운데 다음에 돌릴 것. 현재 화면(`protect`)이 먼저이고,
/// 같으면 현재 위치에서 가까운 순, 그다음 요청 순이다.
pub(super) fn next_decode_job<'a>(
    jobs: impl IntoIterator<Item = &'a DecodeJob>,
    anchor: usize,
) -> Option<&'a DecodeJob> {
    jobs.into_iter()
        .min_by_key(|job| (!job.protect, job.index.abs_diff(anchor), job.seq))
}

/// 한 인덱스의 디코드 상태. 슬롯이 없으면 아직 요청하지 않았거나 내려놓은 페이지다.
///
/// 없음 → `Queued` → `Decoding`(미리보기가 먼저 올 수 있음) → `Ready` 또는 `Failed`.
/// 미리보기는 full 디코드가 실행 중인 동안에만 들고 있다. full 결과가 오면 그것으로
/// 바뀌고, 실패하면 버린다.
#[derive(Debug)]
pub(in crate::app) enum PageSlot<P> {
    Queued(DecodeJob),
    Decoding {
        /// 실행 중인 요청 번호. 같은 번호의 미리보기만 받는다.
        seq: u64,
        preview: Option<P>,
    },
    /// full 디코드 결과.
    Ready(P),
    Failed(AppError),
}

/// 인덱스별 디코드 상태와 대기열. full 디코드는 한 번에 하나만 돌린다(§9.1).
#[derive(Debug)]
pub(in crate::app) struct PageSlots<P> {
    slots: HashMap<usize, PageSlot<P>>,
    /// 실행 중인 full 디코드의 인덱스.
    running: Option<usize>,
    /// 요청 순서를 매기는 번호.
    seq: u64,
}

impl<P> Default for PageSlots<P> {
    fn default() -> Self {
        Self {
            slots: HashMap::new(),
            running: None,
            seq: 0,
        }
    }
}

impl<P> PageSlots<P> {
    /// 모든 상태를 비우고 들고 있던 페이지(미리보기 포함)를 돌려준다. 실행 중인
    /// 디코드의 결과는 호출자가 목록 세대로 버린다.
    pub fn clear(&mut self) -> Vec<P> {
        self.running = None;
        std::mem::take(&mut self.slots)
            .into_values()
            .filter_map(|slot| match slot {
                PageSlot::Decoding { preview, .. } => preview,
                PageSlot::Ready(page) => Some(page),
                PageSlot::Queued(_) | PageSlot::Failed(_) => None,
            })
            .collect()
    }

    /// 화면에 올릴 수 있는 그림: full 결과, 없으면 실행 중인 디코드의 미리보기.
    pub fn get(&self, index: usize) -> Option<&P> {
        match self.slots.get(&index)? {
            PageSlot::Ready(page) => Some(page),
            PageSlot::Decoding { preview, .. } => preview.as_ref(),
            PageSlot::Queued(_) | PageSlot::Failed(_) => None,
        }
    }

    /// full 디코드가 끝난 페이지.
    pub fn ready(&self, index: usize) -> Option<&P> {
        match self.slots.get(&index)? {
            PageSlot::Ready(page) => Some(page),
            _ => None,
        }
    }

    pub fn ready_mut(&mut self, index: usize) -> Option<&mut P> {
        match self.slots.get_mut(&index)? {
            PageSlot::Ready(page) => Some(page),
            _ => None,
        }
    }

    /// 들고 있는 모든 그림(full과 미리보기)과 그 인덱스.
    pub fn iter(&self) -> impl Iterator<Item = (usize, &P)> {
        self.slots.iter().filter_map(|(index, slot)| match slot {
            PageSlot::Ready(page) => Some((*index, page)),
            PageSlot::Decoding {
                preview: Some(page),
                ..
            } => Some((*index, page)),
            _ => None,
        })
    }

    /// 들고 있는 그림 수(full과 미리보기). 제어 채널 상태에 싣는다.
    #[cfg(debug_assertions)]
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// 대기열에 있거나 디코드 중인지.
    pub fn is_loading(&self, index: usize) -> bool {
        matches!(
            self.slots.get(&index),
            Some(PageSlot::Queued(_) | PageSlot::Decoding { .. })
        )
    }

    pub fn any_loading(&self) -> bool {
        self.slots
            .values()
            .any(|slot| matches!(slot, PageSlot::Queued(_) | PageSlot::Decoding { .. }))
    }

    pub fn failed(&self, index: usize) -> Option<&AppError> {
        match self.slots.get(&index)? {
            PageSlot::Failed(error) => Some(error),
            _ => None,
        }
    }

    pub fn is_failed(&self, index: usize) -> bool {
        self.failed(index).is_some()
    }

    pub fn failed_indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.slots
            .iter()
            .filter(|(_, slot)| matches!(slot, PageSlot::Failed(_)))
            .map(|(index, _)| *index)
    }

    /// full 디코드를 대기열에 넣는다. 이미 끝났거나 대기·진행 중이거나 실패한
    /// 인덱스는 그대로 두고 `false`를 돌려준다.
    pub fn request(&mut self, index: usize, protect: bool, prefetch: bool) -> bool {
        if self.slots.contains_key(&index) {
            return false;
        }
        self.seq += 1;
        self.slots.insert(
            index,
            PageSlot::Queued(DecodeJob {
                index,
                protect,
                prefetch,
                seq: self.seq,
            }),
        );
        true
    }

    /// 실행 중인 디코드가 없으면 다음 요청을 골라 `Decoding`으로 옮긴다. 고르는
    /// 중에 `keep`이 `false`인 요청은 시작하지 않고 버린다.
    pub fn start_next(
        &mut self,
        anchor: usize,
        mut keep: impl FnMut(&DecodeJob) -> bool,
    ) -> Option<DecodeJob> {
        if self.running.is_some() {
            return None;
        }
        loop {
            let job = next_decode_job(self.queued(), anchor)?.clone();
            if !keep(&job) {
                self.slots.remove(&job.index);
                continue;
            }
            self.slots.insert(
                job.index,
                PageSlot::Decoding {
                    seq: job.seq,
                    preview: None,
                },
            );
            self.running = Some(job.index);
            return Some(job);
        }
    }

    fn queued(&self) -> impl Iterator<Item = &DecodeJob> {
        self.slots.values().filter_map(|slot| match slot {
            PageSlot::Queued(job) => Some(job),
            _ => None,
        })
    }

    /// 실행 중인 디코드의 미리보기를 받는다. 그 요청이 아직 실행 중일 때만 받고,
    /// 아니면(full이 먼저 왔거나 다른 요청으로 바뀌었으면) 그대로 돌려준다.
    /// 받으면 갈아 끼운 이전 미리보기를 돌려준다.
    pub fn offer_preview(&mut self, index: usize, seq: u64, page: P) -> Result<Option<P>, P> {
        match self.slots.get_mut(&index) {
            Some(PageSlot::Decoding {
                seq: running,
                preview,
            }) if *running == seq => Ok(preview.replace(page)),
            _ => Err(page),
        }
    }

    /// 실행 중인 디코드가 끝났다. 결과를 슬롯에 넣고, 내려놓을 미리보기를 돌려준다.
    pub fn finish(&mut self, index: usize, result: Result<P, AppError>) -> Option<P> {
        if self.running == Some(index) {
            self.running = None;
        }
        let next = match result {
            Ok(page) => PageSlot::Ready(page),
            Err(error) => PageSlot::Failed(error),
        };
        match self.slots.insert(index, next) {
            Some(PageSlot::Decoding { preview, .. }) => preview,
            Some(PageSlot::Ready(page)) => Some(page),
            _ => None,
        }
    }

    /// 시작 전 요청 가운데 `cancel`이 `true`인 것을 버린다.
    pub fn cancel_queued(&mut self, mut cancel: impl FnMut(&DecodeJob) -> bool) {
        self.slots.retain(|_, slot| match slot {
            PageSlot::Queued(job) => !cancel(job),
            _ => true,
        });
    }

    /// `keep`이 `false`인 인덱스의 실패 기록을 지운다.
    pub fn retain_failed(&mut self, mut keep: impl FnMut(usize) -> bool) {
        self.slots.retain(|index, slot| match slot {
            PageSlot::Failed(_) => keep(*index),
            _ => true,
        });
    }

    /// 들고 있는 그림을 내려놓는다. full 결과면 슬롯을 비우고, 디코드 중이면
    /// 미리보기만 내려놓아 실행 중인 디코드는 그대로 둔다.
    pub fn evict(&mut self, index: usize) -> Option<P> {
        match self.slots.get_mut(&index)? {
            PageSlot::Ready(_) => match self.slots.remove(&index) {
                Some(PageSlot::Ready(page)) => Some(page),
                _ => None,
            },
            PageSlot::Decoding { preview, .. } => preview.take(),
            PageSlot::Queued(_) | PageSlot::Failed(_) => None,
        }
    }
}

/// 화면 이동의 커밋 상태(SPEC §3.3, §5.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum NavPhase {
    /// 지금 화면이 `index`의 화면이다.
    Committed,
    /// 이동한 화면이 아직 올라가지 않았다. 그동안 이전 화면을 유지하고, 오래 걸리면
    /// 미리보기를 깐다.
    Pending,
    /// 양쪽 보기에서 뒤로 넘기기 전에 앞의 두 장 크기를 기다린다. 페이지가 도착하면
    /// 뒤로 넘기기를 다시 시도한다. `pending`은 그 전 이동의 커밋도 아직 기다리는지다.
    AwaitingBack { pending: bool },
}

/// 화면 이동 상태. 커밋 단계와 함께, 마지막 이동을 시작한 시각, 손상 파일을
/// 건너뛸 남은 횟수(§5.5), 마지막 이동 방향을 든다.
#[derive(Clone, Copy, Debug)]
pub(in crate::app) struct Nav {
    phase: NavPhase,
    started: Instant,
    skips_left: usize,
    forward: bool,
}

impl Default for Nav {
    fn default() -> Self {
        Self {
            phase: NavPhase::Committed,
            started: Instant::now(),
            skips_left: 0,
            forward: true,
        }
    }
}

impl Nav {
    #[cfg(test)]
    pub fn phase(&self) -> NavPhase {
        self.phase
    }

    /// 이동한 화면이 아직 커밋되지 않았는지.
    pub fn is_pending(&self) -> bool {
        matches!(
            self.phase,
            NavPhase::Pending | NavPhase::AwaitingBack { pending: true }
        )
    }

    pub fn awaiting_back(&self) -> bool {
        matches!(self.phase, NavPhase::AwaitingBack { .. })
    }

    /// 커밋을 기다린 지 `hold`가 지났는지.
    pub fn held_longer_than(&self, hold: Duration) -> bool {
        self.is_pending() && self.started.elapsed() >= hold
    }

    pub fn forward(&self) -> bool {
        self.forward
    }

    pub fn set_forward(&mut self, forward: bool) {
        self.forward = forward;
    }

    /// 새 화면으로 이동을 시작한다. 기다리던 뒤로 넘기기는 버린다. 남겨 두면 다음
    /// 디코드 결과에서 한 화면 더 뒤로 간다.
    pub fn start(&mut self, skip_budget: usize) {
        self.phase = NavPhase::Pending;
        self.started = Instant::now();
        self.skips_left = skip_budget;
    }

    /// 같은 위치를 다시 디코드한다(재시도·표시 정책 변경). 이동 시작 시각과 건너뛰기
    /// 횟수는 그대로 둔다.
    pub fn restart(&mut self) {
        self.phase = NavPhase::Pending;
    }

    /// 화면이 올라갔거나(성공·에러 카드) 기다림을 접었다. 기다리던 뒤로 넘기기는 남긴다.
    pub fn commit(&mut self) {
        self.phase = match self.phase {
            NavPhase::AwaitingBack { .. } => NavPhase::AwaitingBack { pending: false },
            NavPhase::Committed | NavPhase::Pending => NavPhase::Committed,
        };
    }

    /// 목록을 비웠다. 기다리던 것도 모두 버린다.
    pub fn reset(&mut self) {
        self.phase = NavPhase::Committed;
    }

    /// 뒤로 넘기기 전에 앞 페이지 크기를 기다린다.
    pub fn wait_back(&mut self) {
        self.phase = NavPhase::AwaitingBack {
            pending: self.is_pending(),
        };
    }

    /// 기다리던 뒤로 넘기기를 버린다. 커밋 대기는 그대로 둔다.
    pub fn cancel_back(&mut self) {
        if let NavPhase::AwaitingBack { pending } = self.phase {
            self.phase = if pending {
                NavPhase::Pending
            } else {
                NavPhase::Committed
            };
        }
    }

    /// 손상 파일 하나를 건너뛸 수 있으면 횟수를 하나 쓰고 `true`.
    pub fn take_skip(&mut self) -> bool {
        if self.skips_left == 0 {
            return false;
        }
        self.skips_left -= 1;
        true
    }
}

/// 열기·화면 에러가 어디서 났는지. 에러 카드의 재시도가 이것을 따른다(§3.2).
#[derive(Clone, Debug)]
pub(in crate::app) enum ViewError {
    /// 파일·폴더·아카이브 목록을 열지 못했다. 이전 목록은 그대로다. 재시도는 같은
    /// 경로를 다시 연다.
    Open { path: String, error: AppError },
    /// 현재 페이지를 디코드하지 못했다. 재시도는 그 위치를 다시 디코드한다.
    Page(AppError),
}

impl ViewError {
    pub fn error(&self) -> &AppError {
        match self {
            Self::Open { error, .. } | Self::Page(error) => error,
        }
    }

    /// 제어 채널 상태에 싣는 출처 이름.
    #[cfg(debug_assertions)]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Open { .. } => "open",
            Self::Page(_) => "page",
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
            prefetch: false,
            seq,
        }
    }

    fn error() -> AppError {
        AppError::not_found("gone")
    }

    #[test]
    fn decode_queue_picks_current_screen_then_nearest_then_oldest() {
        let queue = [
            job(9, false, 1),
            job(6, false, 2),
            job(4, false, 3),
            job(5, true, 4),
        ];
        assert_eq!(next_decode_job(&queue, 5).map(|job| job.index), Some(5));
        let queue = [job(9, false, 1), job(6, false, 2), job(4, false, 3)];
        // 거리가 같으면 먼저 요청한 것이 앞선다.
        assert_eq!(next_decode_job(&queue, 5).map(|job| job.index), Some(6));
        assert_eq!(next_decode_job(&[], 5), None);
    }

    #[test]
    fn a_slot_moves_from_queued_to_decoding_to_ready() {
        let mut slots: PageSlots<&str> = PageSlots::default();
        assert!(slots.request(3, true, false));
        // 대기 중인 인덱스는 다시 넣지 않는다.
        assert!(!slots.request(3, true, false));
        assert!(slots.is_loading(3) && slots.any_loading());
        assert_eq!(slots.get(3), None);

        let started = slots.start_next(3, |_| true).unwrap();
        assert_eq!(started.index, 3);
        assert!(slots.is_loading(3));
        // 하나가 도는 동안은 다음을 시작하지 않는다.
        assert!(slots.request(4, false, true));
        assert_eq!(slots.start_next(3, |_| true), None);

        assert_eq!(slots.finish(3, Ok("full")), None);
        assert!(!slots.is_loading(3));
        assert_eq!(slots.ready(3), Some(&"full"));
        assert_eq!(slots.get(3), Some(&"full"));
        // 끝난 페이지는 다시 요청하지 않는다.
        assert!(!slots.request(3, true, false));
        assert_eq!(slots.start_next(3, |_| true).map(|job| job.index), Some(4));
    }

    #[test]
    fn previews_belong_to_the_running_request_only() {
        let mut slots: PageSlots<&str> = PageSlots::default();
        slots.request(1, true, false);
        let job = slots.start_next(1, |_| true).unwrap();
        // 다른 요청 번호의 미리보기는 받지 않는다.
        assert_eq!(slots.offer_preview(1, job.seq + 1, "old"), Err("old"));
        assert_eq!(slots.offer_preview(1, job.seq, "preview"), Ok(None));
        assert_eq!(slots.get(1), Some(&"preview"));
        assert_eq!(slots.ready(1), None);
        assert_eq!(slots.len(), 1);
        // full이 오면 미리보기를 돌려주고 full로 바뀐다.
        assert_eq!(slots.finish(1, Ok("full")), Some("preview"));
        assert_eq!(slots.get(1), Some(&"full"));
        // full 뒤에 늦게 온 미리보기는 받지 않는다.
        assert_eq!(slots.offer_preview(1, job.seq, "late"), Err("late"));
        assert_eq!(slots.get(1), Some(&"full"));
    }

    #[test]
    fn a_failure_drops_the_preview_and_blocks_requests() {
        let mut slots: PageSlots<&str> = PageSlots::default();
        slots.request(2, true, false);
        let job = slots.start_next(2, |_| true).unwrap();
        slots.offer_preview(2, job.seq, "preview").unwrap();
        assert_eq!(slots.finish(2, Err(error())), Some("preview"));
        assert!(slots.is_failed(2) && !slots.is_loading(2));
        assert_eq!(slots.get(2), None);
        assert_eq!(slots.failed_indices().collect::<Vec<_>>(), vec![2]);
        assert!(!slots.request(2, true, false));
        slots.retain_failed(|index| index != 2);
        assert!(!slots.is_failed(2));
        assert!(slots.request(2, true, false));
    }

    #[test]
    fn eviction_keeps_a_running_decode() {
        let mut slots: PageSlots<&str> = PageSlots::default();
        slots.request(5, true, false);
        let job = slots.start_next(5, |_| true).unwrap();
        slots.offer_preview(5, job.seq, "preview").unwrap();
        assert_eq!(slots.evict(5), Some("preview"));
        assert!(slots.is_loading(5));
        assert_eq!(slots.finish(5, Ok("full")), None);
        assert_eq!(slots.evict(5), Some("full"));
        assert_eq!(slots.get(5), None);
        assert!(!slots.is_loading(5));
        assert!(slots.request(5, false, true));
    }

    #[test]
    fn cancelled_and_rejected_jobs_leave_no_slot() {
        let mut slots: PageSlots<&str> = PageSlots::default();
        for index in [1, 2, 3] {
            slots.request(index, false, true);
        }
        slots.cancel_queued(|job| job.index == 3);
        assert!(!slots.is_loading(3));
        // 시작할 때 거른 요청(클라우드 전용 등)도 슬롯을 남기지 않는다.
        let started = slots.start_next(1, |job| job.index != 1).unwrap();
        assert_eq!(started.index, 2);
        assert!(!slots.is_loading(1));
        assert!(slots.request(1, false, true));
    }

    #[test]
    fn clear_returns_every_held_picture() {
        let mut slots: PageSlots<&str> = PageSlots::default();
        slots.request(1, true, false);
        let job = slots.start_next(1, |_| true).unwrap();
        slots.offer_preview(1, job.seq, "preview").unwrap();
        slots.request(2, false, true);
        let mut held = slots.clear();
        held.sort();
        assert_eq!(held, vec!["preview"]);
        assert!(!slots.any_loading());
        // 실행 중 표시도 지워져 바로 다음 요청을 시작한다.
        slots.request(7, true, false);
        assert!(slots.start_next(7, |_| true).is_some());
    }

    #[test]
    fn nav_start_commit_and_back_waiting() {
        let mut nav = Nav::default();
        assert_eq!(nav.phase(), NavPhase::Committed);
        nav.start(2);
        assert!(nav.is_pending());
        // 커밋을 기다리는 동안 뒤로 넘기기를 기다릴 수 있다.
        nav.wait_back();
        assert_eq!(nav.phase(), NavPhase::AwaitingBack { pending: true });
        assert!(nav.is_pending() && nav.awaiting_back());
        // 화면이 올라가도 기다리던 뒤로 넘기기는 남는다.
        nav.commit();
        assert_eq!(nav.phase(), NavPhase::AwaitingBack { pending: false });
        assert!(!nav.is_pending() && nav.awaiting_back());
        nav.cancel_back();
        assert_eq!(nav.phase(), NavPhase::Committed);
        // 새 이동은 기다리던 뒤로 넘기기를 버린다.
        nav.wait_back();
        nav.start(1);
        assert_eq!(nav.phase(), NavPhase::Pending);
        nav.wait_back();
        nav.cancel_back();
        assert_eq!(nav.phase(), NavPhase::Pending);
        nav.reset();
        assert_eq!(nav.phase(), NavPhase::Committed);
    }

    #[test]
    fn nav_skip_budget_and_hold() {
        let mut nav = Nav::default();
        assert!(!nav.take_skip());
        nav.start(2);
        assert!(nav.take_skip() && nav.take_skip());
        assert!(!nav.take_skip());
        // 다시 디코드는 남은 횟수를 그대로 둔다.
        nav.commit();
        nav.restart();
        assert!(nav.is_pending() && !nav.take_skip());
        assert!(nav.held_longer_than(Duration::ZERO));
        nav.commit();
        assert!(!nav.held_longer_than(Duration::ZERO));
    }
}
