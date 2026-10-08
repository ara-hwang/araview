import { cleanup, render, screen } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key })
}))

import { GifControls } from "@/components/GifControls"
import { useGifStore } from "@/store/gifStore"

const prevLabel = "menu.gifPrevFrame (,)"
const nextLabel = "menu.gifNextFrame (.)"
const pauseLabel = "menu.gifPause (P)"
const playLabel = "menu.gifPlay (P)"

function activate(frameCount: number) {
  const store = useGifStore.getState()
  store.setActive(true)
  store.setFrameCount(frameCount)
  store.setPlaying(true)
  store.setFrame(0)
}

beforeEach(() => {
  cleanup()
  useGifStore.getState().reset()
})

// 번호 칸이 별도 span이라 카운터 전체 textContent로 찾는다
const counter = (text: string) => (_: string, el: Element | null) =>
  el?.getAttribute("data-slot") === "button-group-text" && el.textContent === text

describe("GifControls", () => {
  it("제어 불가 이미지에서는 컨트롤을 렌더하지 않는다", () => {
    render(<GifControls />)
    expect(screen.queryByLabelText(prevLabel)).toBeNull()
  })

  it("단일 프레임 GIF에서는 컨트롤을 렌더하지 않는다", () => {
    activate(1)
    render(<GifControls />)
    expect(screen.queryByLabelText(prevLabel)).toBeNull()
  })

  it("다중 프레임 GIF에서 재생/정지, 프레임 이동, 카운터를 노출한다", async () => {
    activate(4)
    render(<GifControls />)

    expect(screen.getByLabelText(prevLabel)).toBeTruthy()
    expect(screen.getByLabelText(nextLabel)).toBeTruthy()
    expect(screen.getByText(counter("1/4"))).toBeTruthy()

    screen.getByLabelText(pauseLabel).click()
    expect(useGifStore.getState().playing).toBe(false)
    expect(await screen.findByLabelText(playLabel)).toBeTruthy()

    screen.getByLabelText(nextLabel).click()
    expect(useGifStore.getState().frame).toBe(1)
    expect(await screen.findByText(counter("2/4"))).toBeTruthy()

    screen.getByLabelText(prevLabel).click()
    expect(useGifStore.getState().frame).toBe(0)
  })
})
