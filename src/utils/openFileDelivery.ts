// open-file 이벤트 전달 브리지.
//
// 백엔드는 CLI 인자/두 번째 실행으로 받은 파일 경로를 `open-file` 이벤트로
// 보낸다. 이벤트가 도착한 시점에 현재 라우트의 로더가 아직 등록되지 않았을
// 수 있으므로(앱 시작 직후, 라우트 전환 중) 경로를 보류했다가 다음 등록에
// 전달한다.

export type OpenFileHandler = (filePath: string) => void

let handler: OpenFileHandler | null = null
let pendingPath: string | null = null

/** 라우트가 자신의 로더를 등록한다. 보류된 경로가 있으면 즉시 전달한다. */
export function registerOpenFileHandler(next: OpenFileHandler): () => void {
  handler = next
  if (pendingPath !== null) {
    const path = pendingPath
    pendingPath = null
    next(path)
  }
  return () => {
    // 이미 다른 핸들러로 교체됐다면 해제하지 않는다.
    if (handler === next) handler = null
  }
}

/** 백엔드 이벤트를 현재 핸들러에 전달한다. 핸들러가 없으면 보류한다. */
export function deliverOpenFile(path: string): void {
  if (handler) handler(path)
  else pendingPath = path
}

/** 핸들러 등록 전에 보류된 연결 실행 경로가 있는지 */
export function hasBufferedOpenFile(): boolean {
  return pendingPath !== null
}

/** 테스트 전용 상태 초기화. */
export function resetOpenFileDelivery(): void {
  handler = null
  pendingPath = null
}
