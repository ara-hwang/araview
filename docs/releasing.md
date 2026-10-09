# 릴리스와 업데이트

maintainer용 안내입니다. 사용자용 업데이트 확인 방법은 `usage.md#업데이트-확인`을 봅니다. 배포 규격 진실은 `../SPEC.md` §20을 따릅니다.

릴리스는 `npm run release -- <버전>` 한 줄로 시작합니다. 이 명령이 버전을 올려 커밋하고 `vX.Y.Z` 태그와 함께 푸시하면, `.github/workflows/release.yml`이 검증(fmt, test, clippy, 보안 감사, 라이선스 검사), 설치 프로그램 빌드, 서명, `latest-gpui.json` 생성, GitHub 릴리스 생성까지 자동으로 수행합니다. 공개 저장소라 GitHub 호스팅 표준 러너는 무료입니다. `.github/workflows/ci.yml`이 검증을 담당하며 릴리스 워크플로가 그대로 재사용합니다. PR과 main 푸시에서는 문서 정합성 검사가 자동으로 돌고, Rust 쪽 파일이 바뀌면 가벼운 `cargo check`도 돕니다. main 푸시에서 도는 것은 캐시 때문입니다. PR은 자기 ref와 main의 캐시만 복원할 수 있어서, main이 저장해 두어야 PR이 빈 캐시로 시작하지 않습니다. 전체 Rust 검사(cargo test/clippy, 보안 감사, 라이선스 검사)는 Actions 탭에서 수동 실행하거나 릴리스 때 돕니다.

## 태그로 자동 릴리스

```bash
npm run release -- 2.0.1          # 정확한 버전
npm run release -- patch          # patch | minor | major
npm run release -- patch --dry-run  # 계획만 출력
```

`scripts/release.mjs`가 다음을 순서대로 수행합니다.

1. `main` 브랜치, 깨끗한 작업 트리, `origin/main`과 동기화 상태, 태그 중복 여부를 확인합니다.
2. `src-gpui/Cargo.toml`과 `Cargo.lock`의 버전을 올립니다. `cargo metadata --locked`와 `npm run docs:check`로 검증하고, 실패하면 파일을 되돌립니다.
3. `chore: 버전 X.Y.Z`로 커밋하고 태그를 만든 뒤 `main`과 태그를 한 번에(`--atomic`) 푸시합니다. 푸시 전에 확인을 묻고, `--yes`로 생략할 수 있습니다.

- 트리거는 `v숫자.숫자.숫자` 형식의 태그 푸시뿐입니다(프리릴리스 접미사는 트리거하지 않습니다).
- 첫 단계에서 태그와 `src-gpui/Cargo.toml`, `Cargo.lock`의 버전이 모두 같은지, `TAURI_SIGNING_PRIVATE_KEY` Secrets가 있는지 확인하고, 어긋나면 빌드 전에 실패합니다.
- 릴리스 자산은 `AraView-<버전>-setup.exe`(NSIS 설치 프로그램)와 `latest-gpui.json`(업데이트 피드)입니다.
- 릴리스 본문은 `.github/RELEASE_TEMPLATE.md`를 채워 만듭니다. `{{CHANGES}}`(이전 태그 이후 커밋을 접두사로 분류한 목록), `{{COMPARE}}`(비교 링크) 자리표시자를 워크플로가 치환하므로, 문구를 바꾸려면 이 파일만 고치면 됩니다. 앱의 업데이트 대화상자에는 설치 안내를 뺀 변경 목록만 보입니다.
- 릴리스 노트는 커밋 제목의 접두사로 자동 분류됩니다. `feat`는 "새 기능", `fix`는 "수정", `refactor`와 `perf`는 "개선", `feat!:`처럼 느낌표가 붙은 커밋은 "변경"에 들어갑니다. `docs`, `chore`, `ci`, `build`, `test`, `style`과 머지 커밋은 제외됩니다. 제목이 그대로 노출되므로 사용자에게 읽히는 문장으로 씁니다.
- 저장소 Secrets에 `TAURI_SIGNING_PRIVATE_KEY`와 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`를 등록해야 합니다(아래 "서명키 발급과 등록"). 이름은 예전 Tauri 시절 그대로이고 키 형식은 minisign입니다.
- 실패하면 Actions에서 해당 실행을 재실행합니다. 코드를 고쳐야 하면 태그를 지우고(`git push --delete origin vX.Y.Z`, `git tag -d vX.Y.Z`) 고친 커밋에 다시 붙입니다. 이미 만들어진 릴리스가 있으면 함께 지웁니다.
- 저장소 기본 워크플로 권한이 read여도 `contents: write`를 job에서 직접 선언하므로 별도 토큰 없이 동작합니다. `RELEASE_TOKEN` 시크릿이 있으면 그것을 우선 씁니다.

## 태그 없이 파이프라인 시험

Actions 탭에서 Release 워크플로를 수동 실행(`workflow_dispatch`)하면 검증, 설치 프로그램 빌드, 서명, 피드 생성까지 같은 과정을 돌리고 릴리스는 만들지 않습니다. 설치 프로그램, `.sig`, `latest-gpui.json`, 릴리스 노트가 `release-dry-run` 아티팩트로 남습니다. 워크플로나 빌드 스크립트를 고친 뒤 태그를 달기 전에 쓰세요.

## 업데이트 피드

앱의 업데이트 확인은 인증 없이 읽을 수 있는 공개 저장소 릴리스의 자산을 씁니다. 소스 저장소가 공개이므로 원본 저장소가 곧 피드입니다.

- 피드 주소는 `https://github.com/ara-hwang/araview/releases/latest/download/latest-gpui.json`이고 앱 코드(`src-gpui/src/app/update.rs`)에 있습니다.
- `latest-gpui.json`은 `version`, `notes`, `pub_date`, `platforms.windows-x86_64.{url,signature}`를 가집니다. `signature`는 설치 프로그램의 `.sig` 내용이고, 앱은 내려받은 설치 프로그램을 `src-gpui/update-pubkey.txt`의 공개키로 확인한 뒤에만 실행합니다.
- 앱은 다운로드가 끝나면 설치 프로그램을 실행하고 종료합니다. 설치 프로그램 실행 이후의 실패(사용자 취소, 디스크 부족 등)는 앱이 이미 종료된 뒤라 앱에서 피드백할 수 없고 NSIS 창이 표시합니다.
- 1.x(Tauri) 앱은 `latest.json`을 보는데 2.0부터는 이 파일을 올리지 않습니다. 그래서 1.x의 자동 업데이트는 2.0을 받지 못하고, 사용자가 릴리스의 설치 프로그램을 직접 받아 설치해야 합니다. 같은 폴더에 덮어써서 설정과 최근 파일이 이어집니다.
- 예전 별도 피드 저장소(`araview-updates`)는 삭제했습니다. 0.2.9 이하 설치본은 그 피드를 보므로 자동 업데이트를 받지 못합니다.

