import { createFileRoute, useNavigate } from "@tanstack/react-router"
import { Empty, EmptyContent } from "@/components/ui/empty"
import { Button } from "@/components/ui/button"
import { useEffect } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { useAppStore } from "@/store/appStore"
import { useImageLoader } from "@/hooks/useImageLoader"

export const Route = createFileRoute("/")({
  component: HomePage
})

function HomePage() {
  const app = useAppStore()
  const navigate = useNavigate()

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

  const { handleOpenFile, handleDrop, handleDragOver } = useImageLoader()

  return (
    <div
      onContextMenu={(e) => {
        e.preventDefault()
      }}
      onDrop={handleDrop}
      onDragOver={handleDragOver}
      className="flex h-full w-full flex-col items-center justify-center"
    >
      <Empty>
        <EmptyContent>
          <Button onClick={handleOpenFile}>Open File</Button>
        </EmptyContent>
      </Empty>
    </div>
  )
}
