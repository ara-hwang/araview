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
import { useOpenFileListener } from "@/hooks/useOpenFileListener"
import { usePaintSrcs } from "@/hooks/usePaintSrcs"
import { useRecentFilesStore } from "@/store/recentFilesStore"
import { useFavoritesStore } from "@/store/favoritesStore"
import { HomeFileCard } from "@/components/HomeFileCard"
import { Star } from "@phosphor-icons/react"
import { useTranslation } from "react-i18next"

export const Route = createFileRoute("/")({
  component: HomePage
})

function HomePage() {
  const { t } = useTranslation()
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
  const recordRecentFiles = useSettingsStore((s) => s.recordRecentFiles)
  const autoOpenedRef = useRef(false)

  // 이미지 변경 시 창 제목 변경
  useEffect(() => {
    const title = app.imageInfo ? app.imageInfo.file_name : t("app.title")
    getCurrentWindow()
      .setTitle(title)
      .catch(() => {})
  }, [app.imageInfo, t])

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

  // 파일 연결/두 번째 실행으로 열린 파일은 홈에서도 받아 연다.
  useOpenFileListener(loadImage)

  // 시작 옵션: 마지막 파일 자동 열기 (스토어 로드 후 1회)
  useEffect(() => {
    if (autoOpenedRef.current) return
    if (!autoOpenLastFile) return
    if (!recordRecentFiles) return
    if (app.imageInfo) return
    if (recentFiles.length === 0) return
    autoOpenedRef.current = true
    void loadImage(recentFiles[0])
  }, [
    autoOpenLastFile,
    recordRecentFiles,
    recentFiles,
    app.imageInfo,
    loadImage
  ])
  const visibleRecentFiles = recordRecentFiles ? recentFiles : []
  const urls = usePaintSrcs(
    [...favorites, ...visibleRecentFiles],
    getOrLoadImage
  )
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
      className="relative h-full w-full"
    >
      {/* 셸은 고정하고 홈 콘텐츠만 내부에서 스크롤한다 (헤더가 밀려나지 않게) */}
      <div className="h-full w-full overflow-y-auto">
        <div className="mx-auto flex min-h-full w-full max-w-4xl flex-col justify-center gap-10 p-8 sm:p-12">
          <Empty className="items-start border-none p-0 text-left">
            <EmptyContent className="max-w-md items-start gap-3 text-left">
              <p className="text-muted-foreground text-xs tabular-nums">
                {t("home.emptyTag")}
              </p>
              <EmptyTitle className="text-2xl font-semibold">
                {t("home.emptyTitle")}
              </EmptyTitle>
              <EmptyDescription>{t("home.emptyDesc")}</EmptyDescription>
              <Button onClick={handleOpenFile} className="mt-1">
                {t("home.openFile")}
              </Button>
            </EmptyContent>
          </Empty>

          {favorites.length > 0 && (
            <div className="w-full border-t pt-6">
              <div className="mb-3 flex items-center justify-between">
                <h2 className="text-muted-foreground flex items-center gap-1.5 text-xs tabular-nums">
                  <Star
                    weight="fill"
                    className="size-3.5 fill-yellow-400 text-yellow-400"
                  />
                  {t("home.favorites", { count: favorites.length })}
                </h2>
                <Button
                  size="sm"
                  variant="ghost"
                  className="hover:bg-destructive/10 hover:text-destructive dark:hover:bg-destructive/20"
                  onClick={() => void clearFavorites()}
                >
                  {t("home.clearAll")}
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

          {visibleRecentFiles.length > 0 && (
            <div className="w-full border-t pt-6">
              <div className="mb-3 flex items-center justify-between">
                <h2 className="text-muted-foreground text-xs tabular-nums">
                  {t("home.recent", { count: visibleRecentFiles.length })}
                </h2>
                <Button
                  size="sm"
                  variant="ghost"
                  className="hover:bg-destructive/10 hover:text-destructive dark:hover:bg-destructive/20"
                  onClick={() => void clearRecent()}
                >
                  {t("home.clearAll")}
                </Button>
              </div>
              <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5">
                {visibleRecentFiles.map((path) => (
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
      </div>
      {isDragOver && (
        <div className="border-primary bg-background/80 pointer-events-none absolute inset-0 z-30 flex items-center justify-center border-2 border-dashed">
          <p className="bg-background rounded-md border px-4 py-2 text-sm">
            {t("home.drop")}
          </p>
        </div>
      )}
    </div>
  )
}
