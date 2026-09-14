---
description: Dev 앱 기동 후 Tauri MCP로 UI 실기동 검증 (스크린샷과 클릭스루)
---

Tauri MCP를 이용한 UI 런타임 검증을 수행한다. `$ARGUMENTS`가 있으면
해당 화면이나 조작에 집중하고, 없으면 아래 기본 체크리스트를 전부 수행한다.
MCP 서버에 닿지 않을 때의 폴백 절차는
`.opencode/skills/agent-browser/SKILL.md`(WebView2 CDP)를 따른다.

1. 기동: `npm run dev:up`을 실행한다. 이미 실행 중이면 즉시 READY가 반환된다.
2. 연결: `tauri-mcp driver-session start --port 9223`으로 세션을 시작한다
   (MCP 클라이언트에서는 `driver_session` start). `status`로
   plugin/server 버전 경고를 확인한다.
3. 홈 스크린샷: `webview-screenshot`으로 촬영하고 레이아웃 회귀를 육안으로 확인한다.
4. 파일 열기: `webview-execute-js`로 `open-file` 이벤트를 emit한다
   (`samples/` 내 검증용 파일 사용). 네이티브 `Open` 버튼은 클릭하지 않는다.
5. 클릭스루 (각 단계마다 스크린샷으로 확인):
   - 썸네일 클릭 또는 방향키, `/image` 이동과 이미지 렌더 확인
   - `Zoom in` 버튼 클릭, 배율 텍스트 변경 확인
   - `I` 키(`webview-keyboard` press)로 EXIF 패널 개폐와 섹션 상태 확인
   - `Settings` 클릭 후 `Escape` 키로 닫기 (키보드 닫기 검증)
6. 콘솔: `read-logs --source console`으로 신규 에러를 확인한다.
   vite 접속 로그와 React DevTools 안내는 baseline이다.
7. 결과 보고: 조작별 PASS/FAIL 표로 보고하고, 실패가 있으면 스크린샷 근거와 함께 중단한다.

주의: Rust 재빌드는 앱을 재시작한다. 세션이 끊기면 `driver-session start`를
다시 실행한다.
주의: EXIF 패널은 이미지 변경 시 자동 새로고침되지 않는다.
현재 이미지 기준으로 검증하려면 `I`를 두 번 눌러 닫았다 연다.
