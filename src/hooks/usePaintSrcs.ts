import { useEffect, useState } from "react"
import { convertFileSrc } from "@tauri-apps/api/core"
import type { ImageInfo } from "@/types"

type GetOrLoadImage = (filePath: string) => Promise<ImageInfo>

export function usePaintSrcs(
  paths: string[],
  getOrLoadImage: GetOrLoadImage
): Map<string, string> {
  const [urls, setUrls] = useState<Map<string, string>>(() => new Map())
  const pathsKey = paths.join("\0")

  useEffect(() => {
    const list = pathsKey === "" ? [] : pathsKey.split("\0")
    let cancelled = false
    setUrls(new Map())

    void Promise.all(
      list.map(async (path) => {
        try {
          const info = await getOrLoadImage(path)
          if (cancelled) return
          const src = convertFileSrc(info.file_path)
          setUrls((prev) => {
            const next = new Map(prev)
            next.set(path, src)
            return next
          })
        } catch {
          return
        }
      })
    )

    return () => {
      cancelled = true
    }
  }, [pathsKey, getOrLoadImage])

  return urls
}
