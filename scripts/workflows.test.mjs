import assert from "node:assert/strict"
import { readFileSync } from "node:fs"
import { spawnSync } from "node:child_process"
import test from "node:test"
import { runInNewContext } from "node:vm"

const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8")
const ci = read(".github/workflows/ci.yml")
const release = read(".github/workflows/release.yml")
const build = read(".github/workflows/build-windows.yml")

// These checks exercise the conditions in the real YAML, not a second copy of the
// policy. YAML syntax and action schemas are checked separately with actionlint.
function job(workflow, name) {
  const jobs = workflow.slice(workflow.indexOf("\njobs:\n") + 7)
  const match = jobs.match(new RegExp(`^  ${name}:\\r?\\n([\\s\\S]*?)(?=^  [\\w-]+:|$(?![\\s\\S]))`, "m"))
  assert.ok(match, `Missing job: ${name}`)
  return match[1]
}

function condition(body) {
  const match = body.match(/^    if: (.+)(?:\r?\n((?:      .*(?:\r?\n|$))*))?/m)
  assert.ok(match, "Missing job condition")
  return match[1] === ">-" ? match[2].trim() : match[1]
}

function evaluate(expression, { event = "push", ref = "refs/heads/main", full = false, warm = false,
  changes = "success", rust = "true", docs = "false", cancelled = false } = {}) {
  const code = expression.replace(/^\$\{\{\s*|\s*\}\}$/g, "")
    .replace(/\b(inputs|github)\.([\w-]+)/g, (_, object, key) => `${object}[${JSON.stringify(key)}]`)
  return runInNewContext(code, {
    github: { event_name: event, ref, ref_type: ref.startsWith("refs/tags/") ? "tag" : "branch" },
    inputs: { "full-verification": full, "warm-cache": warm },
    needs: { changes: { result: changes, outputs: { rust, docs } } },
    cancelled: () => cancelled,
  })
}

function cacheSave(body) {
  const match = body.match(/rust-cache-save-if: (.+)/)
  assert.ok(match, "Missing explicit cache write policy")
  return match[1]
}

