import { existsSync, readdirSync, readFileSync } from "node:fs"
import { dirname, join } from "node:path"
/**
 * 문서-코드 정합성 검사 (`npm run docs:check`).
 *
 * 사람이 손으로 옮기다 깨지기 쉬운 대조만 기계가 확인한다. 의존성 없이
 * Node 표준 라이브러리로 동작하며, 하나라도 어긋나면 exit 1로 실패한다.
 *
 * 검사 항목:
 * 1. 버전 일치 (src-gpui/Cargo.toml, Cargo.lock)
 * 2. 지원 확장자 일치 (image.rs, SPEC.md §2 표)
 * 3. 설정 키 일치 (SPEC.md §13 표, settings.rs Settings)
 * 4. 완료 plan 잔류 금지 (docs/plans 아래 .md 없음)
 */
import process from "node:process"
import { fileURLToPath } from "node:url"

const root = join(dirname(fileURLToPath(import.meta.url)), "..")
const failed = []

function read(rel) {
  return readFileSync(join(root, rel), "utf8")
}

function pass(name, detail) {
  console.log(`[PASS] ${name}: ${detail}`)
}

function fail(name, detail) {
  failed.push(name)
  console.log(`[FAIL] ${name}: ${detail}`)
}

function diffMessage(missing, extra) {
  const parts = []
  if (missing.length > 0) parts.push(`누락: ${missing.join(", ")}`)
  if (extra.length > 0) parts.push(`여분: ${extra.join(", ")}`)
  return parts.join(" / ")
}

function checkSetEqual(name, leftLabel, left, rightLabel, right) {
  const missing = right.filter((item) => !left.includes(item))
  const extra = left.filter((item) => !right.includes(item))
  if (missing.length === 0 && extra.length === 0) {
    pass(name, `${leftLabel}와 ${rightLabel} ${left.length}개 일치`)
  } else {
    fail(name, diffMessage(missing, extra))
  }
}

/** SPEC.md의 `## <num>.` 절 구간만 잘라낸다. */
function specSection(spec, num) {
  const start = spec.indexOf(`## ${num}.`)
  const rest = spec.slice(start)
  const next = rest.indexOf("\n## ", 1)
  return next < 0 ? rest : rest.slice(0, next)
}

/** 마크다운 표에서 첫 열의 백틱 이름을 모두 모은다(`jpg`, `jpeg`처럼 한 칸에 여러 개도 포함). */
function tableNames(section) {
  const names = []
  for (const line of section.split("\n")) {
    if (!line.startsWith("|")) continue
    const firstCell = line.split("|")[1] ?? ""
    for (const match of firstCell.matchAll(/`([^`]+)`/g)) names.push(match[1])
  }
  return names
}

/** 1. 버전 */
{
  const cargoToml = read("src-gpui/Cargo.toml")
  const cargoPackage = cargoToml.split(/^\[/m).find((part) => part.startsWith("package]"))
  const cargoVer = cargoPackage?.match(/^\s*version\s*=\s*"([^"]+)"/m)?.[1] ?? null
  const cargoLock = read("Cargo.lock")
  const lockVer =
    cargoLock.match(/\[\[package\]\]\s*\nname = "araview-gpui"\s*\nversion = "([^"]+)"/)?.[1] ?? null
  if (cargoVer && cargoVer === lockVer) pass("버전", `Cargo.toml과 Cargo.lock 모두 ${cargoVer}`)
  else fail("버전", `Cargo.toml=${cargoVer ?? "없음"}, Cargo.lock=${lockVer ?? "없음"}`)
}

/** 2. 지원 확장자 */
{
  const imageRs = read("crates/araview-core/src/image.rs")
  const arrStart = imageRs.indexOf("SUPPORTED_EXTENSIONS")
  const arrEnd = imageRs.indexOf("];", arrStart)
  const code = [...imageRs.slice(arrStart, arrEnd).matchAll(/"([a-z0-9]+)"/g)].map((m) => m[1])
  const spec = read("SPEC.md")
  const documented = tableNames(specSection(spec, 2))
  checkSetEqual("확장자", "image.rs", code, "SPEC.md §2", documented)
}

/** 3. 설정 키 */
{
  const spec = read("SPEC.md")
  const specKeys = tableNames(specSection(spec, 13))
  const settings = read("src-gpui/src/settings.rs")
  const start = settings.indexOf("pub struct Settings {")
  const block = settings.slice(start, settings.indexOf("\n}", start))
  const keys = []
  let pendingRename = null
  for (const line of block.split("\n")) {
    const rename = line.match(/#\[serde\(rename = "([^"]+)"\)\]/)
    if (rename) {
      pendingRename = rename[1]
      continue
    }
    const field = line.match(/^\s*pub (\w+):/)
    if (!field) continue
    const camel = field[1].replace(/_([a-z])/g, (_, letter) => letter.toUpperCase())
    keys.push(pendingRename ?? camel)
    pendingRename = null
  }
  checkSetEqual("설정 키", "SPEC.md §13", specKeys, "Settings", keys)
}

/** 4. 완료 plan 잔류 금지 */
{
  const plansDir = join(root, "docs/plans")
  if (!existsSync(plansDir)) {
    pass("plan 정리", "docs/plans 없음")
  } else {
    const leftover = readdirSync(plansDir).filter((name) => name.endsWith(".md"))
    if (leftover.length === 0) pass("plan 정리", "docs/plans 비어 있음")
    else fail("plan 정리", `완료 plan 잔류: ${leftover.join(", ")}`)
  }
}

console.log("")
if (failed.length > 0) {
  console.log(`문서 정합성 검사 실패: ${failed.length}건 (${failed.join(", ")})`)
  process.exitCode = 1
} else {
  console.log("문서 정합성 검사 통과")
}
