import assert from "node:assert/strict"
import { execFileSync } from "node:child_process"
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import test from "node:test"
import { autoBetaRelease, isReleasePath } from "./auto-beta-release.mjs"
import { isNewerBeta, publishBetaFeed } from "./publish-beta-feed.mjs"

test("문서/사이트 제외, 코드 및 빌드 설정 포함", () => {
  for (const path of ["README.md", "docs/diagram.svg", "site/index.html"]) assert.equal(isReleasePath(path), false)
  for (const path of ["app/src/main.rs", "Cargo.lock", ".github/workflows/release.yml"]) assert.equal(isReleasePath(path), true)
})

test("베타 버전의 숫자 비교 및 잘못된 버전 거부", () => {
  assert.equal(isNewerBeta("2.7.0-beta.10", "2.7.0-beta.9"), true)
  assert.equal(isNewerBeta("2.8.0-beta.1", "2.7.0-beta.99"), true)
  assert.equal(isNewerBeta("2.7.0-beta.9", "2.7.0-beta.10"), false)
  assert.equal(isNewerBeta("2.7.0-beta.10", "2.7.0-beta.10"), false)
  assert.throws(() => isNewerBeta("2.7.0", "2.7.0-beta.10"))
  assert.throws(() => isNewerBeta("2.7.0-beta.10", "invalid"))
})

function temporary(t) {
  const parent = process.env.RUNNER_TEMP ?? (existsSync("/tmp/opencode") ? "/tmp/opencode" : tmpdir())
  const dir = mkdtempSync(join(parent, "araview-release-test-"))
  t.after(() => rmSync(dir, { recursive: true, force: true }))
  return dir
}

for (const current of ["2.7.0-beta.9", "2.7.0-beta.10", "2.7.0-beta.11"]) {
  test(`베타 피드 ${current} 이후 늦게 끝난 beta.10 게시`, (t) => {
    const file = join(temporary(t), "update-feed-beta.json")
    writeFileSync(file, '\uFEFF{"version":"2.7.0-beta.10"}')
    const calls = []
    const gh = (args) => {
      calls.push(args)
      if (args[0] === "api") return JSON.stringify({ assets: [{ name: "update-feed-beta.json" }] })
      if (args[1] === "download") return JSON.stringify({ version: current })
      return ""
    }
    assert.equal(publishBetaFeed("test/repo", file, gh), current === "2.7.0-beta.9")
    assert.equal(calls.some((args) => args[1] === "upload"), current === "2.7.0-beta.9")
  })
}

test("피드 최초 생성, 자산 누락 및 인증/다운로드/JSON 오류 처리", (t) => {
  const file = join(temporary(t), "update-feed-beta.json")
  writeFileSync(file, '{"version":"2.7.0-beta.10"}')
  for (const missingRelease of [true, false]) {
    const calls = []
    publishBetaFeed("test/repo", file, (args) => {
      calls.push(args)
      if (args[0] === "api") {
        if (missingRelease) throw Object.assign(new Error("not found"), { stderr: "gh: Not Found (HTTP 404)" })
        return '{"assets":[]}'
      }
      return ""
    })
    assert.equal(calls.some((args) => args[1] === "create"), missingRelease)
    assert.equal(calls.some((args) => args[1] === "upload"), true)
  }
  for (const failure of ["auth", "download", "json", "version"]) {
    const calls = []
    assert.throws(() => publishBetaFeed("test/repo", file, (args) => {
      calls.push(args)
      if (failure === "auth") throw Object.assign(new Error("unauthorized"), { stderr: "(HTTP 401)" })
      if (args[0] === "api") return '{"assets":[{"name":"update-feed-beta.json"}]}'
      if (failure === "download") throw new Error("network failure")
      return failure === "json" ? "invalid JSON" : '{"version":"invalid"}'
    }))
    assert.equal(calls.some((args) => args[1] === "upload" || args[1] === "create"), false)
  }
})

function repository(t) {
  const root = temporary(t)
  const remote = join(root, "origin.git")
  const cwd = join(root, "runner")
  const git = (dir, ...args) => execFileSync("git", args, { cwd: dir, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }).trim()
  git(root, "init", "--bare", "--initial-branch=main", remote)
  git(root, "clone", remote, cwd)
  git(cwd, "config", "user.name", "test")
  git(cwd, "config", "user.email", "test@example.invalid")
  mkdirSync(join(cwd, "app/src"), { recursive: true })
  mkdirSync(join(cwd, "scripts"))
  writeFileSync(join(cwd, "Cargo.toml"), '[workspace]\nmembers = ["app"]\nresolver = "2"\n')
  writeFileSync(join(cwd, "app/Cargo.toml"), '[package]\nname = "araview"\nversion = "1.0.0-beta.1"\nedition = "2021"\n')
  writeFileSync(join(cwd, "app/src/lib.rs"), "// fixture\n")
  writeFileSync(join(cwd, "vcpkg.json"), '{"version-string": "1.0.0-beta.1"}\n')
  copyFileSync(new URL("./release.mjs", import.meta.url), join(cwd, "scripts/release.mjs"))
  writeFileSync(join(cwd, "scripts/check-docs.mjs"), "// fixture\n")
  execFileSync("cargo", ["generate-lockfile", "--offline"], { cwd, stdio: "pipe" })
  const commit = (path, text, message = "fix: 테스트 변경") => {
    writeFileSync(join(cwd, path), text)
    git(cwd, "add", ".")
    git(cwd, "commit", "-m", message)
    git(cwd, "push", "origin", "main", "--tags")
  }
  commit("README.md", "initial", "chore: 초기화")
  git(cwd, "tag", "v1.0.0-beta.1")
  git(cwd, "push", "origin", "--tags")
  const actions = process.env.GITHUB_ACTIONS
  process.env.GITHUB_ACTIONS = "true"
  t.after(() => {
    if (actions === undefined) delete process.env.GITHUB_ACTIONS
    else process.env.GITHUB_ACTIONS = actions
  })
  return { root, remote, cwd, git, commit }
}

