import { openUrl } from "@tauri-apps/plugin-opener"
import type { ReactNode } from "react"

/** 웹뷰 안에서 이동하지 않고 기본 브라우저로 여는 링크. */
export function ExternalLink({ href, children }: { href: string; children: ReactNode }) {
  return (
    <a
      href={href}
      className="text-foreground underline underline-offset-2 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
      onClick={(e) => {
        e.preventDefault()
        void openUrl(href).catch(() => {})
      }}
    >
      {children}
    </a>
  )
}
