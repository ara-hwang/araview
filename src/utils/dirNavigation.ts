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

/**
 * 양쪽 보기(LTR/RTL)의 쌍 시작 인덱스.
 * `coverAlone = false`면 `[0,1], [2,3], ...`, `true`면 `[0], [1,2], [3,4], ...`.
 * `coverIndex`는 단독으로 보여줄 표지 인덱스다(기본 0번, ComicInfo FrontCover).
 */
export function resolvePairStart(
  index: number,
  total: number,
  coverAlone: boolean,
  coverIndex = 0
): number {
  if (total <= 0) return 0
  const clamped = Math.max(0, Math.min(index, total - 1))
  if (!coverAlone) return clamped - (clamped % 2)
  const starts = dualViewStarts(total, coverAlone, coverIndex)
  let start = starts[0] ?? 0
  for (const candidate of starts) {
    if (candidate > clamped) break
    start = candidate
  }
  return start
}

/**
 * 양쪽 보기의 화면(쌍) 시작 인덱스 목록.
 * 표지 단독이면 표지를 먼저 두고 그 뒤부터 홀수 시작으로 맞춘다.
 */
function dualViewStarts(total: number, coverAlone: boolean, coverIndex: number): number[] {
  if (total <= 0) return []
  const starts: number[] = []
  if (!coverAlone) {
    for (let i = 0; i < total; i += 2) starts.push(i)
    return starts
  }
  const cover = Math.max(0, Math.min(coverIndex, total - 1))
  // 표지 앞 구간(만화 제작 실수로 표지가 0번이 아닐 때)은 0번부터 짝을 맞추고,
  // 짝이 남는 페이지는 단독 화면으로 둔다.
  for (let i = 0; i + 1 < cover; i += 2) starts.push(i)
  if (cover % 2 === 1) starts.push(cover - 1)
  starts.push(cover)
  for (let i = cover + 1; i < total; i += 2) starts.push(i)
  return starts
}

/**
 * 양쪽 보기의 다음/이전 쌍 시작. 이동할 수 없으면 null(비루프 경계).
 * `coverAlone = false`면 `resolveStepIndex(step = 2)`와 같은 결과를 낸다.
 */
export function resolveDualStepIndex(
  currentIndex: number,
  total: number,
  loop: boolean,
  direction: "prev" | "next",
  coverAlone: boolean,
  coverIndex = 0
): number | null {
  if (total <= 1) return null
  if (!coverAlone) return resolveStepIndex(currentIndex, total, 2, loop, direction)

  const starts = dualViewStarts(total, coverAlone, coverIndex)
  const currentStart = resolvePairStart(currentIndex, total, coverAlone, coverIndex)
  const position = starts.indexOf(currentStart)
  if (position < 0) return null

  const nextPosition = direction === "next" ? position + 1 : position - 1
  if (nextPosition >= 0 && nextPosition < starts.length) return starts[nextPosition]
  if (!loop) return null
  return starts[((nextPosition % starts.length) + starts.length) % starts.length]
}

/**
 * 양쪽 보기에서 현재 인덱스 기준으로 함께 표시할 오프셋.
 * 단독 화면(표지, 표지 바로 앞에 남는 페이지)에서는 `[0]`만 반환해 다음 페이지를
 * 디코드하지 않는다.
 */
export function dualPageOffsets(
  index: number,
  total: number,
  coverAlone: boolean,
  coverIndex = 0
): number[] {
  if (total <= 0) return []
  if (!coverAlone) return [0, 1]
  const clamped = Math.max(0, Math.min(index, total - 1))
  const cover = Math.max(0, Math.min(coverIndex, total - 1))
  if (clamped === cover) return [0]
  if (cover % 2 === 1 && clamped === cover - 1) return [0]
  return [0, 1]
}
