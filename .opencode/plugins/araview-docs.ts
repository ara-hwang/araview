import { execFileSync } from "node:child_process"
import { existsSync } from "node:fs"
import { dirname, join, resolve } from "node:path"

/**
 * AraView 문서-코드 정합성 플러그인.
 *
 * `npm run docs:check`(`scripts/check-docs.mjs`)와 같은 검사를 에이전트
 * 세션 안에서 `docs_preflight` 툴로 제공한다. 검사는 읽기 전용이며,
 * 스크립트 출력을 그대로 돌려준다. 수정은 하지 않는다.
 *
 * 로더 규약: `{ content }`를 반환하고 `editor.add`의 `name`은 필수다.
 */

type ToolArgs = {
  directory?: string
}

type ToolExecuteContext = { directory?: string }

type ToolDefinition = {
  /** 이름은 필수다. 빠지면 등록이 조용히 버려진다. */
  name: string
  description: string
  input: {
    type: "object"
    properties: Record<string, unknown>
    required?: string[]
    additionalProperties: boolean
  }
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

const CHECK_TIMEOUT_MS = 120_000
const ROOT_MARKER = "src-gpui/Cargo.toml"

/** 저장소 루트를 찾는다. 세션 디렉터리 기준으로 위로 한 번 확인한다. */
function resolveRoot(start: string): { root: string; error?: string } {
  let dir = resolve(start)
  for (let depth = 0; depth < 4; depth++) {
    if (existsSync(join(dir, ROOT_MARKER))) return { root: dir }
    const parent = dirname(dir)
    if (parent === dir) break
    dir = parent
  }
  return {
    root: resolve(start),
    error: `AraView 저장소를 찾지 못했다: ${start} 및 상위 디렉터리에서 ${ROOT_MARKER} 없음`
  }
}

function pickDirectory(args: ToolArgs, context: ToolExecuteContext, fallback: string): string {
  return args.directory ?? context.directory ?? fallback
}

export default {
  id: "araview.docs",
  async setup(ctx: PluginContextLike) {
    const fallback = ctx.location?.directory ?? process.cwd()
    const tools = (ctx as unknown as { tool: ToolRegistrar }).tool

    await tools.transform((editor) => {
      editor.namespace({
        name: "docs",
        description: "AraView 문서-코드 정합성 검사"
      })

      editor.add({
        name: "preflight",
        description: [
          "문서와 코드의 정합성을 읽기 전용으로 검사한다.",
          "버전 4곳, 지원 확장자, IPC 명령, 설정 키, 플러그인 목록,",
          "완료 plan 잔류, i18n 잔재 키를 scripts/check-docs.mjs로 대조한다.",
          "수정은 하지 않으며 검사 출력을 그대로 돌려준다."
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
        options: { namespace: "docs", codemode: true },
        execute: async (args, context) => {
          const { root, error } = resolveRoot(pickDirectory(args, context, fallback))
          if (error) return { content: `문서 정합성 검사 실패\n\n${error}` }
          try {
            const stdout = execFileSync("node", ["scripts/check-docs.mjs"], {
              cwd: root,
              encoding: "utf8",
              timeout: CHECK_TIMEOUT_MS,
              windowsHide: true,
              stdio: ["ignore", "pipe", "pipe"]
            })
            return { content: stdout }
          } catch (checkError) {
            const failure = checkError as { stdout?: string; stderr?: string }
            const output = [failure.stdout ?? "", failure.stderr ?? ""].join("\n").trim()
            return { content: output === "" ? "문서 정합성 검사 실행 실패" : output }
          }
        }
      })
    })
  }
}
