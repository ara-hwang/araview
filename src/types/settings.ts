export type CacheMode = "off" | "nearby" | "extended"

export type ViewMode = "single" | "left-to-right" | "right-to-left" | "webtoon"

export type fillMode = "fit-to-width" | "fit-to-height" | "fit-to-screen"

export type Settings = {
  loopNavigation: boolean
  cacheMode: CacheMode
  viewMode: ViewMode
}
