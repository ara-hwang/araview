import { describe, expect, it } from "vitest"
import {
  DEFAULT_MOUSE,
  DEFAULT_SHORTCUTS,
  DEFAULT_WHEEL,
  eventToBinding,
  findBindingConflict,
  isValidBinding,
  normalizeBinding,
  sanitizeMouseMap,
  sanitizeShortcutMap,
  sanitizeWheelMap,
  toTauriAccelerator
} from "@/constants/shortcuts"

describe("normalizeBinding", () => {
  it("수식키 순서를 정규화한다", () => {
    expect(normalizeBinding("Shift+Ctrl+E")).toBe("Ctrl+Shift+E")
  })

  it("영문자는 대문자로 통일한다", () => {
    expect(normalizeBinding("ctrl+r")).toBe("Ctrl+R")
  })

  it("기호에 붙은 Shift는 제거한다", () => {
    expect(normalizeBinding("Shift++")).toBe("+")
  })

  it("빈 문자열은 해제를 의미한다", () => {
    expect(normalizeBinding("")).toBe("")
  })

  it("Meta 조합은 거부한다", () => {
    expect(normalizeBinding("Meta+O")).toBeNull()
  })
})

describe("eventToBinding", () => {
  it("이벤트에서 바인딩 문자열을 만든다", () => {
    expect(
      eventToBinding({
        key: "o",
        ctrlKey: true,
        shiftKey: false,
        altKey: false,
        metaKey: false
      })
    ).toBe("Ctrl+O")
  })

  it("스페이스바를 Space로 변환한다", () => {
    expect(
      eventToBinding({
        key: " ",
        ctrlKey: false,
        shiftKey: false,
        altKey: false,
        metaKey: false
      })
    ).toBe("Space")
  })

  it("수식키 단독 입력은 무시한다", () => {
    expect(
      eventToBinding({
        key: "Control",
        ctrlKey: true,
        shiftKey: false,
        altKey: false,
        metaKey: false
      })
    ).toBeNull()
  })
})

describe("defaults", () => {
  it("기본 단축키는 중복이 없다", () => {
    const values = Object.values(DEFAULT_SHORTCUTS)
    expect(new Set(values).size).toBe(values.length)
  })

  it("유효하지 않은 저장값은 기본값으로 되돌린다", () => {
    const sanitized = sanitizeShortcutMap({
      openFile: "Tab",
      zoomIn: "Shift++",
      toggleExif: 123
    })
    expect(sanitized.openFile).toBe(DEFAULT_SHORTCUTS.openFile)
    expect(sanitized.zoomIn).toBe("+")
    expect(sanitized.toggleExif).toBe(DEFAULT_SHORTCUTS.toggleExif)
  })

  it("충돌 액션을 찾는다", () => {
    expect(findBindingConflict(DEFAULT_SHORTCUTS, "zoomIn", "Ctrl+O")).toBe(
      "openFile"
    )
    expect(findBindingConflict(DEFAULT_SHORTCUTS, "zoomIn", "")).toBeNull()
  })

  it("휠과 마우스 기본값을 보존한다", () => {
    expect(
      sanitizeWheelMap({ wheelUp: "zoomIn", wheelDown: "oops" }).wheelUp
    ).toBe("zoomIn")
    expect(
      sanitizeWheelMap({ wheelUp: "zoomIn", wheelDown: "oops" }).wheelDown
    ).toBe(DEFAULT_WHEEL.wheelDown)
    expect(sanitizeMouseMap({ leftDrag: "prev" }).leftDrag).toBe(
      DEFAULT_MOUSE.leftDrag
    )
  })
})

describe("helpers", () => {
  it("예약 바인딩은 유효하지 않다", () => {
    expect(isValidBinding("Tab")).toBe(false)
    expect(isValidBinding("Ctrl+O")).toBe(true)
  })

  it("Tauri 가속기 형태로 변환한다", () => {
    expect(toTauriAccelerator("ArrowLeft")).toBe("Left")
    expect(toTauriAccelerator("Delete")).toBe("Del")
    expect(toTauriAccelerator("")).toBeUndefined()
  })
})
