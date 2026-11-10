#!/usr/bin/env node
/**
 * 릴리스 시작 스크립트: 버전 올리기, 커밋, 태그, 푸시를 한 번에 한다.
 *
 *   npm run release -- patch            main에서 다음 patch (minor|major 동일)
 *   npm run release -- 2.7.0            main에서 정확한 버전
 *   npm run release -- patch --dry-run  파일과 git을 건드리지 않고 계획만 출력
 *   npm run release -- patch --yes      확인 질문 생략
 *
 * 릴리스는 main 한 곳에서 낸다. 태그(vX.Y.Z)가 푸시되면
 * .github/workflows/release.yml이 검증, 설치 프로그램 빌드와 서명, 릴리스 생성을
 * 이어받는다. 이 스크립트는 빌드나 릴리스를 직접 하지 않는다. 의존성 없이 Node
 * 표준 라이브러리만 쓴다.
 */
import { execFileSync, spawnSync } from "node:child_process"
import { readFileSync, writeFileSync } from "node:fs"
import { dirname, join, resolve } from "node:path"
import { createInterface } from "node:readline/promises"
import { fileURLToPath, pathToFileURL } from "node:url"

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..")
const STABLE = /^\d+\.\d+\.\d+$/
const PRERELEASE = /^(\d+)\.(\d+)\.(\d+)-beta\.(\d+)$/

/** 사전 검사 실패. 메인 흐름이 잡아 `중단: ...`으로 알리고 exit 1로 끝낸다. */
class ReleaseError extends Error {}

function fail(message) {
  throw new ReleaseError(message)
}

function git(...gitArgs) {
  return execFileSync("git", gitArgs, { cwd: ROOT, encoding: "utf8" }).trim()
}

function gitQuiet(...gitArgs) {
  const result = spawnSync("git", gitArgs, { cwd: ROOT, encoding: "utf8" })
  return result.status === 0 ? result.stdout.trim() : null
}

