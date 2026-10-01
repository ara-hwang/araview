export type NoteInline =
  | { type: "text"; text: string }
  | { type: "code"; text: string }
  | { type: "bold"; text: string }
  | { type: "link"; text: string; href: string }

export type NoteBlock =
  | { type: "heading"; level: 1 | 2 | 3; inlines: NoteInline[] }
  | { type: "list"; items: NoteInline[][] }
  | { type: "paragraph"; inlines: NoteInline[] }

// `[텍스트](url)`, 맨 URL, `코드`, **굵게**를 한 번에 찾는다.
const INLINE =
  /\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)|(https?:\/\/[^\s<>)]+)|`([^`]+)`|\*\*([^*]+)\*\*/g

// 맨 URL 끝에 붙은 문장부호는 링크에서 뺀다.
const TRAILING_PUNCT = /[.,;:!?]+$/

export function parseInline(source: string): NoteInline[] {
  const out: NoteInline[] = []
  let last = 0
  for (const m of source.matchAll(INLINE)) {
    const start = m.index ?? 0
    if (start > last) out.push({ type: "text", text: source.slice(last, start) })
    if (m[2]) {
      out.push({ type: "link", text: m[1], href: m[2] })
      last = start + m[0].length
    } else if (m[3]) {
      const url = m[3].replace(TRAILING_PUNCT, "")
      out.push({ type: "link", text: url, href: url })
      last = start + url.length
    } else if (m[4]) {
      out.push({ type: "code", text: m[4] })
      last = start + m[0].length
    } else {
      out.push({ type: "bold", text: m[5] })
      last = start + m[0].length
    }
  }
  if (last < source.length) out.push({ type: "text", text: source.slice(last) })
  return out
}

/** 릴리스 노트용 최소 마크다운(제목, 목록, 문단, 링크, 코드, 굵게)만 해석한다. HTML은 해석하지 않는다. */
export function parseReleaseNotes(markdown: string): NoteBlock[] {
  const blocks: NoteBlock[] = []
  let list: NoteInline[][] | null = null
  let paragraph: string[] = []

  const flushParagraph = () => {
    if (paragraph.length > 0) {
      blocks.push({ type: "paragraph", inlines: parseInline(paragraph.join(" ")) })
      paragraph = []
    }
  }
  const flushList = () => {
    if (list) {
      blocks.push({ type: "list", items: list })
      list = null
    }
  }

  for (const raw of markdown.replace(/\r\n?/g, "\n").split("\n")) {
    const line = raw.trim()
    const heading = /^(#{1,3})\s+(.+)$/.exec(line)
    const item = /^[-*]\s+(.+)$/.exec(line)
    if (!line) {
      flushParagraph()
      flushList()
    } else if (heading) {
      flushParagraph()
      flushList()
      blocks.push({
        type: "heading",
        level: heading[1].length as 1 | 2 | 3,
        inlines: parseInline(heading[2])
      })
    } else if (item) {
      flushParagraph()
      list ??= []
      list.push(parseInline(item[1]))
    } else {
      flushList()
      paragraph.push(line)
    }
  }
  flushParagraph()
  flushList()
  return blocks
}
