// 서드파티 패키지별 라이선스 전문을 src-gpui/THIRD_PARTY_LICENSES.json으로 만든다.
// cargo-about(about.toml)이 Windows 배포 대상 의존성의 라이선스 본문을 모은다.
// 패키지에 라이선스 파일이 없으면 cargo-about이 저장소나 표준 문구로 채운다.
// 같은 본문은 한 번만 저장한다(texts), 패키지는 files로 id를 참조한다. 이 JSON은
// 앱에 컴파일 시점에 포함돼 설정의 오픈소스 라이선스 창이 읽는다.
// THIRD_PARTY_LICENSES.md의 2절 표도 같은 결과로 다시 쓴다.
// 사전 준비: cargo install cargo-about --locked --features cli
// 실행: node scripts/generate-license-data.mjs  (cargo-about이 네트워크를 쓴다)
import { execFileSync } from "node:child_process"
import { createHash } from "node:crypto"
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..")

const texts = new Map()

function addText(raw) {
  const text = raw.replaceAll("\r\n", "\n").trim()
  const id = createHash("sha1").update(text).digest("hex").slice(0, 10)
  texts.set(id, text)
  return id
}

// --- Rust (cargo-about) ---------------------------------------------------
const aboutOut = join(mkdtempSync(join(tmpdir(), "araview-about-")), "about.json")
execFileSync("cargo", ["about", "generate", "--format", "json", "-o", aboutOut], {
  cwd: root,
  stdio: ["ignore", "inherit", "inherit"]
})
const about = JSON.parse(readFileSync(aboutOut, "utf8"))

const key = (crate) => `${crate.name}@${crate.version}`
const filesByCrate = new Map()
for (const license of about.licenses) {
  const id = addText(license.text)
  for (const { crate } of license.used_by) {
    const list = filesByCrate.get(key(crate)) ?? []
    // 같은 본문이 중복되지 않게 한다.
    if (!list.some((f) => f.id === id)) list.push({ name: license.name, id })
    filesByCrate.set(key(crate), list)
  }
}

// 게시된 패키지에 LICENSE 파일이 없어 cargo-about이 표준 템플릿으로 대체하는 크레이트.
// 업스트림 저장소의 LICENSE 사본(licenses-extra/)으로 교체한다.
const EXTRA_LICENSES = {
  "alloc-stdlib": "alloc-stdlib.txt",
  exr: "exr.txt",
  libm: "libm.txt",
  "minisign-verify": "minisign-verify.txt",
  "pulp-wasm-simd-flag": "pulp-wasm-simd-flag.txt",
  simd_helpers: "simd_helpers.txt"
}
// 저작권자 자리가 비어 있는 표준 템플릿 문구인지 판별한다.
const isTemplateText = (id) => /<year>|<owner>|<copyright holders?>/i.test(texts.get(id) ?? "")

const packages = []
for (const { package: pkg, license } of about.crates) {
  if (!pkg.source) continue // 프로젝트 자신
  const extra = EXTRA_LICENSES[pkg.name]
  const files = extra
    ? [
        {
          name: "LICENSE (upstream repository)",
          id: addText(readFileSync(join(root, "licenses-extra", extra), "utf8"))
        }
      ]
    : (filesByCrate.get(key(pkg)) ?? [])
  packages.push({
    ecosystem: "rust",
    name: pkg.name,
    version: pkg.version,
    license: license || "unknown",
    files,
    // 저작권자가 없는 표준 템플릿만 있으면 화면이 안내를 붙인다.
    ...(files.length > 0 && files.every((f) => isTemplateText(f.id)) ? { templated: true } : {})
  })
}

packages.sort(
  (a, b) =>
    a.ecosystem.localeCompare(b.ecosystem) ||
    a.name.localeCompare(b.name) ||
    a.version.localeCompare(b.version, undefined, { numeric: true })
)

const out = {
  generated: new Date().toISOString().slice(0, 10),
  texts: Object.fromEntries([...texts].sort(([a], [b]) => a.localeCompare(b))),
  packages
}
writeFileSync(join(root, "src-gpui", "THIRD_PARTY_LICENSES.json"), JSON.stringify(out) + "\n")
const missing = packages.filter((p) => p.files.length === 0)
console.log(
  `${packages.length} packages, ${texts.size} unique texts, ${missing.length} without a license text`
)
if (missing.length > 0) console.log(missing.map((p) => `${p.name}@${p.version}`).join(", "))
const templated = packages.filter((p) => p.templated)
if (templated.length > 0) {
  console.log(
    `template text only (add to EXTRA_LICENSES): ${templated.map((p) => `${p.name}@${p.version}`).join(", ")}`
  )
}

// --- THIRD_PARTY_LICENSES.md 표 갱신 -----------------------------------------
const mdPath = join(root, "THIRD_PARTY_LICENSES.md")
let md = readFileSync(mdPath, "utf8")
const eol = md.includes("\r\n") ? "\r\n" : "\n"

function replaceTable(source, headingPrefix, rows) {
  const lines = source.split(eol)
  const heading = lines.findIndex((line) => line.startsWith(headingPrefix))
  if (heading < 0) throw new Error(`heading not found: ${headingPrefix}`)
  const start = lines.findIndex((line, i) => i > heading && line.startsWith("|"))
  let end = start
  while (end < lines.length && lines[end].startsWith("|")) end += 1
  const [header, divider] = lines.slice(start, start + 2)
  const body = rows.map((cells) => `| ${cells.join(" | ")} |`)
  lines.splice(start, end - start, header, divider, ...body)
  return lines.join(eol)
}

const table = (ecosystem) =>
  packages.filter((p) => p.ecosystem === ecosystem).map((p) => [p.name, p.version, p.license])
md = replaceTable(md, "## 2. ", table("rust"))
writeFileSync(mdPath, md)
