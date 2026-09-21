import { createFileRoute, useNavigate } from "@tanstack/react-router"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { useEffect, useRef } from "react"
import { useTranslation } from "react-i18next"
import { useShallow } from "zustand/react/shallow"

import { AppIcon } from "@/components/AppIcon"
import { RecentFileAttachment } from "@/components/RecentFileAttachment"
import { AttachmentGroup } from "@/components/ui/attachment"
import { Button } from "@/components/ui/button"
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia
} from "@/components/ui/empty"
import { DEV_BUILD_SUFFIX } from "@/constants/app"
import { useImageLoader } from "@/hooks/useImageLoader"
import { useOpenFileListener } from "@/hooks/useOpenFileListener"
import { useRecentFileDetails } from "@/hooks/useRecentFileDetails"
import { useAppStore } from "@/store/appStore"
import { useRecentFilesStore } from "@/store/recentFilesStore"
import { useSettingsStore } from "@/store/settingsStore"
import { hasBufferedOpenFile } from "@/utils/openFileDelivery"

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
  const autoOpenLastFile = useSettingsStore((s) => s.autoOpenLastFile)
  const recordRecentFiles = useSettingsStore((s) => s.recordRecentFiles)
  const autoOpenedRef = useRef(false)

  // 이미지 변경 시 창 제목 변경
  useEffect(() => {
    const title = app.imageInfo ? app.imageInfo.file_name : `${t("app.title")}${DEV_BUILD_SUFFIX}`
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
    if (hasBufferedOpenFile()) return
    if (recentFiles.length === 0) return
    autoOpenedRef.current = true
    void loadImage(recentFiles[0])
  }, [autoOpenLastFile, recordRecentFiles, recentFiles, app.imageInfo, loadImage])
  const visibleRecentFiles = recordRecentFiles ? recentFiles : []
  const details = useRecentFileDetails(visibleRecentFiles, getOrLoadImage)

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
          <Empty className="border-none">
            <EmptyHeader>
              <EmptyMedia>
                <AppIcon className="size-16" />
              </EmptyMedia>
              <EmptyDescription>{t("home.emptyDesc")}</EmptyDescription>
            </EmptyHeader>
            <EmptyContent>
              <Button onClick={handleOpenFile}>{t("home.openFile")}</Button>
            </EmptyContent>
          </Empty>

          {visibleRecentFiles.length > 0 && (
            <div className="w-full border-t pt-6">
              <div className="mb-3 flex items-center justify-between">
                <h2 className="text-xs text-muted-foreground tabular-nums">
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
              <AttachmentGroup>
                {visibleRecentFiles.map((path) => {
                  const detail = details.get(path)
                  return (
                    <RecentFileAttachment
                      key={path}
                      path={path}
                      info={detail?.info}
                      src={detail?.src}
                      status={detail?.status ?? "loading"}
                      onOpen={(p) => void loadImage(p)}
                      onRemove={(p) => void removeRecent(p)}
                    />
                  )
                })}
              </AttachmentGroup>
            </div>
          )}
        </div>
      </div>
      {isDragOver && (
        <div className="pointer-events-none absolute inset-0 z-30 flex items-center justify-center border-2 border-dashed border-primary bg-background/80">
          <p className="rounded-md border bg-background px-4 py-2 text-sm">{t("home.drop")}</p>
        </div>
      )}
    </div>
  )
}
