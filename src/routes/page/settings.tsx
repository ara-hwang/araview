import { createFileRoute } from "@tanstack/react-router"
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import { Label } from "@/components/ui/label"
import { useSettingsStore, type CacheMode } from "@/store/settingsStore"
import { HardDriveIcon, NavigationIcon } from "lucide-react"
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLegend,
  FieldSeparator,
  FieldSet
} from "@/components/ui/field"
import { useShallow } from "zustand/react/shallow"

export const Route = createFileRoute("/page/settings")({
  component: SettingsPage
})

function SettingsPage() {
  const settings = useSettingsStore(
    useShallow((state) => ({
      loopNavigation: state.loopNavigation,
      cacheMode: state.cacheMode,
      viewMode: state.viewMode
    }))
  )
  const setLoopNavigation = useSettingsStore((state) => state.setLoopNavigation)
  const setCacheMode = useSettingsStore((state) => state.setCacheMode)

  return (
    <div className="flex flex-col gap-2 p-2">
      {/* Content */}
      <FieldGroup>
        {/* Navigation */}
        <CustomFieldSet
          icon={<NavigationIcon className="size-4" />}
          title="Navigation"
          description="Navigation mode for the image viewer"
        >
          <RadioGroup
            value={settings.loopNavigation ? "loop" : "stop"}
            onValueChange={(value) => setLoopNavigation(value === "loop")}
          >
            <Field orientation="horizontal">
              <RadioGroupItem value="stop" id="stop" />
              <Label htmlFor="stop">Stop at first / last image</Label>
            </Field>
            <Field orientation="horizontal">
              <RadioGroupItem value="loop" id="loop" />
              <Label htmlFor="loop">Loop through images</Label>
            </Field>
          </RadioGroup>
        </CustomFieldSet>

        <FieldSeparator />

        {/* Cache */}
        <CustomFieldSet
          icon={<HardDriveIcon className="size-4" />}
          title="Cache"
          description="Cache mode for the image viewer"
        >
          <RadioGroup
            value={settings.cacheMode}
            onValueChange={(value) => setCacheMode(value as CacheMode)}
          >
            <Field orientation="horizontal">
              <RadioGroupItem value="off" id="off" />
              <Label htmlFor="off">Off (current image only)</Label>
            </Field>
            <Field orientation="horizontal">
              <RadioGroupItem value="nearby" id="nearby" />
              <Label htmlFor="nearby">Nearby (previous / next preload)</Label>
            </Field>
            <Field orientation="horizontal">
              <RadioGroupItem value="extended" id="extended" />
              <Label htmlFor="extended">Extended (up to ±3 preload)</Label>
            </Field>
            <Field orientation="horizontal">
              <RadioGroupItem value="memory-1gb" id="memory-1gb" />
              <Label htmlFor="memory-1gb">Memory limit (1 GB)</Label>
            </Field>
            <Field orientation="horizontal">
              <RadioGroupItem value="memory-2gb" id="memory-2gb" />
              <Label htmlFor="memory-2gb">Memory limit (2 GB)</Label>
            </Field>
          </RadioGroup>
        </CustomFieldSet>
      </FieldGroup>
    </div>
  )
}

function CustomFieldSet(props: {
  icon: React.ReactNode
  title: string
  description: string
  children: React.ReactNode
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
