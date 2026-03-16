import "./App.css";

import { useImageViewer } from "./hooks/useImageViewer";
import Toolbar from "./components/Toolbar";
import { ImageContainer } from "./components/ImageContainer";
import { StatusBar } from "./components/StatusBar";
import { SettingsDialog } from "./components/SettingsDialog";
import { Empty, EmptyContent } from "./components/ui/empty";
import { Button } from "./components/ui/button";

function App() {
  const {
    image,
    dirImages,
    currentIndex,
    zoom,
    isDragging,
    position,
    loading,
    error,
    settings,
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
    handleSettingsChange,
  } = useImageViewer();

  return (
    <div
      className="flex flex-col h-screen w-screen"
      onContextMenu={handleContextMenu}
      onDrop={handleDrop}
      onDragOver={handleDragOver}
    >
      <Toolbar
        dirImages={dirImages}
        currentIndex={currentIndex}
        zoom={zoom}
        settings={settings}
        onOpenFile={handleOpenFile}
        onNavigate={navigateImage}
        onZoomIn={handleZoomIn}
        onZoomOut={handleZoomOut}
        onResetZoom={handleResetZoom}
        onFitWidth={handleFitWidth}
        onFitHeight={handleFitHeight}
        onFitScreen={handleFitScreen}
        onOpenSettings={handleOpenSettings}
      />

      {!image ? (
        <Empty>
          <EmptyContent>
            <Button onClick={handleOpenFile}>Open File</Button>
          </EmptyContent>
        </Empty>
      ) : (
        <ImageContainer
          image={image}
          loading={loading}
          error={error}
          zoom={zoom}
          isDragging={isDragging}
          position={position}
          background={settings.background}
          containerRef={containerRef}
          imageRef={imageRef}
          onWheel={handleWheel}
          onMouseDown={handleMouseDown}
          onMouseMove={handleMouseMove}
          onMouseUp={handleMouseUp}
          dirImages={dirImages}
          currentIndex={currentIndex}
          settings={settings}
          onNavigate={navigateImage}
          onNavigateToIndex={navigateToIndex}
        />
      )}

      <StatusBar image={image} />

      <SettingsDialog
        open={isSettingsOpen}
        settings={settings}
        onClose={handleCloseSettings}
        onSettingsChange={handleSettingsChange}
      />
    </div>
  );
}

export default App;
