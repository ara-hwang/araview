import { createFileRoute, redirect } from "@tanstack/react-router"
import { useCallback, useEffect, useState } from "react"
import { useTranslation } from "react-i18next"

import { ImageContainer } from "@/components/ImageContainer"
import { RenameDialog } from "@/components/RenameDialog"
import { SaveEditsDialog } from "@/components/SaveEditsDialog"
import { ThumbnailGrid } from "@/components/ThumbnailGrid"
import { toast } from "@/components/ui/toast"
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
import { saveBlockedReason } from "@/hooks/useFileOperations"
import { useFullscreen } from "@/hooks/useFullscreen"
import { useIdleHide } from "@/hooks/useIdleHide"
import { useImageLoader } from "@/hooks/useImageLoader"
import { useImageViewerHotkeys } from "@/hooks/useImageViewerHotkeys"
import { useMultiPageImages } from "@/hooks/useMultiPageImages"
import { useOpenFileListener } from "@/hooks/useOpenFileListener"
import { useViewerElements } from "@/hooks/useViewerElements"
import { useWheelNavigation } from "@/hooks/useWheelNavigation"
import { useZoomPan } from "@/hooks/useZoomPan"
import { getApp, updateDirImagesIndex, useAppStore, zoomIn, zoomOut } from "@/store/appStore"
import { useArchiveProgressStore } from "@/store/archiveProgressStore"
import { getSettings, useSettingsStore } from "@/store/settingsStore"
import { cycleViewerBackground } from "@/store/settingsStore"
import type { SaveEditsPayload } from "@/utils/imageEdits"

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
    openArchiveFromPreview
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
        void useArchiveProgressStore.getState().save(st.archivePath, path)
      }
      void getOrLoadImage(path)
        .then((info) => {
          const cur = useAppStore.getState()
          if (cur.dirImages.images[cur.dirImages.current_index] === path) {
            useAppStore.setState({ imageInfo: info, error: null })
            cur.removeFailedPath(path)
          }
        })
        .catch(() => {})
      // 스크롤 방향 예열: 앞 5장 메타 확보
      const imgs = st.dirImages.images
      for (let k = 1; k <= 5; k += 1) {
        const p = imgs[index + k]
        if (p) void getOrLoadImage(p).catch(() => {})
      }
    },
    [getOrLoadImage]
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

  const handleWheel = useWheelNavigation(zoomPan, navigateImage)

  useOpenFileListener(loadImageExplicit)

  const fullscreen = useFullscreen()

  const toggleGrid = useCallback(() => {
    if (gridOpen) {
      setGridOpen(false)
      return
    }
    setGridOpen(true)
  }, [gridOpen])
  const { toggle: toggleAlwaysOnTop } = useAlwaysOnTop()
  const { copy: copyImage } = useCopyImage()
  const { trashCurrent, revealCurrent, openExternal, copyPathCurrent, renameCurrent, saveEdits } =
    useFileOperations({
      loadImage: loadImageAndReset
    })
  const autoHideUI = useSettingsStore((state) => state.autoHideUI)
  const chromeHidden = useIdleHide(autoHideUI)
  const [renameOpen, setRenameOpen] = useState(false)
  const [saveOpen, setSaveOpen] = useState(false)
  const renameInitialName = useAppStore((state) => state.imageInfo?.file_name) ?? ""
  const closeAndGoHome = useCloseImage()

  /** PSD/SVG/AVIF는 저장 불가라 다이얼로그를 열지 않고 즉시 안내한다. */
  const handleOpenSaveDialog = useCallback(() => {
    const blocked = saveBlockedReason(useAppStore.getState().imageInfo)
    if (blocked) {
      toast.info(t(`toast.save.${blocked}`))
      return
    }
    setSaveOpen(true)
  }, [t])

  // 명령 팔레트에서 뷰어 동작을 실행할 수 있도록 핸들러 등록
  useEffect(() => {
    registerPaletteHandlers({
      onNavigatePrev: () => handleNavigateImage("prev"),
      onNavigateNext: () => handleNavigateImage("next"),
      onJumpPrev10: () => handleNavigateByOffset(-10),
      onJumpNext10: () => handleNavigateByOffset(10),
      onJumpFirst: () => handleNavigateToIndex(0),
      onJumpLast: () => handleNavigateToIndex(dirImages.images.length - 1),
      onToggleExif: () => void toggleExifPanel(),
      onCopyImage: () => void copyImage(),
      onTrashFile: () => void trashCurrent(),
      onRevealInExplorer: () => void revealCurrent(),
      onOpenExternal: () => void openExternal(),
      onRenameFile: () => setRenameOpen(true),
      onCopyPath: () => void copyPathCurrent(),
      onSaveEdits: handleOpenSaveDialog,
      onToggleGrid: toggleGrid
    })
    return () => {
      unregisterPaletteHandlers()
    }
  }, [
    handleNavigateImage,
    handleNavigateByOffset,
    handleNavigateToIndex,
    dirImages.images.length,
    toggleExifPanel,
    copyImage,
    trashCurrent,
    revealCurrent,
    openExternal,
    copyPathCurrent,
    toggleGrid,
    handleOpenSaveDialog
  ])

  /** Esc 닫기: 그리드/다이얼로그가 열려 있거나 입력 중일 때는 뷰어를 닫지 않는다 */
  const handleCloseImage = useCallback(() => {
    if (gridOpen) {
      setGridOpen(false)
      return
    }
    if (renameOpen || saveOpen) return
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
  }, [gridOpen, renameOpen, saveOpen, closeAndGoHome])

  const handleRenameSubmit = useCallback(
    async (newName: string) => {
      const ok = await renameCurrent(newName)
      if (ok) setRenameOpen(false)
    },
    [renameCurrent]
  )

  const handleSaveSubmit = useCallback(
    async (payload: SaveEditsPayload) => {
      const ok = await saveEdits(payload)
      if (ok) setSaveOpen(false)
    },
    [saveEdits]
  )

  const baseContextMenu = useImageViewerContextMenu(dirImages, {
    onOpenFile: handleOpenFile,
    onCloseImage: handleCloseImage,
    onNavigatePrev: () => handleNavigateImage("prev"),
    onNavigateNext: () => handleNavigateImage("next"),
    onToggleExif: () => void toggleExifPanel(),
    onToggleFullscreen: () => void fullscreen.toggle(),
    onToggleAlwaysOnTop: () => void toggleAlwaysOnTop(),
    onCopyImage: () => void copyImage(),
    onTrashFile: () => void trashCurrent(),
    onRevealInExplorer: () => void revealCurrent(),
    onOpenExternal: () => void openExternal(),
    onRenameFile: () => setRenameOpen(true),
    onCopyPath: () => void copyPathCurrent(),
    onSaveEdits: handleOpenSaveDialog,
    onToggleGrid: toggleGrid
  })

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
          void fullscreen.toggle()
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
    [baseContextMenu, fullscreen, handleNavigateImage]
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

  useImageViewerHotkeys({
    onNavigatePrev: () => handleNavigateImage("prev"),
    onNavigateNext: () => handleNavigateImage("next"),
    onJumpPrev10: () => handleNavigateByOffset(-10),
    onJumpNext10: () => handleNavigateByOffset(10),
    onJumpFirst: () => handleNavigateToIndex(0),
    onJumpLast: () => handleNavigateToIndex(dirImages.images.length - 1),
    onOpenFile: handleOpenFile,
    onCloseImage: handleCloseImage,
    onToggleExif: () => void toggleExifPanel(),
    onToggleFullscreen: () => void fullscreen.toggle(),
    onToggleAlwaysOnTop: () => void toggleAlwaysOnTop(),
    onCopyImage: () => void copyImage(),
    onTrashFile: () => void trashCurrent(),
    onRevealInExplorer: () => void revealCurrent(),
    onOpenExternal: () => void openExternal(),
    onCycleBackground: () => cycleViewerBackground(),
    onRenameFile: () => setRenameOpen(true),
    onCopyPath: () => void copyPathCurrent(),
    onSaveEdits: handleOpenSaveDialog,
    onToggleGrid: toggleGrid,
    disabled: gridOpen
  })

  return (
    <div
      className="relative flex h-full w-full flex-col"
      onContextMenu={handleContextMenu}
      onDrop={handleDrop}
      onDragOver={handleDragOver}
      onDragEnter={handleDragEnter}
      onDragLeave={handleDragLeave}
    >
      <ImageContainer
        containerRef={containerRef}
        imageRef={imageRef}
        onWheel={handleWheel}
        onMouseDown={zoomPan.handleMouseDown}
        onMouseMove={zoomPan.handleMouseMove}
        onMouseUp={zoomPan.handleMouseUp}
        onDoubleClick={handleDoubleClick}
        onMiddleClick={handleMiddleClick}
        onNavigate={handleNavigateImage}
        onNavigateToIndex={handleNavigateToIndex}
        onToggleGrid={toggleGrid}
        gridActive={gridOpen}
        getOrLoadImage={getOrLoadImage}
        viewMode={viewMode}
        pages={pages}
        chromeHidden={chromeHidden}
        onRetry={handleRetry}
        onWebtoonIndexChange={handleWebtoonIndexChange}
        webtoonScrollTarget={webtoonScrollTarget}
        onOpenArchiveFromPreview={handleOpenArchiveFromPreview}
      />
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
          onClose={() => setGridOpen(false)}
        />
      )}
      <RenameDialog
        open={renameOpen}
        initialName={renameInitialName}
        onSubmit={(name) => void handleRenameSubmit(name)}
        onClose={() => setRenameOpen(false)}
      />
      <SaveEditsDialog
        open={saveOpen}
        onSubmit={(payload) => void handleSaveSubmit(payload)}
        onClose={() => setSaveOpen(false)}
      />
    </div>
  )
}
