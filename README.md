# AraView

Rust와 [GPUI Kit](https://github.com/longbridge/gpui-kit)으로 만든 Windows 데스크톱 이미지 뷰어

- 일반 이미지 포맷과 만화 아카이브 포맷 지원
- 양쪽 보기, 웹툰 보기 모드 지원
- `ComicInfo.xml` 지원

## 요구 사항

- Windows 11 (x64), Rust stable(`rust-toolchain.toml`), vcpkg의 `libheif[core,aom]:x64-windows`, Node.js `>= 24`(릴리스·문서 검사 스크립트용)

## 로컬 개발

```bash
git clone https://github.com/ara-hwang/araview.git
cd araview
cargo run -p araview-gpui -- "C:/path/to/image.png"
```

`VCPKG_ROOT`가 vcpkg 설치 위치를 가리켜야 합니다. 자세한 환경은 `docs/development.md`를 봅니다.

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
