import { create } from "zustand"

import { clampGifFrame } from "@/utils/gifPlayback"

/**
 * GIF 캔버스 재생 상태. 단일 보기의 `useGifPlayer`가 채우고,
 * 헤더 컨트롤/명령 팔레트/단축키가 읽는다.
 */
type GifPlaybackState = {
  /** 현재 이미지가 캔버스로 제어 가능한 GIF인지 */
  active: boolean
  playing: boolean
  frame: number
  frameCount: number
  setActive: (active: boolean) => void
  setPlaying: (playing: boolean) => void
  setFrame: (frame: number) => void
  setFrameCount: (frameCount: number) => void
  togglePlaying: () => void
  step: (delta: number) => void
  reset: () => void
}

const initialGifState = {
  active: false,
  playing: true,
  frame: 0,
  frameCount: 0
}

export const useGifStore = create<GifPlaybackState>((set, get) => ({
  ...initialGifState,
  setActive: (active) => set({ active }),
  setPlaying: (playing) => set({ playing }),
  setFrame: (frame) => set({ frame: clampGifFrame(frame, get().frameCount) }),
  setFrameCount: (frameCount) => set({ frameCount: Math.max(0, Math.trunc(frameCount)), frame: 0 }),
  togglePlaying: () => set((state) => ({ playing: !state.playing })),
  step: (delta) => {
    const { frame, frameCount } = get()
    set({
      playing: false,
      frame: clampGifFrame(frame + delta, frameCount)
    })
  },
  reset: () => set({ ...initialGifState })
}))

/** 제어 가능한 GIF일 때만 재생/정지를 토글한다. */
export const toggleGifPlayback = () => {
  const state = useGifStore.getState()
  if (!state.active || state.frameCount <= 1) return
  state.togglePlaying()
}

/** 제어 가능한 GIF일 때만 프레임을 이동한다(이동 시 정지). */
export const stepGifFrameBy = (delta: number) => {
  const state = useGifStore.getState()
  if (!state.active || state.frameCount <= 1) return
  state.step(delta)
}
