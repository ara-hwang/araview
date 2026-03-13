import { useEffect, useState } from "react";
import { Minus, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";

const appWindow = getCurrentWindow();

export function TitleBar() {
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setup = async () => {
      setIsMaximized(await appWindow.isMaximized());
      unlisten = await appWindow.onResized(async () => {
        setIsMaximized(await appWindow.isMaximized());
      });
    };

    setup();

    return () => {
      unlisten?.();
    };
  }, []);

  return (
    <div
      className="flex items-center justify-between h-8 bg-[hsl(var(--card))] border-b border-[hsl(var(--border))] flex-shrink-0 select-none"
      data-tauri-drag-region
    >
      <span
        className="px-3 text-xs font-medium text-[hsl(var(--muted-foreground))]"
        data-tauri-drag-region
      >
        Image Viewer
      </span>

      <div className="flex h-full">
        <button
          className="flex items-center justify-center w-11 h-full text-[hsl(var(--muted-foreground))] hover:bg-[hsl(var(--muted))] hover:text-[hsl(var(--foreground))] transition-colors"
          onClick={() => appWindow.minimize()}
          title="Minimize"
        >
          <Minus size={14} />
        </button>
        <button
          className="flex items-center justify-center w-11 h-full text-[hsl(var(--muted-foreground))] hover:bg-[hsl(var(--muted))] hover:text-[hsl(var(--foreground))] transition-colors"
          onClick={() => appWindow.toggleMaximize()}
          title={isMaximized ? "Restore" : "Maximize"}
        >
          <Square size={isMaximized ? 11 : 13} />
        </button>
        <button
          className="flex items-center justify-center w-11 h-full text-[hsl(var(--muted-foreground))] hover:bg-red-600 hover:text-white transition-colors"
          onClick={() => appWindow.close()}
          title="Close"
        >
          <X size={14} />
        </button>
      </div>
    </div>
  );
}
