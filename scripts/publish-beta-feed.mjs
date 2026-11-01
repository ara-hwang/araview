#!/usr/bin/env node
// release 잡의 beta-publication 잠금 안에서 읽기/비교/업로드를 모두 수행한다.
import { execFileSync } from "node:child_process"
import { readFileSync } from "node:fs"
import { resolve } from "node:path"
import { pathToFileURL } from "node:url"

function betaParts(version) {
  const match = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)-beta\.(0|[1-9]\d*)$/.exec(version)
  if (!match) throw new Error(`잘못된 베타 피드 버전: ${version}`)
  return match.slice(1).map(BigInt)
}

export function isNewerBeta(next, current) {
  const a = betaParts(next)
  const b = betaParts(current)
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return a[i] > b[i]
  }
  return false
}

export function publishBetaFeed(repo, feedPath, gh = (args) => execFileSync("gh", args, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] })) {
  const next = JSON.parse(readFileSync(feedPath, "utf8").replace(/^\uFEFF/, ""))
  betaParts(next.version)
  let release
  try {
    release = JSON.parse(gh(["api", `repos/${repo}/releases/tags/beta`]))
  } catch (error) {
    // 인증/통신 오류를 미생성으로 취급하지 않아 기존 피드를 보호한다.
    if (!String(error.stderr).includes("(HTTP 404)")) throw error
    gh(["release", "create", "beta", "--repo", repo, "--target", "main", "--prerelease", "--latest=false", "--title", "AraView beta feed", "--notes", "베타 채널 업데이트 피드"])
    release = { assets: [] }
  }
  if (release.assets.some((asset) => asset.name === "update-feed-beta.json")) {
    const current = JSON.parse(gh(["release", "download", "beta", "--repo", repo, "--pattern", "update-feed-beta.json", "--output", "-"]).replace(/^\uFEFF/, ""))
    if (!isNewerBeta(next.version, current.version)) {
      console.log(`피드 유지: ${current.version} >= ${next.version}`)
      return false
    }
  }
  gh(["release", "upload", "beta", feedPath, "--clobber", "--repo", repo])
  return true
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  publishBetaFeed(process.env.GITHUB_REPOSITORY, "dist/update-feed-beta.json")
}
