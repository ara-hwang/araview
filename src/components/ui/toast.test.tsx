import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key })
}))

import { ToastCopyButton, toCopyText, toast } from "@/components/ui/toast"

describe("toCopyText", () => {
  it("문자열과 숫자를 그대로 반환한다", () => {
    expect(toCopyText("오류")).toBe("오류")
    expect(toCopyText(42)).toBe("42")
  })

  it("null/boolean/undefined는 빈 문자열이다", () => {
    expect(toCopyText(null)).toBe("")
    expect(toCopyText(undefined)).toBe("")
    expect(toCopyText(false)).toBe("")
  })

  it("배열과 엘리먼트의 텍스트를 이어 붙인다", () => {
    expect(toCopyText(["a", "b"])).toBe("ab")
  })
})

describe("toast 헬퍼", () => {
  it("error는 type=error와 timeout(duration 매핑)으로 add를 호출한다", () => {
    const spy = vi.spyOn(toast, "add")
    try {
      const id = toast.error("실패", { description: "원인", duration: 2000 })
      expect(spy).toHaveBeenCalledWith(
        expect.objectContaining({
          title: "실패",
          description: "원인",
          type: "error",
          timeout: 2000
        })
      )
      toast.close(id)
    } finally {
      spy.mockRestore()
    }
  })

  it("success/info는 각 type으로 add를 호출한다", () => {
    const spy = vi.spyOn(toast, "add")
    try {
      const a = toast.success("완료")
      const b = toast.info("안내", { description: "설명" })
      expect(spy).toHaveBeenNthCalledWith(
        1,
        expect.objectContaining({ title: "완료", type: "success" })
      )
      expect(spy).toHaveBeenNthCalledWith(
        2,
        expect.objectContaining({ title: "안내", description: "설명", type: "info" })
      )
      toast.close(a)
      toast.close(b)
    } finally {
      spy.mockRestore()
    }
  })
})

describe("ToastCopyButton", () => {
  it("제목과 설명을 합쳐 클립보드에 복사한다", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.assign(navigator, { clipboard: { writeText } })

    const { unmount } = render(<ToastCopyButton title="실패" description="원인" />)
    try {
      const button = screen.getByRole("button", { name: "toast.copyMessage" })
      fireEvent.click(button)

      await waitFor(() => expect(writeText).toHaveBeenCalledWith("실패\n원인"))
      expect(screen.getByRole("button", { name: "toast.copied" })).toBeDefined()
    } finally {
      unmount()
    }
  })

  it("복사할 텍스트가 없으면 렌더하지 않는다", () => {
    const { container, unmount } = render(<ToastCopyButton />)
    try {
      expect(container.firstChild).toBeNull()
    } finally {
      unmount()
    }
  })
})
