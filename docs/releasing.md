# 릴리스와 업데이트

maintainer용 안내입니다. 사용자용 업데이트 확인 방법은 `usage.md#업데이트-확인`을 봅니다. 배포 규격 진실은 `../SPEC.md` §20을 따릅니다.

릴리스는 `npm run release -- <버전>` 한 줄로 시작합니다. 이 명령이 버전을 올려 커밋하고 `vX.Y.Z` 태그와 함께 푸시하면, `.github/workflows/release.yml`이 검증(테스트, 타입, 포맷, cargo test/clippy, 보안 감사, 라이선스 검사), 서명 빌드, `latest.json` 생성, GitHub 릴리스 생성까지 자동으로 수행합니다. 공개 저장소라 GitHub 호스팅 표준 러너는 무료입니다. `.github/workflows/ci.yml`이 검증을 담당하며 릴리스 워크플로가 그대로 재사용합니다. PR과 main 푸시에서는 프론트엔드 검사만 자동으로 돌고, Rust 검사(cargo test/clippy, 보안 감사, 라이선스 검사)는 Actions 탭에서 수동 실행하거나 릴리스 때 돕니다.

## 태그로 자동 릴리스

```bash
npm run release -- 1.0.1          # 정확한 버전
npm run release -- patch          # patch | minor | major
npm run release -- patch --dry-run  # 계획만 출력
```

`scripts/release.mjs`가 다음을 순서대로 수행합니다.

1. `main` 브랜치, 깨끗한 작업 트리, `origin/main`과 동기화 상태, 태그 중복 여부를 확인합니다.
2. `package.json`, `package-lock.json`, `Cargo.toml`, `Cargo.lock`, `tauri.conf.json`의 버전을 올립니다. `cargo metadata --locked`와 `npm run docs:check`로 검증하고, 실패하면 파일을 되돌립니다.
3. `chore: 버전 X.Y.Z`로 커밋하고 태그를 만든 뒤 `main`과 태그를 한 번에(`--atomic`) 푸시합니다. 푸시 전에 확인을 묻고, `--yes`로 생략할 수 있습니다.

- 트리거는 `v숫자.숫자.숫자` 형식의 태그 푸시뿐입니다(프리릴리스 접미사는 트리거하지 않습니다).
- 첫 단계에서 태그와 `package.json`, `Cargo.toml`, `Cargo.lock`, `tauri.conf.json`의 버전이 모두 같은지, `TAURI_SIGNING_PRIVATE_KEY` Secrets가 있는지 확인하고, 어긋나면 빌드 전에 실패합니다.
- 릴리스 본문은 `.github/RELEASE_TEMPLATE.md`를 채워 만듭니다. `{{CHANGES}}`(이전 태그 이후 커밋을 접두사로 분류한 목록), `{{COMPARE}}`(비교 링크) 자리표시자를 워크플로가 치환하므로, 문구를 바꾸려면 이 파일만 고치면 됩니다.
- 릴리스 노트는 커밋 제목의 접두사로 자동 분류됩니다. `feat`는 "새 기능", `fix`는 "수정", `refactor`와 `perf`는 "개선", `feat!:`처럼 느낌표가 붙은 커밋은 "변경"에 들어갑니다. `docs`, `chore`, `ci`, `build`, `test`, `style`과 머지 커밋은 제외됩니다. 제목이 그대로 노출되므로 사용자에게 읽히는 문장으로 씁니다.
- 저장소 Secrets에 `TAURI_SIGNING_PRIVATE_KEY`와 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`를 등록해야 합니다(아래 "서명키 발급과 등록"). 두 값은 저장소 Settings의 Secrets and variables에서 넣습니다.
- 실패하면 Actions에서 해당 실행을 재실행합니다. 코드를 고쳐야 하면 태그를 지우고(`git push --delete origin vX.Y.Z`, `git tag -d vX.Y.Z`) 고친 커밋에 다시 붙입니다. 이미 만들어진 릴리스가 있으면 함께 지웁니다.
- 저장소 기본 워크플로 권한이 read여도 `contents: write`를 job에서 직접 선언하므로 별도 토큰 없이 동작합니다. `RELEASE_TOKEN` 시크릿이 있으면 그것을 우선 씁니다.

## 업데이트 피드

앱의 updater는 인증 없이 엔드포인트를 받아야 하므로 공개 저장소 릴리스의 자산을 씁니다. 소스 저장소가 공개이므로 원본 저장소가 곧 피드입니다.

- 릴리스 자산은 기본으로 `ara-hwang/araview`에 게시되고, `plugins.updater.endpoints`는 `https://github.com/ara-hwang/araview/releases/latest/download/latest.json`을 가리킵니다.
- 예전 별도 피드 저장소(`araview-updates`)는 삭제했습니다. 0.2.9 이하 설치본은 그 피드를 보므로 자동 업데이트를 받지 못하고, 1.0.0 이상 설치본을 직접 받아 설치해야 합니다.
- 앱은 다운로드가 끝나면 NSIS 설치 관리자를 실행하고 종료됩니다(updater 플러그인의 Windows 동작). 설치 관리자 실행 이후의 실패(사용자 취소, 디스크 부족 등)는 앱이 이미 종료된 뒤라 앱에서 피드백할 수 없고 NSIS 창이 표시합니다.

