# vendor

루트 `Cargo.toml`의 `[patch.crates-io]`로 덮어쓰는 업스트림 크레이트 포크.

| 크레이트            | 기준         | 라이선스    | 이유                                                                                                |
| ---                 | ---          | ---         | ---                                                                                                 |
| `gpui-pre-windows`  | crates.io 0.3.11 | Apache-2.0 | HDR 출력: hwnd 스왑체인과 FP16 scRGB(`src/hdr_output.rs`, `docs/hdr-display.md`). 입력 굶은 프레임 보충(`src/events.rs`, `src/window.rs`), 점프 목록 ICU 의존 제거(`src/destination_list.rs`) |
| `gpui-component`    | crates.io 0.9.1 | Apache-2.0 | 명령 팔레트 휠을 부드러운 스크롤로 처리: 목록의 자체 휠 스크롤을 끄고 핸들을 노출(`src/command/`). 중첩 다이얼로그 퇴장 중 남은 창으로 포커스 트랩 유지(`src/root.rs`) |

기준 대비 변경은 `AraView fork` 주석으로 표시한다. 업스트림을 올릴 때는 새 릴리스를 복사한 뒤 그 변경을 다시 적용한다.
