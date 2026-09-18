import {
  ArrowsHorizontal,
  ArrowsOut,
  ArrowsVertical,
  ArrowClockwise,
  ArrowCounterClockwise,
  CaretDown,
  CaretUp,
  Command,
  Copy,
  FlipHorizontal,
  FlipVertical,
  FolderOpen,
  Gear,
  House,
  Info,
  MagnifyingGlassMinus,
  MagnifyingGlassPlus,
  Minus,
  PushPin,
  Square,
  X
} from "@phosphor-icons/react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { useEffect, useState } from "react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { formatShortcutDisplay } from "@/constants/shortcuts"
import { useAlwaysOnTop } from "@/hooks/useAlwaysOnTop"
import { useCloseImage } from "@/hooks/useCloseImage"
import { requestOpenSettings, usePaletteStore } from "@/hooks/useCommandPalette"
import { useExifLoader } from "@/hooks/useExifLoader"
import { useImageLoader } from "@/hooks/useImageLoader"
import { cn } from "@/lib/utils"
import {
  flipHorizontal,
  flipVertical,
  resetZoomPan,
  rotateCCW,
  rotateCW,
  setZoomToFit,
  zoomIn,
  zoomOut,
  useAppStore
} from "@/store/appStore"
import { updateSettings, useSettingsStore } from "@/store/settingsStore"

import { ButtonGroup } from "./ui/button-group"
import { ButtonGroupText } from "./ui/button-group"
import { Toggle } from "./ui/toggle"
import { ToggleGroup, ToggleGroupItem } from "./ui/toggle-group"

// 윈도우 캡션 버튼: 네이티브처럼 타이틀바 높이를 꽉 채우고 모서리에 붙인다.
// 드래그 영역 안에 있으므로 no-drag로 제외한다 (더블클릭 최대화 방지).
const captionButtonClassName = cn(
  "flex w-12 shrink-0 items-center justify-center text-foreground transition-colors no-drag [&_svg]:pointer-events-none [&_svg]:size-4",
  "hover:bg-foreground/10 active:bg-foreground/20 dark:hover:bg-white/15 dark:active:bg-white/25",
  "focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-ring"
)

const captionCloseButtonClassName = cn(
  captionButtonClassName,
  "hover:bg-destructive hover:text-white active:bg-destructive"
)

