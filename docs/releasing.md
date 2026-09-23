# 릴리스와 업데이트

maintainer용 안내입니다. 사용자용 업데이트 확인 방법은 `usage.md#업데이트-확인`을 봅니다. 배포 규격 진실은 `../SPEC.md` §20을 따릅니다.

릴리스는 태그(`v*`) 푸시로 GitHub Releases에 발행되며, updater 아티팩트(`latest.json`, `.sig`)가 함께 첨부됩니다. 릴리스 본문은 이전 태그 이후 커밋 목록과 비교 링크로 자동 생성됩니다. 태그 푸시 시 릴리스 워크플로가 프런트/Rust 검사를 먼저 돌리고, 통과해야 빌드와 릴리스로 진행합니다. `.github/workflows/ci.yml`은 PR과 main 푸시에서 자동으로 돌지 않습니다. 같은 검사(린트, 커버리지 포함 테스트, 타입, 포맷, npm/cargo 보안 감사, cargo test/clippy)는 Actions 탭에서 해당 워크플로를 수동 실행할 때만 돌아갑니다.

릴리스 생성 권한은 `GITHUB_TOKEN`(`contents: write`)을 씁니다. 저장소 Settings → Actions → General의 워크플로 권한이 `Read and write`여야 하며, `read`로 유지하려면 `contents: write` 권한의 fine-grained PAT를 `RELEASE_TOKEN` 시크릿으로 등록하세요(워크플로가 자동으로 그것을 사용).

## 업데이트 피드 (소스 저장소가 비공개인 동안)

앱의 updater는 인증 없이 엔드포인트를 받아야 하므로, 소스가 비공개인 동안에는 공개 저장소의 릴리스에 자산을 게시합니다.

```powershell
npm run release:local
```

- 릴리스 자산은 기본으로 공개 피드 저장소 `ara-hwang/araview-updates`에 게시됩니다. 소스 저장소에만 올리려면 `-UpdatesRepo ara-hwang/araview`을 넘기세요.

- 피드(`latest.json`)와 설치본이 모두 `ara-hwang/araview-updates` 릴리스에 있고, `plugins.updater.endpoints`는 `https://github.com/ara-hwang/araview-updates/releases/latest/download/latest.json`을 가리킵니다.
- 소스 저장소를 공개로 전환하면 기본값과 endpoint를 원래 릴리스 URL로 되돌립니다. 엔드포인트는 앱에 포함되므로 그다음 릴리스부터 적용됩니다.
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
- `npm run tauri build`를 직접 실행하면 기본 features에 `dev-mcp`가 포함되어 설치본이 loopback MCP 브리지를 띄웁니다(플러그인 기본 base port 9223). 릴리스용 산출물은 `npm run release:local`이 CI와 동일하게 `--no-default-features`로 빌드하므로 브리지가 빠집니다. 직접 빌드할 때도 맞추려면 `npm run tauri -- build -- --no-default-features`를 씁니다.

## 서명 없이 설치본만 만들기

```powershell
npm run tauri build -- --no-sign
```

updater 서명을 건너뜁니다. `.sig`가 없으므로 릴리스 배포에는 쓸 수 없습니다.

## CI 없이 로컬에서 릴리스

CI 빌드가 오래 걸리거나 워크플로를 쓸 수 없을 때, 이 PC에서 빌드해 바로 릴리스할 수 있습니다.

```powershell
# .env.example을 .env.local로 복사해 키와 비밀번호를 채우면 환경변수 없이 동작합니다.
npm run release:local                    # 바로 공개
npm run release:local -- -Publish:$false  # 초안으로 남기기
```

- 키와 비밀번호는 `.env.local`(gitignored)에 두는 걸 권장합니다. `.env.example`을 복사해 값을 채우면 환경변수를 하나도 설정하지 않아도 됩니다. 이미 설정된 환경변수가 `.env.local`보다 우선합니다.
- 서명 키 탐색 순서: `-KeyPath` → `TAURI_SIGNING_PRIVATE_KEY`(`.env.local` 포함) → `araview.key`(저장소 루트) → `~/.tauri/araview.key`. 저장소에 복사본을 두려면 루트에 `araview.key`를 놓으면 되고, `*.key`가 `.gitignore`에 있어 커밋되지 않습니다.
- 비밀번호는 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`(`.env.local` 포함)로 전달합니다. 미설정이면 빌드가 프롬프트로 멈춥니다.
- `.env.local`은 Vite도 읽지만 Vite는 `VITE_` 접두사만 클라이언트로 노출하므로 `TAURI_` 값은 프런트엔드 번들에 포함되지 않습니다.

- `scripts/Publish-LocalRelease.ps1`이 `tauri.conf.json`의 버전으로 태그(`vX.Y.Z`)를 확인하고, 서명 빌드, `latest.json` 생성, `gh release create`(기존 릴리스에는 업로드)까지 수행합니다.
- 산출물(설치본, `.sig`, `latest.json`)은 저장소 안 `release/vX.Y.Z/`에 모입니다. `.gitignore`에 포함되어 커밋되지 않고, `-OutputDir`로 위치를 바꿀 수 있습니다.
- 작업 트리가 지저분하거나 태그가 HEAD와 다른 커밋을 가리키면 중단합니다.
- `.sig`의 키 ID가 `plugins.updater.pubkey`와 다르면 중단합니다.
- 빌드를 건너뛰고 이미 만든 산출물을 올리려면 `-SkipBuild`, GitHub를 건드리지 않고 결과만 확인하려면 `-DryRun`을 씁니다.
- 빌드가 코어를 모두 점유해 PC가 버벅이면 `-Jobs 4`처럼 cargo 병렬도를 낮춥니다(`CARGO_BUILD_JOBS`). 값이 클수록 빠르지만 부하가 커집니다.
- `release:local`은 `--no-default-features`로 빌드해 MCP 브리지(dev-mcp)를 산출물에서 제외합니다. 설치본으로 MCP UI 검증을 하는 경로는 없고, 그 목적은 dev 빌드(`npm run dev:up`, 브리지 `127.0.0.1:9323`)가 담당합니다.
- 같은 태그에 CI가 만든 릴리스가 이미 있으면 그 릴리스의 자산을 덮어씁니다(`--clobber`). CI와 로컬 중 한쪽만 쓰세요.
