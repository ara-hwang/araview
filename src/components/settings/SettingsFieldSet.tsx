import { FieldDescription, FieldLegend, FieldSet } from "@/components/ui/field"
import type { ReactNode } from "react"

export function SettingsFieldSet(props: {
  icon: ReactNode
  title: string
  description: string
  children: ReactNode
}) {
  return (
    <FieldSet>
      <FieldLegend className="flex items-center gap-2">
        {props.icon}
        {props.title}
      </FieldLegend>
      <FieldDescription>{props.description}</FieldDescription>
      {props.children}
    </FieldSet>
  )
}
