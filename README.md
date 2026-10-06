# AraView

Tauri 2 + React 19 + TypeScript 기반의 Windows 데스크톱 이미지 뷰어

- 일반 이미지 포맷과 만화 아카이브 포맷 지원
- 양쪽 보기, 웹툰 보기 모드 지원
- `ComicInfo.xml` 지원

## 요구 사항

- Windows 11 (x64), Node.js `>= 24.15`, npm `>= 11.7`, Rust stable

## 로컬 개발

```bash
git clone https://github.com/ara-hwang/araview.git
cd araview
npm install
npm run tauri dev
```

## 문서

- 사용법(단축키, 설정, 업데이트 확인): `docs/usage.md`
- 개발 안내(환경, 실행, 스크립트, 구조): `docs/development.md`
- 릴리스와 업데이트(maintainer): `docs/releasing.md`
- 기여 안내: `CONTRIBUTING.md`
- 제품 정의: `PRODUCT.md`
- 기능/기술 명세: `SPEC.md`
- 비주얼 시스템: `DESIGN.md`

## 라이선스

EUPL-1.2

외부 의존성 라이선스는 `THIRD_PARTY_LICENSES.md` 참고