## 서명키 발급과 등록 (maintainer 1회)

```powershell
npm run tauri signer generate -- -w "$env:USERPROFILE\.tauri\araview.key"
```

- 키는 repo 밖(`$env:USERPROFILE\.tauri`)에 둡니다. 다른 폴더를 쓰려면 `$env:USERPROFILE` 대신 원하는 경로를 넣습니다.
- 공개키(`araview.key.pub` 내용)는 `src-tauri/tauri.conf.json`의 `plugins.updater.pubkey`에 넣습니다.
- 비밀키와 비밀번호는 repo Secrets에 등록합니다. CI에는 키 파일이 없으므로 비밀키는 파일 내용을 넣습니다.

```powershell
gh secret set TAURI_SIGNING_PRIVATE_KEY --repo ara-hwang/araview --body ((Get-Content -Raw "$env:USERPROFILE\.tauri\araview.key").Trim())
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo ara-hwang/araview
```

값 끝에 개행이 붙으면 base64 디코드가 실패하므로 `.Trim()`으로 공백을 제거합니다.

## 로컬 서명 빌드

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = "$env:USERPROFILE\.tauri\araview.key"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "<키 비밀번호>"
npm run tauri build
```

- `tauri build`에서 `TAURI_SIGNING_PRIVATE_KEY` 값이 존재하는 파일 경로면 그 파일을 읽고, 아니면 값을 키 내용으로 봅니다. 경로와 내용 둘 다 됩니다.
- `TAURI_SIGNING_PRIVATE_KEY_PATH`는 `tauri signer sign` 전용이라 `tauri build`에서는 무시됩니다.
- 비밀번호를 설정하지 않으면 대화형 프롬프트가 뜨므로 비대화형 환경에서는 반드시 설정합니다. (`--ci` 또는 `CI` 환경이면 빈 문자열로 처리)
- 성공하면 `src-tauri/target/release/bundle/nsis/` 아래에 설치본과 `.sig`가 함께 생성됩니다.
- `npm run tauri build`를 직접 실행하면 기본 features에 `dev-mcp`가 포함되어 설치본이 loopback MCP 브리지를 띄웁니다(플러그인 기본 base port 9223). 릴리스용 산출물은 CI가 `--no-default-features`로 빌드하므로 브리지가 빠집니다. 같은 방식으로 빌드하려면 `npm run tauri -- build -- --no-default-features`를 씁니다.
  @@CUT@@ `--no-default-features`로 빌드하므로 브리지가 빠집니다. 직접 빌드할 때도 맞추려면 `npm run tauri -- build -- --no-default-features`를 씁니다.

## 서명 없이 설치본만 만들기

```powershell
npm run tauri build -- --no-sign
```

updater 서명을 건너뜁니다. `.sig`가 없으므로 릴리스 배포에는 쓸 수 없습니다.
