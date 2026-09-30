import { execFileSync } from "node:child_process"
import { existsSync, readFileSync, writeFileSync } from "node:fs"
import { homedir } from "node:os"
import { dirname, join, resolve } from "node:path"

/**
 * AraView 로컬 릴리즈 보조 플러그인.
 *
 * `npm run release:local`(`scripts/Publish-LocalRelease.ps1`)은 태그 생성, 서명 빌드,
 * `gh release create`까지 한 번에 수행하므로, 실행 전 조건이 어긋나면 중간에 무엇이
 * 이미 생겼는지 확인하기 어렵다. 이 플러그인은 실행하지 않고 **읽기 전용으로 조건만
 * 검사**하는 `release_preflight`와, 버전 4곳을 한 번에 올리는 `release_bump`을
 * 제공한다. 릴리스 실행 자체는 `docs/releasing.md`의 `npm run release:local`을 그대로 쓴다.
 *
 * 서명키와 비밀번호는 존재 여부와 경로만 보고, 내용은 절대 출력하지 않는다.
 *
 * 로더는 `.opencode/plugin(s)/`를 스캔하고 default export의 `id`와 `setup`을 요구한다.
 * SDK 패키지를 import하지 않는 이유는 두 가지다. `.opencode/package.json`에
 * `type: module`이 없어 SDK import가 CJS/ESM 충돌을 만든다(TS1479), 그리고
 * 설치된 `@opencode-ai/plugin` 타입이 런타임보다 낡아 `ctx.tool`이 없다.
 * 툴 도메인은 여기서 좁게 선언해 캐스팅한다. SDK가 따라잡으면 캐스팅을 지우면 된다.
 */

type ReleaseLevel = "patch" | "minor" | "major"

type JsonSchema = {
  type: "object"
  properties: Record<string, unknown>
  required?: string[]
  additionalProperties: boolean
}

type ToolArgs = {
  directory?: string
  level?: ReleaseLevel
  version?: string
}

type ToolExecuteContext = { directory?: string }

/**
 * 실행 결과는 `{ content }` 객체로 돌려준다. 문자열이나 `{ output }`는 브리지에서
 * 거부된다(`output`은 출력 스키마 선언이 있을 때만 허용).
 */
type ToolDefinition = {
  /** 이름은 필수다. 빠지면 등록이 조용히 버려진다(2026-09-26 확인). */
  name: string
  description: string
  input: JsonSchema
  options?: { namespace?: string; codemode?: boolean }
  execute: (args: ToolArgs, context: ToolExecuteContext) => Promise<{ content: string }>
}

type ToolEditor = {
  namespace(input: { name: string; description: string }): void
  add(tool: ToolDefinition): void
}

type ToolRegistrar = {
  transform(callback: (editor: ToolEditor) => void): Promise<unknown>
}

/** 로더가 넘기는 컨텍스트 중 필요한 부분만. */
type PluginContextLike = {
  location?: { directory?: string }
}

type Check = {
  name: string
  ok: boolean
  blocking: boolean
  detail: string
}

type PreflightReport = {
  root: string
  version: string | null
  tag: string | null
  ready: boolean
  blockers: string[]
  warnings: string[]
  checks: Check[]
}

type BumpReport = {
  root: string
  from: string
  to: string
  tag: string
  files: { path: string; from: string; to: string }[]
  warnings: string[]
}

const EXEC_TIMEOUT_MS = 20_000
const DEFAULT_UPDATES_REPO = "ara-hwang/araview"
const VERSION_FILES = {
  packageJson: "package.json",
  tauriConfig: "src-tauri/tauri.conf.json",
  cargoToml: "src-tauri/Cargo.toml",
  cargoLock: "src-tauri/Cargo.lock"
} as const

type ExecResult = { ok: boolean; code: number; stdout: string; stderr: string }

/** 명령을 실행한다. 실패해도 예외를 던지지 않고 코드로만 판정한다(읽기 전용 검사라 예외 대신 코드). */
function run(command: string, args: string[], cwd: string): ExecResult {
  try {
    const stdout = execFileSync(command, args, {
      cwd,
      encoding: "utf8",
      timeout: EXEC_TIMEOUT_MS,
      windowsHide: true,
      stdio: ["ignore", "pipe", "pipe"]
    })
    return { ok: true, code: 0, stdout, stderr: "" }
  } catch (error) {
    const failure = error as { status?: number | null; stdout?: string; stderr?: string }
    return {
      ok: false,
      code: typeof failure.status === "number" ? failure.status : 1,
      stdout: failure.stdout ?? "",
      stderr: failure.stderr ?? ""
    }
  }
}

