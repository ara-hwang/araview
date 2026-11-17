//! UI에 의존하지 않는 썸네일 요청 수명과 목록 준비 캐시.
//!
//! 자체 구현. GPUI/Zed 앱 계층의 코드는 가져오지 않는다.

use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;
use std::rc::Rc;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) const THUMB_CHUNK: usize = 8;
/// 배치 안에도 워커가 있으므로 동시에 두 청크만 실행한다.
pub(super) const MAX_THUMB_BATCHES: usize = 2;
const MAX_QUEUED_THUMBS: usize = 256;

#[derive(Clone, Default)]
pub(super) struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

pub(super) struct ThumbBatch {
    pub id: u64,
    pub generation: u64,
    pub items: Vec<Arc<str>>,
    pub cancellation: Cancellation,
}

#[derive(Default)]
pub(super) struct ThumbQueue {
    generation: u64,
    next_id: u64,
    queued: VecDeque<Arc<str>>,
    pending: HashSet<Arc<str>>,
    /// 이전 세대도 실제 완료까지 슬롯을 점유한다. reset으로 실행량을 숨기지 않는다.
    running: HashSet<u64>,
    cancellation: Cancellation,
}

impl ThumbQueue {
    pub fn reset(&mut self, generation: u64) {
        self.cancellation.cancel();
        self.cancellation = Cancellation::default();
        self.generation = generation;
        self.queued.clear();
        self.pending.clear();
    }

    pub fn cancellation(&self) -> Cancellation {
        self.cancellation.clone()
    }

    pub fn enqueue(&mut self, items: impl IntoIterator<Item = Arc<str>>) {
        // 새 가시 범위를 이전 대기 항목보다 먼저 처리하되 실행 중 항목은 건드리지 않는다.
        let items: Vec<_> = items.into_iter().collect();
        let wanted: HashSet<_> = items.iter().cloned().collect();
        let queued: HashSet<_> = self.queued.iter().cloned().collect();
        self.queued.retain(|item| !wanted.contains(item));
        let mut promoted = HashSet::new();
        for item in items.into_iter().rev() {
            if promoted.insert(item.clone())
                && (queued.contains(&item) || self.pending.insert(item.clone()))
            {
                self.queued.push_front(item);
            }
        }
        while self.queued.len() > MAX_QUEUED_THUMBS {
            if let Some(item) = self.queued.pop_back() {
                self.pending.remove(&item);
            }
        }
    }

    pub fn start_next(&mut self) -> Option<ThumbBatch> {
        if self.running.len() >= MAX_THUMB_BATCHES || self.queued.is_empty() {
            return None;
        }
        self.next_id += 1;
        let count = THUMB_CHUNK.min(self.queued.len());
        let items = self.queued.drain(..count).collect();
        self.running.insert(self.next_id);
        Some(ThumbBatch {
            id: self.next_id,
            generation: self.generation,
            items,
            cancellation: self.cancellation.clone(),
        })
    }

    /// 실행 슬롯은 항상 해제하지만 낡은 응답은 새 세대 pending을 변경하지 않는다.
    pub fn finish(&mut self, batch: &ThumbBatch) -> bool {
        if !self.running.remove(&batch.id) || batch.generation != self.generation {
            return false;
        }
        for item in &batch.items {
            self.pending.remove(item);
        }
        true
    }
}

