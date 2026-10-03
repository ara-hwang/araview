#!/usr/bin/env node
/**
 * 프론트엔드 의존성 감사 게이트 (`npm run audit:frontend`).
 *
 * `npm audit --audit-level=moderate`는 moderate 이상 새 자문이 있으면 무조건
 * 실패한다. 하지만 업스트림에 패치 버전이 아직 없는 자문은 생기면 모든 릴리즈가
 * 그 자문이 풀릴 때까지 막힌다. 그래서 "패치 버전이 없는 자문"만 아래 ALLOWED에
 * 이유와 추적 링크를 남기고 통과시킨다. 목록에 없는 자문과 더 이상 보고되지
 * 않는 목록 항목은 그대로 실패한다. 의존성 없이 Node 표준 라이브러리만 쓴다.
 */
import { spawnSync } from "node:child_process"
import process from "node:process"

// 예외는 유효기간이 있다. 업스트림 패치가 나오면 이 항목을 지운다.
// 남겨 두면 "더 이상 보고되지 않는 항목" 검사에서 CI가 실패한다.
const ALLOWED = [
  {
    id: "GHSA-vfj7-8cjw-p6xm",
    reason:
      "braces는 3.0.3이 현재 최신이고 패치 버전이 아직 없다. devDependency(shadcn CLI) 체인에만 있고 배포물에는 포함되지 않는다(npm audit --omit=dev 0건).",
    tracking: "https://github.com/advisories/GHSA-vfj7-8cjw-p6xm"
  }
]

const BLOCKING = new Set(["moderate", "high", "critical"])

function fail(message) {
  console.error(`실패: ${message}`)
  process.exit(1)
}

// npm.cmd는 Windows에서 shell 없이 spawn할 수 없으므로 고정 문자열을 shell로 실행한다.
const audit = spawnSync("npm audit --json", {
  shell: true,
  encoding: "utf8",
  maxBuffer: 16 * 1024 * 1024
})
const output = audit.stdout?.trim() ?? ""
if (output === "") {
  fail(`npm audit 출력이 비었다.${audit.stderr ? `\n${audit.stderr.trim()}` : ""}`)
}

let report
try {
  report = JSON.parse(output)
} catch {
  fail(`npm audit 출력을 해석하지 못했다.\n${output.slice(0, 500)}`)
}
if (report.error) {
  fail(`npm audit 실행 오류: ${report.error.summary ?? report.error.code ?? "알 수 없음"}`)
}

// 이름 있는 자문만 모은다. via의 문자열 항목은 "어느 패키지를 통해 들어왔는지"를
// 가리키는 부모 패키지 이름이라 자문이 아니다.
const advisories = new Map()
for (const entry of Object.values(report.vulnerabilities ?? {})) {
  for (const via of entry.via ?? []) {
    if (typeof via !== "object" || via === null || typeof via.url !== "string") continue
    const id = via.url.match(/GHSA-[a-z0-9-]+/i)?.[0] ?? via.url
    if (!advisories.has(id)) {
      advisories.set(id, {
        id,
        title: via.title ?? id,
        severity: via.severity ?? "unknown",
        url: via.url
      })
    }
  }
}

const allowedIds = new Set(ALLOWED.map((item) => item.id))
const blocking = [...advisories.values()].filter(
  (advisory) => BLOCKING.has(advisory.severity) && !allowedIds.has(advisory.id)
)
const stale = ALLOWED.filter((item) => !advisories.has(item.id))

if (blocking.length > 0) {
  console.error(
    `실패: moderate 이상 새 자문 ${blocking.length}건이 있다. 고치거나, 업스트림에 패치가 없으면 ` +
      "scripts/audit-frontend.mjs의 ALLOWED에 이유와 추적 링크를 남긴다."
  )
  for (const advisory of blocking) {
    console.error(`  ${advisory.id} [${advisory.severity}] ${advisory.title}`)
    console.error(`    ${advisory.url}`)
  }
  process.exit(1)
}
if (stale.length > 0) {
  fail(
    `허용 목록 ${stale.length}건이 더 이상 보고되지 않는다. 지운다: ${stale
      .map((item) => item.id)
      .join(", ")}`
  )
}

console.log(
  `감사 통과: 자문 ${advisories.size}건, 그중 패치 없는 허용 ${ALLOWED.length}건 (moderate 이상 새 자문 0건)`
)
for (const item of ALLOWED) {
  console.log(`  허용 ${item.id}: ${item.reason}`)
  console.log(`    ${item.tracking}`)
}
