import { useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ImageInfo } from "../types";
import { getCacheLimit, getPrefetchDistance } from "../utils/cacheConfig";
import { useSettingsStore } from "@/store/settingsStore";

// Tauri 백엔드에서 불러온 이미지를 메모리 캐시에 저장하고,
// 설정에 따라 캐시 용량/프리패치 범위를 제어하는 훅

export function useImageCache() {
  const settings = useSettingsStore();
  const imageCacheRef = useRef<Map<string, ImageInfo>>(new Map());
  const inflightLoadsRef = useRef<Map<string, Promise<ImageInfo>>>(new Map());

  // LRU 비슷하게, 오래된 항목부터 제거해서 캐시 크기를 limit 이하로 유지
  const trimCacheToLimit = useCallback((limit: number) => {
    const cache = imageCacheRef.current;
    while (cache.size > limit) {
      const oldestKey = cache.keys().next().value;
      if (!oldestKey) break;
      cache.delete(oldestKey);
    }
  }, []);

  const cacheImage = useCallback(
    (filePath: string, imgInfo: ImageInfo) => {
      const cache = imageCacheRef.current;
      if (cache.has(filePath)) cache.delete(filePath);
      cache.set(filePath, imgInfo);
      trimCacheToLimit(getCacheLimit(settings.cacheMode));
    },
    [settings.cacheMode, trimCacheToLimit],
  );

  // 단일 이미지를 캐시/진행 중 요청을 우선 확인한 뒤 필요한 경우만 실제 invoke 호출
  const getOrLoadImage = useCallback(
    async (filePath: string): Promise<ImageInfo> => {
      const cached = imageCacheRef.current.get(filePath);
      if (cached) return cached;

      const inflight = inflightLoadsRef.current.get(filePath);
      if (inflight) return inflight;

      const promise = invoke<ImageInfo>("load_image", { filePath })
        .then((imgInfo) => {
          cacheImage(filePath, imgInfo);
          return imgInfo;
        })
        .finally(() => {
          inflightLoadsRef.current.delete(filePath);
        });

      inflightLoadsRef.current.set(filePath, promise);
      return promise;
    },
    [cacheImage],
  );

  // 현재 인덱스를 기준으로 앞/뒤 prefetchDistance 만큼의 이미지를 미리 로드
  const prefetchNearbyImages = useCallback(
    (
      images: string[],
      index: number,
      loopNavigation: boolean,
      prefetchDistance: number,
    ) => {
      if (!images.length || prefetchDistance <= 0) return;

      const targets = new Set<number>();
      const imageCount = images.length;
      const normalizeIndex = (value: number) =>
        ((value % imageCount) + imageCount) % imageCount;

      for (let offset = 1; offset <= prefetchDistance; offset += 1) {
        const prevIndex = index - offset;
        if (prevIndex >= 0) targets.add(prevIndex);
        else if (loopNavigation && imageCount > 1)
          targets.add(normalizeIndex(prevIndex));

        const nextIndex = index + offset;
        if (nextIndex < imageCount) targets.add(nextIndex);
        else if (loopNavigation && imageCount > 1)
          targets.add(normalizeIndex(nextIndex));
      }

      for (const targetIndex of targets) {
        const targetPath = images[targetIndex];
        if (!targetPath) continue;
        if (imageCacheRef.current.has(targetPath)) continue;
        if (inflightLoadsRef.current.has(targetPath)) continue;
        void getOrLoadImage(targetPath).catch(() => {});
      }
    },
    [getOrLoadImage],
  );

  // 캐시 모드가 바뀌면 즉시 캐시 크기를 재조정
  useEffect(() => {
    trimCacheToLimit(getCacheLimit(settings.cacheMode));
  }, [settings.cacheMode, trimCacheToLimit]);

  return {
    getOrLoadImage,
    prefetchNearbyImages,
    getPrefetchDistance: () => getPrefetchDistance(settings.cacheMode),
  };
}
