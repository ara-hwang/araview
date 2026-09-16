import { cleanup, render, screen } from "@testing-library/react"
import { afterEach, describe, expect, it } from "vitest"

import { ShortcutBadge } from "@/components/settings/ShortcutBadge"
import { DEFAULT_SHORTCUTS } from "@/constants/shortcuts"
import { useSettingsStore } from "@/store/settingsStore"

function setShortcut(actionId: keyof typeof DEFAULT_SHORTCUTS, binding: string) {
  useSettingsStore.setState({
    shortcuts: { ...DEFAULT_SHORTCUTS, [actionId]: binding }
  })
}

describe("ShortcutBadge", () => {
  afterEach(() => {
    cleanup()
    setShortcut("toggleShuffle", DEFAULT_SHORTCUTS.toggleShuffle)
  })

  it("기본 할당 단축키를 표시한다", () => {
    render(<ShortcutBadge actionId="toggleShuffle" />)
    expect(screen.getByText("S")).toBeTruthy()
  })

  it("사용자 지정 단축키를 그대로 표시한다", () => {
    setShortcut("toggleShuffle", "Ctrl+Shift+X")
    render(<ShortcutBadge actionId="toggleShuffle" />)
    expect(screen.getByText("Ctrl+Shift+X")).toBeTruthy()
  })

  it("미할당이면 아무것도 그리지 않는다", () => {
    setShortcut("toggleShuffle", "")
    const { container } = render(<ShortcutBadge actionId="toggleShuffle" />)
    expect(container.querySelector("kbd")).toBeNull()
  })
})