export function parseVersion(version) {
  const match = /^v?(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/.exec(
    version.trim()
  )
  if (!match) return null
  return {
    core: [Number(match[1]), Number(match[2]), Number(match[3])],
    pre: match[4] ? match[4].split(".") : null
  }
}

/** 세미버전 규칙으로 두 버전을 비교한다. a가 크면 양수, b가 크면 음수다. */
export function compareVersions(a, b) {
  const pa = parseVersion(a)
  const pb = parseVersion(b)
  if (!pa || !pb) fail(`버전을 해석하지 못했다: ${a} / ${b}`)
  for (let i = 0; i < 3; i++) {
    if (pa.core[i] !== pb.core[i]) return pa.core[i] - pb.core[i]
  }
  if (!pa.pre && !pb.pre) return 0
  if (!pa.pre) return 1
  if (!pb.pre) return -1
  for (let i = 0; i < Math.min(pa.pre.length, pb.pre.length); i++) {
    const x = pa.pre[i]
    const y = pb.pre[i]
    const xn = /^\d+$/.test(x)
    const yn = /^\d+$/.test(y)
    if (xn && yn) {
      const diff = Number(x) - Number(y)
      if (diff !== 0) return diff
    } else if (xn) {
      return -1
    } else if (yn) {
      return 1
    } else if (x !== y) {
      return x < y ? -1 : 1
    }
  }
  return pa.pre.length - pb.pre.length
}

/** 태그 목록에서 정식(`vX.Y.Z`) 릴리스 중 가장 높은 버전. 없으면 null이다. */
export function latestStable(tags) {
  let latest = null
  for (const tag of tags) {
    const version = tag.trim().replace(/^v/, "")
    if (!STABLE.test(version)) continue
    if (!latest || compareVersions(version, latest) > 0) latest = version
  }
  return latest
}

/** 정식 버전에 patch/minor/major를 적용한다. */
function bumpStable(version, request) {
  const [major, minor, patch] = version.split(".").map(Number)
  if (request === "major") return `${major + 1}.0.0`
  if (request === "minor") return `${major}.${minor + 1}.0`
  return `${major}.${minor}.${patch + 1}`
}

/**
 * 다음 버전을 계산한다. `stableTags`는 저장소의 태그 목록(정식이 아닌 태그는 무시)이다.
 *
 * 베타 채널을 없애기 전에 낸 `X.Y.Z-beta.N`이 버전 파일에 남아 있을 수 있으므로 현재
 * 버전의 프리릴리스 접미사는 받아들인다. 이때 접미사를 뗀 `X.Y.Z`를 기준으로 하되,
 * 세미버전 관례대로 아직 정식으로 내지 않은 `X.Y.Z`는 건너뛰지 않는다(patch는 `X.Y.Z`,
 * `Z`가 0이면 minor도 `X.Y.Z`, `Y.Z`가 0이면 major도 `X.Y.Z`). 그 `X.Y.Z` 이상의 정식
 * 태그가 이미 있으면 그 태그에서 올린다.
 *
 * `patch|minor|major`는 현재 버전이 정식이면 그대로 올린다. 정확한 버전은 `X.Y.Z`
 * 형식이면 되고, 현재보다 커야 한다는 검사는 호출한 쪽이 한다.
 */
export function nextVersion(current, request, stableTags = []) {
  const latest = latestStable(stableTags)
  const released = (core) => latest !== null && compareVersions(latest, core) >= 0
  const beta = PRERELEASE.exec(current)
  if (!beta && !STABLE.test(current)) fail(`현재 버전을 해석하지 못했다: ${current}`)
  const core = beta ? `${beta[1]}.${beta[2]}.${beta[3]}` : current

  if (request === "patch" || request === "minor" || request === "major") {
    if (!beta) return bumpStable(current, request)
    if (released(core)) return bumpStable(latest, request)
    const [, minor, patch] = core.split(".").map(Number)
    if (request === "patch") return core
    if (request === "minor" && patch === 0) return core
    if (request === "major" && minor === 0 && patch === 0) return core
    return bumpStable(core, request)
  }
  if (!STABLE.test(request ?? "")) fail(`버전은 X.Y.Z 또는 patch|minor|major 여야 한다: ${request}`)
  return request
}

// 버전 파일마다 "현재 값을 찾는 정규식"과 "바꿀 위치"를 한곳에 둔다.
// 첫 번째 캡처 그룹 앞, 두 번째 캡처 그룹 뒤를 보존하고 가운데 버전만 교체한다.
const FILES = [
  {
    path: "app/Cargo.toml",
    pattern: /(\[package\][\s\S]*?^version = ")([^"]+)(")/m,
    expected: 1
  },
  {
    path: "Cargo.lock",
    pattern: /(\[\[package\]\]\r?\nname = "araview"\r?\nversion = ")([^"]+)(")/g,
    expected: 1
  },
  {
    path: "vcpkg.json",
    pattern: /("version-string": ")([^"]+)(")/,
    expected: 1
  }
]

function readVersions() {
  return FILES.map((file) => {
    const text = readFileSync(join(ROOT, file.path), "utf8")
    const matches = [...text.matchAll(new RegExp(file.pattern.source, "gm"))]
    if (matches.length < 1) fail(`${file.path}에서 버전을 찾지 못했다.`)
    return { ...file, text, version: matches[0][2] }
  })
}

async function main() {
  const args = process.argv.slice(2)
  const dryRun = args.includes("--dry-run")
  const yes = args.includes("--yes")
  const target = args.find((a) => !a.startsWith("--"))

  const branch = git("rev-parse", "--abbrev-ref", "HEAD")
  if (branch !== "main") fail("main 브랜치에서만 릴리스한다.")
  if (!target) fail("릴리스할 버전을 지정한다: patch, minor, major 또는 X.Y.Z")

  const files = readVersions()
  const versions = new Set(files.map((f) => f.version))
  if (versions.size !== 1) {
    fail(`버전 파일이 서로 다르다: ${files.map((f) => `${f.path}=${f.version}`).join(", ")}`)
  }
  const current = files[0].version
  if (!STABLE.test(current) && !PRERELEASE.test(current)) {
    fail(`현재 버전은 X.Y.Z 이어야 한다(옛 베타 빌드는 X.Y.Z-beta.N 허용): ${current}`)
  }
  // 다음 버전은 이미 나간 정식 태그에 따라 달라지므로 태그를 먼저 받아 둔다.
  git("fetch", "--quiet", "--tags", "origin")
  const next = nextVersion(current, target, git("tag", "--list", "v*").split("\n"))
  const tag = `v${next}`

  // 사전 검사. 하나라도 어긋나면 파일을 쓰기 전에 멈춘다.
  if (compareVersions(next, current) <= 0) fail(`새 버전 ${next}는 현재 ${current}보다 커야 한다.`)
  if (git("status", "--porcelain")) fail("작업 트리에 커밋하지 않은 변경이 있다. 먼저 정리한다.")
  if (!gitQuiet("rev-parse", "--verify", `origin/${branch}`)) {
    fail(`원격에 ${branch} 브랜치가 없다. 먼저 푸시한다.`)
  }
  const [, behind] = git("rev-list", "--left-right", "--count", `HEAD...origin/${branch}`)
    .split(/\s+/)
    .map(Number)
  if (behind > 0) fail(`origin/${branch}보다 ${behind}커밋 뒤처져 있다. 먼저 pull 한다.`)
  if (git("tag", "-l", tag)) fail(`로컬에 태그 ${tag}가 이미 있다.`)
  if (git("ls-remote", "--tags", "origin", `refs/tags/${tag}`))
    fail(`원격에 태그 ${tag}가 이미 있다.`)

  const [ahead] = git("rev-list", "--left-right", "--count", `HEAD...origin/${branch}`)
    .split(/\s+/)
    .map(Number)
  console.log(`브랜치: ${branch}`)
  console.log(`버전: ${current} -> ${next} (태그 ${tag})`)
  for (const file of files) console.log(`  ${file.path}`)
  if (ahead > 0) console.log(`참고: 아직 푸시하지 않은 커밋 ${ahead}개가 함께 올라간다.`)

  if (dryRun) {
    console.log("--dry-run: 파일, 커밋, 태그, 푸시를 하지 않았다.")
    return
  }

  if (!yes) {
    const rl = createInterface({ input: process.stdin, output: process.stdout })
    const answer = await rl.question(
      `${tag} 태그를 푸시하면 공개 릴리스가 시작된다. 진행할까? (y/N) `
    )
    rl.close()
    if (answer.trim().toLowerCase() !== "y") fail("취소했다.")
  }

  // 모든 내용을 먼저 계산해 두고 한꺼번에 쓴다.
  const updated = files.map((file) => {
    let count = 0
    const text = file.text.replace(file.pattern, (_all, pre, _old, post) => {
      count += 1
      return `${pre}${next}${post}`
    })
    if (count !== file.expected)
      fail(`${file.path}에서 ${count}곳을 바꿨다(기대 ${file.expected}곳).`)
    return { path: file.path, text }
  })
  for (const file of updated) writeFileSync(join(ROOT, file.path), file.text)

  function run(command, commandArgs, options = {}) {
    const result = spawnSync(command, commandArgs, { cwd: ROOT, stdio: "inherit", ...options })
    if (result.status !== 0) {
      git("checkout", "--", ...files.map((f) => f.path))
      fail(`${command} ${commandArgs.join(" ")} 실패. 버전 파일을 되돌렸다.`)
    }
  }

  // Cargo.lock이 Cargo.toml과 맞는지, 문서-코드 정합성이 깨지지 않았는지 확인한다.
  run("cargo", ["metadata", "--locked", "--no-deps", "--format-version", "1"], {
    stdio: "ignore"
  })
  run(process.execPath, [join(ROOT, "scripts", "check-docs.mjs")], { stdio: "ignore" })

  git("add", ...files.map((f) => f.path))
  git("commit", "-m", `chore: 버전 ${next}`)
  git("tag", "-a", tag, "-m", tag)
  execFileSync("git", ["push", "--atomic", "origin", branch, tag], { cwd: ROOT, stdio: "inherit" })

  console.log(`\n${tag} 푸시 완료. 진행 상황: https://github.com/ara-hwang/araview/actions`)
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    await main()
  } catch (error) {
    if (!(error instanceof ReleaseError)) throw error
    console.error(`중단: ${error.message}`)
    process.exit(1)
  }
}
