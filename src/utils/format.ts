function numberLocale(): string | undefined {
  if (typeof navigator !== "undefined" && navigator.language) {
    return navigator.language
  }
  return undefined
}

export function formatFileSize(bytes: number, locale?: string): string {
  const loc = locale ?? numberLocale()
  if (bytes < 1024) {
    const n = new Intl.NumberFormat(loc, { useGrouping: false }).format(bytes)
    return `${n} B`
  }
  const oneDecimal = new Intl.NumberFormat(loc, {
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
    useGrouping: false
  })
  if (bytes < 1024 * 1024) return `${oneDecimal.format(bytes / 1024)} KB`
  if (bytes < 1024 * 1024 * 1024) {
    return `${oneDecimal.format(bytes / (1024 * 1024))} MB`
  }
  return `${oneDecimal.format(bytes / (1024 * 1024 * 1024))} GB`
}

export function formatDimensions(
  width: number | null | undefined,
  height: number | null | undefined
): string | null {
  if (
    typeof width !== "number" ||
    typeof height !== "number" ||
    !Number.isFinite(width) ||
    !Number.isFinite(height) ||
    width <= 0 ||
    height <= 0
  ) {
    return null
  }
  return `${Math.round(width)}x${Math.round(height)}`
}
