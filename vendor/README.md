# vendor

루트 `Cargo.toml`의 `[patch.crates-io]`로 덮어쓰는 업스트림 크레이트 포크.

| 크레이트 | 기준 | 라이선스 | 이유 |
| --- | --- | --- | --- |
| `gpui-pre-windows` | crates.io 0.3.9 | Apache-2.0 | HDR 출력: hwnd 스왑체인과 FP16 scRGB(`src/hdr_output.rs`, `docs/hdr-display.md`) |

기준 대비 변경은 `AraView fork` 주석으로 표시한다. 업스트림을 올릴 때는 새 릴리스를 복사한 뒤 그 변경을 다시 적용한다.
