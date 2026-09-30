import { describe, expect, it } from "vitest"

import { parseNotice, splitInlineCode } from "@/utils/licenseNotice"

describe("parseNotice", () => {
  it("parses headings, joined paragraphs, tables, and quotes", () => {
    const blocks = parseNotice(
      [
        "# Title",
        "",
        "line one",
        "line two",
        "",
        "## Section",
        "",
        "| A | B |",
        "| --- | --- |",
        "| x | y |",
        "| z | w |",
        "",
        "> quoted",
        "> text"
      ].join("\n")
    )
    expect(blocks).toEqual([
      { type: "h1", text: "Title" },
      { type: "p", text: "line one line two" },
      { type: "h2", text: "Section" },
      {
        type: "table",
        header: ["A", "B"],
        rows: [
          ["x", "y"],
          ["z", "w"]
        ]
      },
      { type: "quote", lines: ["quoted", "text"] }
    ])
  })

  it("handles CRLF input", () => {
    expect(parseNotice("# T\r\n\r\nbody\r\n")).toEqual([
      { type: "h1", text: "T" },
      { type: "p", text: "body" }
    ])
  })
})

describe("splitInlineCode", () => {
  it("separates code spans", () => {
    expect(splitInlineCode("see `a.rs` now")).toEqual([
      { text: "see ", code: false },
      { text: "a.rs", code: true },
      { text: " now", code: false }
    ])
  })
})
