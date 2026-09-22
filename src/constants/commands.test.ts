import { describe, expect, it } from "vitest"

import { COMMAND_DEFS, filterCommands, isCommandEnabled } from "@/constants/commands"

describe("filterCommands", () => {
  const items = [
    { id: "zoomIn", label: "확대", keywords: ["zoom in"] },
    { id: "zoomOut", label: "축소", keywords: ["zoom out"] },
    { id: "openFile", label: "파일 열기", keywords: ["open", "file"] }
  ] as const

  it("빈 쿼리는 들어온 순서를 유지한다", () => {
    expect(filterCommands(items, "  ").map((i) => i.id)).toEqual(["zoomIn", "zoomOut", "openFile"])
  })

  it("라벨 일치를 별칭 일치보다 먼저 둔다", () => {
    const result = filterCommands(items, "확대")
    expect(result.map((i) => i.id)).toEqual(["zoomIn"])
  })

  it("영문 별칭으로 한글 라벨을 찾는다", () => {
    const result = filterCommands(items, "zoom")
    expect(result.map((i) => i.id)).toEqual(["zoomIn", "zoomOut"])
  })

  it("공백 분리 토큰은 AND로 동작한다", () => {
    const result = filterCommands(items, "zoom out")
    expect(result.map((i) => i.id)).toEqual(["zoomOut"])
  })

  it("대소문자를 구분하지 않는다", () => {
    const result = filterCommands(items, "ZOOM")
    expect(result).toHaveLength(2)
  })

  it("일치 항목이 없으면 빈 배열을 반환한다", () => {
    expect(filterCommands(items, "없는명령어")).toEqual([])
  })
})

describe("isCommandEnabled", () => {
  const ctx = (partial: Partial<{ hasImage: boolean; canNavigate: boolean; isGif: boolean }>) => ({
    hasImage: false,
    canNavigate: false,
    isGif: false,
    ...partial
  })

  it("이미지 필요 명령은 홈에서 비활성화된다", () => {
    const def = COMMAND_DEFS.find((d) => d.id === "zoomIn")!
    expect(isCommandEnabled(def, ctx({}))).toBe(false)
    expect(isCommandEnabled(def, ctx({ hasImage: true }))).toBe(true)
  })

  it("이전/다음은 이동 가능할 때만 활성화된다", () => {
    const def = COMMAND_DEFS.find((d) => d.id === "navigateNext")!
    expect(isCommandEnabled(def, ctx({ hasImage: true }))).toBe(false)
    expect(isCommandEnabled(def, ctx({ hasImage: true, canNavigate: true }))).toBe(true)
  })

  it("파일 열기와 설정은 항상 활성화된다", () => {
    for (const id of ["openFile", "openSettings"] as const) {
      const def = COMMAND_DEFS.find((d) => d.id === id)!
      expect(isCommandEnabled(def, ctx({}))).toBe(true)
    }
  })

  it("팔레트 자체 토글은 목록에 포함하지 않는다", () => {
    expect(COMMAND_DEFS.some((d) => d.id === "togglePalette")).toBe(false)
  })

  it("도크 토글은 이미지가 있을 때만 활성화된다", () => {
    const def = COMMAND_DEFS.find((d) => d.id === "toggleDock")!
    expect(isCommandEnabled(def, ctx({}))).toBe(false)
    expect(isCommandEnabled(def, ctx({ hasImage: true }))).toBe(true)
  })

  it("GIF 제어 명령은 제어 가능한 GIF일 때만 활성화된다", () => {
    for (const id of ["toggleGifPlayback", "gifPrevFrame", "gifNextFrame"] as const) {
      const def = COMMAND_DEFS.find((d) => d.id === id)!
      expect(isCommandEnabled(def, ctx({}))).toBe(false)
      expect(isCommandEnabled(def, ctx({ hasImage: true }))).toBe(false)
      expect(isCommandEnabled(def, ctx({ hasImage: true, isGif: true }))).toBe(true)
    }
  })
})
