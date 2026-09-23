# 업데이트 프로그레스 바 문제 검토 및 해결 방안

> 상태: 구현 완료. 기준 소스: `main` 최신(0.2.3).
> 구현 메모: 계획의 미확정 2건은 모두 계획안을 따랐다(4: resolve 시 추가 동작 없음, 5: `n / total (pct%)` 한 줄 통합).
> 계획 대비 추가한 것: (a) 보조 문구에 총 길이를 쓰기 위해 store에 `totalBytes: number | null`을 함께 두었다(계획은 `downloadedBytes`만 명시).
> (b) 보조 문구에 pct를 통합하면서 `dialog.update.downloadingPct`와 `toast.update.*` 잔여 키 전부를 제거했다.
> (c) installing 단계의 인계 실패 문구로 `dialog.update.installFail`을 추가했다.
> 실전 installing 흐름(서명된 상위 릴리스로 NSIS 인계/자동 재실행)은 이번 변경의 게이트에서 제외했고, 단위 테스트로 단계 전이만 검증했다.
> 관련 문서: `SPEC.md` §20.1(자동 업데이트), `docs/usage.md`(업데이트 확인), `docs/releasing.md`.
> 대상 코드: `src/hooks/useUpdater.ts`, `src/store/updateStore.ts`, `src/components/UpdateDialogs.tsx`, `src/i18n/locales/{ko,en}.json`.
> 근거 크레이트: `tauri-plugin-updater 2.12.0`(`src-tauri/Cargo.lock`), 로컬 소스 `~/.cargo/registry/src/.../tauri-plugin-updater-2.12.0/src/updater.rs`.

## 1. 현재 구현 요약

- 진입점(설정 `지금 확인`, 명령 팔레트) → `checkForUpdatesNow()` → 새 버전이 있으면
  `UpdateDialogs`의 available 단계.
- `startUpdateDownload()`가 `update.downloadAndInstall(onEvent)`를 호출하고
  `Started/Progress/Finished` 이벤트로 `pct`(0-100, 총 길이를 모르면 null)를 갱신한다.
  null이면 1/3 너비 `animate-pulse` 바(불확정 표시)를 보여준다.
- `downloadAndInstall`이 resolve되면 `markReady()` → ready 단계에서
  `지금 다시 시작`(`relaunch()`)/`닫은 후 적용` 버튼을 보여준다.

## 2. 문제 검토

### 2.1 (핵심) 설치 단계에서 앱이 예고 없이 종료된다

`tauri-plugin-updater`의 Windows NSIS 설치 경로는 다음과 같다
(`updater.rs` 2.12.0 기준):

1. `download()` → `Started/Progress/Finished` 이벤트 송신(`commands.rs` 190-205행).
2. 서명 검증(`verify_signature`) → `extract()`로 `*-setup.exe.zip` 해제.
3. `ShellExecuteW`로 NSIS 설치 관리자를 실행한 뒤 **`std::process::exit(0)`으로
   현재 프로세스를 즉시 종료**한다(`updater.rs` 882행. doc 주석 755행:
   "Windows: This function exits the app after launching the updater installer successfully").

따라서 Windows에서 JS 측 `downloadAndInstall()` promise는 **절대 resolve되지 않는다**
(IPC 응답이 프로세스 종료로 유실). 결과:

- `markReady()`와 ready 단계(`지금 다시 시작`/`닫은 후 적용`)는 실제 환경에서
  도달할 수 없는 죽은 UI다. `relaunchAfterUpdate`, `toast.update.deferred`,
  `dialog.update.ready*` 문자열도 마찬가지다.
- 사용자가 보는 마지막 화면은 "업데이트 다운로드 중 100%"(또는 총 길이를 모르면
  불확정 바)인 채로 창이 갑자기 사라지는 것이다. 정상적인 설치 인계가
  크래시처럼 보인다.
- 실제 재시작은 NSIS가 담당한다. 기본값이 `install_mode = Passive`(`config.rs`
  21-22행, NSIS `/P`)이고 `restart_after_install = true`(`updater.rs` 197행)라서
  설치 진행은 NSIS의 네이티브 진행 창이, 재실행은 `/R` 인자가 처리한다. 즉
  ready 단계의 수동 재시작 버튼은 설계가 플러그인 동작과 어긋난다.

### 2.2 다운로드 완료 → 설치 관리자 실행 사이의 무피드백 구간

`Finished` 이벤트 이후에도 서명 검증과 zip 해제, 임시 디렉터리 기록이 남아 있다.
이 구간 동안 바는 100%에서 "업데이트 다운로드 중" 문구 그대로 멈추고, 이어서
앱이 종료된다. "설치를 시작한다"는 안내가 없다.

### 2.3 불확정(indeterminate) 표현의 품질 문제

