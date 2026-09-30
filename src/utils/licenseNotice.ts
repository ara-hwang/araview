// THIRD_PARTY_LICENSES.md 전용 최소 마크다운 파서.
// 지원: # / ## 제목, 문단(줄바꿈된 원문은 공백으로 이음), > 인용, | 표 |.
export type NoticeBlock =
  | { type: "h1" | "h2"; text: string }
  | { type: "p"; text: string }
  | { type: "quote"; lines: string[] }
  | { type: "table"; header: string[]; rows: string[][] }

const splitRow = (line: string): string[] =>
  line
    .trim()
    .replace(/^\|/, "")
    .replace(/\|$/, "")
    .split("|")
    .map((cell) => cell.trim())

const isTableLine = (line: string) => line.trimStart().startsWith("|")
const isSeparatorRow = (cells: string[]) => cells.every((cell) => /^:?-{2,}:?$/.test(cell))

export function parseNotice(source: string): NoticeBlock[] {
  const lines = source.replace(/\r\n/g, "\n").split("\n")
  const blocks: NoticeBlock[] = []
  let i = 0

  while (i < lines.length) {
    const line = lines[i]
    if (line.trim() === "") {
      i += 1
      continue
    }

    if (line.startsWith("## ")) {
      blocks.push({ type: "h2", text: line.slice(3).trim() })
      i += 1
    } else if (line.startsWith("# ")) {
      blocks.push({ type: "h1", text: line.slice(2).trim() })
      i += 1
    } else if (isTableLine(line)) {
      const tableLines: string[] = []
      while (i < lines.length && isTableLine(lines[i])) {
        tableLines.push(lines[i])
        i += 1
      }
      const [header, ...rest] = tableLines.map(splitRow)
      const rows = rest.filter((cells) => !isSeparatorRow(cells))
      blocks.push({ type: "table", header, rows })
    } else if (line.startsWith(">")) {
      const quoteLines: string[] = []
      while (i < lines.length && lines[i].startsWith(">")) {
        quoteLines.push(lines[i].replace(/^>\s?/, ""))
        i += 1
      }
      blocks.push({ type: "quote", lines: quoteLines })
    } else {
      const paragraph: string[] = []
      while (
        i < lines.length &&
        lines[i].trim() !== "" &&
        !lines[i].startsWith("#") &&
        !lines[i].startsWith(">") &&
        !isTableLine(lines[i])
      ) {
        paragraph.push(lines[i].trim())
        i += 1
      }
      // 링크는 클릭 대상이 아니므로 "텍스트 (주소)" 평문으로 푼다.
      const text = paragraph.join(" ").replace(/\[([^\]]+)\]\(([^)]+)\)/g, "$1 ($2)")
      blocks.push({ type: "p", text })
    }
  }
  return blocks
}

/** `코드` 조각을 분리한다. 렌더러가 code 요소로 감싼다. */
export function splitInlineCode(text: string): { text: string; code: boolean }[] {
  return text
    .split(/(`[^`]+`)/)
    .filter((part) => part !== "")
    .map((part) =>
      part.startsWith("`") && part.endsWith("`") && part.length > 1
        ? { text: part.slice(1, -1), code: true }
        : { text: part, code: false }
    )
}
