import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import { Label } from "@/components/ui/label"
import { Slider } from "@/components/ui/slider"
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
  type SettingsState,
  type ViewMode
} from "@/store/settingsStore"
import {
  HardDriveIcon,
  ImagesIcon,
  NavigationIcon,
  TimerIcon
} from "lucide-react"
import type { ReactNode } from "react"
import { useShallow } from "zustand/react/shallow"

export function GeneralSettingsPanel() {
  const settings = useSettingsStore(
    useShallow((state) => ({
      loopNavigation: state.loopNavigation,
      cacheMode: state.cacheMode,
      viewMode: state.viewMode,
      slideshowIntervalMs: state.slideshowIntervalMs,
      autoOpenLastFile: state.autoOpenLastFile
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
        icon={<ImagesIcon />}
        title="보기 모드"
        description="단일 / 양면 / 세로 스크롤 표시 방식"
      >
        <RadioGroup
          value={settings.viewMode}
          onValueChange={(value) =>
            handleSettingsChange({ viewMode: value as ViewMode })
          }
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="single" id="settings-view-single" />
            <Label htmlFor="settings-view-single">단일</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="left-to-right" id="settings-view-ltr" />
            <Label htmlFor="settings-view-ltr">좌→우 두 페이지</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="right-to-left" id="settings-view-rtl" />
            <Label htmlFor="settings-view-rtl">우→좌 두 페이지</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="webtoon" id="settings-view-webtoon" />
            <Label htmlFor="settings-view-webtoon">웹툰 스크롤</Label>
          </Field>
        </RadioGroup>
      </CustomFieldSet>

      <FieldSeparator />

      <CustomFieldSet
        icon={<TimerIcon />}
        title="슬라이드쇼 간격"
        description={`자동 넘김 간격 (${(settings.slideshowIntervalMs / 1000).toFixed(1)}초)`}
      >
        <Slider
          aria-label="Slideshow interval"
          value={[settings.slideshowIntervalMs]}
          min={1000}
          max={30000}
          step={500}
          onValueChange={(value) => {
            const next = Array.isArray(value) ? value[0] : (value as number)
            if (typeof next === "number")
              handleSettingsChange({ slideshowIntervalMs: next })
          }}
        />
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
      <FieldSeparator />

      <CustomFieldSet
        icon={<NavigationIcon />}
        title="시작"
        description="앱 실행 시 마지막으로 본 이미지 자동 열기"
      >
        <RadioGroup
          value={settings.autoOpenLastFile ? "on" : "off"}
          onValueChange={(value) =>
            handleSettingsChange({ autoOpenLastFile: value === "on" })
          }
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="off" id="settings-startup-off" />
            <Label htmlFor="settings-startup-off">홈 화면 표시</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="on" id="settings-startup-on" />
            <Label htmlFor="settings-startup-on">마지막 파일 자동 열기</Label>
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
