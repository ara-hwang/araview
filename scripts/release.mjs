#!/usr/bin/env node
/**
 * 릴리스 시작 스크립트: 버전 올리기, 커밋, 태그, 푸시를 한 번에 한다.
 *
 *   npm run release -- 1.0.1          정확한 버전
 *   npm run release -- patch          patch | minor | major
 *   npm run release -- patch --dry-run  파일과 git을 건드리지 않고 계획만 출력
 *   npm run release -- patch --yes    확인 질문 생략
 *
 * 태그(vX.Y.Z)가 푸시되면 .github/workflows/release.yml이 검증, 서명 빌드,
 * GitHub 릴리스 생성을 이어받는다. 이 스크립트는 빌드나 릴리스를 직접 하지 않는다.
 * 의존성 없이 Node 표준 라이브러리만 쓴다.
 */
import { execFileSync, spawnSync } from "node:child_process"
import { readFileSync, writeFileSync } from "node:fs"
import { dirname, join, resolve } from "node:path"
import { createInterface } from "node:readline/promises"
import { fileURLToPath } from "node:url"

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..")
const SEMVER = /^\d+\.\d+\.\d+$/

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

function compare(a, b) {
  const pa = a.split(".").map(Number)
  const pb = b.split(".").map(Number)
  for (let i = 0; i < 3; i++) {
    if (pa[i] !== pb[i]) return pa[i] - pb[i]
  }
  return 0
}

function nextVersion(current, request) {
  if (SEMVER.test(request)) return request
  const [major, minor, patch] = current.split(".").map(Number)
  if (request === "major") return `${major + 1}.0.0`
  if (request === "minor") return `${major}.${minor + 1}.0`
  if (request === "patch") return `${major}.${minor}.${patch + 1}`
  fail(`버전은 X.Y.Z 또는 patch|minor|major 여야 한다: ${request ?? "(없음)"}`)
}

// 버전 파일마다 "현재 값을 찾는 정규식"과 "바꿀 위치"를 한곳에 둔다.
// 첫 번째 캡처 그룹 앞, 두 번째 캡처 그룹 뒤를 보존하고 가운데 버전만 교체한다.
const FILES = [
  { path: "package.json", pattern: /^(\s*"version":\s*")([^"]+)(")/m, expected: 1 },
  {
    path: "package-lock.json",
    pattern: /("name": "araview",\s*"version": ")([^"]+)(")/g,
    expected: 2
  },
  { path: "src-tauri/tauri.conf.json", pattern: /^(\s*"version":\s*")([^"]+)(")/m, expected: 1 },
  {
    path: "src-tauri/Cargo.toml",
    pattern: /(\[package\][\s\S]*?^version = ")([^"]+)(")/m,
    expected: 1
  },
  {
    path: "src-tauri/Cargo.lock",
    pattern: /(\[\[package\]\]\r?\nname = "araview"\r?\nversion = ")([^"]+)(")/g,
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

const files = readVersions()
const versions = new Set(files.map((f) => f.version))
if (versions.size !== 1) {
  fail(`버전 파일이 서로 다르다: ${files.map((f) => `${f.path}=${f.version}`).join(", ")}`)
}
const current = files[0].version
const next = nextVersion(current, target)
const tag = `v${next}`

// 사전 검사. 하나라도 어긋나면 파일을 쓰기 전에 멈춘다.
if (compare(next, current) <= 0) fail(`새 버전 ${next}는 현재 ${current}보다 커야 한다.`)
if (git("rev-parse", "--abbrev-ref", "HEAD") !== "main") fail("main 브랜치에서만 릴리스한다.")
if (git("status", "--porcelain")) fail("작업 트리에 커밋하지 않은 변경이 있다. 먼저 정리한다.")
git("fetch", "--quiet", "origin")
const [, behind] = git("rev-list", "--left-right", "--count", "HEAD...origin/main")
  .split(/\s+/)
  .map(Number)
if (behind > 0) fail(`origin/main보다 ${behind}커밋 뒤처져 있다. 먼저 pull 한다.`)
if (git("tag", "-l", tag)) fail(`로컬에 태그 ${tag}가 이미 있다.`)
if (git("ls-remote", "--tags", "origin", `refs/tags/${tag}`))
  fail(`원격에 태그 ${tag}가 이미 있다.`)

const [ahead] = git("rev-list", "--left-right", "--count", "HEAD...origin/main")
  .split(/\s+/)
  .map(Number)
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
  const result = spawnSync(command, commandArgs, {
    cwd: ROOT,
    stdio: "inherit",
    shell: true,
    ...options
  })
  if (result.status !== 0) {
    git("checkout", "--", ...files.map((f) => f.path))
    fail(`${command} ${commandArgs.join(" ")} 실패. 버전 파일을 되돌렸다.`)
  }
}

// Cargo.lock이 Cargo.toml과 맞는지, 문서-코드 정합성이 깨지지 않았는지 확인한다.
run("cargo", ["metadata", "--locked", "--no-deps", "--format-version", "1"], {
  cwd: join(ROOT, "src-tauri"),
  stdio: "ignore"
})
run("npm", ["run", "docs:check"], { stdio: "ignore" })

git("add", ...files.map((f) => f.path))
git("commit", "-m", `chore: 버전 ${next}`)
git("tag", "-a", tag, "-m", tag)
execFileSync("git", ["push", "--atomic", "origin", "main", tag], { cwd: ROOT, stdio: "inherit" })

console.log(`\n${tag} 푸시 완료. 진행 상황: https://github.com/ara-hwang/araview/actions`)
