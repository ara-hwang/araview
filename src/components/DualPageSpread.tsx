import { useCallback, useLayoutEffect, useRef, useState } from "react"

import { PixelArtImage } from "@/components/PixelArtImage"
import type { MultiPage } from "@/hooks/useMultiPageImages"
import { cn } from "@/lib/utils"
import type { ImageScalingMode } from "@/store/settingsStore"
import { getPixelArtDetectionPath } from "@/utils/imageRendering"

type DualPageSpreadProps = {
  pages: MultiPage[]
  /** right-to-left: 첫 장을 오른쪽에 둔다 */
  reversed: boolean
  src: (page: MultiPage) => string
  scalingMode: ImageScalingMode
  autoDetectPixelArt: boolean
  onDoubleClick?: (e: React.MouseEvent) => void
}

/**
 * 양쪽 보기 한 화면. 두 장이 모두 로드(또는 실패)한 뒤 함께 나타난다. 따로
 * 나타나면 먼저 로드된 장이 가운데에 그려졌다가 나머지가 로드될 때 옆으로 밀린다.
 * 화면이 바뀌면 `key`로 다시 마운트해 로드 상태를 초기화한다.
 */
export function DualPageSpread({
  pages,
  reversed,
  src,
  scalingMode,
  autoDetectPixelArt,
  onDoubleClick
}: DualPageSpreadProps) {
  const rootRef = useRef<HTMLDivElement>(null)
  const [settled, setSettled] = useState<ReadonlySet<string>>(() => new Set())
  const markSettled = useCallback((path: string) => {
    setSettled((prev) => (prev.has(path) ? prev : new Set(prev).add(path)))
  }, [])

  // 예열된 이미지는 마운트 시점에 이미 complete라 첫 페인트 전에 바로 표시한다.
  // onLoad만 기다리면 전환마다 빈 프레임이 낀다.
  useLayoutEffect(() => {
    const imgs = rootRef.current?.querySelectorAll("img")
    imgs?.forEach((img, index) => {
      const page = pages[index]
      if (page && img.complete && img.naturalWidth > 0) markSettled(page.path)
    })
  }, [markSettled, pages])

  const ready = pages.every((page) => settled.has(page.path))

  return (
    <div
      ref={rootRef}
      className={cn(
        "absolute inset-0 flex items-center justify-center",
        reversed && "flex-row-reverse",
        !ready && "invisible"
      )}
    >
      {pages.map((page, index) => (
        <PixelArtImage
          key={page.path}
          filePath={page.info.file_path}
          detectionPath={getPixelArtDetectionPath(page.info)}
          fileSize={page.info.file_size}
          scalingMode={scalingMode}
          autoDetectPixelArt={autoDetectPixelArt}
          detectionPriority={index === 0}
          src={src(page)}
          alt={page.info.file_name}
          // 마지막 홀수 장은 단일 중앙 표시
          className={
            pages.length === 1
              ? "max-h-full max-w-full object-contain"
              : "max-h-full max-w-[50%] object-contain"
          }
          draggable={false}
          onLoad={() => markSettled(page.path)}
          onError={() => markSettled(page.path)}
          onDoubleClick={onDoubleClick}
        />
      ))}
    </div>
  )
}
