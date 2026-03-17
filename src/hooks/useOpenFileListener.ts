import { useEffect } from "react"
import { listen } from "@tauri-apps/api/event"

// Tauri 백엔드에서 emit 하는 "open-file" 이벤트를 구독해서
// OS/CLI에서 전달된 파일 경로를 자동으로 열어 주는 훅
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
