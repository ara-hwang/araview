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
    hint: "지원 포맷(png/jpg/webp/svg/avif/heic/cbz 등)인지 확인하거나 다른 뷰어로 열어보세요."
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

function kindOf(message: string): AppErrorKind {
  const lower = message.toLowerCase()

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
 * 백엔드/프론트에서 온 원시 에러 문자열을 종류별로 분류한다.
 * `Result<T, String>` 계약을 바꾸지 않고 UI 분기용으로만 사용.
 * 표시는 `titleKey`/`hintKey`를 `t()`로 번역한다.
 */
export function classifyError(message: string): ClassifiedError {
  const kind = kindOf(message)
  return {
    kind,
    ...KEY_BY_KIND[kind],
    ...KO_TEXT[kind]
  }
}