function docsEligible(options) {
  const expression = condition(job(ci, "docs"))
  // GitHub adds success() unless the expression explicitly uses a status function.
  if (!/\b(always|cancelled|failure|success)\(/.test(expression)
    && options?.changes && options.changes !== "success") return false
  return evaluate(expression, options)
}

test("tag reusable verification runs docs even when changes was skipped", () => {
  assert.equal(docsEligible({ full: true, changes: "skipped", ref: "refs/tags/v2.9.3" }), true)
  assert.equal(docsEligible({ full: true, changes: "failure" }), false)
  assert.equal(docsEligible({ full: true, cancelled: true }), false)
})

test("docs change filtering and manual dispatch keep their original coverage", () => {
  const expression = condition(job(ci, "docs"))
  assert.equal(evaluate(expression, { rust: "false", docs: "false" }), false)
  assert.equal(evaluate(expression, { rust: "false", docs: "true" }), true)
  assert.equal(evaluate(expression, { rust: "true", docs: "false" }), true)
  assert.equal(evaluate(expression, { event: "workflow_dispatch", changes: "skipped" }), true)
})

test("build workflow changes trigger Rust checks without including Pages-only edits", () => {
  const expression = new RegExp(ci.match(/if echo "\$files" \| grep -Eq '([^']+)'/)[1])
  for (const path of [
    ".github/workflows/ci.yml", ".github/workflows/release.yml", ".github/workflows/build-windows.yml",
    ".github/actions/setup-rust-native/action.yml", "app/src/main.rs", "crates/araview-core/src/image.rs",
    "vendor/gpui-component/src/lib.rs", "Cargo.lock", "vcpkg.json",
  ]) assert.equal(expression.test(path), true, path)
  for (const path of [".github/workflows/pages.yml", "site/index.html", "docs/releasing.md"]) {
    assert.equal(expression.test(path), false, path)
  }
})

test("actionlint uses the official binary with a digest check before extraction", () => {
  const install = job(ci, "docs").split("- name: Install actionlint")[1].split("- name: Validate workflows")[0]
  assert.doesNotMatch(install, /uses: taiki-e\/install-action|tool: actionlint/)
  assert.match(install, /https:\/\/github\.com\/rhysd\/actionlint\/releases\/download\/v1\.7\.12\/actionlint_1\.7\.12_linux_amd64\.tar\.gz/)
  assert.match(install, /8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8/)
  assert.ok(install.indexOf("sha256sum --check") < install.indexOf("tar -xzf"))
})

test("full verification does not also run the lightweight Rust job", () => {
  const light = condition(job(ci, "rust-check"))
  for (const event of ["push", "pull_request"]) {
    assert.equal(evaluate(light, { event }), true)
    assert.equal(evaluate(light, { event, rust: "false" }), false)
    assert.equal(evaluate(light, { event, full: true }), false)
  }
  assert.equal(evaluate(light, { event: "workflow_dispatch" }), false)
  for (const name of ["rust", "rust-test"]) {
    const expression = condition(job(ci, name))
    assert.equal(evaluate(expression), false)
    assert.equal(evaluate(expression, { full: true }), true)
    assert.equal(evaluate(expression, { event: "workflow_dispatch" }), true)
  }
})

test("automatic main checks save Rust caches but PRs and tags do not", () => {
  const expression = cacheSave(job(ci, "rust-check"))
  assert.equal(evaluate(expression), true)
  assert.equal(evaluate(expression, { event: "pull_request", ref: "refs/pull/123/merge" }), false)
  assert.equal(evaluate(expression, { ref: "refs/tags/v2.9.3" }), false)
  const full = cacheSave(job(ci, "rust-test"))
  assert.equal(evaluate(full, { event: "workflow_dispatch" }), true)
  assert.equal(evaluate(full, { ref: "refs/tags/v2.9.3" }), false)
})

test("release cache warming is opt-in and main-only", () => {
  const expression = condition(job(ci, "warm-release-cache"))
  assert.equal(evaluate(expression, { event: "workflow_dispatch", warm: true }), true)
  assert.equal(evaluate(expression, { event: "workflow_dispatch" }), false)
  assert.equal(evaluate(expression, { warm: true }), false)
  assert.equal(evaluate(expression, { event: "workflow_dispatch", warm: true, ref: "refs/heads/feature" }), false)
  assert.match(job(ci, "warm-release-cache"), /uses: \.\/\.github\/workflows\/build-windows\.yml/)
})

test("only main warming writes sccache; releases and other refs are read-only", () => {
  const expression = build.match(/SCCACHE_GHA_RW_MODE: (.+)/)[1]
  const save = cacheSave(job(build, "build"))
  for (const [options, expected] of [
    [{ warm: true }, "READ_WRITE"],
    [{}, "READ_ONLY"],
    [{ warm: true, ref: "refs/tags/v2.9.3" }, "READ_ONLY"],
    [{ warm: true, ref: "refs/heads/feature" }, "READ_ONLY"],
  ]) {
    assert.equal(evaluate(expression, options), expected)
    assert.equal(evaluate(save, options), expected === "READ_WRITE")
  }
  assert.match(build, /rust-cache-targets: "false"/)
  assert.doesNotMatch(build, /SCCACHE_CACHE_SIZE/)
})

test("compile and verification run in parallel; signing and publication remain gated", () => {
  assert.match(job(release, "verify"), /needs: preflight/)
  assert.match(job(release, "verify"), /full-verification: true/)
  const compile = job(release, "build-windows")
  assert.match(compile, /needs: preflight/)
  assert.match(compile, /uses: \.\/\.github\/workflows\/build-windows\.yml/)
  assert.doesNotMatch(compile, /secrets:/)
  const signing = job(release, "sign-windows")
  assert.match(signing, /needs: \[verify, build-windows\]/)
  assert.match(job(release, "release"), /needs: \[sign-windows\]/)
  // No always()/custom job condition may bypass GitHub's successful-needs gate.
  assert.doesNotMatch(signing, /^    if:/m)
  assert.doesNotMatch(job(release, "release"), /^    if:/m)
  assert.doesNotMatch(build, /secrets\.|contents: write|signer sign|gh release create/)
})

test("only verified signed artifacts are used for publication", () => {
  assert.match(build, /name: unsigned-windows/)
  assert.match(job(release, "sign-windows"), /--name unsigned-windows --dir target/)
  assert.match(job(release, "sign-windows"), /name: release-windows/)
  assert.match(job(release, "release"), /--pattern 'release-\*'/)
  assert.match(job(release, "release"), /name: release-dry-run/)
})

test("manual release dispatch never publishes, even when a tag was selected", () => {
  const publishing = job(release, "release")
  const publish = publishing.match(/- name: Publish release\r?\n\s+if: (.+)/)[1]
  const dryRun = publishing.match(/- name: Upload dry-run artifacts\r?\n\s+if: (.+)/)[1]
  for (const [event, ref, expected] of [
    ["push", "refs/tags/v2.9.3", true],
    ["workflow_dispatch", "refs/tags/v2.9.3", false],
    ["workflow_dispatch", "refs/heads/main", false],
  ]) {
    assert.equal(evaluate(publish, { event, ref }), expected)
    assert.equal(evaluate(dryRun, { event, ref }), !expected)
  }
})

test("shared compilation preserves production flags and exposes timings", () => {
  assert.match(build, /subst R: \$env:GITHUB_WORKSPACE/)
  assert.match(build, /cargo build --release --locked --timings -p araview -p araview-thumb --no-default-features/)
  assert.match(build, /finally \{[\s\S]*subst R: \/D/)
  assert.match(build, /name: Package installer/)
  assert.match(build, /sccache --show-stats/)
  assert.match(build, /path: target\/cargo-timings\//)
})

test("Windows workflow scripts parse in PowerShell", { skip: process.platform !== "win32" }, () => {
  const scripts = []
  for (const body of [job(build, "build"), job(release, "preflight"), job(release, "sign-windows")]) {
    for (const match of body.matchAll(/^        run: \|\r?\n((?:(?: {10}.*)?\r?\n)+)/gm)) {
      scripts.push(match[1].replace(/^ {10}/gm, "").replace(/\$\{\{[\s\S]*?\}\}/g, "test"))
    }
  }
  assert.ok(scripts.length >= 8, "Expected compile, package, preflight and signing scripts")
  const result = spawnSync("pwsh", ["-NoProfile", "-NonInteractive", "-Command", `
    $scripts = ConvertFrom-Json ([Console]::In.ReadToEnd())
    foreach ($script in $scripts) {
      $tokens = $null
      $errors = $null
      [void][System.Management.Automation.Language.Parser]::ParseInput($script, [ref]$tokens, [ref]$errors)
      if ($errors.Count -gt 0) { $errors | ForEach-Object { Write-Output $_.Message }; exit 1 }
    }
  `], { input: JSON.stringify(scripts), encoding: "utf8" })
  assert.ifError(result.error)
  assert.equal(result.status, 0, result.stdout + result.stderr)
})
