import { create } from "zustand"

import type { ImageInfo } from "@/types"

type WidePageState = {
  /** `paths`가 속한 목록 범위. 아카이브 경로이고, 폴더 목록이면 null이다. */
  scope: string | null
  /** 로드해서 가로가 더 길다고 확인된 항목(폴더는 경로, 아카이브는 엔트리명). */
  paths: ReadonlySet<string>
}

/**
 * 양쪽 보기에서 단독 화면으로 보여줄 넓은 페이지 기록. 치수는 로드해야 알 수 있어
 * 본 페이지와 예열한 이웃만 담긴다. 영속화하지 않는다.
 */
export const useWidePageStore = create<WidePageState>(() => ({
  scope: null,
  paths: new Set()
}))

/** 로드한 이미지가 넓은 페이지면 기록한다. 범위가 바뀌면 이전 기록은 버린다. */
export function recordWidePage(
  scope: string | null,
  pathOrEntry: string,
  info: Pick<ImageInfo, "width" | "height">
): void {
  const width = info.width ?? 0
  const height = info.height ?? 0
  if (height <= 0 || width <= height) return
  const state = useWidePageStore.getState()
  if (state.scope === scope && state.paths.has(pathOrEntry)) return
  const paths = new Set(state.scope === scope ? state.paths : [])
  paths.add(pathOrEntry)
  useWidePageStore.setState({ scope, paths })
}
