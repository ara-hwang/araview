import { createRef } from "react"
import { describe, expect, it } from "vitest"
import { render } from "@testing-library/react"
import { Button } from "@/components/ui/button"

describe("Button", () => {
  it("ref를 DOM 버튼으로 전달한다", () => {
    const ref = createRef<HTMLButtonElement>()
    render(<Button ref={ref}>열기</Button>)
    expect(ref.current?.tagName).toBe("BUTTON")
  })
})
