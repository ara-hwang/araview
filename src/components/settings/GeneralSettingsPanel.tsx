import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import { Label } from "@/components/ui/label"
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLegend,
  FieldSeparator,
  FieldSet
} from "@/components/ui/field"
import {
  updateSettings,
  useSettingsStore,
  type CacheMode,
  type SettingsState
} from "@/store/settingsStore"
import { HardDriveIcon, NavigationIcon } from "lucide-react"
import type { ReactNode } from "react"
import { useShallow } from "zustand/react/shallow"

export function GeneralSettingsPanel() {
  const settings = useSettingsStore(
    useShallow((state) => ({
      loopNavigation: state.loopNavigation,
      cacheMode: state.cacheMode
    }))
  )

  const handleSettingsChange = (next: Partial<SettingsState>) => {
    void updateSettings(next)
  }

  return (
    <FieldGroup>
      <CustomFieldSet
        icon={<NavigationIcon />}
        title="내비게이션"
        description="처음/마지막 이미지에서의 이동 방식"
      >
        <RadioGroup
          value={settings.loopNavigation ? "loop" : "stop"}
          onValueChange={(value) =>
            handleSettingsChange({ loopNavigation: value === "loop" })
          }
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="stop" id="settings-nav-stop" />
            <Label htmlFor="settings-nav-stop">처음/마지막에서 멈춤</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="loop" id="settings-nav-loop" />
            <Label htmlFor="settings-nav-loop">처음/마지막으로 순환</Label>
          </Field>
        </RadioGroup>
      </CustomFieldSet>

      <FieldSeparator />

      <CustomFieldSet
        icon={<HardDriveIcon />}
        title="캐시"
        description="이미지 미리 로드 범위"
      >
        <RadioGroup
          value={settings.cacheMode}
          onValueChange={(value) =>
            handleSettingsChange({ cacheMode: value as CacheMode })
          }
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="off" id="settings-cache-off" />
            <Label htmlFor="settings-cache-off">끄기 (현재 이미지만)</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="nearby" id="settings-cache-nearby" />
            <Label htmlFor="settings-cache-nearby">
              인접 (이전/다음 미리 로드)
            </Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="extended" id="settings-cache-extended" />
            <Label htmlFor="settings-cache-extended">
              확장 (최대 ±3 미리 로드)
            </Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="memory-1gb" id="settings-cache-1gb" />
            <Label htmlFor="settings-cache-1gb">메모리 제한 (1 GB)</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="memory-2gb" id="settings-cache-2gb" />
            <Label htmlFor="settings-cache-2gb">메모리 제한 (2 GB)</Label>
          </Field>
        </RadioGroup>
      </CustomFieldSet>
    </FieldGroup>
  )
}

function CustomFieldSet(props: {
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