function readText(path: string): string {
  return readFileSync(path, "utf8")
}

function readJson(path: string): Record<string, unknown> {
  return JSON.parse(readText(path)) as Record<string, unknown>
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")
}

function check(checks: Check[], name: string, ok: boolean, detail: string, blocking = true): void {
  checks.push({ name, ok, detail, blocking })
}

/** 저장소 루트를 찾는다. `.opencode` 플러그인은 세션 디렉터리 기준으로 호출되므로 위로 한 번 확인한다. */
function resolveRoot(start: string): { root: string; error?: string } {
  let dir = resolve(start)
  for (let depth = 0; depth < 4; depth++) {
    if (existsSync(join(dir, VERSION_FILES.tauriConfig))) return { root: dir }
    const parent = dirname(dir)
    if (parent === dir) break
    dir = parent
  }
  return {
    root: resolve(start),
    error: `AraView 저장소를 찾지 못했다: ${start} 및 상위 디렉터리에서 ${VERSION_FILES.tauriConfig} 없음`
  }
}

/** `package.json` / `tauri.conf.json`의 최상위 version. */
function readJsonVersion(path: string): string | null {
  const value = (readJson(path) as { version?: unknown }).version
  return typeof value === "string" ? value : null
}

/** `Cargo.toml`의 `[package]` 섹션 version. 다른 섹션의 version을 집지 않게 섹션으로 범위를 정한다. */
function readCargoTomlVersion(path: string): string | null {
  const text = readText(path)
  const section = text.split(/^\[/m).find((part) => part.startsWith("package]"))
  const match = section?.match(/^\s*version\s*=\s*"([^"]+)"/m)
  return match?.[1] ?? null
}

/** `Cargo.lock`의 `[[package]] name = "araview"` 항목 version. */
function readCargoLockVersion(path: string): string | null {
  const text = readText(path)
  const match = text.match(/\[\[package\]\]\s*\nname = "araview"\s*\nversion = "([^"]+)"/)
  return match?.[1] ?? null
}

function readAllVersions(root: string): Record<string, string | null> {
  return {
    packageJson: readJsonVersion(join(root, VERSION_FILES.packageJson)),
    tauriConfig: readJsonVersion(join(root, VERSION_FILES.tauriConfig)),
    cargoToml: readCargoTomlVersion(join(root, VERSION_FILES.cargoToml)),
    cargoLock: readCargoLockVersion(join(root, VERSION_FILES.cargoLock))
  }
}

/** 현재 버전 문자열이 실제로 한 곳에만 있을 때만 바꾼다. 여러 곳이면 손대지 않고 실패한다. */
function replaceExactlyOnce(text: string, pattern: RegExp, replacement: string, label: string): string {
  const global = new RegExp(pattern.source, pattern.flags.includes("g") ? pattern.flags : `${pattern.flags}g`)
  const found = text.match(global)
  if (!found || found.length !== 1) {
    throw new Error(`${label}: '${found ? found.length : 0}곳 일치 (정확히 1곳이어야 한다). 손대지 않았다.`)
  }
  return text.replace(pattern, replacement)
}

/**
 * `Cargo.toml`의 `[package]` 섹션 안에서만 version을 바꾼다.
 * 다른 섹션이나 의존성 선언에 같은 문자열이 있어도 건드리지 않게 구간으로 자른다.
 */
