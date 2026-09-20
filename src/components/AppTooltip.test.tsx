import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

import { AppTooltip } from "@/components/AppTooltip"
import { Button } from "@/components/ui/button"
import { ButtonGroupText } from "@/components/ui/button-group"
import { Toggle } from "@/components/ui/toggle"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { TooltipProvider } from "@/components/ui/tooltip"

function refWarnings(err: { mock: { calls: unknown[][] } }) {
  return err.mock.calls.filter((args) => String(args[0]).includes("cannot be given refs"))
}

/** content가 있는 경우 호버로 툴팁이 실제로 열리는지 확인한다. */
function renderWithProvider(node: React.ReactNode) {
  return render(<TooltipProvider delay={0}>{node}</TooltipProvider>)
}

describe("AppTooltip", () => {
  it("content가 없으면 트리거만 그대로 렌더한다", () => {
    render(
      <AppTooltip content={null}>
        <Button>열기</Button>
      </AppTooltip>
    )
    expect(screen.getByRole("button", { name: "열기" })).toBeDefined()
  })

  it("툴팁 내용은 호버/포커스 전까지 DOM에 나타나지 않는다", () => {
    const { container } = render(
      <AppTooltip content="실제 크기">
        <Button aria-label="실제 크기">100%</Button>
      </AppTooltip>
    )
    expect(screen.getByRole("button", { name: "실제 크기" })).toBeDefined()
    expect(container.querySelector("[data-slot='tooltip-content']")).toBeNull()
  })

  it("disabled 트리거도 크래시 없이 렌더한다", () => {
    render(
      <AppTooltip content="이미지 없음">
        <Button disabled aria-label="이미지 없음">
          열기
        </Button>
      </AppTooltip>
    )
    const trigger = screen.getByRole("button", { name: "이미지 없음" })
    expect((trigger as HTMLButtonElement).disabled).toBe(true)
  })

  it("Button 트리거 호버 시 툴팁이 열린다", async () => {
    renderWithProvider(
      <AppTooltip content="열기 툴팁">
        <Button aria-label="툴팁 버튼">열기</Button>
      </AppTooltip>
    )
    fireEvent.mouseEnter(screen.getByRole("button", { name: "툴팁 버튼" }))
    await waitFor(() => {
      expect(screen.getByText("열기 툴팁")).toBeDefined()
    })
  })

  it("ToggleGroupItem 트리거 호버 시 툴팁이 열린다", async () => {
    renderWithProvider(
      <ToggleGroup value={[]} onValueChange={() => {}} aria-label="맞춤">
        <AppTooltip content="너비 툴팁">
          <ToggleGroupItem value="width" aria-label="너비">
            W
          </ToggleGroupItem>
        </AppTooltip>
      </ToggleGroup>
    )
    fireEvent.mouseEnter(screen.getByRole("button", { name: "너비" }))
    await waitFor(() => {
      expect(screen.getByText("너비 툴팁")).toBeDefined()
    })
  })

  it("Toggle 트리거 호버 시 툴팁이 열린다", async () => {
    renderWithProvider(
      <AppTooltip content="항상 위 툴팁">
        <Toggle pressed={false} aria-label="항상 위">
          T
        </Toggle>
      </AppTooltip>
    )
    fireEvent.mouseEnter(screen.getByRole("button", { name: "항상 위" }))
    await waitFor(() => {
      expect(screen.getByText("항상 위 툴팁")).toBeDefined()
    })
  })

  it("ref 전달 가능한 트리거에는 ref 경고를 남기지 않는다", () => {
    const err = vi.spyOn(console, "error").mockImplementation(() => {})
    render(
      <TooltipProvider delay={300}>
        <AppTooltip content="열기">
          <Button aria-label="열기">열기</Button>
        </AppTooltip>
      </TooltipProvider>
    )
    expect(refWarnings(err)).toHaveLength(0)
    err.mockRestore()
  })

  it("Toggle 계열 트리거에도 ref 경고를 남기지 않는다 (React 19 ref-as-prop)", () => {
    const err = vi.spyOn(console, "error").mockImplementation(() => {})
    render(
      <TooltipProvider delay={300}>
        <ToggleGroup value={[]} onValueChange={() => {}} aria-label="맞춤">
          <AppTooltip content="너비 맞춤">
            <ToggleGroupItem value="width" aria-label="너비 맞춤">
              W
            </ToggleGroupItem>
          </AppTooltip>
        </ToggleGroup>
        <AppTooltip content="항상 위">
          <Toggle pressed={false} aria-label="항상 위">
            T
          </Toggle>
        </AppTooltip>
        <AppTooltip content="실제 크기">
          <ButtonGroupText aria-label="실제 크기">100%</ButtonGroupText>
        </AppTooltip>
      </TooltipProvider>
    )
    expect(refWarnings(err)).toHaveLength(0)
    err.mockRestore()
  })
})
