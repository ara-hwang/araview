import { describe, expect, it } from "vitest"

import type { ImageInfo } from "@/types"
import {
  animationMimeOf,
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
const apng: ImageInfo = { ...png, mime_type: "image/apng", file_name: "anim.apng" }
// 확장자가 .png인 APNG는 백엔드가 내용으로 판별해 image/apng로 보고한다.
const apngNamedPng: ImageInfo = { ...png, mime_type: "image/apng" }
const apngWithoutMime: ImageInfo = { ...png, mime_type: "", file_name: "ANIM.APNG" }

describe("animationMimeOf", () => {
  it("GIF와 APNG를 MIME 우선, 파일명 보조로 판정한다", () => {
    expect(animationMimeOf(gif)).toBe("image/gif")
    expect(animationMimeOf(gifWithoutMime)).toBe("image/gif")
    expect(animationMimeOf(apng)).toBe("image/apng")
    expect(animationMimeOf(apngNamedPng)).toBe("image/apng")
    expect(animationMimeOf(apngWithoutMime)).toBe("image/apng")
    expect(animationMimeOf({ ...apng, file_name: "renamed.gif" })).toBe("image/apng")
    expect(animationMimeOf(png)).toBeNull()
    expect(animationMimeOf(null)).toBeNull()
  })
})

describe("isGifImage", () => {
  it("MIME 또는 파일명으로 판정한다", () => {
    expect(isGifImage(gif)).toBe(true)
    expect(isGifImage(gifWithoutMime)).toBe(true)
    expect(isGifImage(png)).toBe(false)
    expect(isGifImage(null)).toBe(false)
  })
})

describe("canControlGif", () => {
  it("단일 보기 + GIF/APNG + 디코더 지원일 때만 true", () => {
    expect(canControlGif(gif, "single")).toBe(supportsAnimationControl())
    expect(canControlGif(apng, "single")).toBe(supportsAnimationControl())
    expect(canControlGif(apng, "webtoon")).toBe(false)
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
