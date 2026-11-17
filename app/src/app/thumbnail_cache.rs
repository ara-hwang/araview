//! 도크와 그리드가 공유하는 창 수명의 디코드/GPU 썸네일 캐시.
//!
//! 자체 구현. 공개 ImageCache API와 해제 계약은 GPUI의 Apache-2.0 코드
//! (Zed d2a4fb6ded7c470be25d1541281da3882495b65b,
//! crates/gpui/src/elements/image_cache.rs 및 examples/image_gallery.rs)를 참고했다.
//! 예제의 Vec LRU/해시 키/element provider는 복사하지 않고 동등성 키와 순수 정책을 쓴다.

use super::thumbnail_state::BoundedLru;
use gpui_kit::*;
use std::sync::Arc;

/// 정상 생성 썸네일은 긴 변 256px다. 512개 256² BGRA의 계산 모델은
/// 128MiB다. 보장하는 것은 항목 수이며, 이 수치는 원본 폴백·프레임 버퍼·
/// GPU 드라이버 등을 포함한 바이트 상한이나 실제 GPU 계측값이 아니다.
const THUMB_CACHE_ITEMS: usize = 512;

pub(super) struct ThumbnailCache {
    images: BoundedLru<Resource, ImageCacheItem>,
}

impl ThumbnailCache {
    pub fn new(cx: &mut App) -> Entity<Self> {
        Self::with_capacity(THUMB_CACHE_ITEMS, cx)
    }

    fn with_capacity(capacity: usize, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            cx.on_release(|cache: &mut Self, cx| cache.clear(None, cx))
                .detach();
            Self {
                images: BoundedLru::new(capacity),
            }
        })
    }

    pub fn clear(&mut self, mut window: Option<&mut Window>, cx: &mut App) {
        for item in self.images.drain() {
            Self::release(item, window.as_deref_mut(), cx);
        }
    }

    fn insert(
        &mut self,
        resource: Resource,
        item: ImageCacheItem,
        window: Option<&mut Window>,
        cx: &mut App,
    ) {
        if let Some(evicted) = self.images.insert(resource, item) {
            Self::release(evicted, window, cx);
        }
    }

    fn release(item: ImageCacheItem, window: Option<&mut Window>, cx: &mut App) {
        if let Some(Ok(image)) = item.get() {
            cx.drop_image(image, window);
        }
        // 아직 로드 중이면 ImageCacheItem의 소유 Task가 여기서 취소된다.
    }
}

impl ImageCache for ThumbnailCache {
    fn load(
        &mut self,
        resource: &Resource,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
        if let Some(item) = self.images.get_mut(resource) {
            return item.use_image(window);
        }
        let item = ImageCacheItem::new(resource, cx);
        let result = item.use_image(window);
        self.insert(resource.clone(), item, Some(window), cx);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::ThumbnailCache;
    use gpui::{ImageCacheItem, Resource, TestAppContext};
    use gpui_kit as gpui;
    use std::sync::Arc;

    /// 실제 GPUI ImageCacheItem으로 디코드 완료·LRU 퇴출·명시적 clear·
    /// 엔티티 release를 검증한다. 드라이버/GPU VRAM 계측 테스트는 아니다.
    #[gpui::test(seeds(1, 7, 42))]
    fn decoded_images_leave_the_scoped_cache_on_eviction_clear_and_owner_release(
        cx: &mut TestAppContext,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let sources: Vec<Resource> = (0..4)
            .map(|index| {
                let path = dir.path().join(format!("thumb-{index}.png"));
                image::RgbaImage::new(2, 2).save(&path).unwrap();
                Resource::from(path)
            })
            .collect();
        let cache = cx.update(|cx| ThumbnailCache::with_capacity(2, cx));
        let start = |source: &Resource, cx: &mut TestAppContext| {
            cache.update(cx, |cache, cx| {
                let item = ImageCacheItem::new(source, cx);
                cache.insert(source.clone(), item, None, cx);
            });
        };
        let loaded = |source: &Resource, cx: &mut TestAppContext| {
            cache.update(cx, |cache, _| {
                Arc::downgrade(
                    &cache
                        .images
                        .get_mut(source)
                        .unwrap()
                        .get()
                        .unwrap()
                        .unwrap(),
                )
            })
        };
        start(&sources[0], cx);
        cx.run_until_parked();
        let first = loaded(&sources[0], cx);
        start(&sources[1], cx);
        cx.run_until_parked();
        let second = loaded(&sources[1], cx);
        assert!(first.upgrade().is_some());
        start(&sources[2], cx);
        cx.run_until_parked();
        let third = loaded(&sources[2], cx);
        assert!(
            first.upgrade().is_none(),
            "oldest decoded image must leave cache"
        );
        cache.update(cx, |cache, cx| cache.clear(None, cx));
        cx.run_until_parked();
        assert!(second.upgrade().is_none());
        assert!(third.upgrade().is_none());
        start(&sources[3], cx);
        cx.run_until_parked();
        let last = loaded(&sources[3], cx);
        cx.update(|_| drop(cache));
        cx.run_until_parked();
        assert!(
            last.upgrade().is_none(),
            "owner release must discard decoded image"
        );
    }
}