test("실제 릴리스 스크립트: 문서만 변경/기배포 HEAD는 무배포, 코드 변경은 1회 배포", (t) => {
  const { cwd, git, commit } = repository(t)
  commit("README.md", "docs only", "docs: 안내 수정")
  assert.equal(autoBetaRelease(cwd), "")
  commit("app/src/lib.rs", "// changed\n")
  assert.equal(autoBetaRelease(cwd), "v1.0.0-beta.2")
  assert.equal(git(cwd, "rev-parse", "HEAD"), git(cwd, "rev-parse", "origin/main"))
  assert.equal(autoBetaRelease(cwd), "")
})

for (const mode of ["code", "release", "exhausted"]) {
  test(`실제 atomic push 충돌 처리: ${mode}`, (t) => {
    const { root, remote, cwd, git, commit } = repository(t)
    const writer = join(root, "writer")
    git(root, "clone", remote, writer)
    git(writer, "config", "user.name", "writer")
    git(writer, "config", "user.email", "writer@example.invalid")
    // docs 검증 직후 경쟁자가 커밋을 푸시해 실제 release.mjs의 atomic push를 거부시킨다.
    commit("scripts/check-docs.mjs", `
      import { execFileSync } from "node:child_process"
      import { existsSync, writeFileSync } from "node:fs"
      const marker = ${JSON.stringify(join(root, "raced"))}
      if (!existsSync(marker)) {
        writeFileSync(marker, "done")
        const git = (...args) => execFileSync("git", args, { cwd: ${JSON.stringify(writer)}, stdio: "pipe" })
        git("pull", "--ff-only")
        writeFileSync(${JSON.stringify(join(writer, "app/src/lib.rs"))}, "// competing change\\n")
        git("add", ".")
        git("commit", "-m", "fix: 동시 푸시")
        git("push", "origin", "main")
        if (${JSON.stringify(mode)} === "release") {
          execFileSync(process.execPath, ["scripts/release.mjs", "beta", "--yes"], { cwd: ${JSON.stringify(writer)}, stdio: "pipe" })
        }
      }
    `)
    if (mode === "exhausted") {
      assert.throws(() => autoBetaRelease(cwd, { attempts: 1 }), /1회 실패/)
      assert.equal(git(cwd, "tag", "--list", "v1.0.0-beta.2"), "")
      return
    }
    assert.equal(autoBetaRelease(cwd), mode === "release" ? "" : "v1.0.0-beta.2")
    assert.equal(readFileSync(join(cwd, "app/src/lib.rs"), "utf8"), "// competing change\n")
    assert.equal(git(cwd, "tag", "--list", "v1.0.0-beta.*").split("\n").length, 2)
  })
}

test("검증 실패는 재시도하지 않고 작업 트리/로컬 커밋 보호", (t) => {
  const { cwd, git, commit } = repository(t)
  commit("scripts/check-docs.mjs", "process.exit(1)\n")
  assert.throws(() => autoBetaRelease(cwd), /원격 변경이 없어/)
  assert.equal(git(cwd, "tag", "--list", "v1.0.0-beta.2"), "")
  writeFileSync(join(cwd, "README.md"), "uncommitted")
  assert.throws(() => autoBetaRelease(cwd), /깨끗한 main/)
  git(cwd, "add", ".")
  git(cwd, "commit", "-m", "docs: 로컬 변경")
  const head = git(cwd, "rev-parse", "HEAD")
  assert.throws(() => autoBetaRelease(cwd))
  assert.equal(git(cwd, "rev-parse", "HEAD"), head)
})

test("문서 전용 푸시 제외 및 조건부 릴리스 호출", () => {
  const workflow = readFileSync(new URL("../.github/workflows/beta-release.yml", import.meta.url), "utf8")
  assert.match(workflow, /paths-ignore:/)
  assert.match(workflow, /steps\.bump\.outputs\.tag != ''/)
})

test("베타 게시 직렬화 및 단조 증가 피드 갱신", () => {
  const workflow = readFileSync(new URL("../.github/workflows/release.yml", import.meta.url), "utf8")
  assert.match(workflow, /group:.*beta-publication/)
  assert.match(workflow, /queue: max/)
  assert.match(workflow, /node scripts\/publish-beta-feed\.mjs/)
})
