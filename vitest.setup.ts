import { configure } from "@testing-library/dom"

// jsdom 기반 테스트는 CPU를 많이 쓰는 작업(릴리즈 빌드, dev 앱, 병렬 워커)이
// 떠 있으면 개별 테스트가 수 초씩 늘어날 수 있다. waitFor 기본 허용은 1000ms라
// 그 순간 Store 반영을 못 기다리고 flake가 났다(2026-09-26, ViewTabPanel).
// 느려도 통과해야 하고, 실제로 깨진 테스트는 vitest의 testTimeout이 잡는다.
configure({ asyncUtilTimeout: 5_000 })
