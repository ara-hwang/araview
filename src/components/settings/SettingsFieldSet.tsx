import type { ReactNode } from "react"

import { ShortcutBadge } from "@/components/settings/ShortcutBadge"
import { FieldDescription, FieldLegend, FieldSet } from "@/components/ui/field"
import type { ShortcutActionId } from "@/constants/shortcuts"

export function SettingsFieldSet(props: {
  icon: ReactNode
  title: string
  description?: string
  shortcutId?: ShortcutActionId
  children: ReactNode
}) {
  return (
    <FieldSet>
      <FieldLegend className="flex items-center gap-2">
        {props.icon}
        {props.title}
        {props.shortcutId ? <ShortcutBadge actionId={props.shortcutId} /> : null}
      </FieldLegend>
      {props.description ? <FieldDescription>{props.description}</FieldDescription> : null}
      {props.children}
    </FieldSet>
  )
}
