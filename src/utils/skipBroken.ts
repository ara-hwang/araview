/** 로드 실패 시 자동 스킵 연쇄가 무한히 이어지지 않도록 상한 */
export const MAX_SKIP_ATTEMPTS = 5

/**
 * 실패한 인덱스 뒤에서 로드 가능한 다음 인덱스를 찾는다.
 * loop가 꺼져 있으면 끝에서 멈추고(null), 켜져 있으면 wrap한다.
 * 이미 실패한 경로는 건너뛴다.
 */
export function findSkipTarget(
  images: readonly string[],
  failedIndex: number,
  failed: ReadonlySet<string>,
  loop: boolean
): number | null {
  if (images.length === 0) return null
  const total = images.length
  for (let attempt = 1; attempt <= MAX_SKIP_ATTEMPTS; attempt += 1) {
    let candidate = failedIndex + attempt
    if (candidate >= total) {
      if (!loop) return null
      candidate = candidate % total
    }
    const path = images[candidate]
    if (path !== undefined && !failed.has(path)) return candidate
  }
  return null
}
