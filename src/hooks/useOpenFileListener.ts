import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";

export function useOpenFileListener(
  loadImage: (filePath: string) => Promise<void>,
) {
  useEffect(() => {
    const unlistenPromise = listen<string>("open-file", (event) => {
      loadImage(event.payload);
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [loadImage]);
}
