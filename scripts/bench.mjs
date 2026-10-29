#!/usr/bin/env node
/**
 * 코어 벤치 기준선 기록과 비교 (이슈 #125, `docs/releasing.md` "성능 표").
 *
 * criterion은 `target/criterion/<그룹>/<벤치>/new/sample.json`에 회차별 시간(ns)을
 * 남긴다. 이 스크립트는 그것을 읽어 p50/p95를 뽑고, 릴리스마다 비교할 수 있는
 * 작은 JSON으로 저장한다. 표준 라이브러리만 쓰며 네트워크를 쓰지 않는다.
 *
 * 사용:
 *   cargo bench -p araview-core                  # 측정
 *   node scripts/bench.mjs record --label 2.7.0  # 기준선 저장
 *   node scripts/bench.mjs compare <기준선.json>  # 현재 측정과 비교
 *
 * 비교는 p50이 기준선보다 `--limit`(기본 10%) 넘게 느려지면 exit 1로 실패한다.
 * 새 벤치와 사라진 벤치는 실패가 아니라 보고 대상이다.
 */
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs"
import { dirname, join, relative, resolve, sep } from "node:path"
import process from "node:process"
import { fileURLToPath } from "node:url"

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..")
const criterionDir = join(root, "target", "criterion")
const defaultOut = join(root, "crates", "araview-core", "benches", "baselines")

function usage(message) {
  if (message) console.error(`[FAIL] ${message}`)
  console.error(
    "사용: node scripts/bench.mjs record [--label <이름>] [--out <폴더>]\n" +
      "      node scripts/bench.mjs compare <기준선.json> [--limit 0.10]"
  )
  process.exit(2)
}

/** `--이름 값` 형태의 옵션과 나머지 인자를 나눈다. */
function parseArgs(args) {
  const options = {}
  const rest = []
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index]
    if (!arg.startsWith("--")) {
      rest.push(arg)
      continue
    }
    const value = args[index + 1]
    if (value === undefined || value.startsWith("--")) usage(`${arg} 값이 없습니다`)
    options[arg.slice(2)] = value
    index += 1
  }
  return { options, rest }
}

/** `target/criterion` 아래 모든 `new/sample.json` 경로. */
function sampleFiles(dir) {
  if (!existsSync(dir)) return []
  const found = []
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name)
    if (entry.isDirectory()) found.push(...sampleFiles(path))
    else if (entry.name === "sample.json" && path.endsWith(join("new", "sample.json"))) found.push(path)
  }
  return found
}

/** 정렬된 표본에서 백분위를 고른다(선형 보간 없이 가까운 값). */
function percentile(sorted, fraction) {
  if (sorted.length === 0) return 0
  const index = Math.min(sorted.length - 1, Math.max(0, Math.round(fraction * (sorted.length - 1))))
  return sorted[index]
}

/**
 * `sample.json` 하나를 요약한다. 시간 단위는 ns다.
 *
 * criterion은 `times`에 **배치 전체** 시간을, `iters`에 그 배치의 반복 수를 남긴다.
 * 입력이 작은 벤치는 한 배치에 수천 번을 묶으므로 그대로 읽으면 2048배까지
 * 부풀려진다. 회차별로 `times / iters`로 나눠 반복 1회 시간으로 맞춘다.
 */
function summarize(path) {
  const sample = JSON.parse(readFileSync(path, "utf8"))
  const times = sample.times ?? []
  const iters = sample.iters ?? []
  const perIteration = []
  for (let index = 0; index < times.length; index += 1) {
    const time = times[index]
    const iterations = iters[index] ?? 1
    if (!Number.isFinite(time) || !Number.isFinite(iterations) || iterations <= 0) continue
    perIteration.push(time / iterations)
  }
  if (perIteration.length === 0) return undefined
  const sorted = [...perIteration].sort((a, b) => a - b)
  const mean = perIteration.reduce((sum, value) => sum + value, 0) / perIteration.length
  return {
    p50_ns: round(percentile(sorted, 0.5)),
    p95_ns: round(percentile(sorted, 0.95)),
    mean_ns: round(mean),
    samples: perIteration.length,
  }
}

function round(value) {
  return Math.round(value * 100) / 100
}

/** 벤치 디렉터리 구조에서 이름을 만든다(`decode/jpeg`). */
function benchName(file) {
  const parts = relative(criterionDir, dirname(dirname(file))).split(sep)
  return parts.join("/")
}

/** 현재 측정 전체를 기준선 JSON 모양으로 모은다. */
function collect() {
  const benchmarks = {}
  for (const file of sampleFiles(criterionDir)) {
    const summary = summarize(file)
    if (summary) benchmarks[benchName(file)] = summary
  }
  return benchmarks
}

