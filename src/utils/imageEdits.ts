export type SaveOutputFormat = "keep" | "png" | "jpg" | "webp"

export type SaveEditsPayload = {
  rotationCw: number
  flipH: boolean
  flipV: boolean
  /** None = 원본 유지 */
  format: string | null
  overwrite: boolean
  newFileName: string | null
}

type DescribeT = (key: string, vars?: Record<string, string | number>) => string

/** 현재 회전/반전 상태를 사람이 읽을 요약문으로 */
export function describeTransform(
  rotation: number,
  flipH: boolean,
  flipV: boolean,
  t?: DescribeT
): string {
  const translate: DescribeT =
    t ?? ((key: string) => key.split(".").pop() ?? key)
  const parts: string[] = []
  if (rotation !== 0)
    parts.push(translate("dialog.save.transform.rotated", { deg: rotation }))
  if (flipH) parts.push(translate("dialog.save.transform.flipH"))
  if (flipV) parts.push(translate("dialog.save.transform.flipV"))
  return parts.length > 0
    ? parts.join(" · ")
    : translate("dialog.save.transform.none")
}

/** 저장 다이얼로그 선택 → 백엔드 SaveImageOptions 페이로드 */
export function buildSaveEditsPayload(
  rotation: number,
  flipH: boolean,
  flipV: boolean,
  output: SaveOutputFormat,
  overwrite: boolean,
  newFileName: string
): SaveEditsPayload {
  return {
    rotationCw: rotation,
    flipH,
    flipV,
    format: output === "keep" ? null : output,
    overwrite,
    newFileName: overwrite || !newFileName.trim() ? null : newFileName.trim()
  }
}
