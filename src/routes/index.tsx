import { useShallow } from "zustand/react/shallow"
import { createFileRoute, useNavigate } from "@tanstack/react-router"
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyTitle
} from "@/components/ui/empty"
import { Button } from "@/components/ui/button"
import { useEffect } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { X } from "lucide-react"
import { useAppStore } from "@/store/appStore"
import { useImageLoader } from "@/hooks/useImageLoader"
import { usePaintSrcs } from "@/hooks/usePaintSrcs"
import { useRecentFilesStore } from "@/store/recentFilesStore"

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
    handleDragOver
  } = useImageLoader()
  const urls = usePaintSrcs(recentFiles, getOrLoadImage)

  return (
    <div
      onContextMenu={(e) => {
        e.preventDefault()
      }}
      onDrop={handleDrop}
      onDragOver={handleDragOver}
      className="flex h-full w-full flex-col items-center justify-center gap-8 p-8"
    >
      <Empty>
        <EmptyContent>
          <EmptyTitle>표시할 이미지가 없습니다</EmptyTitle>
          <EmptyDescription>
            파일을 여기로 끌어다 놓거나 Open File 버튼으로 선택하세요.
          </EmptyDescription>
          <Button onClick={handleOpenFile}>Open File</Button>
        </EmptyContent>
      </Empty>

      {recentFiles.length > 0 && (
        <div className="w-full max-w-4xl">
          <div className="mb-3 flex items-center justify-between">
            <h2 className="text-muted-foreground text-sm font-medium">
              최근 파일
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
            {recentFiles.map((path) => {
              const name = path.split(/[\\/]/).pop() ?? path
              const src = urls.get(path)
              return (
                <div
                  key={path}
                  className="group border-border bg-card hover:border-primary relative overflow-hidden rounded-md border"
                >
                  <button
                    type="button"
                    onClick={() => void loadImage(path)}
                    className="flex w-full flex-col items-start gap-2 p-2 text-left"
                    title={path}
                  >
                    <div className="bg-muted aspect-square w-full overflow-hidden rounded">
                      {src ? (
                        <img
                          src={src}
                          alt=""
                          loading="lazy"
                          className="h-full w-full object-cover"
                          draggable={false}
                          onError={(e) => {
                            ;(
                              e.currentTarget as HTMLImageElement
                            ).style.visibility = "hidden"
                          }}
                        />
                      ) : null}
                    </div>
                    <span className="w-full truncate text-xs">{name}</span>
                  </button>
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation()
                      void removeRecent(path)
                    }}
                    className="bg-background/80 absolute top-1 right-1 rounded p-1 opacity-0 transition-opacity group-hover:opacity-100"
                    title="목록에서 제거"
                  >
                    <X className="size-3" />
                  </button>
                </div>
              )
            })}
          </div>
        </div>
      )}
    </div>
  )
}
