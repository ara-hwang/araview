/** 폴더/아카이브 이동의 인덱스 계산. `useDirectoryNavigation`이 쓴다. */

/** 방향 이동의 다음 인덱스. 이동할 수 없으면 null(비루프 경계). */
export function resolveStepIndex(
  currentIndex: number,
  total: number,
  step: number,
  loop: boolean,
  direction: "prev" | "next"
): number | null {
  if (total <= 1) return null
  const last = total - 1
  if (direction === "prev") {
    const next = currentIndex - step
    if (next >= 0) return next
    if (!loop) return null
    return ((next % total) + total) % total
  }
  const next = currentIndex + step
  if (next <= last) return next
  if (loop) return next % total
  // 끝에서 멈춤 모드라도 마지막 장은 볼 수 있게 clamp한다.
  if (currentIndex === last) return null
  return last
}

/** 오프셋 점프 인덱스. 루프면 wrap, 아니면 clamp. */
export function resolveOffsetIndex(
  currentIndex: number,
  total: number,
  offset: number,
  loop: boolean
): number {
  const raw = currentIndex + offset
  if (loop) return ((raw % total) + total) % total
  return Math.max(0, Math.min(raw, total - 1))
}
