import { useEffect, useState } from "react"
import { convertFileSrc, invoke } from "@tauri-apps/api/core"
import type { ImageInfo, ThumbnailInfo } from "@/types"

type GetOrLoadImage = (filePath: string) => Promise<ImageInfo>

const THUMB_MAX_SIDE = 128

// 썸네일 스트립용 축소 이미지 URL을 로드하는 훅.
// 백엔드 `generate_thumbnail`이 실패하면(아카이브 엔트리, SVG/HEIC 등)
// 원본 로드로 조용히 폴백한다.
export function useThumbnailSrcs(
  paths: string[],
  getOrLoadImage: GetOrLoadImage
): Map<string, string> {
  const [urls, setUrls] = useState<Map<string, string>>(() => new Map())
  const pathsKey = paths.join("\0")

  useEffect(() => {
    const list = pathsKey === "" ? [] : pathsKey.split("\0")
    let cancelled = false
    setUrls(new Map())

    const put = (path: string, filePath: string) => {
      if (cancelled) return
      const src = convertFileSrc(filePath)
      setUrls((prev) => {
        const next = new Map(prev)
        next.set(path, src)
        return next
      })
    }

    void Promise.all(
      list.map(async (path) => {
        try {
          const thumb = await invoke<ThumbnailInfo>("generate_thumbnail", {
            filePath: path,
            maxSide: THUMB_MAX_SIDE
          })
          put(path, thumb.file_path)
        } catch {
          try {
            const info = await getOrLoadImage(path)
            put(path, info.file_path)
          } catch {
            return
          }
        }
      })
    )

    return () => {
      cancelled = true
    }
  }, [pathsKey, getOrLoadImage])

  return urls
}
