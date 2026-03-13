import { useImageViewer } from "./hooks/useImageViewer";
import { Toolbar } from "./components/Toolbar";
import { ImageContainer } from "./components/ImageContainer";
import { StatusBar } from "./components/StatusBar";
import "./App.css";

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
    containerRef,
    imageRef,
    handleOpenFile,
    navigateImage,
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
        onOpenFile={handleOpenFile}
        onNavigate={navigateImage}
        onZoomIn={handleZoomIn}
        onZoomOut={handleZoomOut}
        onResetZoom={handleResetZoom}
        onFitWidth={handleFitWidth}
        onFitHeight={handleFitHeight}
        onFitScreen={handleFitScreen}
      />

      <ImageContainer
        image={image}
        loading={loading}
        error={error}
        zoom={zoom}
        isDragging={isDragging}
        position={position}
        containerRef={containerRef}
        imageRef={imageRef}
        onWheel={handleWheel}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
      />

      <StatusBar image={image} />
    </div>
  );
}

export default App;
