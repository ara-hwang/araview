---
description: Dev 앱 기동 후 agent-browser(WebView2 CDP)로 UI 실기동 검증 (스크린샷과 클릭스루)
---

agent-browser를 이용한 UI 런타임 검증을 수행한다. 상세 절차는
`.opencode/skills/agent-browser/SKILL.md`를 따른다. `$ARGUMENTS`가 있으면
해당 화면이나 조작에 집중하고, 없으면 아래 기본 체크리스트를 전부 수행한다.

1. 기동: WebView2 CDP 포트를 건 셸에서 `npm run dev:up`을 실행한다.
   이미 실행 중이면 즉시 READY가 반환된다.
   ```powershell
   $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9222"
   npm run dev:up
   ```
2. 연결: `http://127.0.0.1:9222/json/list`에서 WebView 타깃을 확인한다.
   이후 모든 명령에 `--cdp 9222`를 붙인다 (stateful `connect` 사용 금지).
   명명 세션(`$env:AGENT_BROWSER_SESSION`)을 반드시 사용한다.
3. 홈 스크린샷: `screenshot`으로 촬영하고 레이아웃 회귀를 육안으로 확인한다.
4. 파일 열기: `eval`로 `open-file` 이벤트를 emit한다
   (`samples/` 내 검증용 파일 사용). 네이티브 `Open` 버튼은 클릭하지 않는다.
5. 클릭스루 (각 단계마다 스크린샷으로 확인):
   - 썸네일 클릭 또는 방향키, `/image` 이동과 이미지 렌더 확인
   - `button[title="Zoom in (+)"]` 클릭, 배율 텍스트 변경 확인
   - `button[title="EXIF info (I)"]` 토글 또는 `I` 키, 패널 개폐와 섹션 상태 확인
   - `button[title="Settings"]` 클릭 후 `Escape` 키로 닫기 (키보드 닫기 검증)
6. 콘솔: `console` 명령으로 신규 에러를 확인한다.
   기존 base-ui ref warning은 제외한다.
7. 종료: `agent-browser close`로 세션을 닫는다 (dev 앱은 유지).
8. 결과 보고: 조작별 PASS/FAIL 표로 보고하고, 실패가 있으면 스크린샷 근거와 함께 중단한다.

주의: Rust 재빌드는 앱을 재시작하고 CDP 타깃을 무효화한다.
`/json/list`를 다시 조회하고 `--cdp 9222`로 계속한다 (포트는 유지된다).
주의: EXIF 패널은 이미지 변경 시 자동 새로고침되지 않는다.
현재 이미지 기준으로 검증하려면 `I`를 두 번 눌러 닫았다 연다.
