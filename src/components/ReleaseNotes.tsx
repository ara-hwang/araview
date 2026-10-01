import type { ReactNode } from "react"

import { ExternalLink } from "@/components/ExternalLink"
import { parseReleaseNotes, type NoteInline } from "@/utils/releaseNotes"

function Inline({ node }: { node: NoteInline }): ReactNode {
  switch (node.type) {
    case "code":
      return <code className="rounded bg-muted px-1 py-0.5 text-xs">{node.text}</code>
    case "bold":
      return <strong className="font-semibold text-foreground">{node.text}</strong>
    case "link":
      return <ExternalLink href={node.href}>{node.text}</ExternalLink>
    default:
      return node.text
  }
}

function Inlines({ nodes }: { nodes: NoteInline[] }) {
  return nodes.map((node, i) => <Inline key={i} node={node} />)
}

export function ReleaseNotes({ markdown }: { markdown: string }) {
  const blocks = parseReleaseNotes(markdown)
  return (
    <div className="flex flex-col gap-2 p-3 text-sm [overflow-wrap:anywhere] text-muted-foreground">
      {blocks.map((block, i) => {
        if (block.type === "heading") {
          return (
            <p
              key={i}
              className={
                block.level === 3
                  ? "mt-1 text-xs font-medium text-foreground"
                  : "text-sm font-semibold text-foreground"
              }
            >
              <Inlines nodes={block.inlines} />
            </p>
          )
        }
        if (block.type === "list") {
          return (
            <ul key={i} className="flex list-disc flex-col gap-1 pl-5">
              {block.items.map((item, j) => (
                <li key={j}>
                  <Inlines nodes={item} />
                </li>
              ))}
            </ul>
          )
        }
        return (
          <p key={i}>
            <Inlines nodes={block.inlines} />
          </p>
        )
      })}
    </div>
  )
}
