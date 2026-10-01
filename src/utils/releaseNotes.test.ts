import { describe, expect, it } from "vitest"

import { parseInline, parseReleaseNotes } from "@/utils/releaseNotes"

describe("parseReleaseNotes", () => {
  it("제목, 목록, 문단을 블록으로 나눈다", () => {
    const blocks = parseReleaseNotes("## 변경 사항\n\n### 수정\n\n- 하나\n- 둘\n\n끝 문장")
    expect(blocks.map((b) => b.type)).toEqual(["heading", "heading", "list", "paragraph"])
    expect(blocks[0]).toMatchObject({ level: 2 })
    expect(blocks[1]).toMatchObject({ level: 3 })
    expect(blocks[2]).toMatchObject({ type: "list" })
    expect((blocks[2] as { items: unknown[] }).items).toHaveLength(2)
  })

  it("CRLF와 연속 줄을 처리한다", () => {
    const blocks = parseReleaseNotes("a\r\nb\r\n\r\n- c")
    expect(blocks).toHaveLength(2)
    expect(blocks[0]).toMatchObject({ type: "paragraph" })
  })

  it("빈 문자열은 블록이 없다", () => {
    expect(parseReleaseNotes("")).toEqual([])
  })

  it("HTML은 텍스트로 남긴다", () => {
    const [block] = parseReleaseNotes("<script>alert(1)</script>")
    expect(block).toMatchObject({
      inlines: [{ type: "text", text: "<script>alert(1)</script>" }]
    })
  })
})

describe("parseInline", () => {
  it("맨 URL 끝의 문장부호를 링크에서 뺀다", () => {
    expect(parseInline("보기: https://example.com/a.")).toEqual([
      { type: "text", text: "보기: " },
      { type: "link", text: "https://example.com/a", href: "https://example.com/a" },
      { type: "text", text: "." }
    ])
  })

  it("마크다운 링크, 코드, 굵게를 해석한다", () => {
    expect(parseInline("[문서](https://x.io) `a` **b**")).toEqual([
      { type: "link", text: "문서", href: "https://x.io" },
      { type: "text", text: " " },
      { type: "code", text: "a" },
      { type: "text", text: " " },
      { type: "bold", text: "b" }
    ])
  })

  it("http(s)가 아닌 링크는 텍스트로 둔다", () => {
    expect(parseInline("[x](javascript:alert(1))")).toEqual([
      { type: "text", text: "[x](javascript:alert(1))" }
    ])
  })
})
