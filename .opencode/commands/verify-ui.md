---
description: Dev 앱 기동 후 Tauri MCP로 UI 실기동 검증 (스크린샷과 클릭스루)
---

Tauri MCP를 이용한 UI 런타임 검증을 수행한다. `$ARGUMENTS`가 있으면 해당 화면이나 조작에 집중하고, 없으면 아래 기본 체크리스트를 전부 수행한다.

1. 기동: bash로 `npm run dev:up`을 실행한다. 이미 실행 중이면 즉시 READY가 반환된다.
2. 연결: `driver_session` status를 확인하고, 연결이 없으면 start한다.
3. 홈 스크린샷: `webview_screenshot`으로 촬영하고 레이아웃 회귀를 육안으로 확인한다.
4. 클릭스루 (각 단계마다 스크린샷으로 확인):
   - 최근 파일 썸네일 클릭, `/image` 이동과 이미지 렌더 확인
   - `button[title="Zoom in (+)"]` 클릭, 배율 텍스트 변경 확인
   - `button[title="EXIF info (I)"]` 토글, 패널 개폐와 empty 상태 확인
   - `button[title="Settings"]` 클릭 후 `Escape` 키로 닫기 (키보드 닫기 검증)
5. 콘솔: `read_logs` (source console)로 신규 에러를 확인한다. 기존 base-ui ref warning은 제외한다.
6. 결과 보고: 조작별 PASS/FAIL 표로 보고하고, 실패가 있으면 스크린샷 근거와 함께 중단한다.

주의: `Open` 버튼은 네이티브 파일 다이얼로그를 띄우므로 클릭하지 않는다.
