#!/usr/bin/env node
// 전용 Actions 체크아웃에서만 실행한다. 재시도 시 이 실행이 만든 버전 커밋/태그를 버린다.
import { execFileSync, spawnSync } from "node:child_process"
import { appendFileSync } from "node:fs"
import { resolve } from "node:path"
import { pathToFileURL } from "node:url"

export function isReleasePath(path) {
  return !path.endsWith(".md") && !path.startsWith("docs/") && !path.startsWith("site/")
}

export function autoBetaRelease(cwd, { attempts = 3 } = {}) {
  if (process.env.GITHUB_ACTIONS !== "true") throw new Error("전용 GitHub Actions 러너에서만 실행한다.")
  const git = (...args) => execFileSync("git", args, { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }).trim()
  if (git("branch", "--show-current") !== "main" || git("status", "--porcelain")) {
    throw new Error("깨끗한 main 체크아웃이 필요하다.")
  }
  git("fetch", "origin", "main", "--tags")
  // 체크아웃에만 있는 커밋은 자동 정리 대상이 아니다.
  git("merge-base", "--is-ancestor", "HEAD", "origin/main")

  for (let attempt = 1; attempt <= attempts; attempt++) {
    git("fetch", "origin", "main", "--tags")
    // 이 함수는 전용 러너에서만 호출한다. 사용자 작업 트리에서는 실행하지 않는다.
    git("reset", "--hard", "origin/main")
    const base = git("rev-parse", "HEAD")
    const previous = spawnSync("git", ["describe", "--tags", "--match", "v[0-9]*-beta.[0-9]*", "--abbrev=0", "HEAD"], { cwd, encoding: "utf8" })
    const paths = previous.status === 0
      ? git("diff", "--name-only", "-z", previous.stdout.trim(), "HEAD")
      : git("ls-tree", "-r", "--name-only", "-z", "HEAD")
    if (!paths.split("\0").filter(Boolean).some(isReleasePath)) {
      console.log("미릴리스 코드 변경 없음: 베타 생략")
      return ""
    }

    const tags = new Set(git("tag", "--list").split("\n"))
    const result = spawnSync(process.execPath, ["scripts/release.mjs", "beta", "--yes"], { cwd, stdio: "inherit" })
    if (result.status === 0) return git("describe", "--tags", "--exact-match", "HEAD")

    git("fetch", "origin", "main", "--no-tags")
    if (git("rev-parse", "origin/main") === base) {
      throw new Error("베타 릴리스 실패: 원격 변경이 없어 재시도하지 않는다.")
    }
    // atomic push 실패로 로컬에만 남은, 이번 시도가 생성한 태그만 제거한다.
    if (git("rev-parse", "HEAD") !== base) {
      for (const tag of git("tag", "--points-at", "HEAD").split("\n").filter(Boolean)) {
        if (!tags.has(tag)) git("tag", "-d", tag)
      }
    }
    git("reset", "--hard", base)
    console.log(`동시 푸시 감지: 최신 main으로 재시도 (${attempt}/${attempts})`)
  }
  throw new Error(`동시 푸시로 ${attempts}회 실패함. Actions에서 재실행한다.`)
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  if (process.env.GITHUB_ACTIONS !== "true") throw new Error("전용 GitHub Actions 러너에서만 실행한다.")
  const tag = autoBetaRelease(process.cwd())
  if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, `tag=${tag}\n`)
}
