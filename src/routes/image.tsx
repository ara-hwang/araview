import { CaretDown, CaretLeft, CaretRight, CaretUp } from "@phosphor-icons/react"
import { createFileRoute, redirect } from "@tanstack/react-router"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import { ImageContainer } from "@/components/ImageContainer"
import { ImageNavBar } from "@/components/ImageNavBar"
import { RenameDialog } from "@/components/RenameDialog"
import { ThumbnailGrid } from "@/components/ThumbnailGrid"
import { Button } from "@/components/ui/button"
import type { WebtoonScrollTarget } from "@/components/WebtoonContinuousView"
import type { MouseAction } from "@/constants/shortcuts"
import { useAlwaysOnTop } from "@/hooks/useAlwaysOnTop"
import { useCloseImage } from "@/hooks/useCloseImage"
import { registerPaletteHandlers, unregisterPaletteHandlers } from "@/hooks/useCommandPalette"
import { useImageViewerContextMenu } from "@/hooks/useContextMenu"
import { useCopyImage } from "@/hooks/useCopyImage"
import { useDirectoryNavigation } from "@/hooks/useDirectoryNavigation"
import { useExifLoader } from "@/hooks/useExifLoader"
import { useFileOperations } from "@/hooks/useFileOperations"
import { useFullscreen } from "@/hooks/useFullscreen"
import { useIdleHide } from "@/hooks/useIdleHide"
import { useImageLoader } from "@/hooks/useImageLoader"
import { useImageViewerHotkeys } from "@/hooks/useImageViewerHotkeys"
import { useMultiPageImages } from "@/hooks/useMultiPageImages"
import { useOpenFileListener } from "@/hooks/useOpenFileListener"
import { useViewerElements } from "@/hooks/useViewerElements"
import { useWheelNavigation } from "@/hooks/useWheelNavigation"
import { useZoomPan } from "@/hooks/useZoomPan"
import type { ViewerActionHandlers } from "@/hooks/viewerActions"
import { cn } from "@/lib/utils"
import { getApp, updateDirImagesIndex, useAppStore, zoomIn, zoomOut } from "@/store/appStore"
import { useArchiveProgressStore } from "@/store/archiveProgressStore"
import { getSettings, updateSettings, useSettingsStore } from "@/store/settingsStore"

export const Route = createFileRoute("/image")({
  beforeLoad: () => {
    if (!getApp().imageInfo) {
      throw redirect({ to: "/" })
    }
  },
  component: ImagePage
})

