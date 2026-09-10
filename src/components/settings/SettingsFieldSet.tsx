import { FieldDescription, FieldLegend, FieldSet } from "@/components/ui/field"
import { ShortcutBadge } from "@/components/settings/ShortcutBadge"
import type { ShortcutActionId } from "@/constants/shortcuts"
import type { ReactNode } from "react"

export function SettingsFieldSet(props: {
  icon: ReactNode
  title: string
  description: string
  shortcutId?: ShortcutActionId
  children: ReactNode
}) {
  return (
    <FieldSet>
      <FieldLegend className="flex items-center gap-2">
        {props.icon}
        {props.title}
        {props.shortcutId ? (
          <ShortcutBadge actionId={props.shortcutId} />
        ) : null}
      </FieldLegend>
      <FieldDescription>{props.description}</FieldDescription>
      {props.children}
    </FieldSet>
  )
}
