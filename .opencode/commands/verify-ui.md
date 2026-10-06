---
description: Dev 앱 기동 후 Tauri MCP로 UI 실기동 검증 (스크린샷과 클릭스루)
---

Tauri MCP를 이용한 UI 런타임 검증을 수행한다. `$ARGUMENTS`가 있으면
해당 화면이나 조작에 집중하고, 없으면 아래 기본 체크리스트를 전부 수행한다.
폴백 경로는 없다. 2단계의 `driver-session start`가 실패하면
`get_setup_instructions`로 브리지를 복구하거나 세션을 재시작하고, 그래도
안 되면 검증을 조용히 건너뛰지 말고 실패 원인을 그대로 보고한다. MCP가 없어 보일 때의
점검 순서는 `docs/playbooks.md`의 "When Tauri MCP looks missing"을 따른다.

1. 기동: `npm run dev:up`을 실행한다. 이미 실행 중이면 즉시 READY가 반환된다.
   dev 브리지 포트는 `:9323` 고정이다(`src-tauri/src/lib.rs`). 기본값 9223을
   쓰지 않는 이유는 설치본이나 다른 Tauri 앱이 선점하면 dev 앱이 조용히 다른
   포트로 밀려 엉뚱한 앱을 검증하게 되기 때문이다.
2. 연결: `tauri-mcp driver-session start --port 9323`으로 세션을 시작한다
   (MCP 클라이언트에서는 `driver_session` start). `status`로
   plugin/server 버전 경고를 확인한다.
3. 대상 확인: `ipc-get-backend-state`(클라이언트: `ipc_get_backend_state`)로
   `environment.debug === true`와 identifier `com.araview.viewer.dev`를
   확인한다. `debug: false`이거나 identifier가 `com.araview.viewer`면 설치본에
   붙은 것이므로 즉시 중단하고 보고한다. 그 상태의 이후 결과는 신뢰하지 않는다.
4. 홈 스크린샷: `webview-screenshot`으로 촬영하고 레이아웃 회귀를 육안으로 확인한다.
5. 파일 열기: `webview-execute-js`로 `open-file` 이벤트를 emit한다
   (`samples/` 내 검증용 파일 사용). 네이티브 `Open` 버튼은 클릭하지 않는다.
6. 클릭스루 (각 단계마다 스크린샷으로 확인):
   - 썸네일 클릭 또는 방향키, `/image` 이동과 이미지 렌더 확인
   - `Zoom in` 버튼 클릭, 배율 텍스트 변경 확인
   - `I` 키(`webview-keyboard` press)로 EXIF 패널 개폐와 섹션 상태 확인
   - `Settings` 클릭 후 `Escape` 키로 닫기 (키보드 닫기 검증)
7. 콘솔: `read-logs --source console`으로 신규 에러를 확인한다.
   vite 접속 로그와 React DevTools 안내는 baseline이다.
8. 결과 보고: 조작별 PASS/FAIL 표로 보고하고, 실패가 있으면 스크린샷 근거와 함께 중단한다.

주의: Rust 재빌드는 앱을 재시작한다. 세션이 끊기면 `driver-session start`를
다시 실행한다.
주의: EXIF 패널은 이미지 변경 시 자동 새로고침되지 않는다.
현재 이미지 기준으로 검증하려면 `I`를 두 번 눌러 닫았다 연다.
