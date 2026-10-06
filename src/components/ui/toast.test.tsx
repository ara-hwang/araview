import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key })
}))

import { ToastCopyButton, buildErrorCopyText, toCopyText, toast } from "@/components/ui/toast"

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

describe("buildErrorCopyText", () => {
  it("제목·설명·상세·시각을 순서대로 합친다", () => {
    const text = buildErrorCopyText({
      title: "실패",
      description: "원인",
      details: "code: corrupt\npath: D:\\a.png",
      createdAt: 0
    })
    const lines = text.split("\n")
    expect(lines[0]).toBe("실패")
    expect(lines[1]).toBe("원인")
    expect(lines[2]).toBe("code: corrupt")
    expect(lines[3]).toBe("path: D:\\a.png")
    expect(lines[4]).toMatch(/^time: /)
  })

  it("상세가 없어도 시각 줄은 붙는다", () => {
    const text = buildErrorCopyText({ title: "실패", createdAt: 0 })
    expect(text.split("\n")[0]).toBe("실패")
    expect(text).toContain("\ntime: ")
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

  it("details는 data에 텍스트로 저장하고 생성 시각을 함께 기록한다", () => {
    const spy = vi.spyOn(toast, "add")
    try {
      const id = toast.error("실패", {
        description: "원인",
        details: "code: corrupt\npath: D:\\a.png"
      })
      expect(spy).toHaveBeenCalledWith(
        expect.objectContaining({
          title: "실패",
          type: "error",
          data: expect.objectContaining({
            details: "code: corrupt\npath: D:\\a.png",
            createdAt: expect.any(Number)
          })
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
  it("제목·설명·상세·시각을 합쳐 클립보드에 복사한다", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.assign(navigator, { clipboard: { writeText } })

    const { unmount } = render(
      <ToastCopyButton
        title="실패"
        description="원인"
        details="code: corrupt"
        createdAt={0}
      />
    )
    try {
      const button = screen.getByRole("button", { name: "toast.copyMessage" })
      fireEvent.click(button)

      await waitFor(() => expect(writeText).toHaveBeenCalledTimes(1))
      const copied: string = writeText.mock.calls[0][0]
      const lines = copied.split("\n")
      expect(lines[0]).toBe("실패")
      expect(lines[1]).toBe("원인")
      expect(lines[2]).toBe("code: corrupt")
      expect(lines[3]).toMatch(/^time: /)
      // 복사 완료 표시는 writeText 약속이 풀린 뒤의 상태 갱신이라 렌더를 기다린다.
      expect(await screen.findByRole("button", { name: "toast.copied" })).toBeDefined()
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
