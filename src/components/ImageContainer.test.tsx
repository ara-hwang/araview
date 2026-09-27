import { cleanup, render, screen } from "@testing-library/react"
import { createRef } from "react"
import { beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (p: string) => p,
  invoke: async () => ({
    classification: "continuous",
    confidence: 0,
    pixel_scale: null,
    method: "unsupported"
  })
}))

vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => vi.fn()
}))

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key })
}))

vi.mock("@/components/ui/toast", () => ({
  toast: { info: vi.fn(), error: vi.fn() }
}))

import { ImageContainer } from "@/components/ImageContainer"
import { closeImage, useAppStore } from "@/store/appStore"
import { DEFAULT_SETTINGS, useSettingsStore } from "@/store/settingsStore"

const noop = () => {}

beforeEach(() => {
  cleanup()
  closeImage()
  useSettingsStore.setState({ ...DEFAULT_SETTINGS })
})

describe("ImageContainer single mode", () => {
  it("맞춤 줌이 preflight/flex에 눌리지 않도록 이미지를 무제약으로 렌더한다", () => {
    useAppStore.setState({
      imageInfo: {
        file_path: "/pics/a.jpg",
        source_path: "/pics/a.jpg",
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

  it("축소 배율에서는 설정과 무관하게 부드러운 표시 클래스를 적용한다", () => {
    useSettingsStore.setState({ imageScalingMode: "pixelated", autoDetectPixelArt: false })
    useAppStore.setState({
      imageInfo: {
        file_path: "/pics/manga.jpg",
        source_path: "/pics/manga.jpg",
        file_name: "manga.jpg",
        file_size: 123,
        mime_type: "image/jpeg",
        width: 1600,
        height: 2400
      },
      imageSize: { width: 1600, height: 2400 },
      zoom: 0.35,
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

    const img = screen.getByAltText("manga.jpg")
    // nearest 축소는 스크린톤을 계단·무아레로 깨뜨리므로 smooth가 강제된다.
    expect(img.className).toContain("image-rendering-smooth")
    expect(img.className).not.toContain("image-rendering-pixelated")
  })

  it("픽셀 보존 모드는 레이아웃 크기로 확대하고 픽셀 클래스를 적용한다", () => {
    useSettingsStore.setState({ imageScalingMode: "pixelated", autoDetectPixelArt: false })
    useAppStore.setState({
      imageInfo: {
        file_path: "/pics/sprite.png",
        source_path: "/pics/sprite.png",
        file_name: "sprite.png",
        file_size: 123,
        mime_type: "image/png",
        width: 800,
        height: 600
      },
      imageSize: { width: 800, height: 600 },
      zoom: 2,
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

    const img = screen.getByAltText("sprite.png")
    expect(img.className).toContain("image-rendering-pixelated")
    expect(img.style.width).toBe("1600px")
    expect(img.style.height).toBe("1200px")
    expect(img.style.transform).toContain("scale(1, 1)")
  })

  it("SVG는 레이아웃 크기로 줌하고 transform에는 반전/회전만 남긴다", () => {
    useAppStore.setState({
      imageInfo: {
        file_path: "/pics/vector.svg",
        source_path: "/pics/vector.svg",
        file_name: "vector.svg",
        file_size: 456,
        mime_type: "image/svg+xml",
        width: 800,
        height: 600
      },
      containerSize: { width: 1000, height: 700 },
      imageSize: { width: 800, height: 600 },
      zoom: 2,
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

    const img = screen.getByAltText("vector.svg")
    // 표시 크기에서 재래스터되도록 레이아웃 크기에 줌을 곱한다.
    expect(img.style.width).toBe("1600px")
    expect(img.style.height).toBe("1200px")
    expect(img.style.transform).toContain("scale(1, 1)")
    expect(img.style.transform).not.toContain("scale(2")
  })

  it("SVG 반전은 줌과 분리된 부호로 transform에 남긴다", () => {
    useAppStore.setState({
      imageInfo: {
        file_path: "/pics/vector.svg",
        source_path: "/pics/vector.svg",
        file_name: "vector.svg",
        file_size: 456,
        mime_type: "image/svg+xml",
        width: 800,
        height: 600
      },
      containerSize: { width: 1000, height: 700 },
      imageSize: { width: 800, height: 600 },
      zoom: 2,
      position: { x: 10, y: -5 },
      rotation: 90,
      flipH: true,
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

    const img = screen.getByAltText("vector.svg")
    expect(img.style.width).toBe("1600px")
    expect(img.style.height).toBe("1200px")
    expect(img.style.transform).toContain("scale(-1, 1)")
    expect(img.style.transform).toContain("rotate(90deg)")
  })
})
