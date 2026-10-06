#!/usr/bin/env node
/**
 * npm 버전 가드 (`preinstall`).
 *
 * npm 11.6 이하는 `npm install`만으로 package-lock.json에서
 * `@tailwindcss/oxide-wasm32-wasi`의 번들 항목(`@emnapi/*`)을 지운다. 그렇게
 * 바뀐 락파일은 `npm ci`가 동기화 오류로 거부하므로 설치를 실패시켜 알린다.
 * npm은 루트 `preinstall`을 락파일을 쓴 뒤에 실행하므로 변경 자체는 막지
 * 못한다. 그래서 되돌리는 방법을 메시지에 함께 적는다. `.npmrc`의
 * `engine-strict`는 의존성의 engines까지 강제해서 쓰지 않는다. 의존성 없이
 * Node 표준 라이브러리만 쓴다.
 */
import process from "node:process"

const MIN = [11, 7, 0]

const version = process.env.npm_config_user_agent?.match(/\bnpm\/(\d+)\.(\d+)\.(\d+)/)
// npm이 아닌 실행(직접 node 호출 등)은 검사 대상이 아니다.
if (!version) process.exit(0)

const current = version.slice(1).map(Number)
const tooOld = MIN.some(
  (min, i) => current[i] < min && MIN.slice(0, i).every((m, j) => current[j] === m)
)
if (tooOld) {
  console.error(
    [
      `실패: npm ${current.join(".")}은 package-lock.json을 깨뜨린다. npm ${MIN.join(".")} 이상이 필요하다.`,
      "  1. `git checkout package-lock.json`으로 락파일을 되돌린다.",
      "  2. `npm install -g npm@11`로 npm을 올린 뒤 다시 설치한다."
    ].join("\n")
  )
  process.exit(1)
}
