import { useEffect } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import {
  deliverOpenFile,
  registerOpenFileHandler
} from "@/utils/openFileDelivery"

/**
 * 현재 라우트의 이미지 로더를 `open-file` 이벤트에 연결한다.
 * 라우트가 바뀌면 이전 등록은 자동 해제된다.
 */
export function useOpenFileListener(
  loadImage: (filePath: string) => Promise<void>
) {
  useEffect(() => {
    const unregister = registerOpenFileHandler((filePath) => {
      void loadImage(filePath)
    })
    return unregister
  }, [loadImage])
}

/**
 * 앱 루트에서 1회 실행한다. Tauri 리스너를 등록한 뒤 백엔드에 준비 완료를
 * 알려(`frontend_ready`), 시작 시 받은 파일 경로를 놓치지 않고 전달받는다.
 */
export function useOpenFileBridge() {
  useEffect(() => {
    let disposed = false
    const unlistenPromise = listen<string>("open-file", (event) => {
      deliverOpenFile(event.payload)
    })
    void unlistenPromise.then(() => {
      if (!disposed) void invoke("frontend_ready").catch(() => {})
    })
    return () => {
      disposed = true
      void unlistenPromise.then((unlisten) => unlisten())
    }
  }, [])
}
