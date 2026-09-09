import { createFileRoute, redirect } from "@tanstack/react-router"
import { ImageContainer } from "@/components/ImageContainer"
import { getApp, useAppStore } from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"
import { useDirectoryNavigation } from "@/hooks/useDirectoryNavigation"
import { useExifLoader } from "@/hooks/useExifLoader"
import { useImageLoader } from "@/hooks/useImageLoader"
import { useMultiPageImages } from "@/hooks/useMultiPageImages"
import { useSlideshow } from "@/hooks/useSlideshow"
import { useFullscreen } from "@/hooks/useFullscreen"
import { useCopyImage } from "@/hooks/useCopyImage"
import { useFileOperations } from "@/hooks/useFileOperations"
import { useIdleHide } from "@/hooks/useIdleHide"
import { cycleViewerBackground } from "@/store/settingsStore"
import { RenameDialog } from "@/components/RenameDialog"
import { SaveEditsDialog } from "@/components/SaveEditsDialog"
import { toggleShuffleAndRefresh } from "@/utils/directoryOptions"
import type { SaveEditsPayload } from "@/utils/imageEdits"
import { useImageViewerContextMenu } from "@/hooks/useContextMenu"
import { useImageViewerHotkeys } from "@/hooks/useImageViewerHotkeys"
import { useOpenFileListener } from "@/hooks/useOpenFileListener"
import { useViewerElements } from "@/hooks/useViewerElements"
import { useWheelNavigation } from "@/hooks/useWheelNavigation"
import { useZoomPan } from "@/hooks/useZoomPan"
import { useCallback, useState } from "react"

export const Route = createFileRoute("/image")({
  beforeLoad: () => {
    if (!getApp().imageInfo) {
      throw redirect({ to: "/" })
    }
  },
  component: ImagePage
})

function ImagePage() {
  const dirImages = useAppStore((state) => state.dirImages)

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
    getOrLoadImage
  } = useImageLoader()

  const { viewMode, pages } = useMultiPageImages(getOrLoadImage)
  const { toggleExifPanel } = useExifLoader()

  // 이미지 로드 후 줌/팬 초기화
  const loadImageAndReset = useCallback(
    (filePath: string, options?: { refreshDirectory?: boolean }) =>
      loadImage(filePath, { ...options, onAfterLoad: zoomPan.resetView }),
    [loadImage, zoomPan.resetView]
  )

  const { navigateImage, navigateToIndex } = useDirectoryNavigation(
    loadImageAndReset,
    loadArchiveImageByIndex
  )

  const handleWheel = useWheelNavigation(zoomPan, navigateImage)

  useOpenFileListener(loadImageAndReset)

  const slideshow = useSlideshow(() => void navigateImage("next"))
  const fullscreen = useFullscreen()
  const { copy: copyImage } = useCopyImage()
  const {
    trashCurrent,
    revealCurrent,
    openExternal,
    copyPathCurrent,
    renameCurrent,
    saveEdits,
    toggleFavoriteCurrent
  } = useFileOperations({
    loadImage: loadImageAndReset
  })
  const autoHideUI = useSettingsStore((state) => state.autoHideUI)
  const chromeHidden = useIdleHide(autoHideUI)
  const [renameOpen, setRenameOpen] = useState(false)
  const [saveOpen, setSaveOpen] = useState(false)
  const renameInitialName =
    useAppStore((state) => state.imageInfo?.file_name) ?? ""

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

  const handleContextMenu = useImageViewerContextMenu(dirImages, {
    onOpenFile: handleOpenFile,
    onNavigatePrev: () => void navigateImage("prev"),
    onNavigateNext: () => void navigateImage("next"),
    onToggleExif: () => void toggleExifPanel(),
    onToggleSlideshow: slideshow.toggle,
    onToggleFullscreen: () => void fullscreen.toggle(),
    onCopyImage: () => void copyImage(),
    onTrashFile: () => void trashCurrent(),
    onRevealInExplorer: () => void revealCurrent(),
    onOpenExternal: () => void openExternal(),
    onRenameFile: () => setRenameOpen(true),
    onCopyPath: () => void copyPathCurrent(),
    onSaveEdits: () => setSaveOpen(true),
    onToggleFavorite: () => void toggleFavoriteCurrent()
  })

  useImageViewerHotkeys({
    onNavigatePrev: () => void navigateImage("prev"),
    onNavigateNext: () => void navigateImage("next"),
    onOpenFile: handleOpenFile,
    onToggleExif: () => void toggleExifPanel(),
    onToggleSlideshow: slideshow.toggle,
    onToggleFullscreen: () => void fullscreen.toggle(),
    onCopyImage: () => void copyImage(),
    onTrashFile: () => void trashCurrent(),
    onRevealInExplorer: () => void revealCurrent(),
    onOpenExternal: () => void openExternal(),
    onCycleBackground: () => cycleViewerBackground(),
    onRenameFile: () => setRenameOpen(true),
    onCopyPath: () => void copyPathCurrent(),
    onToggleShuffle: () => toggleShuffleAndRefresh(),
    onSaveEdits: () => setSaveOpen(true),
    onToggleFavorite: () => void toggleFavoriteCurrent()
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
        onNavigate={navigateImage}
        onNavigateToIndex={navigateToIndex}
        getOrLoadImage={getOrLoadImage}
        viewMode={viewMode}
        pages={pages}
        slideshowActive={slideshow.active}
        slideshowIntervalMs={slideshow.intervalMs}
        onToggleSlideshow={slideshow.toggle}
        onToggleFullscreen={() => void fullscreen.toggle()}
        chromeHidden={chromeHidden}
      />
      {isDragOver && (
        <div className="border-primary bg-background/80 pointer-events-none absolute inset-0 z-30 flex items-center justify-center border-2 border-dashed">
          <p className="bg-background rounded-md border px-4 py-2 text-sm">
            여기에 놓아 열기
          </p>
        </div>
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
