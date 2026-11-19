# Windows 의존성의 Chrono 제거

[RUSTSEC-2026-0543](https://rustsec.org/advisories/RUSTSEC-2026-0543.html)은
`chrono`의 모든 버전이 유지보수 종료됐다고 분류한다. Windows x64에서 쓰는
경로를 Jiff로 교체하고 `deny.toml`은 그대로 유지한다.

- `gpui-base`와 `gpui-component` 0.9.1: [GPUI Kit #4015](https://github.com/longbridge/gpui-kit/pull/4015)의 달력·시간 입력 변경을 백포트한다. AraView는 이 위젯의 날짜 타입을 직접 사용하지 않는다. `gpui-component`의 기존 AraView 포크는 유지한다.
- `gpui-pre`와 `gpui-pre-scheduler` 0.3.12: [Zed #67089](https://github.com/zed-industries/zed/pull/67089)의 스케줄러·접근성 시각 변경을 바탕으로 한다. Windows의 `SystemTime`은 100ns 단위이므로 wall clock에는 Jiff `Timestamp`를 써 테스트 시계의 나노초를 보존한다. 단조 시계는 바꾸지 않는다.
- `rawler` 0.8.0: EXIF 날짜와 오프셋만 Jiff로 이전한다. 카메라·렌즈 프로필은 변경하지 않는다. `data/testdata`는 업스트림 RAW 통합 테스트에만 쓰여 포크에 넣지 않는다. 그 테스트를 실행하려면 동일한 0.8.0 배포본에서 복원한다.
- `trash`는 Jiff로 이전한 정식 5.4 버전을 사용한다. `gpui-fast`는 AraView가 활성화하지 않는 선택 의존성이라 잠금 파일에만 남아 있다. 그 기능을 도입할 경우 호환되는 스케줄러 및 Chrono 제거 버전을 먼저 확인한다.

업스트림의 호환 버전에 해당 변경이 들어오면 각 포크를 제거한다. 포크를
제거하거나 Cargo 기능을 바꿀 때는 `cargo deny check`와 아래 그래프를 확인한다.

```sh
cargo tree --locked --all-features --target x86_64-pc-windows-msvc --invert chrono
cargo test --manifest-path vendor/gpui-pre-scheduler/Cargo.toml --lib
cargo test --manifest-path vendor/rawler/Cargo.toml --lib exif_time_tests
```

첫 명령은 활성 Windows 그래프에서 `chrono`의 역의존성을 출력하지 않아야 한다.
