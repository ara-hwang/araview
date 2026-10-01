import { existsSync, readdirSync, readFileSync, statSync } from "node:fs"
import { dirname, join } from "node:path"
/**
 * 문서-코드 정합성 검사 (`npm run docs:check`).
 *
 * 사람이 손으로 옮기다 깨지기 쉬운 대조만 기계가 확인한다. 의존성 없이
 * Node 표준 라이브러리로 동작하며, 하나라도 어긋나면 exit 1로 실패한다.
 *
 * 검사 항목:
 * 1. 버전 4곳 일치 (package.json, tauri.conf.json, Cargo.toml, Cargo.lock)
 * 2. 지원 확장자 일치 (imageExtensions.ts, image.rs, tauri.conf.json)
 * 3. IPC 명령 일치 (SPEC.md §15 표, lib.rs invoke_handler, Rust command 정의)
 * 4. 설정 키 일치 (SPEC.md §13 표, settingsStore.ts SettingsState)
 * 5. 플러그인 목록 일치 (development.md, Cargo.toml)
 * 6. 완료 plan 잔류 금지 (docs/plans 아래 .md 없음)
 * 7. i18n 잔재 키 금지 (header.*Title 중 코드 미참조 키 없음)
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

/** 마크다운 표에서 첫 열이 백틱으로 감싼 행의 이름만 모은다. */
function tableFirstColumn(section) {
  const names = []
  for (const line of section.split("\n")) {
    const match = line.match(/^\|\s*`([^`]+)`/)
    if (match) names.push(match[1])
  }
  return names
}

function srcFiles(dir) {
  const found = []
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry)
    if (statSync(path).isDirectory()) found.push(...srcFiles(path))
    else if (path.endsWith(".ts") || path.endsWith(".tsx")) found.push(path)
  }
  return found
}

/** 1. 버전 4곳 */
{
  const pkg = JSON.parse(read("package.json"))
  const tauriConf = JSON.parse(read("src-tauri/tauri.conf.json"))
  const cargoToml = read("src-tauri/Cargo.toml")
  const cargoPackage = cargoToml.split(/^\[/m).find((part) => part.startsWith("package]"))
  const cargoVer = cargoPackage?.match(/^\s*version\s*=\s*"([^"]+)"/m)?.[1] ?? null
  const cargoLock = read("src-tauri/Cargo.lock")
  const lockVer =
    cargoLock.match(/\[\[package\]\]\s*\nname = "araview"\s*\nversion = "([^"]+)"/)?.[1] ?? null
  const versions = {
    "package.json": pkg.version ?? null,
    "tauri.conf.json": tauriConf.version ?? null,
    "Cargo.toml": cargoVer,
    "Cargo.lock": lockVer
  }
  const expected = versions["tauri.conf.json"]
  const mismatched = Object.entries(versions).filter(([, value]) => value !== expected)
  if (mismatched.length === 0) pass("버전", `4곳 모두 ${expected}`)
  else
    fail(
      "버전",
      diffMessage(
        mismatched.map(([key, value]) => `${key}=${value ?? "없음"}`),
        []
      )
    )
}

/** 2. 지원 확장자 */
{
  const front = [...read("src/constants/imageExtensions.ts").matchAll(/"([a-z0-9]+)"/g)].map(
    (m) => m[1]
  )
  const imageRs = read("src-tauri/src/image.rs")
  const arrStart = imageRs.indexOf("SUPPORTED_EXTENSIONS")
  const arrEnd = imageRs.indexOf("];", arrStart)
  const back = [...imageRs.slice(arrStart, arrEnd).matchAll(/"([a-z0-9]+)"/g)].map((m) => m[1])
  const tauriConf = JSON.parse(read("src-tauri/tauri.conf.json"))
  const assoc = tauriConf.bundle.fileAssociations.flatMap((group) => group.ext)
  checkSetEqual("확장자 프론트/백엔드", "imageExtensions.ts", front, "image.rs", back)
  checkSetEqual(
    "확장자 설정",
    "소스 코드",
    [...new Set([...front, ...back])],
    "tauri.conf.json",
    assoc
  )
}

/** 3. IPC 명령 */
{
  const spec = read("SPEC.md")
  const specCommands = tableFirstColumn(specSection(spec, 15))
  const lib = read("src-tauri/src/lib.rs")
  const handlerBlock = lib.slice(lib.indexOf("generate_handler!["))
  const invoked = [...handlerBlock.matchAll(/^\s*([a-z][a-z0-9_]*)\s*,?\s*$/gm)].map((m) => m[1])
  checkSetEqual("IPC 표/등록", "SPEC.md §15", specCommands, "invoke_handler", invoked)
  const defined = []
  const commandFiles = [
    "src-tauri/src/commands.rs",
    "src-tauri/src/cache.rs",
    "src-tauri/src/thumb_shell.rs",
    "src-tauri/src/lib.rs"
  ]
  for (const rel of commandFiles) {
    const text = read(rel)
    const attr = "#[tauri::command]"
    let index = text.indexOf(attr)
    while (index >= 0) {
      const fnMatch = text.slice(index).match(/(?:pub\s+)?(?:async\s+)?fn (\w+)/)
      if (fnMatch) defined.push(fnMatch[1])
      index = text.indexOf(attr, index + attr.length)
    }
  }
  const unimplemented = invoked.filter((name) => !defined.includes(name))
  if (unimplemented.length === 0)
    pass("IPC 정의", `등록 ${invoked.length}개 모두 command 정의 있음`)
  else fail("IPC 정의", `정의 없음: ${unimplemented.join(", ")}`)
}

/** 4. 설정 키 */
{
  const spec = read("SPEC.md")
  const specKeys = tableFirstColumn(specSection(spec, 13))
  const store = read("src/store/settingsStore.ts")
  const typeStart = store.indexOf("export type SettingsState = {")
  const typeBlock = store.slice(typeStart, store.indexOf("\n}", typeStart))
  const storeKeys = [...typeBlock.matchAll(/^\s{2}(\w+)\??:/gm)].map((m) => m[1])
  checkSetEqual("설정 키", "SPEC.md §13", specKeys, "SettingsState", storeKeys)
}

/** 5. 플러그인 목록 */
{
  const devDoc = read("docs/development.md")
  const pluginLine = devDoc.split("\n").find((line) => line.includes("Tauri Plugins")) ?? ""
  const docPlugins = [...pluginLine.matchAll(/`([^`]+)`/g)].map((m) => m[1])
  const cargo = read("src-tauri/Cargo.toml")
  const cargoPlugins = [...cargo.matchAll(/^tauri-plugin-([a-z0-9-]+)\s*=/gm)].map((m) => m[1])
  checkSetEqual("플러그인", "development.md", docPlugins, "Cargo.toml", cargoPlugins)
}

/** 6. 완료 plan 잔류 금지 */
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

/** 7. i18n 잔재 키 (header.*Title 중 코드 미참조) */
{
  const ko = JSON.parse(read("src/i18n/locales/ko.json"))
  const candidates = Object.keys(ko.header ?? {}).filter(
    (key) => key.endsWith("Title") && key !== "settingsTitle"
  )
  const sources = srcFiles(join(root, "src"))
  const dead = candidates.filter(
    (key) => !sources.some((path) => readFileSync(path, "utf8").includes(`header.${key}`))
  )
  if (dead.length === 0)
    pass(
      "i18n 잔재 키",
      candidates.length === 0
        ? "header.*Title 잔재 없음"
        : `header.*Title ${candidates.length}개 모두 코드 참조됨`
    )
  else fail("i18n 잔재 키", `미참조: ${dead.map((key) => `header.${key}`).join(", ")}`)
}

console.log("")
if (failed.length > 0) {
  console.log(`문서 정합성 검사 실패: ${failed.length}건 (${failed.join(", ")})`)
  process.exitCode = 1
} else {
  console.log("문서 정합성 검사 통과")
}
