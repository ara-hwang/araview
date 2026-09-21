import { describe, expect, it } from "vitest"

import { DEFAULT_WHEEL } from "@/constants/shortcuts"
import { resolveWheelAction, toWheelSlot } from "@/utils/wheelAction"

const event = (overrides: Partial<Parameters<typeof toWheelSlot>[0]> = {}) => ({
  deltaY: -1,
  ctrlKey: false,
  shiftKey: false,
  altKey: false,
  ...overrides
})

describe("toWheelSlot", () => {
  it("방향과 수식키를 슬롯 이름으로 만든다", () => {
    expect(toWheelSlot(event({ deltaY: -1 }))).toBe("wheelUp")
    expect(toWheelSlot(event({ deltaY: 1 }))).toBe("wheelDown")
    expect(toWheelSlot(event({ deltaY: -1, ctrlKey: true }))).toBe("ctrl+wheelUp")
    expect(toWheelSlot(event({ deltaY: 1, shiftKey: true }))).toBe("shift+wheelDown")
    expect(toWheelSlot(event({ deltaY: 1, altKey: true }))).toBe("alt+wheelDown")
  })
})

describe("resolveWheelAction", () => {
  it("설정된 동작을 돌려준다", () => {
    expect(resolveWheelAction(event({ deltaY: -1 }), DEFAULT_WHEEL)).toBe("prev")
    expect(resolveWheelAction(event({ deltaY: 1 }), DEFAULT_WHEEL)).toBe("next")
    expect(resolveWheelAction(event({ deltaY: -1, ctrlKey: true }), DEFAULT_WHEEL)).toBe("zoomIn")
  })

  it("none으로 설정된 슬롯은 null이다", () => {
    expect(resolveWheelAction(event({ deltaY: -1, shiftKey: true }), DEFAULT_WHEEL)).toBeNull()
  })

  it("세로 이동이 없으면 null이다", () => {
    expect(resolveWheelAction(event({ deltaY: 0 }), DEFAULT_WHEEL)).toBeNull()
  })
})
