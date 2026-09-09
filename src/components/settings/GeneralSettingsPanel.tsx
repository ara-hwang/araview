import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import { Label } from "@/components/ui/label"
import { Slider } from "@/components/ui/slider"
import { Switch } from "@/components/ui/switch"
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
  type DirSortKey,
  type SettingsState,
  type ViewerBackground,
  type ViewMode
} from "@/store/settingsStore"
import { applySortSettings } from "@/utils/directoryOptions"
import {
  ArrowUpDown,
  HardDriveIcon,
  ImagesIcon,
  NavigationIcon,
  PaletteIcon,
  PresentationIcon,
  Shuffle,
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
      autoOpenLastFile: state.autoOpenLastFile,
      viewerBackground: state.viewerBackground,
      autoHideUI: state.autoHideUI,
      sortKey: state.sortKey,
      sortDescending: state.sortDescending,
      shuffle: state.shuffle,
      includeSubfolders: state.includeSubfolders
    }))
  )

  const handleSettingsChange = (next: Partial<SettingsState>) => {
    void updateSettings(next)
  }

  const handleSortChange = (
    next: Partial<
      Pick<
        SettingsState,
        "sortKey" | "sortDescending" | "shuffle" | "includeSubfolders"
      >
    >
  ) => {
    void applySortSettings(next)
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
        icon={<ArrowUpDown />}
        title="정렬"
        description="폴더 내 이미지 순서 (S 키로 셔플 토글)"
      >
        <RadioGroup
          value={settings.shuffle ? "shuffle" : settings.sortKey}
          onValueChange={(value) => {
            if (value === "shuffle") {
              handleSortChange({ shuffle: true })
            } else {
              handleSortChange({
                shuffle: false,
                sortKey: value as DirSortKey
              })
            }
          }}
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="name" id="settings-sort-name" />
            <Label htmlFor="settings-sort-name">이름</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="date" id="settings-sort-date" />
            <Label htmlFor="settings-sort-date">수정한 날짜</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="size" id="settings-sort-size" />
            <Label htmlFor="settings-sort-size">파일 크기</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="shuffle" id="settings-sort-shuffle" />
            <Label htmlFor="settings-sort-shuffle">
              <span className="flex items-center gap-1">
                <Shuffle className="size-3.5" /> 셔플
              </span>
            </Label>
          </Field>
        </RadioGroup>
        <RadioGroup
          value={settings.sortDescending ? "desc" : "asc"}
          onValueChange={(value) =>
            handleSortChange({ sortDescending: value === "desc" })
          }
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="asc" id="settings-sort-asc" />
            <Label htmlFor="settings-sort-asc">오름차순</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="desc" id="settings-sort-desc" />
            <Label htmlFor="settings-sort-desc">내림차순</Label>
          </Field>
        </RadioGroup>
        <Field orientation="horizontal">
          <Switch
            id="settings-subfolders"
            checked={settings.includeSubfolders}
            onCheckedChange={(checked) =>
              handleSortChange({ includeSubfolders: checked === true })
            }
          />
          <Label htmlFor="settings-subfolders">하위 폴더 포함</Label>
        </Field>
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
        icon={<PaletteIcon />}
        title="배경"
        description="이미지 영역 배경 (B 키로 순환)"
      >
        <RadioGroup
          value={settings.viewerBackground}
          onValueChange={(value) =>
            handleSettingsChange({
              viewerBackground: value as ViewerBackground
            })
          }
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="theme" id="settings-bg-theme" />
            <Label htmlFor="settings-bg-theme">테마 기본값</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="black" id="settings-bg-black" />
            <Label htmlFor="settings-bg-black">검정</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="white" id="settings-bg-white" />
            <Label htmlFor="settings-bg-white">흰색</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="checker" id="settings-bg-checker" />
            <Label htmlFor="settings-bg-checker">
              체커보드 (투명 이미지 확인용)
            </Label>
          </Field>
        </RadioGroup>
      </CustomFieldSet>

      <FieldSeparator />

      <CustomFieldSet
        icon={<PresentationIcon />}
        title="프레젠테이션"
        description="3초 무입력 시 상단바/하단바 자동 숨김 (움직이면 복귀)"
      >
        <Field orientation="horizontal">
          <Switch
            id="settings-autohide"
            checked={settings.autoHideUI}
            onCheckedChange={(checked) =>
              handleSettingsChange({ autoHideUI: checked === true })
            }
          />
          <Label htmlFor="settings-autohide">UI 자동 숨김</Label>
        </Field>
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
