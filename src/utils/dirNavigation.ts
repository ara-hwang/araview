import { isArchiveFilePath } from "@/utils/archiveFile"

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

/** 혼자 한 화면을 쓰는 인덱스(폴더 안 아카이브 등). 비어 있으면 기존 쌍 규칙만 쓴다. */
export type SoloIndices = ReadonlySet<number>

const NO_SOLO: SoloIndices = new Set()

/** 폴더 목록에서 이미지로 그릴 수 없는 아카이브 인덱스. 아카이브 안이면 빈 집합이다. */
export function archiveSoloIndices(images: readonly string[], inArchive: boolean): SoloIndices {
  if (inArchive) return NO_SOLO
  const solo = new Set<number>()
  images.forEach((path, index) => {
    if (isArchiveFilePath(path)) solo.add(index)
  })
  return solo
}

/**
 * 넓은 페이지(가로가 더 긴 펼침 스캔)를 단독 인덱스에 더한다. 반쪽 칸에 넣으면
 * 절반 크기로 줄어들므로 혼자 한 화면을 쓴다. 더할 것이 없으면 `solo`를 그대로 돌려준다.
 */
export function withWideSolo(
  solo: SoloIndices,
  images: readonly string[],
  widePaths: ReadonlySet<string>
): SoloIndices {
  if (widePaths.size === 0) return solo
  const next = new Set(solo)
  images.forEach((path, index) => {
    if (widePaths.has(path)) next.add(index)
  })
  return next.size === solo.size ? solo : next
}

/**
 * 단독 화면이 섞인 양쪽 보기의 화면 시작 목록. 앞에서부터 두 장씩 묶되,
 * 단독 페이지(또는 그 바로 앞에 남는 페이지)는 혼자 한 화면이 된다.
 * 단독 집합이 표지 하나면 `dualViewStarts`의 표지 단독 배치와 같다.
 * `anchor`는 항상 화면 시작이 되고, 그 바로 앞에 남는 페이지는 혼자 한 화면이 된다.
 */
function soloAwareStarts(
  total: number,
  isSolo: (index: number) => boolean,
  anchor?: number
): number[] {
  const starts: number[] = []
  let i = 0
  while (i < total) {
    starts.push(i)
    i += isSolo(i) || i + 1 >= total || isSolo(i + 1) || i + 1 === anchor ? 1 : 2
  }
  return starts
}

function screenStarts(
  total: number,
  coverAlone: boolean,
  coverIndex: number,
  solo: SoloIndices,
  anchor?: number
): number[] {
  if (solo.size === 0) return dualViewStarts(total, coverAlone, coverIndex)
  const cover = Math.max(0, Math.min(coverIndex, total - 1))
  return soloAwareStarts(total, (i) => solo.has(i) || (coverAlone && i === cover), anchor)
}

/** 시작 목록에서 `index`를 담는 화면의 시작. */
function startContaining(starts: readonly number[], index: number): number {
  let start = starts[0] ?? 0
  for (const candidate of starts) {
    if (candidate > index) break
    start = candidate
  }
  return start
}

/**
 * 양쪽 보기(LTR/RTL)의 쌍 시작 인덱스.
 * `coverAlone = false`면 `[0,1], [2,3], ...`, `true`면 `[0], [1,2], [3,4], ...`.
 * `coverIndex`는 단독으로 보여줄 표지 인덱스다(기본 0번, ComicInfo FrontCover).
 * `solo`에 든 인덱스(폴더 안 아카이브, 넓은 페이지)는 혼자 한 화면이 된다.
 * `anchor`는 지금 화면의 시작 인덱스다. 넓은 페이지는 로드하면서 알게 되므로, 뒤늦게
 * 알려진 앞쪽 페이지가 짝을 밀어도 지금 화면을 기준으로 배치가 이어지게 한다.
 */
export function resolvePairStart(
  index: number,
  total: number,
  coverAlone: boolean,
  coverIndex = 0,
  solo: SoloIndices = NO_SOLO,
  anchor?: number
): number {
  if (total <= 0) return 0
  const clamped = Math.max(0, Math.min(index, total - 1))
  if (solo.size > 0)
    return startContaining(screenStarts(total, coverAlone, coverIndex, solo, anchor), clamped)
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
 * `anchored`면 `currentIndex`를 화면 시작으로 고정한다(`resolvePairStart`의 `anchor`).
 */
export function resolveDualStepIndex(
  currentIndex: number,
  total: number,
  loop: boolean,
  direction: "prev" | "next",
  coverAlone: boolean,
  coverIndex = 0,
  solo: SoloIndices = NO_SOLO,
  anchored = false
): number | null {
  if (total <= 1) return null
  if (!coverAlone && solo.size === 0)
    return resolveStepIndex(currentIndex, total, 2, loop, direction)

  const anchor = anchored ? currentIndex : undefined
  const starts = screenStarts(total, coverAlone, coverIndex, solo, anchor)
  const currentStart = resolvePairStart(currentIndex, total, coverAlone, coverIndex, solo, anchor)
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

/**
 * 양쪽 보기에서 현재 화면에 실제로 보이는 인덱스 목록.
 * `dualPageOffsets`의 오프셋을 인덱스로 옮기고, 목록 밖이면 루프 설정에 따라
 * wrap하거나 버린다(단독 화면이면 한 장만).
 * 뷰어가 로드하는 대상(`useMultiPageImages`)과 도크 하이라이트가 같은 목록을
 * 쓰도록 한 곳에 모아 둔다.
 * `anchored`면 `index`를 화면 시작으로 고정한다(`resolvePairStart`의 `anchor`).
 */
export function dualPageIndices(
  index: number,
  total: number,
  coverAlone: boolean,
  coverIndex = 0,
  loop = false,
  solo: SoloIndices = NO_SOLO,
  anchored = false
): number[] {
  if (total <= 0) return []
  const clamped = Math.max(0, Math.min(index, total - 1))
  if (solo.size > 0) {
    // 단독 화면이 섞이면 화면 경계가 목록에서 정해지므로 wrap 없이 그 화면만 담는다.
    const starts = screenStarts(total, coverAlone, coverIndex, solo, anchored ? clamped : undefined)
    const start = startContaining(starts, clamped)
    const next = starts[starts.indexOf(start) + 1] ?? total
    return Array.from({ length: Math.min(next - start, 2) }, (_, k) => start + k)
  }
  const indices: number[] = []
  for (const offset of dualPageOffsets(clamped, total, coverAlone, coverIndex)) {
    const raw = clamped + offset
    const target =
      raw >= 0 && raw < total ? raw : loop && total > 1 ? ((raw % total) + total) % total : -1
    if (target >= 0 && !indices.includes(target)) indices.push(target)
  }
  return indices
}
