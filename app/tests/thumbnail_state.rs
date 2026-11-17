//! GPUI/Windows 없이도 실행하는 썸네일 수명·가상 목록 준비 회귀 검사.
//! RUSTUP_TOOLCHAIN=stable rustc --edition=2024 --test app/tests/thumbnail_state.rs -o /tmp/araview-thumbnail-tests
//! /tmp/araview-thumbnail-tests
//! 성능 표본: 위 컴파일에 -O 추가 후 --ignored --nocapture

#[path = "../src/app/thumbnail_state.rs"]
mod thumbnail_state;
