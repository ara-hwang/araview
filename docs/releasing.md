# 릴리스와 업데이트

maintainer용 안내입니다. 사용자용 업데이트 확인 방법은 `usage.md#업데이트-확인`을 봅니다. 배포 규격 진실은 `../SPEC.md` §20을 따릅니다.

릴리스 기본 경로는 태그 푸시다. 버전을 올린 커밋에 `vX.Y.Z` 태그를 푸시하면 `.github/workflows/release.yml`이 검증(테스트, 타입, 포맷, cargo test/clippy, 보안 감사, 라이선스 검사), 서명 빌드, `latest.json` 생성, GitHub 릴리스 생성까지 자동으로 수행한다. 공개 저장소라 GitHub 호스팅 표준 러너는 무료다. `npm run release:local`은 CI를 못 쓸 때의 대체 경로다. `.github/workflows/ci.yml`은 PR과 main 푸시에서 자동으로 돌지 않고, Actions 탭에서 수동 실행할 때만 같은 검사를 돌린다.

## 태그로 자동 릴리스

```powershell
# 1. 버전을 올리고 커밋한다(release_bump 도구 또는 4개 파일 직접 수정).
# 2. 푸시한 뒤 태그를 만든다.
git push origin main
git tag v1.0.1
git push origin v1.0.1
```

- 트리거는 `v숫자.숫자.숫자` 형식의 태그 푸시뿐이다(프리릴리스 접미사는 트리거하지 않는다).
- 첫 단계에서 태그와 `package.json`, `Cargo.toml`, `Cargo.lock`, `tauri.conf.json`의 버전이 모두 같은지, `TAURI_SIGNING_PRIVATE_KEY` Secrets가 있는지 확인하고, 어긋나면 빌드 전에 실패한다.
- 릴리스 본문은 `.github/RELEASE_TEMPLATE.md`를 채워 만든다. `{{CHANGES}}`(이전 태그 이후 커밋 목록), `{{COMPARE}}`(비교 링크) 자리표시자를 워크플로가 치환하므로, 문구를 바꾸려면 이 파일만 고친다.
- 저장소 Secrets에 `TAURI_SIGNING_PRIVATE_KEY`와 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`를 등록해야 한다(아래 "서명키 발급과 등록"). 두 값은 저장소 Settings의 Secrets and variables에서 넣는다.
- 실패하면 Actions에서 해당 실행을 재실행한다. 코드를 고쳐야 하면 태그를 지우고(`git push --delete origin vX.Y.Z`, `git tag -d vX.Y.Z`) 고친 커밋에 다시 붙인다. 이미 만들어진 릴리스가 있으면 함께 지운다.
- 저장소 기본 워크플로 권한이 read여도 `contents: write`를 job에서 직접 선언하므로 별도 토큰 없이 동작한다. `RELEASE_TOKEN` 시크릿이 있으면 그것을 우선 쓴다.

## 업데이트 피드

앱의 updater는 인증 없이 엔드포인트를 받아야 하므로 공개 저장소 릴리스의 자산을 씁니다. 소스 저장소가 공개이므로 원본 저장소가 곧 피드입니다.

```powershell
npm run release:local
```

- 릴리스 자산은 기본으로 `ara-hwang/araview`에 게시되고, `plugins.updater.endpoints`는 `https://github.com/ara-hwang/araview/releases/latest/download/latest.json`을 가리킵니다.
- 예전 별도 피드 저장소(`araview-updates`)는 삭제했다. 0.2.9 이하 설치본은 그 피드를 보므로 자동 업데이트를 받지 못하고, 1.0.0 이상 설치본을 직접 받아 설치해야 한다.
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

## 로컬에서 릴리스 (대체 경로)

CI를 쓸 수 없을 때 쓴다. 서명 빌드와 태그 푸시, GitHub 릴리스 생성까지 이 PC에서 수행한다. 이 스크립트가 태그를 푸시하면 `release.yml`도 같은 태그로 돌아 원본 저장소에 같은 버전의 릴리스를 함께 만든다. 원본 저장소에는 CI 산출물만 남기려면 로컬 대체 경로를 쓰지 말고 태그 푸시를 쓴다.

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
- 같은 태그에 릴리스가 이미 있으면 그 릴리스의 자산을 덮어씁니다(`--clobber`). 같은 태그를 CI도 처리하므로 원본 저장소로 올릴 때는 둘 중 하나만 쓴다.

## opencode 플러그인 도구

`.opencode/plugins/araview-release.ts`가 `release` 네임스페이스로 두 툴을 등록합니다. 둘 다 릴리지를 실행하지 않습니다.

- `release_preflight`: 실행 전 조건을 읽기 전용으로 검사해 READY/BLOCKED를 냅니다. 버전 4곳 일치, 작업 트리, 원격 동기화, 태그와 HEAD 관계, `gh` 로그인, 서명키 존재(경로만), 비밀번호 설정, updater endpoint가 가리키는 저장소, 기존 빌드 산출물을 봅니다. 서명키와 비밀번호 내용은 절대 출력하지 않습니다.
- `release_bump`: `level`(patch/minor/major) 또는 `version`으로 `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.lock`의 `araview` 항목을 한 번에 올립니다. 네 곳이 이미 어긋나 있거나 태그가 있으면 거부하고, downgrade도 막습니다. 커밋은 하지 않습니다.
- `docs_preflight`(`.opencode/plugins/araview-docs.ts`, `docs` 네임스페이스): `npm run docs:check`와 같은 문서 정합성 검사를 읽기 전용으로 실행해 출력을 그대로 돌려줍니다. 수정은 하지 않습니다.

```text
릴리즈 준비:
1. release_preflight 로 BLOCKED 항목부터 해결한다.
2. 버전이 올려야 하면 release_bump 로 올리고 커밋한다(태그 vX.Y.Z는 아직 없음).
3. 프리플라이트가 READY면 커밋을 푸시하고 `vX.Y.Z` 태그를 푸시한다(CI가 릴리스). CI를 못 쓸 때만 npm run release:local 을 실행한다.
```

- 플러그인 로드는 `.opencode/plugin/`과 `.opencode/plugins/` 양쪽 디렉터리에서 이루어지며, 파일은 default export에 `id`와 `setup`을 두어야 합니다. `setup`이 주는 `ctx.tool`은 런타임에만 있고 설치된 `@opencode-ai/plugin` 타입에는 아직 없어, 툴 도메인을 파일 안에서 좁게 선언해 캐스팅합니다.
- `editor.add`의 `name`은 필수입니다. 빠지면 등록이 조용히 버려지고 툴이 카탈로그에 뜨지 않습니다.
- 툴 실행 결과는 `{ content: string }`로 돌려야 합니다. 문자열이나 `{ output }`은 브리지에서 거부됩니다(`output`은 출력 스키마 선언이 있을 때만 허용).
- `.opencode/plugins/*.ts`는 `tsconfig.json`의 `include`가 `src`뿐이고 `.oxlintrc.json`/`.oxfmtrc.json`이 `.opencode`를 제외하므로 저장소 검사 대상이 아닙니다. opencode 자체 포맷터가 이 파일에 걸리면 `formatter exited unsuccessfully`가 로그에 남는데(oxfmt이 `.opencode`를 무시해서), 무해합니다.