impl Drop for ThumbQueue {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

/// 같은 목록 인스턴스와 검색어면 전체 파일명 검색/인덱스 할당을 반복하지 않는다.
#[derive(Default)]
pub(super) struct GridItems {
    list: Option<Arc<Vec<Arc<str>>>>,
    query: String,
    items: Rc<Vec<usize>>,
}

impl GridItems {
    pub fn get(&mut self, list: &Arc<Vec<Arc<str>>>, query: &str) -> Rc<Vec<usize>> {
        if self.list.as_ref().is_some_and(|old| Arc::ptr_eq(old, list)) && self.query == query {
            return self.items.clone();
        }
        self.list = Some(list.clone());
        self.query = query.to_owned();
        self.items = Rc::new(
            list.iter()
                .enumerate()
                .filter_map(|(index, item)| {
                    let label = item.rsplit(['/', '\\']).next().unwrap_or(item);
                    (query.is_empty() || label.to_lowercase().contains(query)).then_some(index)
                })
                .collect(),
        );
        self.items.clone()
    }
}

/// 마지막 키에 대한 반복 데이터 생성 결과. 렌더 사이에는 Rc만 복제한다.
pub(super) struct Prepared<K, V> {
    value: Option<(K, Rc<Vec<V>>)>,
}
impl<K, V> Default for Prepared<K, V> {
    fn default() -> Self {
        Self { value: None }
    }
}
impl<K: PartialEq, V> Prepared<K, V> {
    pub fn get(&mut self, key: K, create: impl FnOnce() -> Vec<V>) -> Rc<Vec<V>> {
        if let Some((old, value)) = &self.value
            && *old == key
        {
            return value.clone();
        }
        let value = Rc::new(create());
        self.value = Some((key, value.clone()));
        value
    }
}

/// 개수 상한 LRU. 퇴출 값을 호출자에게 넘겨 GPU 정리와 정책을 분리한다.
pub(super) struct BoundedLru<K, V> {
    capacity: usize,
    clock: u64,
    values: HashMap<K, (V, u64)>,
}
impl<K: Eq + Hash + Clone, V> BoundedLru<K, V> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self {
            capacity,
            clock: 0,
            values: HashMap::new(),
        }
    }
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.clock += 1;
        self.values.get_mut(key).map(|(value, used)| {
            *used = self.clock;
            value
        })
    }
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.clock += 1;
        if let Some((old, _)) = self.values.insert(key, (value, self.clock)) {
            return Some(old);
        }
        if self.values.len() <= self.capacity {
            return None;
        }
        let oldest = self
            .values
            .iter()
            .min_by_key(|(_, (_, used))| used)
            .map(|(key, _)| key.clone())?;
        self.values.remove(&oldest).map(|(value, _)| value)
    }
    pub fn drain(&mut self) -> impl Iterator<Item = V> + '_ {
        self.values.drain().map(|(_, (value, _))| value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn names(range: std::ops::Range<usize>) -> Vec<Arc<str>> {
        range.map(|i| Arc::from(format!("page-{i}"))).collect()
    }
    #[test]
    fn stale_a_b_a_completion_preserves_replacement_pending() {
        let mut queue = ThumbQueue::default();
        queue.enqueue(names(0..1));
        let a = queue.start_next().unwrap();
        queue.reset(1);
        queue.reset(2);
        queue.enqueue(names(0..1));
        let replacement = queue.start_next().unwrap();
        assert!(a.cancellation.is_cancelled());
        assert!(!queue.finish(&a));
        assert_eq!(queue.pending.len(), 1);
        queue.enqueue(names(0..1));
        assert!(queue.start_next().is_none());
        assert!(queue.finish(&replacement));
        assert!(queue.pending.is_empty());
    }
    #[test]
    fn reversed_completion_and_duplicate_finish_are_isolated() {
        let mut queue = ThumbQueue::default();
        queue.enqueue(names(0..16));
        let first = queue.start_next().unwrap();
        let second = queue.start_next().unwrap();
        assert!(queue.finish(&second));
        assert_eq!(queue.pending.len(), 8);
        assert!(!queue.finish(&second));
        assert!(queue.finish(&first));
        assert!(queue.pending.is_empty());
    }
    #[test]
    fn rapid_generation_changes_never_hide_active_slots() {
        let mut queue = ThumbQueue::default();
        queue.enqueue(names(0..1024));
        let first = queue.start_next().unwrap();
        let second = queue.start_next().unwrap();
        for generation in 1..100 {
            queue.reset(generation);
            queue.enqueue(names(0..1024));
            assert!(queue.start_next().is_none());
            assert!(queue.queued.len() <= MAX_QUEUED_THUMBS);
        }
        assert!(!queue.finish(&first));
        let latest = queue.start_next().unwrap();
        assert_eq!(latest.generation, 99);
        assert!(!queue.finish(&second));
        assert!(queue.finish(&latest));
    }
    #[test]
    fn newest_visible_items_are_prioritized_and_deduplicated() {
        let mut queue = ThumbQueue::default();
        queue.enqueue(names(0..40));
        queue.enqueue(names(32..40));
        queue.enqueue(names(32..40));
        assert_eq!(queue.start_next().unwrap().items, names(32..40));
        assert_eq!(queue.start_next().unwrap().items, names(0..8));
        assert_eq!(queue.pending.len(), 40);
    }
    #[test]
    fn owner_drop_cancels_background_boundaries() {
        let token = {
            let queue = ThumbQueue::default();
            queue.cancellation()
        };
        assert!(token.is_cancelled());
    }
    #[test]
    fn filter_cache_invalidates_for_replacement_and_query() {
        let list = Arc::new(vec![Arc::from("C:\\x\\ABC.png"), Arc::from("/x/b.png")]);
        let mut cache = GridItems::default();
        let all = cache.get(&list, "");
        assert_eq!(&*all, &[0, 1]);
        assert!(Rc::ptr_eq(&all, &cache.get(&list, "")));
        assert_eq!(&*cache.get(&list, "abc"), &[0]);
        let replaced = Arc::new(vec![Arc::from("/x/other.png"), Arc::from("/x/ABC.jpg")]);
        assert_eq!(&*cache.get(&replaced, "abc"), &[1]);
        assert!(cache.get(&replaced, "missing").is_empty());
    }
    #[test]
    fn prepared_sizes_invalidate_for_every_layout_input() {
        let mut cache = Prepared::default();
        let first = cache.get((100, true, 20_u32, 0_u32), || vec![20; 100]);
        assert!(Rc::ptr_eq(
            &first,
            &cache.get((100, true, 20, 0), || panic!("cached"))
        ));
        for key in [
            (101, true, 20, 0),
            (101, false, 20, 0),
            (101, false, 21, 0),
            (101, false, 21, 1),
        ] {
            assert!(!Rc::ptr_eq(&first, &cache.get(key, || vec![])));
        }
    }
    #[test]
    fn lru_evicts_unused_and_drains_once() {
        let mut cache = BoundedLru::new(2);
        assert_eq!(cache.insert("a", 1), None);
        assert_eq!(cache.insert("b", 2), None);
        assert_eq!(cache.get_mut(&"a"), Some(&mut 1));
        assert_eq!(cache.insert("c", 3), Some(2));
        assert_eq!(cache.insert("a", 4), Some(1));
        let mut released: Vec<_> = cache.drain().collect();
        released.sort();
        assert_eq!(released, vec![3, 4]);
        assert_eq!(cache.drain().count(), 0);
    }
    #[test]
    #[ignore = "self-contained release-mode preparation / scheduling / retention benchmark"]
    fn preparation_benchmark() {
        use std::{hint::black_box, time::Instant};
        let list: Arc<Vec<Arc<str>>> = Arc::new(
            (0..100_000)
                .map(|i| Arc::from(format!("C:\\images\\page-{i:06}.jpg")))
                .collect(),
        );
        let start = Instant::now();
        for _ in 0..100 {
            let indices: Vec<_> = list
                .iter()
                .enumerate()
                .filter(|(_, item)| {
                    item.rsplit(['/', '\\'])
                        .next()
                        .unwrap()
                        .to_lowercase()
                        .contains("123")
                })
                .map(|(i, _)| i)
                .collect();
            black_box(indices);
            black_box((0..list.len()).map(|_| (64_f32, 0_f32)).collect::<Vec<_>>());
        }
        let before = start.elapsed();
        let start = Instant::now();
        let mut filter = GridItems::default();
        let mut sizes = Prepared::default();
        for _ in 0..100 {
            black_box(filter.get(&list, "123"));
            black_box(sizes.get((list.len(), true, 64_u32), || {
                (0..list.len()).map(|_| (64_f32, 0_f32)).collect()
            }));
        }
        let after = start.elapsed();
        let mut queue = ThumbQueue::default();
        queue.enqueue(list.iter().cloned());
        let mut started = 0;
        while queue.start_next().is_some() {
            started += 1;
        }
        let mut cache = BoundedLru::new(512);
        for i in 0..10_000 {
            black_box(cache.insert(i, i));
        }
        println!(
            "100k list, 100 renders: before={before:?}, after={after:?}; thumbnail burst batches: before={}, after={started}; 10k decoded 256x256 BGRA thumbnails retention model: before={} bytes, after={} bytes ({} entries)",
            list.len().div_ceil(THUMB_CHUNK),
            10_000 * 256 * 256 * 4_u64,
            cache.values.len() as u64 * 256 * 256 * 4,
            cache.values.len()
        );
        assert_eq!(started, MAX_THUMB_BATCHES);
        assert_eq!(cache.values.len(), 512);
    }
}