function replaceInCargoPackageSection(text: string, current: string, target: string): string {
  const start = text.search(/^\[package\][^\S\r\n]*$/m)
  if (start < 0) throw new Error(`${VERSION_FILES.cargoToml}: [package] 섹션을 찾지 못했다`)
  const tail = text.slice(start + 1)
  const nextHeader = tail.search(/^\[/m)
  const end = nextHeader < 0 ? text.length : start + 1 + nextHeader
  const section = text.slice(start, end)
  const pattern = new RegExp(`^(\\s*version\\s*=\\s*")${escapeRegExp(current)}(")`, "m")
  const found = section.match(new RegExp(pattern.source, "gm"))
  if (!found || found.length !== 1) {
    throw new Error(
      `${VERSION_FILES.cargoToml} [package]: version = "${current}"가 ${found ? found.length : 0}곳 (정확히 1곳이어야 한다). 손대지 않았다.`
    )
  }
  return text.slice(0, start) + section.replace(pattern, `$1${target}$2`) + text.slice(end)
}

function nextVersion(current: string, level: ReleaseLevel): string {
  const match = current.match(/^(\d+)\.(\d+)\.(\d+)$/)
  if (!match) throw new Error(`현재 버전이 semver가 아니다: ${current}`)
  const [, major, minor, patch] = match
  const majorNum = Number(major)
  const minorNum = Number(minor)
  const patchNum = Number(patch)
  if (level === "major") return `${majorNum + 1}.0.0`
  if (level === "minor") return `${majorNum}.${minorNum + 1}.0`
  return `${majorNum}.${minorNum}.${patchNum + 1}`
}

/** Publish-LocalRelease.ps1의 탐색 순서를 그대로 따른다. 경로와 출처만 보고 내용은 읽지 않는다. */
function findSigningKey(root: string): { source: string; path: string | null } {
  const fromEnv = process.env.TAURI_SIGNING_PRIVATE_KEY
  if (fromEnv) {
    return existsSync(fromEnv)
      ? { source: "TAURI_SIGNING_PRIVATE_KEY (경로)", path: fromEnv }
      : { source: "TAURI_SIGNING_PRIVATE_KEY (키 내용)", path: null }
  }
  const candidates = [
    join(root, "araview.key"),
    join(homedir(), ".tauri", "araview.key")
  ]
  for (const candidate of candidates) {
    if (existsSync(candidate)) return { source: "탐색 경로", path: candidate }
  }
  return { source: "찾지 못함", path: null }
}

/** `.env.local`은 스크립트가 직접 읽는다. 여기서는 이름 존재만 확인하고 값은 읽지 않는다. */
function envLocalHas(root: string, name: string): boolean {
  const path = join(root, ".env.local")
  if (!existsSync(path)) return false
  return readText(path)
    .split(/\r?\n/)
    .some((line) => {
      const trimmed = line.trim()
      return trimmed !== "" && !trimmed.startsWith("#") && trimmed.split("=")[0]?.trim() === name
    })
}

function deriveReleaseRepo(endpoint: string | null): string | null {
  const match = endpoint?.match(/^https:\/\/github\.com\/([^/]+\/[^/]+)\/releases\//)
  return match?.[1] ?? null
}

export function collectPreflight(root: string): PreflightReport {
  const checks: Check[] = []
  const warnings: string[] = []

  const versions = readAllVersions(root)
  const version = versions.tauriConfig
  const tag = version ? `v${version}` : null
  if (!version) {
    check(checks, "버전", false, `${VERSION_FILES.tauriConfig}의 version을 읽지 못했다`, true)
    return finish(root, null, null, checks)
  }

  const versionEntries = Object.entries(versions)
  const mismatched = versionEntries.filter(([, value]) => value !== version)
  check(
    checks,
    "버전 일관성",
    mismatched.length === 0,
    mismatched.length === 0
      ? `4곳 모두 ${version} (${versionEntries.map(([key]) => key).join(", ")})`
      : `불일치: ${mismatched.map(([key, value]) => `${key}=${value ?? "없음"}`).join(", ")} (기준 ${VERSION_FILES.tauriConfig}=${version})`,
    true
  )

  const branch = run("git", ["rev-parse", "--abbrev-ref", "HEAD"], root)
  const branchName = branch.stdout.trim()
  if (!branch.ok) {
    check(checks, "git 저장소", false, "git rev-parse 실패. AraView 저장소 루트에서 실행해야 한다", true)
    return finish(root, version, tag, checks)
  }
  check(checks, "브랜치", true, branchName, false)

  const status = run("git", ["status", "--porcelain"], root)
  const dirty = status.stdout.split(/\r?\n/).filter((line) => line.trim() !== "")
  check(
    checks,
    "작업 트리",
    dirty.length === 0,
    dirty.length === 0
      ? "clean"
      : `${dirty.length}개 변경됨 (${dirty
          .slice(0, 5)
          .join(", ")}${dirty.length > 5 ? ", ..." : ""})`,
    true
  )

  const head = run("git", ["rev-parse", "HEAD"], root).stdout.trim()
  const upstream = run("git", ["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"], root)
  if (!upstream.ok) {
    warnings.push("업스트림이 없다. 태그 푸시 전 원격 저장소를 확인한다.")
  } else {
    const counts = run("git", ["rev-list", "--left-right", "--count", "@{upstream}...HEAD"], root)
    const [behind, ahead] = counts.stdout.trim().split(/\s+/).map((value) => Number(value ?? 0))
    check(
      checks,
      "원격 동기화",
      (behind ?? 0) === 0,
      counts.ok
        ? `${upstream.stdout.trim()} 대비 ${ahead ?? 0} ahead / ${behind ?? 0} behind`
        : "rev-list 실패",
      true
    )
  }

  const tagCommit = tag ? run("git", ["rev-list", "-n", "1", tag], root) : null
  if (!tag) {
    check(checks, "태그", false, "버전을 읽지 못해 태그를 확인할 수 없다", true)
  } else if (tagCommit?.ok) {
    const commit = tagCommit.stdout.trim()
    check(
      checks,
      "태그",
      commit === head,
      commit === head
        ? `${tag}가 HEAD와 같다`
        : `${tag}가 ${commit.slice(0, 7)}를 가리키지만 HEAD는 ${head.slice(0, 7)}. 스크립트가 중단한다`,
      true
    )
  } else {
    check(checks, "태그", true, `${tag} 없음. 스크립트가 HEAD에 새로 만든다`, false)
  }

  const ghVersion = run("gh", ["--version"], root)
  const ghAuth = ghVersion.ok ? run("gh", ["auth", "status"], root) : ghVersion
  const ghAccount =
    ghAuth.stdout
      .split(/\r?\n/)
      .map((line) => line.trim())
      .find((line) => line.toLowerCase().includes("logged in to")) ?? ghAuth.stdout.split(/\r?\n/)[0]?.trim()
  check(
    checks,
    "gh CLI",
    ghVersion.ok && ghAuth.ok,
    !ghVersion.ok
      ? "gh를 찾지 못했다. 태그 푸시와 릴리스 생성에 필요하다"
      : ghAuth.ok
        ? (ghAccount ?? "인증됨")
        : "gh 로그인 필요. `gh auth login`",
    true
  )

  const key = findSigningKey(root)
  check(
    checks,
    "서명키",
    key.path !== null || key.source.includes("키 내용"),
    key.path ? `${key.source}: ${key.path}` : key.source === "찾지 못함" ? "찾지 못했다 (-KeyPath, TAURI_SIGNING_PRIVATE_KEY, araview.key, ~/.tauri/araview.key)" : key.source,
    true
  )

  const passwordInEnv = process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD !== undefined
  const passwordInFile = envLocalHas(root, "TAURI_SIGNING_PRIVATE_KEY_PASSWORD")
  check(
    checks,
    "서명키 비밀번호",
    passwordInEnv || passwordInFile,
    passwordInEnv
      ? "TAURI_SIGNING_PRIVATE_KEY_PASSWORD 설정됨"
      : passwordInFile
        ? ".env.local에 있음"
        : "설정되지 않았다. 미설정 시 빌드가 대화형 프롬프트에서 멈춘다",
    true
  )

  const config = readJson(join(root, VERSION_FILES.tauriConfig)) as {
    plugins?: { updater?: { pubkey?: string; endpoints?: string[] } }
  }
  const updater = config.plugins?.updater
  const pubkey = updater?.pubkey?.trim() ?? ""
  const endpoint = updater?.endpoints?.[0] ?? null
  check(checks, "updater 공개키", pubkey !== "", pubkey === "" ? "비어 있다" : "설정됨", true)

  const releaseRepo = deriveReleaseRepo(endpoint) ?? DEFAULT_UPDATES_REPO
  check(
    checks,
    "릴리스 대상 저장소",
    true,
    `endpoint ${endpoint ?? "없음"} → ${releaseRepo} (스크립트 기본값 ${DEFAULT_UPDATES_REPO})`,
    false
  )
  if ((deriveReleaseRepo(endpoint) ?? DEFAULT_UPDATES_REPO) !== DEFAULT_UPDATES_REPO) {
    warnings.push(
      `endpoint가 ${DEFAULT_UPDATES_REPO}를 가리키지 않는다. 스크립트 호출에 -UpdatesRepo ${deriveReleaseRepo(endpoint) ?? "<repo>"}를 넘긴다.`
    )
  }

  const bundleDir = join(root, "src-tauri", "target", "release", "bundle", "nsis")
  const nsisFiles = existsSync(bundleDir) ? listFiles(bundleDir).filter((name) => name.includes(version)) : []
  const hasInstaller = nsisFiles.some((name) => name.endsWith("setup.exe"))
  const hasSignature = nsisFiles.some((name) => name.endsWith(".sig"))
  const outputDir = join(root, "release", tag ?? "")
  const collected = existsSync(outputDir) ? listFiles(outputDir) : []
  const bundleState = hasInstaller && hasSignature ? `번들에 ${version} 설치본과 .sig 있음` : "번들에 이번 버전 산출물 없음(빌드 필요)"
  const collectedState = collected.length > 0 ? `, release/${tag}/에 ${collected.length}개 모임` : ""
  check(checks, "빌드 산출물", true, `${bundleState}${collectedState}`, false)

  return finish(root, version, tag, checks, warnings)
}

/** 디렉터리 한 곳의 파일 이름만 읽는다(내용은 읽지 않는다). */
function listFiles(dir: string): string[] {
  try {
    return execFileSync("cmd", ["/c", "dir", "/b", dir], {
      encoding: "utf8",
      windowsHide: true,
      stdio: ["ignore", "pipe", "pipe"]
    })
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter((line) => line !== "")
  } catch {
    return []
  }
}

function finish(
  root: string,
  version: string | null,
  tag: string | null,
  checks: Check[],
  warnings: string[] = []
): PreflightReport {
  const blockers = checks.filter((item) => !item.ok && item.blocking).map((item) => `${item.name}: ${item.detail}`)
  const failedInformational = checks
    .filter((item) => !item.ok && !item.blocking)
    .map((item) => `${item.name}: ${item.detail}`)
  return {
    root,
    version,
    tag,
    ready: blockers.length === 0,
    blockers: [...blockers, ...failedInformational],
    warnings,
    checks
  }
}

function renderPreflight(report: PreflightReport): string {
  const lines = [
    `릴리즈 프리플라이트: ${report.version ? `${report.tag} (${report.version})` : "버전 확인 실패"}`,
    `경로: ${report.root}`,
    `판정: ${report.ready ? "READY" : "BLOCKED"}`,
    ""
  ]
  for (const item of report.checks) {
    const mark = item.ok ? "PASS" : item.blocking ? "FAIL" : "WARN"
    lines.push(`  [${mark}] ${item.name}: ${item.detail}`)
  }
  if (report.blockers.length > 0) {
    lines.push("", "해결 필요:")
    for (const blocker of report.blockers) lines.push(`  - ${blocker}`)
  }
  if (report.warnings.length > 0) {
    lines.push("", "참고:")
    for (const warning of report.warnings) lines.push(`  - ${warning}`)
  }
  lines.push(
    "",
    report.ready
      ? "다음 단계: 필요하면 npm test와 cargo test를 먼저 돌리고, npm run release:local 로 빌드와 릴리스를 수행한다."
      : "다음 단계: 위 FAIL을 해결한다. 이 플러그인은 릴리스를 실행하지 않는다."
  )
  return lines.join("\n")
}

export function applyVersionBump(
  root: string,
  level: ReleaseLevel | undefined,
  explicit: string | undefined
): BumpReport {
  const versions = readAllVersions(root)
  const current = versions.tauriConfig
  if (!current) throw new Error(`${VERSION_FILES.tauriConfig}에서 버전을 읽지 못했다`)

  const mismatched = Object.entries(versions).filter(([key, value]) => key !== "tauriConfig" && value !== current)
  if (mismatched.length > 0) {
    throw new Error(
      `버전이 어긋나 있어 올릴 기준을 정할 수 없다: ${mismatched
        .map(([key, value]) => `${key}=${value ?? "없음"}`)
        .join(", ")} (${VERSION_FILES.tauriConfig}=${current})`
    )
  }

  let target: string
  if (explicit !== undefined) {
    if (!/^\d+\.\d+\.\d+$/.test(explicit)) throw new Error(`version은 x.y.z 형태여야 한다: ${explicit}`)
    target = explicit
  } else if (level) {
    target = nextVersion(current, level)
  } else {
    throw new Error("level(patch|minor|major) 또는 version 중 하나가 필요하다")
  }
  if (target === current) throw new Error(`이미 ${current}다. 올릴 버전이 없다`)
  if (compareVersion(target, current) <= 0) {
    throw new Error(`버전을 되돌릴 수 없다: ${current} → ${target}`)
  }

  const tag = `v${target}`
  const localTag = run("git", ["rev-list", "-n", "1", tag], root)
  if (localTag.ok) throw new Error(`태그 ${tag}가 이미 있다. 스크립트가 HEAD와 다른 커밋을 가리켜 중단한다`)
  const remoteTag = run("git", ["ls-remote", "--tags", "--exit-code", "origin", tag], root)
  if (remoteTag.ok) throw new Error(`원격에 태그 ${tag}가 이미 있다`)

  const warnings: string[] = []
  const dirty = run("git", ["status", "--porcelain"], root).stdout.split(/\r?\n/).filter((line) => line.trim() !== "")
  if (dirty.length > 0) {
    warnings.push(`버전과 무관한 변경 ${dirty.length}개가 이미 있다: ${dirty.slice(0, 5).join(", ")}`)
  }

  // 4곳을 먼저 모두 계산한 뒤에 한 번에 쓴다. 하나라도 패턴이 어긋나면 아무것도 바꾸지 않는다.
  const quoted = escapeRegExp(current)
  const edits: { path: string; next: string; label: string }[] = []
  edits.push({
    path: VERSION_FILES.packageJson,
    label: VERSION_FILES.packageJson,
    next: replaceExactlyOnce(
      readText(join(root, VERSION_FILES.packageJson)),
      new RegExp(`("version"\\s*:\\s*")${quoted}(")`),
      `$1${target}$2`,
      VERSION_FILES.packageJson
    )
  })
  edits.push({
    path: VERSION_FILES.tauriConfig,
    label: VERSION_FILES.tauriConfig,
    next: replaceExactlyOnce(
      readText(join(root, VERSION_FILES.tauriConfig)),
      new RegExp(`("version"\\s*:\\s*")${quoted}(")`),
      `$1${target}$2`,
      VERSION_FILES.tauriConfig
    )
  })
  const cargoToml = readText(join(root, VERSION_FILES.cargoToml))
  edits.push({
    path: VERSION_FILES.cargoToml,
    label: `${VERSION_FILES.cargoToml} [package]`,
    next: replaceInCargoPackageSection(cargoToml, current, target)
  })
  edits.push({
    path: VERSION_FILES.cargoLock,
    label: `${VERSION_FILES.cargoLock} (name = "araview")`,
    next: replaceExactlyOnce(
      readText(join(root, VERSION_FILES.cargoLock)),
      new RegExp(`(\\[\\[package\\]\\]\\s*\\nname = "araview"\\s*\\nversion = ")${quoted}(")`),
      `$1${target}$2`,
      `${VERSION_FILES.cargoLock} araview`
    )
  })

  for (const edit of edits) writeFileSync(join(root, edit.path), edit.next, "utf8")

  const after = readAllVersions(root)
  const drifted = Object.entries(after).filter(([, value]) => value !== target)
  if (drifted.length > 0) {
    throw new Error(`기록 후 확인이 어긋났다: ${drifted.map(([key, value]) => `${key}=${value}`).join(", ")}`)
  }

  return {
    root,
    from: current,
    to: target,
    tag,
    files: edits.map((edit) => ({ path: edit.label, from: current, to: target })),
    warnings
  }
}

function compareVersion(left: string, right: string): number {
  const parse = (value: string) => value.split(".").map((part) => Number(part))
  const a = parse(left)
  const b = parse(right)
  for (let index = 0; index < 3; index++) {
    const diff = (a[index] ?? 0) - (b[index] ?? 0)
    if (diff !== 0) return diff
  }
  return 0
}

function renderBump(report: BumpReport): string {
  const lines = [
    `버전 ${report.from} → ${report.to} (태그 ${report.tag})`,
    `경로: ${report.root}`,
    "",
    "변경한 파일:"
  ]
  for (const file of report.files) lines.push(`  - ${file.path}: ${file.from} → ${file.to}`)
  if (report.warnings.length > 0) {
    lines.push("", "참고:")
    for (const warning of report.warnings) lines.push(`  - ${warning}`)
  }
  lines.push(
    "",
    "커밋은 하지 않았다. 확인 후 커밋하고, 그 다음 npm run release:local 을 실행한다."
  )
  return lines.join("\n")
}

function pickDirectory(args: { directory?: string }, context: ToolExecuteContext, fallback: string): string {
  return args.directory ?? context.directory ?? fallback
}

function toolDirectory(args: ToolArgs, context: ToolExecuteContext, fallback: string) {
  return resolveRoot(pickDirectory(args, context, fallback))
}

export default {
  id: "araview.release",
  async setup(ctx: PluginContextLike) {
    const fallback = ctx.location?.directory ?? process.cwd()
    const tools = (ctx as unknown as { tool: ToolRegistrar }).tool

    await tools.transform((editor) => {
      editor.namespace({
        name: "release",
        description: "AraView 로컬 릴리즈 준비(버전 관리와 사전 검사)"
      })

      editor.add({
        name: "preflight",
        description: [
          "AraView 로컬 릴리즈 실행 전 조건을 읽기 전용으로 검사한다.",
          "버전 4곳(package.json, src-tauri/Cargo.toml, tauri.conf.json, Cargo.lock) 일치 여부,",
          "작업 트리, 원격 동기화, 태그와 HEAD 관계, gh 인증, 서명키 존재(경로만), 서명키 비밀번호 설정,",
          "updater endpoint가 가리키는 릴리스 저장소, 기존 빌드 산출물을 확인해 READY/BLOCKED를 낸다.",
          "릴리지를 실행하지 않으며 태그도 만들지 않는다. 서명키와 비밀번호 내용은 절대 출력하지 않는다."
        ].join(" "),
        input: {
          type: "object",
          properties: {
            directory: {
              type: "string",
              description: "AraView 저장소 경로. 기본값은 현재 세션 디렉터리"
            }
          },
          additionalProperties: false
        },
        options: { namespace: "release", codemode: true },
        execute: async (args, context) => {
          const { root, error } = toolDirectory(args, context, fallback)
          if (error) return { content: `릴리즈 프리플라이트 실패\n\n${error}` }
          return { content: renderPreflight(collectPreflight(root)) }
        }
      })

      editor.add({
        name: "bump",
        description: [
          "릴리즈 버전을 네 곳(package.json, src-tauri/Cargo.toml, src-tauri/tauri.conf.json,",
          "src-tauri/Cargo.lock의 araview 항목)을 한 번에 올린다.",
          "level로 patch/minor/major를 고르거나 version으로 정확한 버전을 준다.",
          "네 곳이 이미 어긋나 있으면 기준을 잃기 때문에 거부한다. 태그가 이미 있으면 중단한다.",
          "커밋과 릴리스 실행은 하지 않는다."
        ].join(" "),
        input: {
          type: "object",
          properties: {
            level: {
              type: "string",
              enum: ["patch", "minor", "major"],
              description: "올릴 단계. version을 주면 무시된다"
            },
            version: {
              type: "string",
              description: "정확한 x.y.z. 주면 level 대신 이 값으로 올라간다"
            },
            directory: {
              type: "string",
              description: "AraView 저장소 경로. 기본값은 현재 세션 디렉터리"
            }
          },
          additionalProperties: false
        },
        options: { namespace: "release", codemode: true },
        execute: async (args, context) => {
          const { root, error } = toolDirectory(args, context, fallback)
          if (error) return { content: `버전 올리기 실패\n\n${error}` }
          try {
            return { content: renderBump(applyVersionBump(root, args.level, args.version)) }
          } catch (bumpError) {
            return { content: `버전 올리기 실패\n\n${(bumpError as Error).message}` }
          }
        }
      })
    })
  }
}