- `pct === null`일 때 `w-1/3 animate-pulse`는 위치 이동 없이 투명도만 깜빡이는
  정적 막대다. 진행 중임을 나타내는 일반적인 indeterminate 패턴이 아니다.

- `animate-pulse`에는 `prefers-reduced-motion` 게이팅이 없다. 저장소 규칙은
  모션 변경에 reduced-motion 처리를 함께 내도록 요구한다(`AGENTS.md`).
- 서버가 `Content-Length`를 주지 않으면 다운로드 내내 수치 정보가 전혀 없다.
  누적 바이트는 이미 계산하고 있지만(`useUpdater.ts`의 `downloaded`) UI에
  노출하지 않는다.

### 2.4 문서/문자열 불일치

- `SPEC.md` §20.1: "있으면 다운로드 및 설치 → 완료 시 다시 시작"은 수동 재시작을
  암시하지만 실제로는 NSIS가 자동 재시작한다.
- `docs/usage.md`: "토스트에서 다운로드 및 설치 후 다시 시작할 수 있습니다"는
  Dialog 기반 현재 UI와 어긋난다.
- `toast.update` 아래에 사용되지 않는 잔여 키가 남아 있다(`install`, `installed`,
  `restart`, `installFail` 등. 실제 사용은 `latest`, `checkFail`, `deferred`,
  `restartFail`뿐이며 `deferred`/`restartFail`도 3.4의 정리 대상이다).

### 2.5 플러그인 한계로 수용할 사항

NSIS 실행(`ShellExecuteW` 반환값 ≤ 32)까지의 실패는 promise reject로 돌아와
기존 에러/재시도 UI가 동작한다. 그러나 NSIS 내부 실패(사용자 취소, 디스크 부족)는
앱이 이미 종료된 뒤라 앱에서 피드백할 수 없다. NSIS passive 창이 실패를 보여주므로
수용하고 문서에만 명시한다.

## 3. 해결 방안

### 3.1 단계 모델에 `installing` 추가

`UpdateStage`를 `idle | available | downloading | installing`으로 바꾼다
(ready는 3.4에서 제거).

- `updateStore.ts`: `markInstalling()` 추가(`{ stage: "installing", pct: 100, error: null }`).
- `useUpdater.ts`: `Finished` 이벤트 처리에서 `markInstalling()`을 호출한다.
  현재의 "total이 있으면 100으로 마무리" 로직은 `markInstalling()`으로 대체된다.
- `downloadAndInstall`의 resolve 경로는 Windows에서 도달 불가이므로 별도 stage
  변경 없이 방어 코드로만 둔다.
- 닫기 가드(`dismissUpdate`, `handleOpenChange`, `showCloseButton`)에
  `installing`도 다운로드 중과 동일하게 닫기 불가 조건을 추가한다.

### 3.2 `installing` 단계 UI

- 바는 100%로 고정하고, 문구로 인계 사실을 알린다.
  - ko `dialog.update.installingTitle`: `설치를 시작합니다`
  - ko `dialog.update.installingDesc`: `다운로드가 완료되었습니다. 앱이 잠시 닫히고 설치가 진행된 뒤 새 버전이 자동으로 실행됩니다.`
  - en도 동일 의미로 추가.
- 버튼과 닫기(X)를 제공하지 않는다. 이 상태에서 `ShellExecuteW` 실패로
  promise가 reject되면 `error`가 설정되므로, 에러 표시와 `취소`/`다시 시도`
  버튼은 downloading/installing 공통으로 렌더한다(재시도 시 `startDownload()`가
  stage를 downloading으로 되돌리는 현행 흐름 재사용).

### 3.3 불확정 표시 개선

- `animate-pulse` 대신 좌→우로 이동하는 indeterminate 키프레임을
  `src/App.css`의 `@theme`에 추가한다(예: `--animate-progress-indeterminate`,
  `ease-motion-in-out`, 1.2s 무한 반복)하고, `@media (prefers-reduced-motion:
reduce)`에서 애니메이션을 끄고 정적 바로 대체한다.
- store에 `downloadedBytes: number`를 추가하고 `Progress` 핸들러에서 누적한다.
  총 길이가 있으면 `12.3 MB / 45.6 MB (27%)`, 없으면 `12.3 MB 다운로드됨`을
  보조 문구로 표시한다. 포맷은 기존 `src/utils/format.ts`의 `formatFileSize`를
  재사용한다. `Started`에서 0으로 초기화한다.

### 3.4 데드 코드와 문서 정리

- ready 단계 제거: `UpdateStage`에서 `ready` 삭제, `markReady()` 삭제,
  `UpdateDialogs`의 ready 분기와 `data-testid="update-restart-now"`/
  `update-apply-on-exit` 삭제, `relaunchAfterUpdate()` 삭제.
