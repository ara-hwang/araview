import assert from "node:assert/strict"
import test from "node:test"
import { compareVersions, latestStable, nextVersion } from "./release.mjs"

test("정식 버전에 patch/minor/major를 적용", () => {
  assert.equal(nextVersion("2.7.0", "patch", []), "2.7.1")
  assert.equal(nextVersion("2.7.0", "minor", []), "2.8.0")
  assert.equal(nextVersion("2.7.0", "major", []), "3.0.0")
  assert.equal(nextVersion("2.7.0", "patch", ["v2.7.0"]), "2.7.1")
  assert.equal(nextVersion("2.7.0", "minor", ["v2.7.0"]), "2.8.0")
  assert.equal(nextVersion("2.7.0", "major", ["v2.7.0"]), "3.0.0")
})

test("베타 채널을 없애기 전의 프리릴리스 버전에서도 다음 정식 버전을 계산", () => {
  // main(2.8.0-beta.N)을 그대로 이어받은 직후: 접미사를 뗀 2.8.0을 건너뛰지 않는다.
  const tags = ["v2.7.0", "v2.7.1"]
  assert.equal(nextVersion("2.8.0-beta.5", "patch", tags), "2.8.0")
  assert.equal(nextVersion("2.8.0-beta.5", "minor", tags), "2.8.0")
  assert.equal(nextVersion("2.8.0-beta.5", "major", tags), "3.0.0")
  assert.equal(nextVersion("3.0.0-beta.2", "major", tags), "3.0.0")
  assert.equal(nextVersion("2.8.1-beta.2", "minor", tags), "2.9.0")
  assert.equal(nextVersion("2.8.0-beta.5", "2.8.0", tags), "2.8.0")
  assert.equal(nextVersion("2.8.0-beta.5", "2.8.3", tags), "2.8.3")
  // 같은 X.Y.Z가 이미 정식으로 나갔으면 그 정식 태그에서 올린다(2.7.0-beta.46 + v2.7.0).
  assert.equal(nextVersion("2.7.0-beta.46", "patch", tags), "2.7.2")
  assert.equal(nextVersion("2.7.0-beta.46", "minor", tags), "2.8.0")
})

test("숫자가 아닌 버전과 프리릴리스 요청을 거부", () => {
  assert.throws(() => nextVersion("2.8.0-beta.5", "2.8.0-beta.6", []), /X\.Y\.Z/)
  assert.throws(() => nextVersion("2.8.0-beta.5", "beta", []), /X\.Y\.Z/)
  assert.throws(() => nextVersion("2.8.0-beta.5", undefined, []), /X\.Y\.Z/)
  assert.throws(() => nextVersion("2.8", "patch", []), /해석하지 못했다/)
  for (const request of ["patch", "minor", "major"]) {
    assert.doesNotMatch(nextVersion("2.7.0-beta.30", request, ["v2.6.1"]), /NaN/)
  }
})

test("태그 목록에서 가장 높은 정식 버전만 고름", () => {
  assert.equal(latestStable([]), null)
  assert.equal(latestStable(["v2.6.1", "v2.7.0-beta.46", "v2.7.0", "legacy"]), "2.7.0")
  assert.equal(latestStable(["v2.10.0", "v2.9.9"]), "2.10.0")
})

test("세미버전 비교", () => {
  assert.ok(compareVersions("2.8.0", "2.7.9") > 0)
  assert.ok(compareVersions("2.7.10", "2.7.9") > 0)
  assert.ok(compareVersions("2.8.0", "2.8.0-beta.5") > 0)
  assert.equal(compareVersions("2.8.0+build.7", "2.8.0"), 0)
})