## 서명키 발급과 등록 (maintainer 1회)

서명은 Tauri CLI의 `signer`로 하고(저장소에는 의존성을 두지 않고 `npx`로 받아 씁니다), 키 형식은 minisign입니다. 기존 1.x와 같은 키를 그대로 씁니다. 새로 만들어야 하면 다음과 같이 합니다.

```powershell
npx --yes @tauri-apps/cli@2.11.4 signer generate -w "$env:USERPROFILE\.tauri\araview.key"
```

- 키는 repo 밖(`$env:USERPROFILE\.tauri`)에 둡니다. 다른 폴더를 쓰려면 `$env:USERPROFILE` 대신 원하는 경로를 넣습니다. `*.key`는 `.gitignore`에 있습니다.
- 공개키(`araview.key.pub` 내용)는 `src-gpui/update-pubkey.txt`에 넣습니다. 키를 바꾸면 이전 키로 서명한 설치본의 업데이트 확인이 새 릴리스를 거부하므로, 키 교체는 사용자가 직접 새 설치 프로그램을 받아야 하는 변경입니다.
- 비밀키와 비밀번호는 repo Secrets에 등록합니다. CI에는 키 파일이 없으므로 비밀키는 파일 내용을 넣습니다.

```powershell
gh secret set TAURI_SIGNING_PRIVATE_KEY --repo ara-hwang/araview --body ((Get-Content -Raw "$env:USERPROFILE\.tauri\araview.key").Trim())
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo ara-hwang/araview
```

값 끝에 개행이 붙으면 base64 디코드가 실패하므로 `.Trim()`으로 공백을 제거합니다.

## 로컬 설치 프로그램

```powershell
pwsh scripts/Build-Installer.ps1
```

- 릴리스 빌드와 NSIS 설치 프로그램(`target/AraView-<버전>-setup.exe`)을 만듭니다. 서명과 업로드는 하지 않습니다.
- 설치 프로그램은 1.x 설치본과 같은 정체성(설치 폴더 `%LOCALAPPDATA%\AraView`, 실행 파일 `araview.exe`, 제거 항목 `AraView`, 시작 메뉴 바로가기 `AraView`)을 씁니다. 그래서 1.x 위에 설치하면 같은 자리를 덮어쓰고 기존 파일 연결, 탐색기 PSD 썸네일 등록, 바로가기가 그대로 유효합니다.
- 실제 설치본을 건드리지 않고 설치 흐름을 시험하려면 `-Suffix " Test"`로 정체성에 접미사를 붙여 만듭니다.
- 서명 없이 만든 설치 프로그램은 릴리스 배포에 쓸 수 없습니다.