- `@tauri-apps/plugin-process` 의존 제거: `package.json`,
  `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`의 `plugin(tauri_plugin_process::init())`,
  `src-tauri/capabilities/default.json`의 `process:default`,
  `THIRD_PARTY_LICENSES.md`의 해당 행. 다른 사용처가 없음을 검색으로 확인했다
  (`relaunch`는 `useUpdater.ts`가 유일).
- i18n: `dialog.update.ready*` 4개와 미사용 `toast.update.*` 잔여 키를 정리하고,
  `deferred` 토스트도 ready 제거와 함께 삭제한다(`dismissUpdate(deferred)` 인자
  자체를 제거).
- 문서: `SPEC.md` §20.1의 흐름을 "다운로드 → 설치 관리자 인계 후 앱 종료 →
  NSIS가 설치와 재실행"으로 수정, `docs/usage.md`의 토스트 표현을 Dialog 기반으로
  수정, `docs/releasing.md`에 NSIS 내부 실패는 앱 피드백 불가(2.5)를 한 줄 추가.

### 3.5 변경 지점 목록

1. `src/store/updateStore.ts`: stage/`markInstalling`/`downloadedBytes` 추가, ready 삭제.
2. `src/hooks/useUpdater.ts`: `Finished` → `markInstalling()`, 바이트 누적,
   `relaunchAfterUpdate`/`deferred` 제거.
3. `src/components/UpdateDialogs.tsx`: installing 분기, 공통 에러 UI,
   불확정 바 클래스 교체, 닫기 가드 확장.
4. `src/App.css`: indeterminate 키프레임 + reduced-motion 폴백.
5. `src/i18n/locales/ko.json`, `en.json`: installing 키 추가, ready/미사용 키 삭제.
6. `package.json`, `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`,
   `src-tauri/capabilities/default.json`, `THIRD_PARTY_LICENSES.md`: plugin-process 제거.
7. `SPEC.md` §20.1, `docs/usage.md`, `docs/releasing.md`: 흐름/표현 동기화.
8. 테스트: `src/store/updateStore.test.ts`, `src/hooks/useUpdater.test.ts`,
   `src/components/UpdateDialogs.test.tsx`, `src/i18n/i18n.test.ts`(키 파리티).

## 4. 검증

- 단위 테스트:
  - `Finished` 이벤트에서 stage가 `installing`, `pct`가 100이 된다.
  - `contentLength`가 null이면 다운로드 중 `pct`는 null을 유지하고
    `downloadedBytes`만 누적된다.
  - installing 단계에서 Dialog 닫기 시도가 무시되고, 에러 설정 시
    `취소`/`다시 시도`가 노출된다.
  - ready 관련 기존 테스트(`markReady`, `relaunchAfterUpdate`, deferred 토스트)는 삭제.
- 공통 게이트: `npm test`, `npx tsc --noEmit`, `npm run lint:fix`, `npm run format`,
  Rust 의존 변경이 있으므로 `cd src-tauri && cargo test && cargo clippy && cargo fmt`.
- 런타임(한계 명시): installing 실전 흐름은 서명된 더 최신 릴리스가 있어야만
  재현된다. `docs/releasing.md`의 `Publish-LocalRelease`로 현재 버전보다 높은
  더미 태그를 만들어 검증할 수 있으나 이는 별도 릴리스 작업이므로 이 변경의
  필수 게이트로 두지 않는다. 대신 `npm run dev:up` + Tauri MCP로 available
  다이얼로그까지의 회귀(설정 `지금 확인` 진입, 최신 토스트 또는 다이얼로그
  표시)와 단위 테스트로 단계 전이를 검증하고, 실 서명 흐름 미검증 사실을
  보고한다.

## 5. 결정 사항

### 확정 (계획안)

- (1) ready 단계와 `relaunchAfterUpdate`는 제거한다. Windows 전용 제품에서
  플러그인이 `process::exit(0)`으로 종료하고 NSIS가 재실행하므로 수동 재시작 UI는
  영구히 도달 불가하다.
- (2) `installing` 단계를 신설해 종료 전 사용자에게 인계 사실을 안내한다.
- (3) 불확정 바는 이동형 키프레임 + reduced-motion 폴백 + 누적 바이트 표시로
  교체한다.

### 미확정 (계획안을 따름)

- (4) `downloadAndInstall` resolve 시(Windows에서 도달 불가) 별도 완료 처리가
  필요한가? (계획안: 아니오. 아무 동작도 추가하지 않는다.)
- (5) 보조 문구의 바이트 표기를 총 길이가 있을 때도 항상 보여줄 것인가?
  (계획안: 보여준다. `n / total (pct%)` 한 줄로 통합.)
