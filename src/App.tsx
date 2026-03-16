import "./App.css";

import { useImageViewer } from "./hooks/useImageViewer";
import Toolbar from "./components/Toolbar";
import { ImageContainer } from "./components/ImageContainer";
import { StatusBar } from "./components/StatusBar";
import { SettingsDialog } from "./components/SettingsDialog";
import { Empty, EmptyContent } from "./components/ui/empty";
import { Button } from "./components/ui/button";
import { Separator } from "./components/ui/separator";
import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useAppStore } from "./store/appStore";

function App() {
  const app = useAppStore();

  const {
    isSettingsOpen,
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
    handleOpenSettings,
    handleCloseSettings,
  } = useImageViewer();

  // 이미지 변경 시 창 제목 변경
  useEffect(() => {
    const title = app.imageInfo ? app.imageInfo.file_name : "Image Viewer";
    getCurrentWindow()
      .setTitle(title)
      .catch(() => {});
  }, [app.imageInfo]);

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

      {!app.imageInfo ? (
        <Empty>
          <EmptyContent>
            <Button onClick={handleOpenFile}>Open File</Button>
          </EmptyContent>
        </Empty>
      ) : (
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
      )}

      <Separator />

      <StatusBar />

      <SettingsDialog open={isSettingsOpen} onClose={handleCloseSettings} />
    </div>
  );
}

export default App;
