import { useMemo } from "react"
import { useTranslation } from "react-i18next"

import { parseNotice, splitInlineCode, type NoticeBlock } from "@/utils/licenseNotice"

type LicenseNoticeProps = {
  content: string
}

/** 이 행 수를 넘는 표는 패키지 목록(가상화)으로 대체해 DOM을 키우지 않는다. */
const MAX_INLINE_ROWS = 40

function Inline({ text }: { text: string }) {
  return (
    <>
      {splitInlineCode(text).map((part, index) =>
        part.code ? (
          <code key={index} className="rounded bg-muted px-1 py-0.5 font-mono text-xs">
            {part.text}
          </code>
        ) : (
          <span key={index}>{part.text}</span>
        )
      )}
    </>
  )
}

function NoticeTable({ block }: { block: Extract<NoticeBlock, { type: "table" }> }) {
  const { t } = useTranslation()
  if (block.rows.length > MAX_INLINE_ROWS) {
    return (
      <p className="rounded-lg border bg-muted/30 px-3 py-2 text-muted-foreground">
        {t("settings.licenses.tablePointer", { count: block.rows.length })}
      </p>
    )
  }
  return (
    <div className="overflow-x-auto rounded-lg border">
      <table className="w-full border-collapse text-left text-sm">
        <thead className="bg-muted/60 text-xs text-muted-foreground">
          <tr>
            {block.header.map((cell, index) => (
              <th key={index} scope="col" className="px-3 py-2 font-medium">
                {cell}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {block.rows.map((cells, rowIndex) => (
            <tr key={rowIndex} className="border-t">
              {cells.map((cell, cellIndex) => (
                <td key={cellIndex} className="px-3 py-1.5 align-top">
                  <Inline text={cell} />
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

/** 고지 문서(마크다운)를 제목, 문단, 표로 렌더링한다. */
export function LicenseNotice({ content }: LicenseNoticeProps) {
  const blocks = useMemo(() => parseNotice(content), [content])

  return (
    <div className="flex flex-col gap-4 p-4 text-sm leading-relaxed select-text">
      {blocks.map((block, index) => {
        switch (block.type) {
          case "h1":
            return (
              <h3 key={index} className="text-base font-semibold">
                {block.text}
              </h3>
            )
          case "h2":
            return (
              <h4 key={index} className="mt-2 border-b pb-1 text-sm font-semibold">
                {block.text}
              </h4>
            )
          case "p":
            return (
              <p key={index} className="text-muted-foreground">
                <Inline text={block.text} />
              </p>
            )
          case "quote":
            return (
              <blockquote
                key={index}
                className="border-l-2 pl-3 font-mono text-xs leading-relaxed whitespace-pre-wrap text-muted-foreground"
              >
                {block.lines.join("\n")}
              </blockquote>
            )
          case "table":
            return <NoticeTable key={index} block={block} />
        }
      })}
    </div>
  )
}