function formatNs(ns) {
  if (ns >= 1e9) return `${(ns / 1e9).toFixed(2)} s`
  if (ns >= 1e6) return `${(ns / 1e6).toFixed(2)} ms`
  if (ns >= 1e3) return `${(ns / 1e3).toFixed(2)} µs`
  return `${ns.toFixed(0)} ns`
}

function formatDelta(current, baseline) {
  if (!baseline) return "신규"
  const delta = (current - baseline) / baseline
  const sign = delta >= 0 ? "+" : ""
  return `${sign}${(delta * 100).toFixed(1)}%`
}

function record(args) {
  const { options } = parseArgs(args)
  const label = options.label ?? new Date().toISOString().slice(0, 10)
  const outDir = options.out ? resolve(options.out) : defaultOut
  const benchmarks = collect()
  const names = Object.keys(benchmarks).sort()
  if (names.length === 0) {
    console.error(
      `[FAIL] 측정값을 찾지 못했습니다: ${criterionDir}\n` +
        "먼저 `cargo bench -p araview-core`를 실행하세요."
    )
    process.exit(1)
  }
  mkdirSync(outDir, { recursive: true })
  const dest = join(outDir, `${label}.json`)
  const baseline = { label, created: new Date().toISOString(), benchmarks }
  writeFileSync(dest, `${JSON.stringify(baseline, null, 2)}\n`)
  console.log(`기준선 ${names.length}개를 저장했습니다: ${relative(root, dest)}`)
  report(names, benchmarks, undefined, 0)
}

function compare(args) {
  const { options, rest } = parseArgs(args)
  const baselinePath = rest[0]
  if (!baselinePath) usage("비교할 기준선 JSON 경로가 필요합니다")
  if (!existsSync(baselinePath)) usage(`기준선 파일이 없습니다: ${baselinePath}`)
  const limit = Number(options.limit ?? 0.1)
  if (!Number.isFinite(limit) || limit < 0) usage("--limit은 0 이상의 숫자여야 합니다")
  const baseline = JSON.parse(readFileSync(baselinePath, "utf8"))
  const before = baseline.benchmarks ?? {}
  const current = collect()
  const names = [...new Set([...Object.keys(before), ...Object.keys(current)])].sort()
  if (names.every((name) => !current[name])) {
    console.error(
      `[FAIL] 현재 측정값이 없습니다: ${criterionDir}\n` + "먼저 `cargo bench -p araview-core`를 실행하세요."
    )
    process.exit(1)
  }
  const regressions = report(names, current, before, limit)
  if (regressions.length > 0) {
    console.error(
      `[FAIL] p50이 ${(limit * 100).toFixed(0)}% 넘게 느려진 벤치 ${regressions.length}개: ` +
        regressions.join(", ")
    )
    process.exit(1)
  }
  console.log("[PASS] 기준선 대비 기준을 넘는 회귀가 없습니다")
}

/** 표를 찍고, `limit`을 넘은 벤치 이름을 돌려준다. */
function report(names, current, baseline, limit) {
  const rows = names.map((name) => {
    const now = current[name]
    const before = baseline?.[name]
    const ratio = before && now ? now.p50_ns / before.p50_ns : 1
    return { name, now, before, ratio }
  })
  const width = Math.max(4, ...rows.map((row) => row.name.length))
  console.log(
    `${"bench".padEnd(width)}  ${"기준 p50".padStart(10)}  ${"현재 p50".padStart(10)}  ` +
      `${"변화".padStart(8)}  ${"현재 p95".padStart(10)}`
  )
  const regressions = []
  for (const { name, now, before, ratio } of rows) {
    if (!now) {
      console.log(`${name.padEnd(width)}  ${formatNs(before.p50_ns).padStart(10)}  ${"—".padStart(10)}  ${"사라짐".padStart(8)}`)
      continue
    }
    console.log(
      `${name.padEnd(width)}  ${(before ? formatNs(before.p50_ns) : "—").padStart(10)}  ` +
        `${formatNs(now.p50_ns).padStart(10)}  ${formatDelta(now.p50_ns, before?.p50_ns).padStart(8)}  ` +
        `${formatNs(now.p95_ns).padStart(10)}`
    )
    if (before && ratio > 1 + limit) regressions.push(name)
  }
  return regressions
}

const [command, ...args] = process.argv.slice(2)
switch (command) {
  case "record":
    record(args)
    break
  case "compare":
    compare(args)
    break
  default:
    usage(command ? `알 수 없는 명령: ${command}` : undefined)
}
