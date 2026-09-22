import { beforeEach, describe, expect, it } from "vitest"

import { stepGifFrameBy, toggleGifPlayback, useGifStore } from "@/store/gifStore"

function activate(frameCount = 4) {
  const store = useGifStore.getState()
  store.setActive(true)
  store.setFrameCount(frameCount)
  store.setPlaying(true)
  store.setFrame(0)
}

beforeEach(() => {
  useGifStore.getState().reset()
})

describe("gifStore", () => {
  it("프레임 수를 설정하면 0번 프레임에서 시작한다", () => {
    activate(4)
    const state = useGifStore.getState()
    expect(state.frameCount).toBe(4)
    expect(state.frame).toBe(0)
    expect(state.active).toBe(true)
  })

  it("step은 프레임을 이동하고 재생을 멈춘다", () => {
    activate(4)
    stepGifFrameBy(1)
    expect(useGifStore.getState().frame).toBe(1)
    expect(useGifStore.getState().playing).toBe(false)
  })

  it("프레임 이동은 양 끝에서 멈춘다", () => {
    activate(3)
    stepGifFrameBy(-1)
    expect(useGifStore.getState().frame).toBe(0)
    useGifStore.getState().setFrame(2)
    stepGifFrameBy(1)
    expect(useGifStore.getState().frame).toBe(2)
  })

  it("제어 불가 상태(비활성/단일 프레임)에서는 토글이 무시된다", () => {
    toggleGifPlayback()
    expect(useGifStore.getState().playing).toBe(true)

    activate(1)
    toggleGifPlayback()
    expect(useGifStore.getState().playing).toBe(true)

    stepGifFrameBy(1)
    expect(useGifStore.getState().frame).toBe(0)
  })

  it("reset은 초기 상태로 돌린다", () => {
    activate(4)
    stepGifFrameBy(2)
    useGifStore.getState().reset()
    expect(useGifStore.getState()).toMatchObject({
      active: false,
      playing: true,
      frame: 0,
      frameCount: 0
    })
  })
})
