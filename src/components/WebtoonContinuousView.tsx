import { convertFileSrc } from "@tauri-apps/api/core"
import { useEffect, useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"
import type { ImageInfo } from "@/types"
import { errorMessage } from "@/utils/appError"

type GetOrLoadImage = (filePath: string) => Promise<ImageInfo>

export type WebtoonScrollTarget = {
  index: number
  nonce: number
} | null

function WebtoonLazyPage({
  path,
  index,
  getOrLoadImage,
  registerRef
}: {
  path: string
  index: number
  getOrLoadImage: GetOrLoadImage
  registerRef: (index: number, el: HTMLDivElement | null) => void
}) {
  const { t } = useTranslation()
  const wrapRef = useRef<HTMLDivElement>(null)
  const [info, setInfo] = useState<ImageInfo | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [nonce, setNonce] = useState(0)
  const name = path.split(/[\\/]/).pop() ?? path

  useEffect(() => {
    registerRef(index, wrapRef.current)
    return () => registerRef(index, null)
  }, [index, registerRef])

  useEffect(() => {
    const el = wrapRef.current
    if (!el) return
    let cancelled = false
    let loaded = false

    const load = async () => {
      if (loaded) return
      loaded = true
      try {
        const data = await getOrLoadImage(path)
        if (!cancelled) {
          setInfo(data)
          setError(null)
        }
      } catch (e) {
        if (!cancelled) {
          setError(errorMessage(e))
          loaded = false
        }
      }
    }

    if (typeof IntersectionObserver === "undefined") {
      void load()
      return () => {
        cancelled = true
      }
    }

    const io = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          void load()
        }
      },
      { rootMargin: "200% 0px" }
    )
    io.observe(el)
    return () => {
      cancelled = true
      io.disconnect()
    }
  }, [path, getOrLoadImage, nonce])

  return (
    <div ref={wrapRef} data-webtoon-index={index} className="flex w-full justify-center">
      {info && !error ? (
        <img
          src={convertFileSrc(info.file_path)}
          alt={name}
          className="max-w-full object-contain"
          draggable={false}
          loading="lazy"
          decoding="async"
          onError={() => {
            setInfo(null)
            setError(t("error.corrupt.title"))
          }}
        />
      ) : error ? (
        <div className="flex w-full max-w-2xl flex-col items-center gap-2 rounded-lg border border-destructive/20 bg-background p-6 text-center">
          <p className="text-sm text-muted-foreground">
            {name}: {error}
          </p>
          <Button
            size="sm"
            variant="outline"
            onClick={() => {
              setError(null)
              setNonce((n) => n + 1)
            }}
          >
            {t("viewer.error.retry")}
          </Button>
        </div>
      ) : (
        <div
          aria-hidden="true"
          className="min-h-64 w-full max-w-2xl animate-pulse rounded bg-muted/40"
        />
      )}
    </div>
  )
}

export function WebtoonContinuousView({
  images,
  currentIndex,
  getOrLoadImage,
  onCenterChange,
  scrollTarget
}: {
  images: string[]
  currentIndex: number
  getOrLoadImage: GetOrLoadImage
  onCenterChange: (index: number) => void
  scrollTarget: WebtoonScrollTarget
}) {
  const scrollRef = useRef<HTMLDivElement>(null)
  const itemRefs = useRef(new Map<number, HTMLDivElement>())
  const centerRef = useRef(currentIndex)
  centerRef.current = currentIndex
  const onCenterRef = useRef(onCenterChange)
  onCenterRef.current = onCenterChange
  const rafRef = useRef(0)

  const registerRef = useRef((index: number, el: HTMLDivElement | null) => {
    if (el) itemRefs.current.set(index, el)
    else itemRefs.current.delete(index)
  }).current

  // 초기 진입과 외부 점프(슬라이더, 썸네일, 단축키) 시 해당 인덱스로 스크롤
  useEffect(() => {
    if (!scrollTarget) return
    const el = itemRefs.current.get(scrollTarget.index)
    el?.scrollIntoView({ block: "start", behavior: "auto" })
  }, [scrollTarget])

  // viewMode 전환 직후 현재 위치로 스크롤
  useEffect(() => {
    const frame = requestAnimationFrame(() => {
      const el = itemRefs.current.get(centerRef.current)
      el?.scrollIntoView({ block: "start", behavior: "auto" })
    })
    return () => cancelAnimationFrame(frame)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  useEffect(() => {
    const container = scrollRef.current
    if (!container) return

    const findCenter = () => {
      rafRef.current = 0
      const rect = container.getBoundingClientRect()
      const midY = rect.top + rect.height / 2
      let best = -1
      let bestDist = Number.POSITIVE_INFINITY
      for (const [idx, el] of itemRefs.current) {
        const r = el.getBoundingClientRect()
        if (r.bottom < rect.top || r.top > rect.bottom) continue
        const dist = Math.abs(r.top + r.height / 2 - midY)
        if (dist < bestDist) {
          bestDist = dist
          best = idx
        }
      }
      if (best >= 0 && best !== centerRef.current) {
        onCenterRef.current(best)
      }
    }

    const onScroll = () => {
      if (rafRef.current !== 0) return
      rafRef.current = requestAnimationFrame(findCenter)
    }

    container.addEventListener("scroll", onScroll, { passive: true })
    return () => {
      container.removeEventListener("scroll", onScroll)
      if (rafRef.current !== 0) cancelAnimationFrame(rafRef.current)
    }
  }, [])

  if (images.length === 0) return null

  return (
    <div
      ref={scrollRef}
      role="region"
      tabIndex={0}
      className={cn(
        "absolute inset-0 overflow-x-hidden overflow-y-auto focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
      )}
      aria-label="webtoon-scroll"
    >
      <div className="flex flex-col items-center gap-2 py-2">
        {images.map((path, i) => (
          <WebtoonLazyPage
            key={path}
            path={path}
            index={i}
            getOrLoadImage={getOrLoadImage}
            registerRef={registerRef}
          />
        ))}
      </div>
    </div>
  )
}
