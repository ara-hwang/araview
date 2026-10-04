import { cleanup, fireEvent, render, screen } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (p: string) => p,
  invoke: vi.fn(async () => ({
    classification: "continuous",
    confidence: 0,
    pixel_scale: null,
    method: "unsupported"
  }))
}))

import { DualPageSpread } from "@/components/DualPageSpread"
import type { MultiPage } from "@/hooks/useMultiPageImages"

const page = (name: string): MultiPage => ({
  path: `/pics/${name}`,
  info: {
    file_path: `/pics/${name}`,
    source_path: `/pics/${name}`,
    mime_type: "image/jpeg",
    file_name: name,
    file_size: 10,
    width: 800,
    height: 1200
  }
})

function renderSpread(pages: MultiPage[]) {
  return render(
    <DualPageSpread
      pages={pages}
      reversed={false}
      src={(p) => p.info.file_path}
      scalingMode="smooth"
      autoDetectPixelArt={false}
    />
  )
}

beforeEach(() => {
  cleanup()
})

describe("DualPageSpread", () => {
  it("두 장이 모두 로드될 때까지 숨겼다가 함께 표시한다", () => {
    const { container } = renderSpread([page("1.jpg"), page("2.jpg")])
    const root = container.firstElementChild as HTMLElement
    expect(root.className).toContain("invisible")

    // 한 장만 먼저 보이면 가운데에 그려졌다가 나머지가 로드될 때 밀린다.
    fireEvent.load(screen.getByAltText("1.jpg"))
    expect(root.className).toContain("invisible")

    fireEvent.load(screen.getByAltText("2.jpg"))
    expect(root.className).not.toContain("invisible")
  })

  it("한 장이 로드에 실패해도 나머지를 표시한다", () => {
    const { container } = renderSpread([page("1.jpg"), page("2.jpg")])
    const root = container.firstElementChild as HTMLElement

    fireEvent.error(screen.getByAltText("1.jpg"))
    fireEvent.load(screen.getByAltText("2.jpg"))
    expect(root.className).not.toContain("invisible")
  })
})
