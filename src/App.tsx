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
    handleOpenFile,
    navigateImage,
    handleZoomIn,
    handleZoomOut,
    handleResetZoom,
    handleWheel,
    handleMouseDown,
    handleMouseMove,
    handleMouseUp,
    handleDrop,
    handleDragOver,
  } = useImageViewer();

  return (
    <div
      className="flex flex-col h-screen w-screen"
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
      />

      <ImageContainer
        image={image}
        loading={loading}
        error={error}
        zoom={zoom}
        isDragging={isDragging}
        position={position}
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
