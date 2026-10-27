# AraView

<img width="1600" height="1000" alt="viewer-light" src="https://github.com/user-attachments/assets/b000c143-89f8-4459-8588-9c2cb840a988" />

Rust와 [GPUI Kit](https://github.com/longbridge/gpui-kit)으로 만든 Windows, macOS, Linux 데스크톱 이미지 뷰어

- 일반 이미지 포맷과 만화 아카이브 포맷 지원 (`ComicInfo.xml` 포함)
- 양쪽 보기, 웹툰 보기 모드 등 지원

## 요구 사항

- Windows 11 (x64): 정식 지원
- macOS 11 이상, Linux x64(X11/Wayland): 실험적 지원(빌드는 확인하지만 실제 동작은 충분히 검증하지 않음)
- Rust stable(`rust-toolchain.toml`), `libheif` 1.21 이상(Windows는 vcpkg의 `libheif[libde265,aom]:x64-windows`), Node.js `>= 24`(릴리스·문서 검사 스크립트용)
- OS별 준비와 배포 묶음(Windows 설치 프로그램, macOS dmg, Linux tar.gz)은 `docs/development.md` 참고.

## 로컬 개발

```bash
git clone https://github.com/ara-hwang/araview.git
cd araview
cargo run -p araview -- "C:/path/to/image.png"   # macOS, Linux는 ~/Pictures/image.png 같은 경로
```

Windows는 `VCPKG_ROOT`가 vcpkg 설치 위치를 가리켜야 함.
macOS와 Linux는 `pkg-config`가 `libheif`를 찾아야 함.

자세한 환경은 `docs/development.md`를 확인.

## 문서

- 사용법(단축키, 설정, 업데이트 확인): `docs/usage.md`
- 개발 안내(환경, 실행, 스크립트, 구조): `docs/development.md`
- 릴리스와 업데이트(maintainer): `docs/releasing.md`
- 기여 안내: `CONTRIBUTING.md`
- 코드 작성 규칙: `CODING_STANDARDS.md`
- 작업별 절차(포맷 추가, 실행 중인 창을 조작하는 런타임 확인): `docs/playbooks.md`
- 제품 정의: `PRODUCT.md`
- 기능/기술 명세: `SPEC.md`
- 비주얼 시스템: `DESIGN.md`

## 라이선스

EUPL-1.2

외부 의존성 라이선스는 `THIRD_PARTY_LICENSES.md` 참고
