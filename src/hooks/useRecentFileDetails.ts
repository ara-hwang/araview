import { convertFileSrc } from "@tauri-apps/api/core"
import { useEffect, useState } from "react"

import type { ImageInfo } from "@/types"

export type RecentFileStatus = "loading" | "done" | "error"

export type RecentFileDetail = {
  status: RecentFileStatus
  info?: ImageInfo
  src?: string
}

type GetOrLoadImage = (filePath: string) => Promise<ImageInfo>

/**
 * 최근 파일 경로마다 ImageInfo + 썸네일 src를 확보한다.
 * 아카이브(application/*)는 썸네일 대신 아이콘으로 표시하므로 src를 만들지 않는다.
 * 실패한 경로는 error 상태로 남겨 목록에서 제거할 수 있게 한다.
 */
export function useRecentFileDetails(
  paths: string[],
  getOrLoadImage: GetOrLoadImage
): Map<string, RecentFileDetail> {
  const [details, setDetails] = useState<Map<string, RecentFileDetail>>(() => new Map())
  const pathsKey = paths.join("\0")

  useEffect(() => {
    const list = pathsKey === "" ? [] : pathsKey.split("\0")
    let cancelled = false
    setDetails(new Map(list.map((path) => [path, { status: "loading" as const }])))
    if (list.length === 0) return

    void Promise.all(
      list.map(async (path) => {
        try {
          const info = await getOrLoadImage(path)
          if (cancelled) return
          const isArchive = info.mime_type.startsWith("application/")
          const src = isArchive ? undefined : convertFileSrc(info.file_path)
          setDetails((prev) => {
            const next = new Map(prev)
            next.set(path, { status: "done", info, src })
            return next
          })
        } catch {
          if (cancelled) return
          setDetails((prev) => {
            const next = new Map(prev)
            next.set(path, { status: "error" })
            return next
          })
        }
      })
    )

    return () => {
      cancelled = true
    }
  }, [pathsKey, getOrLoadImage])

  return details
}
