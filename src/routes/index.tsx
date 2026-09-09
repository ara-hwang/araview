import { useShallow } from "zustand/react/shallow"
import { createFileRoute, useNavigate } from "@tanstack/react-router"
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyTitle
} from "@/components/ui/empty"
import { Button } from "@/components/ui/button"
import { useEffect, useRef } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { useAppStore } from "@/store/appStore"
import { useSettingsStore } from "@/store/settingsStore"
import { useImageLoader } from "@/hooks/useImageLoader"
import { usePaintSrcs } from "@/hooks/usePaintSrcs"
import { useRecentFilesStore } from "@/store/recentFilesStore"
import { useFavoritesStore } from "@/store/favoritesStore"
import { HomeFileCard } from "@/components/HomeFileCard"
import { Star } from "lucide-react"

export const Route = createFileRoute("/")({
  component: HomePage
})

function HomePage() {
  const app = useAppStore(
    useShallow((state) => ({
      imageInfo: state.imageInfo,
      dirImages: state.dirImages
    }))
  )
  const navigate = useNavigate()
  const recentFiles = useRecentFilesStore((s) => s.files)
  const removeRecent = useRecentFilesStore((s) => s.remove)
  const clearRecent = useRecentFilesStore((s) => s.clear)
  const favorites = useFavoritesStore((s) => s.files)
  const toggleFavorite = useFavoritesStore((s) => s.toggle)
  const removeFavorite = useFavoritesStore((s) => s.remove)
  const clearFavorites = useFavoritesStore((s) => s.clear)
  const autoOpenLastFile = useSettingsStore((s) => s.autoOpenLastFile)
  const autoOpenedRef = useRef(false)

  // 이미지 변경 시 창 제목 변경
  useEffect(() => {
    const title = app.imageInfo ? app.imageInfo.file_name : "Image Viewer"
    getCurrentWindow()
      .setTitle(title)
      .catch(() => {})
  }, [app.imageInfo])

  // 이미지가 로드되면 이미지 페이지로 이동
  useEffect(() => {
    if (app.imageInfo) {
      void navigate({ to: "/image" })
    }
  }, [app.imageInfo, navigate])

  const {
    loadImage,
    getOrLoadImage,
    handleOpenFile,
    handleDrop,
    handleDragOver,
    handleDragEnter,
    handleDragLeave,
    isDragOver
  } = useImageLoader()

  // 시작 옵션: 마지막 파일 자동 열기 (스토어 로드 후 1회)
  useEffect(() => {
    if (autoOpenedRef.current) return
    if (!autoOpenLastFile) return
    if (app.imageInfo) return
    if (recentFiles.length === 0) return
    autoOpenedRef.current = true
    void loadImage(recentFiles[0])
  }, [autoOpenLastFile, recentFiles, app.imageInfo, loadImage])
  const urls = usePaintSrcs([...favorites, ...recentFiles], getOrLoadImage)
  const isFavorite = (path: string) => favorites.includes(path)

  return (
    <div
      onContextMenu={(e) => {
        e.preventDefault()
      }}
      onDrop={handleDrop}
      onDragOver={handleDragOver}
      onDragEnter={handleDragEnter}
      onDragLeave={handleDragLeave}
      className="relative mx-auto flex h-full w-full max-w-4xl flex-col justify-center gap-10 p-8 sm:p-12"
    >
      {isDragOver && (
        <div className="border-primary bg-background/80 pointer-events-none absolute inset-0 z-30 flex items-center justify-center border-2 border-dashed">
          <p className="bg-background rounded-md border px-4 py-2 text-sm">
            여기에 놓아 열기
          </p>
        </div>
      )}
      <Empty className="items-start border-none p-0 text-left">
        <EmptyContent className="max-w-md items-start gap-3 text-left">
          <p className="text-muted-foreground text-xs tabular-nums">
            01, Empty
          </p>
          <EmptyTitle className="text-2xl font-semibold">
            표시할 이미지가 없습니다
          </EmptyTitle>
          <EmptyDescription>
            파일을 여기로 끌어다 놓거나 Open File 버튼으로 선택하세요.
          </EmptyDescription>
          <Button onClick={handleOpenFile} className="mt-1">
            Open File
          </Button>
        </EmptyContent>
      </Empty>

      {favorites.length > 0 && (
        <div className="w-full border-t pt-6">
          <div className="mb-3 flex items-center justify-between">
            <h2 className="text-muted-foreground flex items-center gap-1.5 text-xs tabular-nums">
              <Star className="size-3.5 fill-yellow-400 text-yellow-400" />
              즐겨찾기, {favorites.length}
            </h2>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => void clearFavorites()}
            >
              전체 지우기
            </Button>
          </div>
          <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5">
            {favorites.map((path) => (
              <HomeFileCard
                key={path}
                path={path}
                src={urls.get(path)}
                isFavorite
                onOpen={(p) => void loadImage(p)}
                onRemove={(p) => void removeFavorite(p)}
                onToggleFavorite={(p) => void toggleFavorite(p)}
              />
            ))}
          </div>
        </div>
      )}

      {recentFiles.length > 0 && (
        <div className="w-full border-t pt-6">
          <div className="mb-3 flex items-center justify-between">
            <h2 className="text-muted-foreground text-xs tabular-nums">
              최근 파일, {recentFiles.length}
            </h2>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => void clearRecent()}
            >
              전체 지우기
            </Button>
          </div>
          <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5">
            {recentFiles.map((path) => (
              <HomeFileCard
                key={path}
                path={path}
                src={urls.get(path)}
                isFavorite={isFavorite(path)}
                onOpen={(p) => void loadImage(p)}
                onRemove={(p) => void removeRecent(p)}
                onToggleFavorite={(p) => void toggleFavorite(p)}
              />
            ))}
          </div>
        </div>
      )}
    </div>
  )
}
