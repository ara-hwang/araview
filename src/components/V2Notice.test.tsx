import { fireEvent, render, screen } from "@testing-library/react"
import { beforeEach, describe, expect, it } from "vitest"

import { V2Notice } from "@/components/V2Notice"

describe("V2Notice", () => {
  beforeEach(() => localStorage.clear())

  it("처음에는 안내를 보이고 닫으면 다시 보이지 않는다", () => {
    const first = render(<V2Notice />)
    expect(screen.getByTestId("v2-notice")).toBeTruthy()
    fireEvent.click(screen.getByTestId("v2-notice-close"))
    expect(screen.queryByTestId("v2-notice")).toBeNull()
    first.unmount()

    render(<V2Notice />)
    expect(screen.queryByTestId("v2-notice")).toBeNull()
  })
})
