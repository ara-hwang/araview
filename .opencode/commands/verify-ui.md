---
description: 개발 앱을 띄워 화면과 조작을 직접 확인 (스크린샷과 액션 파이프)
---

개발 빌드의 UI를 직접 확인한다. `$ARGUMENTS`가 있으면 해당 화면이나 조작에 집중하고, 없으면
아래 기본 체크리스트를 전부 수행한다. 사용자의 마우스를 허락 없이 움직이지 않는다.
절차의 상세는 `docs/playbooks.md`의 "Runtime check"를 따른다.

1. 기동: `cargo run -p araview-gpui -- samples/exif-sample.jpg`로 개발 빌드를 띄운다
   (식별자 `com.araview.viewer.dev`). 설치본(`com.araview.viewer`)이 대상이면 즉시 중단하고 보고한다.
2. 홈: 인자 없이 실행해 홈 화면 스크린샷을 찍고 레이아웃 회귀를 육안으로 확인한다.
3. 파일 열기: `samples/`의 이미지와 `sample-comicinfo.cbz`를 연다. 헤드리스로 대신할 수 있는
   조작은 `cargo test -p araview-gpui`가 맡는다.
4. 조작: `araview.exe action:<id>`로 동작을 실행하고 단계마다 스크린샷으로 확인한다.
   - 다음/이전 페이지, 확대/축소, 보기 모드 전환(`viewLtr` 등), 정보 패널(`toggleExif`)
   - 설정(`openSettings`)을 열어 각 탭을 확인하고 `Escape`로 닫기
5. 결과 보고: 조작별 PASS/FAIL 표로 보고하고, 실패가 있으면 스크린샷 근거와 함께 중단한다.

주의: 디버그 빌드만 `action:` 통로를 가진다. 릴리스 빌드에는 없다.
