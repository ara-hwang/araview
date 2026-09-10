export type AppErrorKind =
  "not-found" | "permission" | "unsupported" | "corrupt" | "unknown"
export type ClassifiedError = {
  kind: AppErrorKind
  titleKey: string
  hintKey: string
  /** @deprecated Use titleKey/hintKey with i18n. Kept for tests. */
  title: string
  /** @deprecated Use titleKey/hintKey with i18n. Kept for tests. */
  hint: string
}

const KO_TEXT: Record<AppErrorKind, { title: string; hint: string }> = {
  "not-found": {
    title: "파일을 찾을 수 없습니다",
    hint: "파일이 이동·삭제되었거나 아카이브 항목이 없을 수 있습니다. 다른 파일을 열어보세요."
  },
  permission: {
    title: "파일에 접근할 수 없습니다",
    hint: "권한이 없거나 다른 프로그램이 사용 중일 수 있습니다. 권한을 확인한 뒤 다시 시도하세요."
  },
  unsupported: {
    title: "지원하지 않는 형식입니다",
    hint: "지원 포맷(png/jpg/webp/svg/avif/heic/cbz/cbr 등)인지 확인하거나 다른 뷰어로 열어보세요."
  },
  corrupt: {
    title: "파일을 읽는 중 문제가 발생했습니다",
    hint: "파일이 손상되었을 수 있습니다. 마지막으로 본 이미지나 홈으로 돌아갈 수 있습니다."
  },
  unknown: {
    title: "이미지를 불러오지 못했습니다",
    hint: "다시 시도하거나 홈으로 돌아가 다른 파일을 열어보세요."
  }
}

const KEY_BY_KIND: Record<AppErrorKind, { titleKey: string; hintKey: string }> =
  {
    "not-found": {
      titleKey: "error.notFound.title",
      hintKey: "error.notFound.hint"
    },
    permission: {
      titleKey: "error.permission.title",
      hintKey: "error.permission.hint"
    },
    unsupported: {
      titleKey: "error.unsupported.title",
      hintKey: "error.unsupported.hint"
    },
    corrupt: {
      titleKey: "error.corrupt.title",
      hintKey: "error.corrupt.hint"
    },
    unknown: {
      titleKey: "error.unknown.title",
      hintKey: "error.unknown.hint"
    }
  }

/** 백엔드가 반환하는 고정 에러 문자열 → 종류. 전체 일치만 해당한다. */
const EXACT_KIND: ReadonlyMap<string, AppErrorKind> = new Map([
  ["File not found", "not-found"],
  ["Path not found", "not-found"],
  ["Archive not found", "not-found"],
  ["Cannot get parent directory", "not-found"],
  ["No images found in directory", "not-found"],
  ["No images found in archive", "not-found"],
  ["Unsupported image format", "unsupported"],
  ["Unsupported archive format", "unsupported"],
  ["Not an archive file", "unsupported"],
  ["Unsupported path type", "unsupported"],
  ["Archive entry too large", "unsupported"]
])

/** 소문자로 바꾼 메시지의 접두사로 판정한다 (동적 suffix 허용). */
const PREFIX_KIND: ReadonlyArray<readonly [string, AppErrorKind]> = [
  ["entry not found", "not-found"],
  ["failed to decode image", "corrupt"],
  ["cannot decode image", "corrupt"],
  ["failed to read zip", "corrupt"],
  ["failed to read 7z", "corrupt"],
  ["failed to read rar", "corrupt"],
  ["failed to read tar", "corrupt"],
  ["failed to read entry data", "corrupt"]
]

/** 백엔드 `app_error.rs`의 ErrorCode와 1:1 대응 (snake_case). */
export type BackendErrorCode =
  | "not_found"
  | "permission"
  | "unsupported"
  | "too_large"
  | "corrupt"
  | "invalid_input"
  | "already_exists"
  | "unknown"

/** Tauri command가 Err로 돌려주는 구조화 에러. 구버전 문자열도 올 수 있다. */
export type BackendError = {
  code: BackendErrorCode
  message?: unknown
}

/** 코드 → 종류. null이면 message 레거시 매칭으로 폴백한다. */
const CODE_KIND: ReadonlyMap<BackendErrorCode, AppErrorKind> = new Map([
  ["not_found", "not-found"],
  ["permission", "permission"],
  ["unsupported", "unsupported"],
  ["too_large", "unsupported"],
  ["corrupt", "corrupt"]
])

function codeOf(input: unknown): BackendErrorCode | null {
  if (typeof input === "object" && input !== null && "code" in input) {
    const code = (input as { code?: unknown }).code
    if (typeof code === "string" && CODE_KIND.has(code as BackendErrorCode)) {
      return code as BackendErrorCode
    }
    // 알 수 없는 코드도 invalid_input/already_exists/unknown 계열이면
    // 레거시 매칭으로 넘긴다 (아래 kindOf가 message로 판정).
    if (typeof code === "string") return "unknown"
  }
  return null
}

/**
 * invoke 실패값을 사람용 메시지로. 구조화 에러는 message 필드,
 * 문자열은 그대로, 그 외는 String() 변환.
 */
export function errorMessage(input: unknown): string {
  if (typeof input === "string") return input
  if (typeof input === "object" && input !== null && "message" in input) {
    const message = (input as { message?: unknown }).message
    if (typeof message === "string") return message
  }
  return String(input)
}

function kindOf(message: string): AppErrorKind {
  // 백엔드 고정 문자열은 정확히 매칭한다 (영문 부분일치보다 우선).
  // 새 백엔드 에러를 추가하면 아래 표에도 행을 추가할 것.
  const exact = EXACT_KIND.get(message)
  if (exact) return exact

  const lower = message.toLowerCase()

  for (const [prefix, kind] of PREFIX_KIND) {
    if (lower.startsWith(prefix)) return kind
  }

  if (
    lower.includes("not found") ||
    lower.includes("no images found") ||
    lower.includes("cannot get parent") ||
    lower.includes("entry not found") ||
    lower.includes("archive not found")
  ) {
    return "not-found"
  }

  if (
    lower.includes("permission") ||
    lower.includes("access") ||
    lower.includes("denied") ||
    lower.includes("os error 5")
  ) {
    return "permission"
  }

  if (
    lower.includes("unsupported") ||
    lower.includes("not an archive") ||
    lower.includes("no exif data")
  ) {
    return "unsupported"
  }

  if (
    lower.includes("corrupt") ||
    lower.includes("invalid") ||
    lower.includes("decode") ||
    lower.includes("failed to open") ||
    lower.includes("failed to read")
  ) {
    return "corrupt"
  }

  return "unknown"
}

/**
 * 백엔드/프론트에서 온 원시 에러를 종류별로 분류한다.
 * 구조화 에러(`{code, message}`)는 코드 우선, 문자열은 레거시 매칭.
 * 표시는 `titleKey`/`hintKey`를 `t()`로 번역한다.
 */
export function classifyError(input: unknown): ClassifiedError {
  const code = codeOf(input)
  const message = errorMessage(input)
  const kind =
    code !== null && CODE_KIND.has(code)
      ? (CODE_KIND.get(code) as AppErrorKind)
      : kindOf(message)
  return {
    kind,
    ...KEY_BY_KIND[kind],
    ...KO_TEXT[kind]
  }
}
