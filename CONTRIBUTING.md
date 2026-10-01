# 기여 안내

AraView에 관심을 가져 주셔서 감사합니다. 버그 제보와 제안은 [이슈](https://github.com/ara-hwang/araview/issues)로 받습니다. 보안 취약점은 공개 이슈 대신 [비공개 제보](https://github.com/ara-hwang/araview/security/advisories/new)를 이용해 주세요.

## 코드 기여

코드를 보내기 전에 이슈로 먼저 방향을 이야기해 주세요. 제품 범위와 맞지 않는 변경은 받지 못할 수 있습니다. 작은 수정(오타, 명확한 버그 수정)은 바로 PR을 열어도 됩니다.

개발 환경과 구조는 [docs/development.md](docs/development.md)를 봅니다. 제품 방향은 [PRODUCT.md](PRODUCT.md), 기능 명세는 [SPEC.md](SPEC.md)가 기준입니다.

## PR 전에 확인할 것

```bash
npx tsc --noEmit
npm run lint
npm run format:check
npm test
cd src-tauri && cargo fmt --check && cargo clippy && cargo test
```

- 지원 포맷이나 IPC 명령, 설정 키를 바꾸면 `npm run docs:check`가 통과하도록 `SPEC.md`와 문서도 함께 고칩니다.
- 화면을 바꾼 PR은 변경 전후 스크린샷을 첨부해 주세요.
- 하나의 PR에는 하나의 목적만 담아 주세요.

## 커밋 메시지

`type: 한글 요약` 형식을 씁니다. type은 `feat`, `fix`, `docs`, `refactor`, `test`, `build`, `ci`, `chore` 중에서 고릅니다.

```text
fix: 양쪽 보기에서 도크와 그리드가 두 장을 모두 하이라이트하도록 수정
```

릴리스 노트는 커밋 제목으로 자동 생성됩니다. `feat`는 새 기능, `fix`는 수정, `refactor`와 `perf`는 개선으로 들어가고, 지원 포맷 제거처럼 사용자에게 영향이 있는 변경은 `feat!:`처럼 느낌표를 붙이면 "변경"으로 분류됩니다. 나머지 type은 노트에서 빠집니다. 제목이 그대로 노출되므로 사용자가 읽을 문장으로 써 주세요.

## 라이선스

기여한 코드는 이 저장소와 같은 [EUPL-1.2](LICENSE)로 배포됩니다.
