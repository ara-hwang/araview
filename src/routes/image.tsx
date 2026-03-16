import { createFileRoute, useNavigate, redirect } from "@tanstack/react-router";
import { useImageViewer } from "@/hooks/useImageViewer";
import Toolbar from "@/components/Toolbar";
import { ImageContainer } from "@/components/ImageContainer";
import { StatusBar } from "@/components/StatusBar";
import { Separator } from "@/components/ui/separator";
import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useAppStore, getApp } from "@/store/appStore";

export const Route = createFileRoute("/image")({
  beforeLoad: () => {
    if (!getApp().imageInfo) {
      throw redirect({ to: "/" });
    }
  },
  component: ImagePage,
});

function ImagePage() {
  const app = useAppStore();
  const navigate = useNavigate();

  const {
    containerRef,
    imageRef,
    handleOpenFile,
    navigateImage,
    navigateToIndex,
    handleZoomIn,
    handleZoomOut,
    handleResetZoom,
    handleFitWidth,
    handleFitHeight,
    handleFitScreen,
    handleWheel,
    handleMouseDown,
    handleMouseMove,
    handleMouseUp,
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

      <ImageContainer
        containerRef={containerRef}
        imageRef={imageRef}
        onWheel={handleWheel}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onNavigate={navigateImage}
        onNavigateToIndex={navigateToIndex}
      />

      <Separator />

      <StatusBar />
    </div>
  );
}
