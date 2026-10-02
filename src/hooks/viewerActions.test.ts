import { beforeEach, describe, expect, it, vi } from "vitest"

import { runViewerCommand, scrollWebtoonBy, type ViewerActionHandlers } from "@/hooks/viewerActions"
import { useSettingsStore } from "@/store/settingsStore"

beforeEach(() => {
  document.body.innerHTML = ""
})

describe("scrollWebtoonBy", () => {
  it("웹툰 스크롤 영역의 데이터 속성으로 안정적으로 찾는다", () => {
    const region = document.createElement("div")
    region.dataset.webtoonScrollRegion = "true"
    region.setAttribute("aria-label", "웹툰 읽기 영역")
    region.scrollBy = vi.fn()
    document.body.append(region)

    expect(scrollWebtoonBy(240)).toBe(true)
    expect(region.scrollBy).toHaveBeenCalledWith({ top: 240, behavior: "auto" })
  })

  it("스크롤 영역이 없으면 이동하지 않는다", () => {
    expect(scrollWebtoonBy(-240)).toBe(false)
  })
})

function spyHandlers(): ViewerActionHandlers {
  return {
    onNavigatePrev: vi.fn(),
    onNavigateNext: vi.fn(),
    onJumpPrev10: vi.fn(),
    onJumpNext10: vi.fn(),
    onJumpFirst: vi.fn(),
    onJumpLast: vi.fn(),
    onOpenFile: vi.fn(),
    onCloseImage: vi.fn(),
    onToggleExif: vi.fn(),
    onToggleFullscreen: vi.fn(),
    onToggleAlwaysOnTop: vi.fn(),
    onCopyImage: vi.fn(),
    onTrashFile: vi.fn(),
    onRevealInExplorer: vi.fn(),
    onOpenExternal: vi.fn(),
    onRenameFile: vi.fn(),
    onCopyPath: vi.fn(),
    onToggleGrid: vi.fn(),
    onToggleDock: vi.fn()
  }
}

describe("runViewerCommand", () => {
  it("핸들러 명령은 대응하는 핸들러 하나만 부른다", () => {
    const h = spyHandlers()
    expect(runViewerCommand("toggleDock", h)).toBe(true)
    expect(h.onToggleDock).toHaveBeenCalledTimes(1)
    expect(h.onToggleGrid).not.toHaveBeenCalled()
  })

  it("웹툰에서 좌우 팬은 이전/다음 이미지로 이동한다", () => {
    useSettingsStore.setState({ viewMode: "webtoon" })
    const h = spyHandlers()
    runViewerCommand("panLeft", h)
    runViewerCommand("panRight", h)
    expect(h.onNavigatePrev).toHaveBeenCalledTimes(1)
    expect(h.onNavigateNext).toHaveBeenCalledTimes(1)
  })

  it("웹툰이 아니면 좌우 팬은 이미지를 넘기지 않는다", () => {
    useSettingsStore.setState({ viewMode: "single" })
    const h = spyHandlers()
    runViewerCommand("panLeft", h)
    expect(h.onNavigatePrev).not.toHaveBeenCalled()
  })

  it("팔레트 토글은 처리하지 않는다", () => {
    expect(runViewerCommand("togglePalette", spyHandlers())).toBe(false)
  })
})
