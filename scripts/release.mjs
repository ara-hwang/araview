#!/usr/bin/env node
/**
 * 릴리스 시작 스크립트: 버전 올리기, 커밋, 태그, 푸시를 한 번에 한다.
 *
 *   npm run release -- beta            main에서 다음 베타 (2.7.0-beta.1 -> 2.7.0-beta.2)
 *   npm run release -- 2.7.0-beta.1    main에서 정확한 베타 버전
 *   npm run release -- patch           release에서 안정 patch (minor|major 동일)
 *   npm run release -- 2.7.0           release에서 정확한 안정 버전
 *   npm run release -- beta --dry-run  파일과 git을 건드리지 않고 계획만 출력
 *   npm run release -- beta --yes      확인 질문 생략
 *
 * main은 베타 채널, release는 안정 채널이다. 태그(vX.Y.Z 또는
 * vX.Y.Z-beta.N)가 푸시되면 .github/workflows/release.yml이 검증, 설치 프로그램
 * 빌드와 서명, 릴리스 생성을 이어받는다. 이 스크립트는 빌드나 릴리스를 직접 하지
 * 않는다. 의존성 없이 Node 표준 라이브러리만 쓴다.
 */
import { execFileSync, spawnSync } from "node:child_process"
import { readFileSync, writeFileSync } from "node:fs"
import { dirname, join, resolve } from "node:path"
import { createInterface } from "node:readline/promises"
import { fileURLToPath } from "node:url"

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..")
const STABLE = /^\d+\.\d+\.\d+$/
const BETA = /^(\d+)\.(\d+)\.(\d+)-beta\.(\d+)$/

const args = process.argv.slice(2)
const dryRun = args.includes("--dry-run")
const yes = args.includes("--yes")
const target = args.find((a) => !a.startsWith("--"))

function fail(message) {
  console.error(`중단: ${message}`)
  process.exit(1)
}

function git(...gitArgs) {
  return execFileSync("git", gitArgs, { cwd: ROOT, encoding: "utf8" }).trim()
}

function gitQuiet(...gitArgs) {
  const result = spawnSync("git", gitArgs, { cwd: ROOT, encoding: "utf8" })
  return result.status === 0 ? result.stdout.trim() : null
}

function parseVersion(version) {
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
function compareVersions(a, b) {
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

function nextVersion(current, request, channel) {
  if (channel === "beta") {
    if (request === "beta") {
      const match = BETA.exec(current)
      if (match) {
        return `${match[1]}.${match[2]}.${match[3]}-beta.${Number(match[4]) + 1}`
      }
      // 안정 버전에서 시작하는 첫 베타는 다음 minor의 beta.1이다.
      const [major, minor] = current.split(".").map(Number)
      return `${major}.${minor + 1}.0-beta.1`
    }
    if (!BETA.test(request)) fail(`베타 채널 버전은 X.Y.Z-beta.N 형식이어야 한다: ${request}`)
    return request
  }

  if (request === "patch" || request === "minor" || request === "major") {
    const [major, minor, patch] = current.split(".").map(Number)
    if (request === "major") return `${major + 1}.0.0`
    if (request === "minor") return `${major}.${minor + 1}.0`
    return `${major}.${minor}.${patch + 1}`
  }
  if (!STABLE.test(request)) fail(`안정 채널 버전은 X.Y.Z 형식이어야 한다: ${request}`)
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

const branch = git("rev-parse", "--abbrev-ref", "HEAD")
let channel
if (branch === "main") channel = "beta"
else if (branch === "release") channel = "stable"
else fail("main(베타) 또는 release(안정) 브랜치에서만 릴리스한다.")

const files = readVersions()
const versions = new Set(files.map((f) => f.version))
if (versions.size !== 1) {
  fail(`버전 파일이 서로 다르다: ${files.map((f) => `${f.path}=${f.version}`).join(", ")}`)
}
const current = files[0].version
if (channel === "stable" && !STABLE.test(current)) {
  fail(`${branch}(안정 채널)의 현재 버전은 X.Y.Z 이어야 한다: ${current}`)
}
const next = nextVersion(current, target, channel)
const tag = `v${next}`

// 사전 검사. 하나라도 어긋나면 파일을 쓰기 전에 멈춘다.
if (compareVersions(next, current) <= 0) fail(`새 버전 ${next}는 현재 ${current}보다 커야 한다.`)
if (git("status", "--porcelain")) fail("작업 트리에 커밋하지 않은 변경이 있다. 먼저 정리한다.")
git("fetch", "--quiet", "origin")
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
console.log(`채널: ${channel === "beta" ? "베타" : "안정"} (${branch})`)
console.log(`버전: ${current} -> ${next} (태그 ${tag})`)
for (const file of files) console.log(`  ${file.path}`)
if (ahead > 0) console.log(`참고: 아직 푸시하지 않은 커밋 ${ahead}개가 함께 올라간다.`)

if (dryRun) {
  console.log("--dry-run: 파일, 커밋, 태그, 푸시를 하지 않았다.")
  process.exit(0)
}

if (!yes) {
  const rl = createInterface({ input: process.stdin, output: process.stdout })
  const answer = await rl.question(
    `${tag} 태그를 푸시하면 ${channel === "beta" ? "공개 베타" : "공개"} 릴리스가 시작된다. 진행할까? (y/N) `
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
