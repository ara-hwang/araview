import { describe, expect, it } from "vitest"

import type { ImageInfo } from "@/types"
import { shouldUsePreviewThumbnail } from "@/utils/previewThumbnail"

function info(partial: Partial<ImageInfo>): ImageInfo {
  return {
    file_path: "C:/pics/a.jpg",
    source_path: "C:/pics/a.jpg",
    file_name: "a.jpg",
    file_size: 1000,
    mime_type: "image/jpeg",
    width: 100,
    height: 100,
    ...partial
  }
}

describe("shouldUsePreviewThumbnail", () => {
  it("큰 픽셀 수 이미지는 프리뷰를 쓴다", () => {
    expect(shouldUsePreviewThumbnail(info({ width: 4000, height: 3000 }))).toBe(true)
  })

  it("작아도 파일이 크면 프리뷰를 쓴다", () => {
    expect(shouldUsePreviewThumbnail(info({ width: 800, height: 600, file_size: 4_000_000 }))).toBe(
      true
    )
  })

  it("작은 이미지는 프리뷰를 쓰지 않는다", () => {
    expect(shouldUsePreviewThumbnail(info({ width: 800, height: 600, file_size: 200_000 }))).toBe(
      false
    )
  })

  it("치수를 모르는 파일은 크기 기준으로만 판단한다", () => {
    expect(shouldUsePreviewThumbnail(info({ width: null, height: null, file_size: 200_000 }))).toBe(
      false
    )
    expect(
      shouldUsePreviewThumbnail(info({ width: null, height: null, file_size: 3_000_000 }))
    ).toBe(true)
  })

  it("APNG도 프레임 캔버스가 처리하므로 제외한다", () => {
    expect(shouldUsePreviewThumbnail(info({ mime_type: "image/apng", file_size: 9_000_000 }))).toBe(
      false
    )
  })

  it("GIF는 프레임 캔버스가 처리하므로 제외한다", () => {
    expect(shouldUsePreviewThumbnail(info({ mime_type: "image/gif", file_size: 9_000_000 }))).toBe(
      false
    )
  })
})