// 위쪽 툴바
export default function Header({
  onHideMenuBar
}: {
  onHideMenuBar?: () => void
} = {}) {
  const { t } = useTranslation()
  const [appWindow] = useState(() => getCurrentWindow())
  const zoom = useAppStore((state) => state.zoom)
  const hasImage = useAppStore((state) => state.imageInfo !== null)
  const shortcuts = useSettingsStore((state) => state.shortcuts)
  const menuBarHidden = useSettingsStore((state) => state.menuBarHidden)
  const fitMode = useSettingsStore((state) => state.fitMode)
  const withShortcut = (label: string, binding: string) =>
    binding ? `${label} (${formatShortcutDisplay(binding)})` : label
  // ToggleGroup(single)은 값 배열로 동작한다. auto는 선택 없음(빈 배열)으로 둔다.
  const fitValue = fitMode === "auto" ? [] : [fitMode]
  const handleFitChange = (value: string[]) => {
    const next = value[0]
    if (next === "width" || next === "height" || next === "screen") {
      setZoomToFit(next)
    }
  }
  const [isMaximized, setIsMaximized] = useState(false)

  const { handleOpenFile } = useImageLoader()
  const handleGoHome = useCloseImage()
  const { toggleExifPanel } = useExifLoader()
  const { alwaysOnTop, toggle: toggleAlwaysOnTop } = useAlwaysOnTop()

  useEffect(() => {
    let cancelled = false
    let unlisten: (() => void) | undefined

    const syncMaximized = () => {
      void appWindow
        .isMaximized()
        .then((value) => {
          if (!cancelled) setIsMaximized(value)
        })
        .catch(() => {})
    }

    syncMaximized()
    void appWindow
      .onResized(syncMaximized)
      .then((fn) => {
        unlisten = fn
      })
      .catch(() => {})

    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [appWindow])

  const handleMinimize = () => {
    void appWindow.minimize()
  }

  const handleMaximize = () => {
    void (async () => {
      try {
        await appWindow.toggleMaximize()
        setIsMaximized(await appWindow.isMaximized())
      } catch {
        // jsdom/테스트 환경에서는 Tauri 창 API가 없으므로 무시
      }
    })()
  }

  const handleClose = () => {
    void appWindow.close()
  }

  const handleHideMenuBar = () => {
    if (menuBarHidden) {
      // 오버레이로 peek 중인 상태에서는 다시 누르면 영구 복구한다
      void updateSettings({ menuBarHidden: false })
      return
    }
    void updateSettings({ menuBarHidden: true })
    onHideMenuBar?.()
  }

  return (
    <div data-tauri-drag-region className="flex items-stretch border-b bg-background">
      {/* 좁은 창에서도 캡션 버튼 위로 내용이 겹치지 않도록 클립한다 */}
      <div
        data-tauri-drag-region
        className="flex min-w-0 flex-1 items-center justify-between gap-2 overflow-hidden p-2"
      >
        <ButtonGroup aria-label={t("palette.group.file")} className="no-drag">
          <Button
            variant="outline"
            onClick={handleGoHome}
            disabled={!hasImage}
            title={withShortcut(t("header.home"), shortcuts.closeImage)}
            aria-label={withShortcut(t("header.home"), shortcuts.closeImage)}
          >
            <House data-icon="inline-start" />
            <span className="hidden min-[1440px]:inline">{t("header.home")}</span>
          </Button>
          <Button
            variant="outline"
            onClick={handleOpenFile}
            title={withShortcut(t("header.open"), shortcuts.openFile)}
            aria-label={withShortcut(t("header.open"), shortcuts.openFile)}
          >
            <FolderOpen data-icon="inline-start" />
            <span className="hidden min-[1440px]:inline">{t("header.open")}</span>
          </Button>
        </ButtonGroup>

        {/* ButtonGroup은 동작 버튼용, ToggleGroup은 상태 토글용으로 구분한다 (shadcn).
            둘을 섞으므로 바깥은 gap flex div로 두고, 연결된 둥근 모서리 병합이 섞이지 않게 한다. */}
        <div className="flex min-w-0 items-center gap-2 overflow-hidden no-drag">
          <div className="flex min-w-0 items-center gap-2">
            <ToggleGroup
              variant="outline"
              spacing={0}
              value={fitValue}
              onValueChange={handleFitChange}
              disabled={!hasImage}
              aria-label={t("header.fitGroup")}
              className="no-drag"
            >
              <ToggleGroupItem
                value="width"
                disabled={!hasImage}
                title={withShortcut(t("header.fitWidth"), shortcuts.fitWidth)}
                aria-label={withShortcut(t("header.fitWidth"), shortcuts.fitWidth)}
              >
                <ArrowsHorizontal data-icon="inline-start" />
                <span className="hidden min-[1440px]:inline">{t("header.fitWidth")}</span>
              </ToggleGroupItem>
              <ToggleGroupItem
                value="height"
                disabled={!hasImage}
                title={withShortcut(t("header.fitHeight"), shortcuts.fitHeight)}
                aria-label={withShortcut(t("header.fitHeight"), shortcuts.fitHeight)}
              >
                <ArrowsVertical data-icon="inline-start" />
                <span className="hidden min-[1440px]:inline">{t("header.fitHeight")}</span>
              </ToggleGroupItem>
              <ToggleGroupItem
                value="screen"
                disabled={!hasImage}
                title={withShortcut(t("header.fitScreen"), shortcuts.fitScreen)}
                aria-label={withShortcut(t("header.fitScreen"), shortcuts.fitScreen)}
              >
                <ArrowsOut data-icon="inline-start" />
                <span className="hidden min-[1440px]:inline">{t("header.fitScreen")}</span>
              </ToggleGroupItem>
            </ToggleGroup>

            <ButtonGroup aria-label={t("header.zoomGroup")} className="no-drag">
              <Button
                variant="outline"
                onClick={zoomOut}
                disabled={!hasImage}
                title={withShortcut(t("header.zoomOut"), shortcuts.zoomOut)}
                aria-label={withShortcut(t("header.zoomOut"), shortcuts.zoomOut)}
              >
                <MagnifyingGlassMinus data-icon="inline-start" />
                <span className="hidden min-[1440px]:inline">{t("header.zoomOut")}</span>
              </Button>
              <ButtonGroupText
                className={cn(
                  "hidden tabular-nums no-drag focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none sm:flex",
                  !hasImage && "opacity-50"
                )}
                render={
                  <button
                    type="button"
                    disabled={!hasImage}
                    onClick={hasImage ? resetZoomPan : undefined}
                    title={withShortcut(t("menu.actualSize"), shortcuts.resetView)}
                    aria-label={withShortcut(t("menu.actualSize"), shortcuts.resetView)}
                  />
                }
              >
                {Math.round(zoom * 100)}%
              </ButtonGroupText>
              <Button
                variant="outline"
                onClick={zoomIn}
                disabled={!hasImage}
                title={withShortcut(t("header.zoomIn"), shortcuts.zoomIn)}
                aria-label={withShortcut(t("header.zoomIn"), shortcuts.zoomIn)}
              >
                <MagnifyingGlassPlus data-icon="inline-start" />
                <span className="hidden min-[1440px]:inline">{t("header.zoomIn")}</span>
              </Button>
            </ButtonGroup>

            <ButtonGroup
              aria-label={t("header.transformGroup")}
              className="hidden no-drag min-[840px]:flex"
            >
              <Button
                variant="outline"
                size="icon"
                onClick={rotateCCW}
                disabled={!hasImage}
                title={withShortcut(t("menu.rotateCcw"), shortcuts.rotateCCW)}
                aria-label={withShortcut(t("menu.rotateCcw"), shortcuts.rotateCCW)}
              >
                <ArrowCounterClockwise />
              </Button>
              <Button
                variant="outline"
                size="icon"
                onClick={rotateCW}
                disabled={!hasImage}
                title={withShortcut(t("menu.rotateCw"), shortcuts.rotateCW)}
                aria-label={withShortcut(t("menu.rotateCw"), shortcuts.rotateCW)}
              >
                <ArrowClockwise />
              </Button>
              <Button
                variant="outline"
                size="icon"
                onClick={flipHorizontal}
                disabled={!hasImage}
                title={withShortcut(t("menu.flipH"), shortcuts.flipH)}
                aria-label={withShortcut(t("menu.flipH"), shortcuts.flipH)}
              >
                <FlipHorizontal />
              </Button>
              <Button
                variant="outline"
                size="icon"
                onClick={flipVertical}
                disabled={!hasImage}
                title={withShortcut(t("menu.flipV"), shortcuts.flipV)}
                aria-label={withShortcut(t("menu.flipV"), shortcuts.flipV)}
              >
                <FlipVertical />
              </Button>
            </ButtonGroup>
          </div>

          {/* 이미지 종속(Info)은 캔버스 조작 쪽에, 전역(Command/Settings)은 우측 유틸군에 둔다 */}
          <ButtonGroup className="no-drag">
            <Button
              variant="outline"
              onClick={() => void toggleExifPanel()}
              disabled={!hasImage}
              title={withShortcut(t("header.info"), shortcuts.toggleExif)}
              aria-label={withShortcut(t("header.info"), shortcuts.toggleExif)}
            >
              <Info data-icon="inline-start" />
              <span className="hidden min-[1440px]:inline">{t("header.info")}</span>
            </Button>
          </ButtonGroup>

          <ButtonGroup aria-label={t("palette.group.system")} className="no-drag">
            <Button
              variant="outline"
              onClick={() => usePaletteStore.getState().setOpen(true)}
              title={withShortcut(t("palette.open"), shortcuts.togglePalette)}
              aria-label={withShortcut(t("palette.open"), shortcuts.togglePalette)}
            >
              <Command data-icon="inline-start" />
              <span className="hidden min-[1440px]:inline">{t("palette.open")}</span>
            </Button>
            <Button
              variant="outline"
              onClick={() => requestOpenSettings()}
              title={t("header.settingsTitle")}
              aria-label={t("header.settingsTitle")}
            >
              <Gear data-icon="inline-start" />
              <span className="hidden min-[1440px]:inline">{t("header.settings")}</span>
            </Button>
          </ButtonGroup>

          <Toggle
            variant="outline"
            pressed={alwaysOnTop}
            onPressedChange={() => void toggleAlwaysOnTop()}
            title={withShortcut(t("header.alwaysOnTop"), shortcuts.toggleAlwaysOnTop)}
            aria-label={withShortcut(t("header.alwaysOnTop"), shortcuts.toggleAlwaysOnTop)}
            className="hidden no-drag md:inline-flex"
          >
            <PushPin weight={alwaysOnTop ? "fill" : "regular"} />
          </Toggle>
          <ButtonGroup className="no-drag">
            <Button
              variant="outline"
              size="icon"
              onClick={handleHideMenuBar}
              title={t(menuBarHidden ? "header.showMenuBar" : "header.hideMenuBar")}
              aria-label={t(menuBarHidden ? "header.showMenuBar" : "header.hideMenuBar")}
            >
              {menuBarHidden ? <CaretDown /> : <CaretUp />}
            </Button>
          </ButtonGroup>
        </div>
      </div>

      {/* 윈도우 캡션 버튼: 타이틀바 우측 끝을 꽉 채운다. 드래그/더블클릭 최대화 제외. */}
      <div className="flex shrink-0 items-stretch no-drag">
        <button
          type="button"
          onClick={handleMinimize}
          title={t("header.minimize")}
          aria-label={t("header.minimize")}
          className={captionButtonClassName}
        >
          <Minus />
        </button>
        <button
          type="button"
          onClick={handleMaximize}
          title={t("header.maximize")}
          aria-label={t("header.maximize")}
          aria-pressed={isMaximized}
          className={captionButtonClassName}
        >
          {isMaximized ? <Copy /> : <Square />}
        </button>
        <button
          type="button"
          onClick={handleClose}
          title={t("header.close")}
          aria-label={t("header.close")}
          className={captionCloseButtonClassName}
        >
          <X />
        </button>
      </div>
    </div>
  )
}