function ImagePage() {
  const { t } = useTranslation()
  const dirImages = useAppStore((state) => state.dirImages)
  const archivePath = useAppStore((state) => state.archivePath)
  const failedPaths = useAppStore((state) => state.failedPaths)

  const { containerRef, imageRef } = useViewerElements()

  const zoomPan = useZoomPan()

  const {
    loadImage,
    loadArchiveImageByIndex,
    handleOpenFile,
    handleDrop,
    handleDragOver,
    handleDragEnter,
    handleDragLeave,
    isDragOver,
    getOrLoadImage,
    openArchiveFromPreview,
    prefetchAround
  } = useImageLoader()

  const { viewMode, pages } = useMultiPageImages(getOrLoadImage)
  const { toggleExifPanel } = useExifLoader()

  // 이미지 로드 후 줌/팬 초기화
  const loadImageAndReset = useCallback(
    (filePath: string, options?: { refreshDirectory?: boolean }) =>
      loadImage(filePath, { ...options, onAfterLoad: zoomPan.resetView }),
    [loadImage, zoomPan.resetView]
  )

  /** 파일 연동·드롭·홈에서 직접 열 때는 만화 모드로 진입 */
  const loadImageExplicit = useCallback(
    (filePath: string) =>
      loadImage(filePath, { archiveOpen: "full", onAfterLoad: zoomPan.resetView }),
    [loadImage, zoomPan.resetView]
  )

  const handleOpenArchiveFromPreview = useCallback(() => {
    void openArchiveFromPreview({ onAfterLoad: zoomPan.resetView })
  }, [openArchiveFromPreview, zoomPan.resetView])

  const { navigateImage, navigateToIndex, navigateByOffset } = useDirectoryNavigation(
    loadImageAndReset,
    loadArchiveImageByIndex
  )

  const [webtoonScrollTarget, setWebtoonScrollTarget] = useState<WebtoonScrollTarget>(null)
  const [gridOpen, setGridOpen] = useState(false)

  // Webtoon 연속 스크롤: 중앙 이미지 변경을 가벼운 인덱스 동기화로 처리.
  // 전체 reload 없이 imageInfo만 맞춘다.
  const handleWebtoonIndexChange = useCallback(
    (index: number) => {
      const st = useAppStore.getState()
      if (st.dirImages.current_index === index) return
      const path = st.dirImages.images[index]
      if (!path) return
      updateDirImagesIndex(index)
      if (st.archivePath) {
        void useArchiveProgressStore.getState().save(st.archivePath, path, {
          index,
          total: st.dirImages.images.length
        })
      }
      void getOrLoadImage(path)
        .then((info) => {
          const cur = useAppStore.getState()
          if (cur.dirImages.images[cur.dirImages.current_index] === path) {
            useAppStore.setState({ imageInfo: info, error: null, errorCode: null })
            cur.removeFailedPath(path)
          }
        })
        .catch(() => {})
      // 주변 예열은 일반 이동과 같은 규칙(캐시 모드 거리, 아카이브 선추출)을 따른다.
      prefetchAround(index)
    },
    [getOrLoadImage, prefetchAround]
  )

  const scrollWebtoonTo = useCallback(
    (index: number) => {
      const st = useAppStore.getState()
      const clamped = Math.max(0, Math.min(index, st.dirImages.images.length - 1))
      setWebtoonScrollTarget({ index: clamped, nonce: Date.now() })
      handleWebtoonIndexChange(clamped)
    },
    [handleWebtoonIndexChange]
  )

  const isWebtoon = viewMode === "webtoon"

  const handleNavigateImage = useCallback(
    (direction: "prev" | "next") => {
      if (isWebtoon) {
        const cur = useAppStore.getState().dirImages.current_index
        scrollWebtoonTo(direction === "prev" ? cur - 1 : cur + 1)
        return
      }
      void navigateImage(direction)
    },
    [isWebtoon, navigateImage, scrollWebtoonTo]
  )

  const handleNavigateToIndex = useCallback(
    (index: number) => {
      if (isWebtoon) {
        scrollWebtoonTo(index)
        return
      }
      void navigateToIndex(index)
    },
    [isWebtoon, navigateToIndex, scrollWebtoonTo]
  )

  const handleNavigateByOffset = useCallback(
    (offset: number) => {
      if (isWebtoon) {
        const cur = useAppStore.getState().dirImages.current_index
        scrollWebtoonTo(cur + offset)
        return
      }
      void navigateByOffset(offset)
    },
    [isWebtoon, navigateByOffset, scrollWebtoonTo]
  )

  const handleRetry = useCallback(() => {
    const st = useAppStore.getState()
    const current = st.dirImages.images[st.dirImages.current_index]
    if (!current) return
    if (st.archivePath) {
      void loadArchiveImageByIndex(st.archivePath, current)
      return
    }
    if (st.archivePreviewPath) {
      void loadImageAndReset(st.archivePreviewPath, { refreshDirectory: false })
      return
    }
    void loadImageAndReset(current, { refreshDirectory: false })
  }, [loadArchiveImageByIndex, loadImageAndReset])

  const handleWheel = useWheelNavigation(navigateImage)

  useOpenFileListener(loadImageExplicit)

  const { toggle: toggleFullscreen } = useFullscreen()

  const gridReturnFocusRef = useRef<HTMLElement | null>(null)
  const handleGridClose = useCallback(() => {
    const returnFocus = gridReturnFocusRef.current
    gridReturnFocusRef.current = null
    setGridOpen(false)
    // 그리드를 열기 전의 컨트롤로 돌아가고, 그 컨트롤이 사라졌을 때만
    // 도크의 현재 썸네일로 포커스를 돌려준다.
    requestAnimationFrame(() => {
      const target =
        returnFocus?.isConnected &&
        returnFocus.tabIndex >= 0 &&
        returnFocus.getClientRects().length > 0
          ? returnFocus
          : (document.querySelector<HTMLElement>('[data-grid-toggle="true"]') ??
            document.querySelector<HTMLElement>('[data-dock-current="true"]'))
      target?.focus({ preventScroll: true })
    })
  }, [])
  const toggleGrid = useCallback(
    (trigger?: HTMLButtonElement) => {
      if (gridOpen) {
        handleGridClose()
        return
      }
      const active = trigger ?? document.activeElement
      gridReturnFocusRef.current = active instanceof HTMLElement ? active : null
      setGridOpen(true)
    },
    [gridOpen, handleGridClose]
  )
  const toggleDock = useCallback(() => {
    void updateSettings({ dockVisible: !useSettingsStore.getState().dockVisible })
  }, [])
  const { toggle: toggleAlwaysOnTop } = useAlwaysOnTop()
  const { copy: copyImage } = useCopyImage()
  const { trashCurrent, revealCurrent, openExternal, copyPathCurrent, renameCurrent } =
    useFileOperations({
      loadImage: loadImageAndReset
    })
  const autoHideUI = useSettingsStore((state) => state.autoHideUI)
  const chromeHidden = useIdleHide(autoHideUI)
  const dockPosition = useSettingsStore((state) => state.dockPosition)
  const dockVisible = useSettingsStore((state) => state.dockVisible)
  const dockThumbSize = useSettingsStore((state) => state.dockThumbSize)
  const dockShowName = useSettingsStore((state) => state.dockShowName)
  const dockShowIndex = useSettingsStore((state) => state.dockShowIndex)
  const [renameOpen, setRenameOpen] = useState(false)
  const renameInitialName = useAppStore((state) => state.imageInfo?.file_name) ?? ""
  const closeAndGoHome = useCloseImage()

  /** Esc 닫기: 그리드/다이얼로그가 열려 있거나 입력 중일 때는 뷰어를 닫지 않는다 */
  const handleCloseImage = useCallback(() => {
    if (gridOpen) {
      handleGridClose()
      return
    }
    if (renameOpen) return
    const active = document.activeElement
    if (
      active &&
      (active.tagName === "INPUT" ||
        active.tagName === "TEXTAREA" ||
        active.hasAttribute("contenteditable"))
    ) {
      return
    }
    closeAndGoHome()
  }, [gridOpen, handleGridClose, renameOpen, closeAndGoHome])

  const handleRenameSubmit = useCallback(
    async (newName: string) => {
      const ok = await renameCurrent(newName)
      if (ok) setRenameOpen(false)
    },
    [renameCurrent]
  )

  /** 단축키, 명령 팔레트, 컨텍스트 메뉴가 함께 쓰는 뷰어 동작 */
  const viewerActions: ViewerActionHandlers = useMemo(
    () => ({
      onNavigatePrev: () => handleNavigateImage("prev"),
      onNavigateNext: () => handleNavigateImage("next"),
      onJumpPrev10: () => handleNavigateByOffset(-10),
      onJumpNext10: () => handleNavigateByOffset(10),
      onJumpFirst: () => handleNavigateToIndex(0),
      onJumpLast: () => handleNavigateToIndex(dirImages.images.length - 1),
      onOpenFile: () => void handleOpenFile(),
      onCloseImage: handleCloseImage,
      onToggleExif: () => void toggleExifPanel(),
      onToggleFullscreen: () => void toggleFullscreen(),
      onToggleAlwaysOnTop: () => void toggleAlwaysOnTop(),
      onCopyImage: () => void copyImage(),
      onTrashFile: () => void trashCurrent(),
      onRevealInExplorer: () => void revealCurrent(),
      onOpenExternal: () => void openExternal(),
      onRenameFile: () => setRenameOpen(true),
      onCopyPath: () => void copyPathCurrent(),
      onToggleGrid: () => toggleGrid(),
      onToggleDock: toggleDock
    }),
    [
      handleNavigateImage,
      handleNavigateByOffset,
      handleNavigateToIndex,
      dirImages.images.length,
      handleOpenFile,
      handleCloseImage,
      toggleExifPanel,
      toggleFullscreen,
      toggleAlwaysOnTop,
      copyImage,
      trashCurrent,
      revealCurrent,
      openExternal,
      copyPathCurrent,
      toggleGrid,
      toggleDock
    ]
  )

  // 명령 팔레트에서 뷰어 동작을 실행할 수 있도록 핸들러 등록
  useEffect(() => {
    registerPaletteHandlers(viewerActions)
    return () => {
      unregisterPaletteHandlers()
    }
  }, [viewerActions])

  const baseContextMenu = useImageViewerContextMenu(dirImages, viewerActions)

  const runMouseAction = useCallback(
    (action: MouseAction, e?: React.MouseEvent) => {
      switch (action) {
        case "prev":
          handleNavigateImage("prev")
          break
        case "next":
          handleNavigateImage("next")
          break
        case "zoomIn":
          zoomIn()
          break
        case "zoomOut":
          zoomOut()
          break
        case "toggleFullscreen":
          void toggleFullscreen()
          break
        case "contextMenu":
          if (e) void baseContextMenu(e)
          break
        case "pan":
        case "none":
        default:
          break
      }
    },
    [baseContextMenu, toggleFullscreen, handleNavigateImage]
  )

  const handleDoubleClick = useCallback(
    (e: React.MouseEvent) => {
      const action = getSettings().mouse.doubleClick
      if (action === "pan" || action === "none") return
      e.preventDefault()
      runMouseAction(action, e)
    },
    [runMouseAction]
  )

  const handleMiddleClick = useCallback(
    (e: React.MouseEvent) => {
      const action = getSettings().mouse.middleClick
      if (action === "pan" || action === "none") {
        e.preventDefault()
        return
      }
      e.preventDefault()
      runMouseAction(action, e)
    },
    [runMouseAction]
  )

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      const action = getSettings().mouse.rightClick
      if (!action || action === "contextMenu") {
        void baseContextMenu(e)
        return
      }
      if (action === "pan" || action === "none") {
        e.preventDefault()
        return
      }
      e.preventDefault()
      runMouseAction(action, e)
    },
    [baseContextMenu, runMouseAction]
  )

  useImageViewerHotkeys({ ...viewerActions, disabled: gridOpen })

  const showDock = dirImages.images.length > 1
  // 도크는 항상 마운트해 두고 CSS로만 감춘다. 접었다 펼 때 목록 스크롤 위치와
  // 로드한 썸네일이 유지된다. (display:none이라 읽기 영역은 그대로 넓어진다)
  const dockCollapsedVisible = showDock && !dockVisible && !chromeHidden

  const dockNode = showDock ? (
    <ImageNavBar
      onNavigate={handleNavigateImage}
      onNavigateToIndex={handleNavigateToIndex}
      getOrLoadImage={getOrLoadImage}
      onToggleGrid={toggleGrid}
      gridActive={gridOpen}
      onToggleDock={toggleDock}
      position={dockPosition}
      thumbSize={dockThumbSize}
      showName={dockShowName}
      showIndex={dockShowIndex}
      hidden={!dockVisible || chromeHidden}
    />
  ) : null

  const ExpandIcon =
    dockPosition === "top"
      ? CaretDown
      : dockPosition === "bottom"
        ? CaretUp
        : dockPosition === "left"
          ? CaretRight
          : CaretLeft

  // 접힘 바의 펼치기 버튼은 도크의 접기 버튼과 같은 쪽(가로 도크는 우측,
  // 세로 도크는 아래)에 두어 마우스를 움직이지 않고 바로 토글할 수 있게 한다.
  const collapsedNode = dockCollapsedVisible ? (
    <div
      className={cn(
        "flex shrink-0 bg-background",
        (dockPosition === "top" || dockPosition === "bottom") &&
          "h-6 w-full flex-row items-center justify-end border-border pr-1",
        dockPosition === "top" && "border-b",
        dockPosition === "bottom" && "border-t",
        (dockPosition === "left" || dockPosition === "right") &&
          "h-full w-6 flex-col items-center justify-end border-border pb-1",
        dockPosition === "left" && "border-r",
        dockPosition === "right" && "border-l"
      )}
    >
      <Button
        variant="ghost"
        size="icon-sm"
        onClick={toggleDock}
        title={t("viewer.nav.dockShow")}
        aria-label={t("viewer.nav.dockShow")}
      >
        <ExpandIcon />
      </Button>
    </div>
  ) : null

  // 접힘 상태에서도 도크는 display:none으로 남겨 스크롤 위치와 썸네일을 유지하고,
  // 그 옆에 얇은 엣지 바를 함께 그린다.
  const dockArea = (
    <>
      {dockNode}
      {collapsedNode}
    </>
  )

  return (
    <div
      className="relative flex h-full w-full flex-col bg-background"
      onContextMenu={handleContextMenu}
      onDrop={handleDrop}
      onDragOver={handleDragOver}
      onDragEnter={handleDragEnter}
      onDragLeave={handleDragLeave}
    >
      {dockPosition === "top" && <div className="shrink-0">{dockArea}</div>}
      <div className="flex min-h-0 min-w-0 flex-1 flex-row">
        {dockPosition === "left" && <div className="flex min-h-0 shrink-0 pb-6">{dockArea}</div>}
        <div className="flex min-h-0 min-w-0 flex-1">
          <ImageContainer
            containerRef={containerRef}
            imageRef={imageRef}
            onWheel={handleWheel}
            onMouseDown={zoomPan.handleMouseDown}
            onMouseMove={zoomPan.handleMouseMove}
            onMouseUp={zoomPan.handleMouseUp}
            onDoubleClick={handleDoubleClick}
            onMiddleClick={handleMiddleClick}
            onNavigateToIndex={handleNavigateToIndex}
            getOrLoadImage={getOrLoadImage}
            viewMode={viewMode}
            pages={pages}
            onRetry={handleRetry}
            onWebtoonIndexChange={handleWebtoonIndexChange}
            webtoonScrollTarget={webtoonScrollTarget}
            onOpenThumbnailGrid={toggleGrid}
            onOpenArchiveFromPreview={handleOpenArchiveFromPreview}
          />
        </div>
        {dockPosition === "right" && <div className="flex min-h-0 shrink-0 pb-6">{dockArea}</div>}
      </div>
      {dockPosition === "bottom" && <div className="mb-6 shrink-0">{dockArea}</div>}
      {isDragOver && (
        <div className="pointer-events-none absolute inset-0 z-30 flex items-center justify-center border-2 border-dashed border-primary bg-background/80">
          <p className="rounded-md border bg-background px-4 py-2 text-sm">{t("home.drop")}</p>
        </div>
      )}
      {gridOpen && (
        <ThumbnailGrid
          dirImages={dirImages}
          archivePath={archivePath}
          getOrLoadImage={getOrLoadImage}
          failedPaths={failedPaths}
          onNavigateToIndex={handleNavigateToIndex}
          onClose={handleGridClose}
        />
      )}
      <RenameDialog
        open={renameOpen}
        initialName={renameInitialName}
        onSubmit={(name) => void handleRenameSubmit(name)}
        onClose={() => setRenameOpen(false)}
      />
    </div>
  )
}
