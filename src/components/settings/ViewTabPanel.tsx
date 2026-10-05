import {
  BookOpenText,
  Images,
  ImageSquare,
  ListBullets,
  Palette,
  Presentation,
  PushPin,
  Stack
} from "@phosphor-icons/react"
import { useEffect, useState } from "react"
import { useTranslation } from "react-i18next"
import { useShallow } from "zustand/react/shallow"

import { SettingsFieldSet } from "@/components/settings/SettingsFieldSet"
import { ShortcutBadge } from "@/components/settings/ShortcutBadge"
import { Field, FieldGroup, FieldSeparator } from "@/components/ui/field"
import { Label } from "@/components/ui/label"
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import { Slider } from "@/components/ui/slider"
import { Switch } from "@/components/ui/switch"
import { applyManualViewMode } from "@/store/appStore"
import {
  updateSettings,
  useSettingsStore,
  type DockPosition,
  type DockThumbSize,
  type ImageScalingMode,
  type SettingsState,
  type ViewerBackground,
  type ViewMode
} from "@/store/settingsStore"

const firstSliderValue = (value: number | readonly number[]) =>
  typeof value === "number" ? value : (value[0] ?? 0)

export function ViewTabPanel() {
  const { t } = useTranslation()
  const settings = useSettingsStore(
    useShallow((state) => ({
      viewMode: state.viewMode,
      webtoonImageGap: state.webtoonImageGap,
      webtoonPageBoundaries: state.webtoonPageBoundaries,
      webtoonFitWidth: state.webtoonFitWidth,
      webtoonShowProgress: state.webtoonShowProgress,
      webtoonThumbnailJump: state.webtoonThumbnailJump,
      viewerBackground: state.viewerBackground,
      resumeReading: state.resumeReading,
      showCoverAlone: state.showCoverAlone,
      showWidePageAlone: state.showWidePageAlone,
      showComicInfo: state.showComicInfo,
      comicAutoDualView: state.comicAutoDualView,
      autoHideUI: state.autoHideUI,
      menuBarHidden: state.menuBarHidden,
      alwaysOnTop: state.alwaysOnTop,
      dockPosition: state.dockPosition,
      dockVisible: state.dockVisible,
      dockThumbSize: state.dockThumbSize,
      dockShowName: state.dockShowName,
      dockShowIndex: state.dockShowIndex,
      imageScalingMode: state.imageScalingMode,
      autoDetectPixelArt: state.autoDetectPixelArt
    }))
  )
  const [webtoonImageGap, setWebtoonImageGap] = useState(settings.webtoonImageGap)

  useEffect(() => {
    setWebtoonImageGap(settings.webtoonImageGap)
  }, [settings.webtoonImageGap])

  const handleSettingsChange = (next: Partial<SettingsState>) => {
    void updateSettings(next)
  }

  return (
    <FieldGroup>
      <SettingsFieldSet
        icon={<Images className="size-6" />}
        title={t("settings.view.title")}
        description={t("settings.view.desc")}
      >
        <RadioGroup
          value={settings.viewMode}
          onValueChange={(value) => applyManualViewMode(value as ViewMode)}
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="single" id="settings-view-single" />
            <Label htmlFor="settings-view-single">{t("settings.view.single")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="left-to-right" id="settings-view-ltr" />
            <Label htmlFor="settings-view-ltr">{t("settings.view.ltr")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="right-to-left" id="settings-view-rtl" />
            <Label htmlFor="settings-view-rtl">{t("settings.view.rtl")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="webtoon" id="settings-view-webtoon" />
            <Label htmlFor="settings-view-webtoon">{t("settings.view.webtoon")}</Label>
          </Field>
        </RadioGroup>
      </SettingsFieldSet>

      <FieldSeparator />

      <SettingsFieldSet
        icon={<ImageSquare className="size-6" />}
        title={t("settings.imageRendering.title")}
        description={t("settings.imageRendering.desc")}
      >
        <RadioGroup
          value={settings.imageScalingMode}
          onValueChange={(value) =>
            handleSettingsChange({ imageScalingMode: value as ImageScalingMode })
          }
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="auto" id="settings-image-rendering-auto" />
            <Label htmlFor="settings-image-rendering-auto">
              {t("settings.imageRendering.auto")}
            </Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="smooth" id="settings-image-rendering-smooth" />
            <Label htmlFor="settings-image-rendering-smooth">
              {t("settings.imageRendering.smooth")}
            </Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="pixelated" id="settings-image-rendering-pixelated" />
            <Label htmlFor="settings-image-rendering-pixelated">
              {t("settings.imageRendering.pixelated")}
            </Label>
          </Field>
        </RadioGroup>
        <Field
          orientation="horizontal"
          data-disabled={settings.imageScalingMode !== "auto" ? true : undefined}
        >
          <Switch
            id="settings-image-rendering-auto-detect"
            checked={settings.autoDetectPixelArt}
            disabled={settings.imageScalingMode !== "auto"}
            onCheckedChange={(checked) =>
              handleSettingsChange({ autoDetectPixelArt: checked === true })
            }
          />
          <Label htmlFor="settings-image-rendering-auto-detect">
            {t("settings.imageRendering.autoDetect")}
          </Label>
        </Field>
      </SettingsFieldSet>

      <FieldSeparator />

      <SettingsFieldSet
        icon={<Stack className="size-6" />}
        title={t("settings.webtoon.title")}
        description={t("settings.webtoon.desc")}
      >
        <Field>
          <div className="flex items-center justify-between gap-4">
            <Label id="settings-webtoon-gap-label">{t("settings.webtoon.imageGap")}</Label>
            <span className="text-xs font-medium text-muted-foreground tabular-nums">
              {webtoonImageGap} px
            </span>
          </div>
          <Slider
            id="settings-webtoon-gap"
            aria-labelledby="settings-webtoon-gap-label"
            value={[webtoonImageGap]}
            min={0}
            max={64}
            step={2}
            onValueChange={(value) => setWebtoonImageGap(firstSliderValue(value))}
            onValueCommitted={(value) =>
              handleSettingsChange({ webtoonImageGap: firstSliderValue(value) })
            }
          />
        </Field>
        <Field orientation="horizontal">
          <Switch
            id="settings-webtoon-boundaries"
            checked={settings.webtoonPageBoundaries}
            onCheckedChange={(checked) =>
              handleSettingsChange({ webtoonPageBoundaries: checked === true })
            }
          />
          <Label htmlFor="settings-webtoon-boundaries">
            {t("settings.webtoon.pageBoundaries")}
          </Label>
        </Field>
        <Field orientation="horizontal">
          <Switch
            id="settings-webtoon-fit-width"
            checked={settings.webtoonFitWidth}
            onCheckedChange={(checked) =>
              handleSettingsChange({ webtoonFitWidth: checked === true })
            }
          />
          <Label htmlFor="settings-webtoon-fit-width">{t("settings.webtoon.fitWidth")}</Label>
        </Field>
        <Field orientation="horizontal">
          <Switch
            id="settings-webtoon-show-progress"
            checked={settings.webtoonShowProgress}
            onCheckedChange={(checked) =>
              handleSettingsChange({ webtoonShowProgress: checked === true })
            }
          />
          <Label htmlFor="settings-webtoon-show-progress">
            {t("settings.webtoon.showProgress")}
          </Label>
        </Field>
        <Field orientation="horizontal">
          <Switch
            id="settings-webtoon-thumbnail-jump"
            checked={settings.webtoonThumbnailJump}
            onCheckedChange={(checked) =>
              handleSettingsChange({ webtoonThumbnailJump: checked === true })
            }
          />
          <Label htmlFor="settings-webtoon-thumbnail-jump">
            {t("settings.webtoon.thumbnailJump")}
          </Label>
        </Field>
      </SettingsFieldSet>

      <FieldSeparator />

      <SettingsFieldSet
        icon={<BookOpenText className="size-6" />}
        title={t("settings.reading.title")}
        description={t("settings.reading.desc")}
      >
        <Field orientation="horizontal">
          <Switch
            id="settings-reading-resume"
            checked={settings.resumeReading}
            onCheckedChange={(checked) => handleSettingsChange({ resumeReading: checked === true })}
          />
          <Label htmlFor="settings-reading-resume">{t("settings.reading.resume")}</Label>
        </Field>
        <Field orientation="horizontal">
          <Switch
            id="settings-reading-comic-auto-dual"
            checked={settings.comicAutoDualView}
            onCheckedChange={(checked) =>
              handleSettingsChange({ comicAutoDualView: checked === true })
            }
          />
          <Label htmlFor="settings-reading-comic-auto-dual">
            {t("settings.reading.comicAutoDual")}
          </Label>
        </Field>
        <Field orientation="horizontal">
          <Switch
            id="settings-reading-cover-alone"
            checked={settings.showCoverAlone}
            onCheckedChange={(checked) =>
              handleSettingsChange({ showCoverAlone: checked === true })
            }
          />{" "}
          <Label htmlFor="settings-reading-cover-alone">{t("settings.reading.coverAlone")}</Label>
        </Field>
        <Field orientation="horizontal">
          <Switch
            id="settings-reading-wide-alone"
            checked={settings.showWidePageAlone}
            onCheckedChange={(checked) =>
              handleSettingsChange({ showWidePageAlone: checked === true })
            }
          />
          <Label htmlFor="settings-reading-wide-alone">{t("settings.reading.wideAlone")}</Label>
        </Field>
        <Field orientation="horizontal">
          <Switch
            id="settings-reading-comic-info"
            checked={settings.showComicInfo}
            onCheckedChange={(checked) => handleSettingsChange({ showComicInfo: checked === true })}
          />
          <Label htmlFor="settings-reading-comic-info">{t("settings.reading.showComicInfo")}</Label>
        </Field>
      </SettingsFieldSet>

      <FieldSeparator />

      <SettingsFieldSet
        icon={<Palette className="size-6" />}
        title={t("settings.bg.title")}
        description={t("settings.bg.desc")}
        shortcutId="cycleBackground"
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
            <Label htmlFor="settings-bg-theme">{t("settings.bg.theme")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="black" id="settings-bg-black" />
            <Label htmlFor="settings-bg-black">{t("settings.bg.black")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="white" id="settings-bg-white" />
            <Label htmlFor="settings-bg-white">{t("settings.bg.white")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="checker" id="settings-bg-checker" />
            <Label htmlFor="settings-bg-checker">{t("settings.bg.checker")}</Label>
          </Field>
        </RadioGroup>
      </SettingsFieldSet>

      <FieldSeparator />

      <SettingsFieldSet
        icon={<ListBullets className="size-6" />}
        title={t("settings.dock.title")}
        description={t("settings.dock.desc")}
      >
        <Field orientation="horizontal">
          <Switch
            id="settings-dock-visible"
            checked={settings.dockVisible}
            onCheckedChange={(checked) => handleSettingsChange({ dockVisible: checked === true })}
          />
          <Label htmlFor="settings-dock-visible">{t("settings.dock.visible")}</Label>
        </Field>
        <RadioGroup
          value={settings.dockPosition}
          onValueChange={(value) => handleSettingsChange({ dockPosition: value as DockPosition })}
          aria-label={t("settings.dock.position")}
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="top" id="settings-dock-top" />
            <Label htmlFor="settings-dock-top">{t("settings.dock.top")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="bottom" id="settings-dock-bottom" />
            <Label htmlFor="settings-dock-bottom">{t("settings.dock.bottom")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="left" id="settings-dock-left" />
            <Label htmlFor="settings-dock-left">{t("settings.dock.left")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="right" id="settings-dock-right" />
            <Label htmlFor="settings-dock-right">{t("settings.dock.right")}</Label>
          </Field>
        </RadioGroup>
        <RadioGroup
          value={settings.dockThumbSize}
          onValueChange={(value) => handleSettingsChange({ dockThumbSize: value as DockThumbSize })}
          aria-label={t("settings.dock.thumbSize")}
        >
          <Field orientation="horizontal">
            <RadioGroupItem value="s" id="settings-dock-s" />
            <Label htmlFor="settings-dock-s">{t("settings.dock.thumbS")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="m" id="settings-dock-m" />
            <Label htmlFor="settings-dock-m">{t("settings.dock.thumbM")}</Label>
          </Field>
          <Field orientation="horizontal">
            <RadioGroupItem value="l" id="settings-dock-l" />
            <Label htmlFor="settings-dock-l">{t("settings.dock.thumbL")}</Label>
          </Field>
        </RadioGroup>
        <Field orientation="horizontal">
          <Switch
            id="settings-dock-showname"
            checked={settings.dockShowName}
            onCheckedChange={(checked) => handleSettingsChange({ dockShowName: checked === true })}
          />
          <Label htmlFor="settings-dock-showname">{t("settings.dock.showName")}</Label>
        </Field>
        <Field orientation="horizontal">
          <Switch
            id="settings-dock-showindex"
            checked={settings.dockShowIndex}
            onCheckedChange={(checked) => handleSettingsChange({ dockShowIndex: checked === true })}
          />
          <Label htmlFor="settings-dock-showindex">{t("settings.dock.showIndex")}</Label>
        </Field>
      </SettingsFieldSet>

      <FieldSeparator />

      <SettingsFieldSet
        icon={<Presentation className="size-6" />}
        title={t("settings.present.title")}
        description={t("settings.present.desc")}
      >
        <Field orientation="horizontal">
          <Switch
            id="settings-autohide"
            checked={settings.autoHideUI}
            onCheckedChange={(checked) => handleSettingsChange({ autoHideUI: checked === true })}
          />
          <Label htmlFor="settings-autohide">{t("settings.present.autohide")}</Label>
        </Field>
        <Field orientation="horizontal">
          <Switch
            id="settings-hide-menubar"
            checked={settings.menuBarHidden}
            onCheckedChange={(checked) => handleSettingsChange({ menuBarHidden: checked === true })}
          />
          <Label htmlFor="settings-hide-menubar">{t("settings.present.hideMenuBar")}</Label>
        </Field>
      </SettingsFieldSet>

      <FieldSeparator />

      <SettingsFieldSet
        icon={<PushPin className="size-6" />}
        title={t("settings.window.title")}
        description={t("settings.window.desc")}
      >
        <Field orientation="horizontal">
          <Switch
            id="settings-always-on-top"
            checked={settings.alwaysOnTop}
            onCheckedChange={(checked) => handleSettingsChange({ alwaysOnTop: checked === true })}
          />
          <Label htmlFor="settings-always-on-top">
            {t("settings.window.alwaysOnTop")}
            <ShortcutBadge actionId="toggleAlwaysOnTop" />
          </Label>
        </Field>
      </SettingsFieldSet>
    </FieldGroup>
  )
}
