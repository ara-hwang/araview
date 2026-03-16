import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useImageViewer } from "@/hooks/useImageViewer";
import Toolbar from "@/components/Toolbar";
import { StatusBar } from "@/components/StatusBar";
import { Empty, EmptyContent } from "@/components/ui/empty";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useAppStore } from "@/store/appStore";

export const Route = createFileRoute("/")({
  component: HomePage,
});

function HomePage() {
  const app = useAppStore();
  const navigate = useNavigate();

  const {
    handleOpenFile,
    handleZoomIn,
    handleZoomOut,
    handleResetZoom,
    handleFitWidth,
    handleFitHeight,
    handleFitScreen,
    handleDrop,
    handleDragOver,
    handleContextMenu,
  } = useImageViewer();

  // 이미지 변경 시 창 제목 변경
  useEffect(() => {
    const title = app.imageInfo ? app.imageInfo.file_name : "Image Viewer";
    getCurrentWindow()
      .setTitle(title)
      .catch(() => {});
  }, [app.imageInfo]);

  // 이미지가 로드되면 이미지 페이지로 이동
  useEffect(() => {
    if (app.imageInfo) {
      void navigate({ to: "/image" });
    }
  }, [app.imageInfo, navigate]);

  const handleOpenSettings = () => {
    void navigate({ to: "/settings" });
  };

  return (
    <div
      className="flex flex-col h-screen w-screen"
      onContextMenu={handleContextMenu}
      onDrop={handleDrop}
      onDragOver={handleDragOver}
    >
      <Toolbar
        onOpenFile={handleOpenFile}
        onZoomIn={handleZoomIn}
        onZoomOut={handleZoomOut}
        onResetZoom={handleResetZoom}
        onFitWidth={handleFitWidth}
        onFitHeight={handleFitHeight}
        onFitScreen={handleFitScreen}
        onOpenSettings={handleOpenSettings}
      />

      <Separator />

      <Empty>
        <EmptyContent>
          <Button onClick={handleOpenFile}>Open File</Button>
        </EmptyContent>
      </Empty>

      <Separator />

      <StatusBar />
    </div>
  );
}
