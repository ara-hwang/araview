import { useEffect } from "react"
import { listen } from "@tauri-apps/api/event"

export function useOpenFileListener(
  loadImage: (filePath: string) => Promise<void>
) {
  useEffect(() => {
    const unlistenPromise = listen<string>("open-file", (event) => {
      loadImage(event.payload)
    })
    return () => {
      // 언마운트 시 이벤트 리스너를 정리해 메모리 누수를 방지
      unlistenPromise.then((fn) => fn())
    }
  }, [loadImage])
}
