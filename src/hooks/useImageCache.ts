import { useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ImageInfo, Settings } from "../types";
import { getCacheLimit, getPrefetchDistance } from "../utils/cacheConfig";

export function useImageCache(
  settings: Pick<Settings, "cacheMode" | "loopNavigation">,
) {
  const imageCacheRef = useRef<Map<string, ImageInfo>>(new Map());
  const inflightLoadsRef = useRef<Map<string, Promise<ImageInfo>>>(new Map());

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

  useEffect(() => {
    trimCacheToLimit(getCacheLimit(settings.cacheMode));
  }, [settings.cacheMode, trimCacheToLimit]);

  return {
    getOrLoadImage,
    prefetchNearbyImages,
    getPrefetchDistance: () => getPrefetchDistance(settings.cacheMode),
  };
}
