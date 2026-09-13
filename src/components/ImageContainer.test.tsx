import { createRef } from "react"
import { beforeEach, describe, expect, it, vi } from "vitest"
import { render, screen } from "@testing-library/react"

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (p: string) => p
}))

vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => vi.fn()
}))

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key })
}))

vi.mock("sonner", () => ({
  toast: { info: vi.fn(), error: vi.fn() }
}))

import { ImageContainer } from "@/components/ImageContainer"
import { closeImage, useAppStore } from "@/store/appStore"

const noop = () => {}

beforeEach(() => {
  closeImage()
})

describe("ImageContainer single mode", () => {
  it("맞춤 줌이 preflight/flex에 눌리지 않도록 이미지를 무제약으로 렌더한다", () => {
    useAppStore.setState({
      imageInfo: {
        file_path: "/pics/a.jpg",
        file_name: "a.jpg",
        file_size: 123,
        mime_type: "image/jpeg",
        width: 2000,
        height: 1000
      },
      containerSize: { width: 1000, height: 700 },
      imageSize: { width: 2000, height: 1000 },
      zoom: 0.5,
      position: { x: 0, y: 0 },
      rotation: 0,
      flipH: false,
      flipV: false
    })

    render(
      <ImageContainer
        containerRef={createRef<HTMLDivElement>()}
        imageRef={createRef<HTMLImageElement>()}
        onWheel={noop}
        onMouseDown={noop}
        onMouseMove={noop}
        onMouseUp={noop}
      />
    )

    const img = screen.getByAltText("a.jpg")
    // Tailwind preflight(img{max-width:100%})와 flex-shrink가
    // width/height + scale(zoom) 맞춤 계산을 깨뜨리므로 무제약이어야 한다.
    expect(img.className).toContain("max-w-none")
    expect(img.className).toContain("max-h-none")
    expect(img.className).toContain("shrink-0")
    expect(img.className).not.toContain("object-contain")
    // 파일 전환 시 이전 줌에서 새 줌으로 보간되는 전환 애니메이션을 막기 위해
    // transform 트랜지션을 두지 않는다.
    expect(img.className).not.toContain("transition-transform")
    expect(img.style.maxWidth).toBe("none")
    expect(img.style.maxHeight).toBe("none")
    // 렌더 박스 x 줌 = 맞춤 치수 계약
    expect(img.style.width).toBe("2000px")
    expect(img.style.height).toBe("1000px")
    expect(img.style.transform).toContain("scale(0.5")
  })
})
