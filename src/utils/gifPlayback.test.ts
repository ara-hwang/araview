import { describe, expect, it } from "vitest"

import type { ImageInfo } from "@/types"
import {
  canControlGif,
  clampGifFrame,
  isGifImage,
  stepGifFrame,
  supportsAnimationControl
} from "@/utils/gifPlayback"

const gif: ImageInfo = {
  mime_type: "image/gif",
  file_name: "anim.gif",
  file_path: "C:/x/anim.gif",
  source_path: "C:/x/anim.gif",
  file_size: 10,
  width: 4,
  height: 4
}
const png: ImageInfo = {
  mime_type: "image/png",
  file_name: "photo.png",
  file_path: "C:/x/photo.png",
  source_path: "C:/x/photo.png",
  file_size: 10,
  width: 4,
  height: 4
}
const gifWithoutMime: ImageInfo = { ...gif, mime_type: "", file_name: "ANIM.GIF" }

describe("isGifImage", () => {
  it("MIME 또는 파일명으로 판정한다", () => {
    expect(isGifImage(gif)).toBe(true)
    expect(isGifImage(gifWithoutMime)).toBe(true)
    expect(isGifImage(png)).toBe(false)
    expect(isGifImage(null)).toBe(false)
  })
})

describe("canControlGif", () => {
  it("단일 보기 + GIF + 디코더 지원일 때만 true", () => {
    expect(canControlGif(gif, "single")).toBe(supportsAnimationControl())
    expect(canControlGif(gif, "webtoon")).toBe(false)
    expect(canControlGif(gif, "left-to-right")).toBe(false)
    expect(canControlGif(png, "single")).toBe(false)
  })
})

describe("clampGifFrame", () => {
  it("[0, frameCount-1]로 자른다", () => {
    expect(clampGifFrame(-3, 5)).toBe(0)
    expect(clampGifFrame(2, 5)).toBe(2)
    expect(clampGifFrame(9, 5)).toBe(4)
    expect(clampGifFrame(1.7, 5)).toBe(1)
    expect(clampGifFrame(Number.NaN, 5)).toBe(0)
    expect(clampGifFrame(3, 0)).toBe(0)
  })
})

describe("stepGifFrame", () => {
  it("프레임 이동은 경계에서 멈춘다", () => {
    expect(stepGifFrame(0, 4, -1)).toBe(0)
    expect(stepGifFrame(0, 4, 1)).toBe(1)
    expect(stepGifFrame(3, 4, 1)).toBe(3)
    expect(stepGifFrame(3, 4, -2)).toBe(1)
    expect(stepGifFrame(0, 0, 1)).toBe(0)
  })
})
