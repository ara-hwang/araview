import { beforeEach, describe, expect, it, vi } from "vitest"

import { scrollWebtoonBy } from "@/hooks/useImageViewerHotkeys"

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
