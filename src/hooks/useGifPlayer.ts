import { useEffect, useRef, useState, type RefObject } from "react"

import { applyImageNaturalSize } from "@/store/appStore"
import { useGifStore } from "@/store/gifStore"
import { GIF_MIME, type AnimationMime } from "@/utils/gifPlayback"

/** 프레임 duration이 0/누락인 GIF가 폭주하지 않게 하는 하한 */
const MIN_FRAME_MS = 20
const DEFAULT_FRAME_MS = 100

/**
 * WebCodecs ImageDecoder로 GIF/APNG를 캔버스에 그려 재생/정지/프레임 이동을 가능하게 한다.
 * 단일 보기 + GIF/APNG + 디코더 지원일 때만 `enabled`가 true이며, 그 외에는 기존
 * `<img>` 네이티브 애니메이션을 그대로 쓴다.
 *
 * 디코더가 준비되면 `gifStore`에 active/frameCount/playing을 기록하고, store의
 * frame/playing 변화가 곧 그리기/스케줄링 트리거가 된다. 실패하면 `failed`를
 * 돌려주고 호출부가 `<img>`로 폴백한다.
 */
export function useGifPlayer(
  canvasRef: RefObject<HTMLCanvasElement | null>,
  src: string | null,
  enabled: boolean,
  smoothingEnabled = true,
  type: AnimationMime = GIF_MIME
): { failed: boolean } {
  const [failed, setFailed] = useState(false)
  const [decoderReady, setDecoderReady] = useState(0)
  const decoderRef = useRef<ImageDecoder | null>(null)
  const repetitionRef = useRef(Number.POSITIVE_INFINITY)
  const cyclesRef = useRef(0)
  const playing = useGifStore((state) => state.playing)
  const frame = useGifStore((state) => state.frame)
  const frameCount = useGifStore((state) => state.frameCount)

  // 이미지가 바뀌면 디코더를 새로 만들고 첫 프레임 치수로 뷰를 초기화한다.
  useEffect(() => {
    if (!enabled || !src) {
      useGifStore.getState().reset()
      setFailed(false)
      setDecoderReady(0)
      return
    }

    let cancelled = false
    let localDecoder: ImageDecoder | null = null
    const controller = new AbortController()
    const gifStore = useGifStore.getState()
    gifStore.reset()
    setFailed(false)

    const load = async () => {
      try {
        const response = await fetch(src, { signal: controller.signal })
        if (!response.ok) throw new Error(`Animation fetch failed: ${response.status}`)
        const data = await response.arrayBuffer()
        if (cancelled) return

        const decoder = new ImageDecoder({ data, type, preferAnimation: true })
        await decoder.tracks.ready
        await decoder.completed
        if (cancelled) {
          decoder.close()
          return
        }
        const track = decoder.tracks.selectedTrack
        const count = track?.frameCount ?? 0
        if (count <= 1) {
          // 정지 GIF/PNG: 캔버스 제어 없이 <img> 한 장으로 보여준다.
          decoder.close()
          return
        }

        // 첫 프레임을 한 번 디코드해 표시 치수를 얻고 즉시 그린다.
        const { image } = await decoder.decode({ frameIndex: 0 })
        const width = image.displayWidth
        const height = image.displayHeight
        image.close()
        if (cancelled) {
          decoder.close()
          return
        }
        if (width <= 0 || height <= 0) {
          decoder.close()
          return
        }

        localDecoder = decoder
        decoderRef.current = decoder
        repetitionRef.current = track?.repetitionCount ?? Number.POSITIVE_INFINITY
        cyclesRef.current = 0
        applyImageNaturalSize(width, height)

        const next = useGifStore.getState()
        next.setFrameCount(count)
        next.setFrame(0)
        // OS의 모션 최소화 설정을 존중해 정지 상태로 시작한다.
        next.setPlaying(!window.matchMedia("(prefers-reduced-motion: reduce)").matches)
        next.setActive(true)
        setDecoderReady((value) => value + 1)
      } catch (e) {
        if (!cancelled && (e as Error | undefined)?.name !== "AbortError") {
          setFailed(true)
        }
      }
    }

    void load()

    return () => {
      cancelled = true
      controller.abort()
      if (localDecoder) {
        localDecoder.close()
        if (decoderRef.current === localDecoder) decoderRef.current = null
      }
      useGifStore.getState().reset()
    }
  }, [enabled, src, type])

  // store의 frame/playing에 맞춰 현재 프레임을 그리고, 재생 중이면 다음 프레임을 예약한다.
  useEffect(() => {
    const decoder = decoderRef.current
    const canvas = canvasRef.current
    if (!enabled || !decoder || !canvas || frameCount <= 0) return

    let cancelled = false
    let timer: number | undefined

    const draw = async () => {
      try {
        const { image } = await decoder.decode({ frameIndex: frame })
        if (cancelled) {
          image.close()
          return
        }
        const ctx = canvas.getContext("2d")
        if (!ctx) {
          image.close()
          return
        }
        if (canvas.width !== image.displayWidth || canvas.height !== image.displayHeight) {
          canvas.width = image.displayWidth
          canvas.height = image.displayHeight
        }
        ctx.imageSmoothingEnabled = smoothingEnabled
        ctx.clearRect(0, 0, canvas.width, canvas.height)
        ctx.drawImage(image, 0, 0)
        const durationMs = Math.max(
          MIN_FRAME_MS,
          (image.duration ?? DEFAULT_FRAME_MS * 1000) / 1000
        )
        image.close()

        if (!cancelled && useGifStore.getState().playing) {
          timer = window.setTimeout(() => {
            const state = useGifStore.getState()
            if (!state.playing || state.frameCount <= 0) return
            const next = state.frame + 1
            if (next < state.frameCount) {
              state.setFrame(next)
              return
            }
            // 마지막 프레임 뒤: 유한 반복 GIF면 멈추고, 아니면 처음으로 돌아간다.
            cyclesRef.current += 1
            if (cyclesRef.current >= repetitionRef.current) {
              state.setPlaying(false)
              return
            }
            state.setFrame(0)
          }, durationMs)
        }
      } catch {
        if (!cancelled) setFailed(true)
      }
    }

    void draw()

    return () => {
      cancelled = true
      if (timer !== undefined) window.clearTimeout(timer)
    }
  }, [canvasRef, decoderReady, enabled, frame, frameCount, playing, smoothingEnabled])

  return { failed }
}
