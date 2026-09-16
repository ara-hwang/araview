import { cn } from "@/lib/utils"

type AppIconProps = {
  className?: string
  title?: string
}

/**
 * AraView 앱 아이콘 (obangsaek 2x2).
 * 라이트/다크는 `.dark` 클래스에 따라 하단 우측 타일만 바뀐다.
 * DESIGN.md `App Icon`이 소스 오브 트루스다.
 */
export function AppIcon({ className, title }: AppIconProps) {
  return (
    <svg
      viewBox="0 0 1024 1024"
      role={title ? "img" : undefined}
      aria-hidden={title ? undefined : true}
      aria-label={title}
      className={cn("size-16 shrink-0", className)}
    >
      {/* eslint-disable-next-line shadcn/no-raw-colors -- Obangsaek brand tile, see DESIGN.md App Icon. */}
      <rect x="72" y="72" width="408" height="408" rx="48" fill="#D9382B" />
      {/* eslint-disable-next-line shadcn/no-raw-colors -- Obangsaek brand tile, see DESIGN.md App Icon. */}
      <rect x="544" y="72" width="408" height="408" rx="48" fill="#E5A81C" />
      {/* eslint-disable-next-line shadcn/no-raw-colors -- Obangsaek brand tile, see DESIGN.md App Icon. */}
      <rect x="72" y="544" width="408" height="408" rx="48" fill="#2E8B7A" />
      <path
        d="M592 544 H904 Q952 544 952 592 V802 L802 952 H592 Q544 952 544 904 V592 Q544 544 592 544 Z"
        className="fill-[#171717] dark:fill-[#F5F5F5]"
      />
      <path d="M802 952 L952 802 L952 952 Z" className="fill-[#525252] dark:fill-[#171717]" />
    </svg>
  )
}
