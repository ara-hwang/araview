import {
  formatShortcutDisplay,
  type ShortcutActionId
} from "@/constants/shortcuts"
import { useSettingsStore } from "@/store/settingsStore"
import { cn } from "cn"

type ShortcutBadgeProps = {
  actionId: ShortcutActionId
  className?: string
}

/** 설정 항목에 연결된 단축키를 현재 할당값으로 표시한다. 미할당이면 렌더링하지 않는다. */
export function ShortcutBadge({ actionId, className }: ShortcutBadgeProps) {
  const binding = useSettingsStore((state) => state.shortcuts[actionId])
  if (!binding) return null
  return (
    <kbd
      className={cn(
        "border-border bg-muted text-muted-foreground inline-flex h-5 min-w-5 shrink-0 items-center justify-center rounded-sm border px-1 font-mono text-xs leading-none tabular-nums",
        className
      )}
    >
      {formatShortcutDisplay(binding)}
    </kbd>
  )
}
