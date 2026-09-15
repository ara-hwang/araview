# 개발 안내

개발자용 안내입니다. 기능/기술 규격 진실은 `../SPEC.md`, 제품 정의는 `../PRODUCT.md`, 계획은 `../ROADMAP.md`를 따릅니다.

## 기술 스택

- **Frontend**: React 18, TypeScript, Vite 6, Tailwind CSS 4, TanStack Router v1, Zustand 5
- **Backend**: Rust, Tauri 2
- **Tauri Plugins**: `dialog`, `fs`, `opener`, `store`, `window-state`, `updater`, `process`

## 요구 사항

- **OS**: Windows 10/11 (x64)
- **Node.js**: `>= 22`
- **Rust**: stable (`1.94` 이상 권장)

HEIC/HEIF를 쓰려면 [vcpkg](https://vcpkg.io/)로 `libheif`를 설치합니다.

```powershell
vcpkg install "libheif[core]:x64-windows"
$env:VCPKG_ROOT = "<vcpkg root>"
$env:Path += ";$env:VCPKG_ROOT\installed\x64-windows\bin"
```

`[core]`는 HEVC 디코더 `libde265`만 넣고, 인코더 `x265`는 빼는 설치입니다. 앱은 HEIC를 인코드하지 않습니다.

앱은 `libheif`를 동적 링크합니다. `embedded-libheif` Cargo 기능은 켜지 마세요. 설치본은 vcpkg DLL이 실행 파일 옆에 있어야 합니다. `VCPKG_ROOT`가 있으면 `src-tauri/build.rs`가 `heif.dll`과 `libde265.dll`을 복사합니다.

## 시작하기

```bash
git clone https://github.com/ara-hwang/araview.git
cd araview
npm install
npm run tauri dev
```

`npm run tauri dev` 실행 시 Vite(`http://localhost:1420`)와 Tauri 앱이 함께 실행됩니다.

Tauri MCP로 UI를 검증할 때는 `npm run dev:up`을 씁니다. 이 스크립트는 `src-tauri/tauri.dev.conf.json`을 함께 적용해 개발 빌드에서만 `withGlobalTauri`를 켭니다(프로덕션은 꺼져 있고 CSP가 적용됩니다). MCP 브리지도 이때 loopback(`127.0.0.1:9223`)에만 바인딩됩니다.

개발(디버그) 빌드는 확장자 연결이 설치 버전과 섞이지 않도록 `AraView (Dev)` 이름으로 등록됩니다. 기본 앱 목록에서 `AraView`(설치 버전)와 구분해 선택하세요.

## 스크립트

```bash
# 개발
npm run tauri dev

# 프론트엔드 빌드 (tsc + vite)
npm run build

# 데스크톱 앱 빌드
npm run tauri build

# 테스트
npm test
npm run test:watch

# 타입 체크 / 포맷
npx tsc --noEmit
npx prettier --check "src/**/*.{ts,tsx}"
npx prettier --write "src/**/*.{ts,tsx}"

# 러스트 테스트 / 린트
cd src-tauri && cargo test
cd src-tauri && cargo clippy
```

빌드 결과물은 `src-tauri/target/release/bundle/` 아래에 생성됩니다.

## 동작 구조 요약

- 프론트엔드는 Tauri IPC로 백엔드 명령을 호출합니다. 전체 계약은 `SPEC.md` §16이 진실입니다.
- 백엔드는 파일 경로/메타데이터를 반환하고, 프론트는 `convertFileSrc`로 렌더링합니다.
- 파일 연동으로 앱이 실행되면 `open-file` 이벤트를 통해 대상 파일을 자동 오픈합니다.

## 프로젝트 구조

```text
src/
  components/        # UI 컴포넌트
  hooks/             # 비즈니스 로직 훅
  routes/            # TanStack Router 라우트
  store/             # Zustand 상태 저장소
  constants/         # 지원 확장자 등 상수
  types/             # 공용 타입

src-tauri/
  src/commands.rs    # Tauri command
  src/file_assoc.rs  # Windows 확장자 연결(레지스트리)
  src/image.rs       # MIME/확장자 판별, load_viewable
  src/heif.rs        # HEIC/HEIF 디코드 및 JPEG sidecar
  src/psd_sidecar.rs # PSD 합성 디코드(`psd` 크레이트) 및 JPEG sidecar (읽기 전용)
  src/process_temp.rs # 프로세스 수명 임시 디렉터리
  src/archive.rs     # 아카이브 목록/추출 처리 (cbz/zip, cb7/7z, cbr/rar, cbt)
  src/lib.rs         # Tauri 앱 설정 및 command 등록
```
