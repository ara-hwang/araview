import { cleanup, render, screen } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "ko" } })
}))

import { ExifPanel } from "@/components/ExifPanel"
import { closeImage, useAppStore } from "@/store/appStore"

beforeEach(() => {
  cleanup()
  closeImage()
})

const svgInfo = {
  file_path: "/pics/vector.svg",
  file_name: "vector.svg",
  file_size: 456,
  mime_type: "image/svg+xml",
  width: 800,
  height: 600
}

const svgDetails = {
  file_path: "/pics/vector.svg",
  file_size: 456,
  width: 800,
  height: 600,
  color_mode: "unknown",
  bits_per_channel: null,
  created_unix: null,
  modified_unix: null,
  dpi_x: null,
  dpi_y: null,
  icc_status: "unchecked" as const,
  icc_name: null,
  icc_bytes: null
}

describe("ExifPanel SVG", () => {
  it("벡터 안내 문구를 표시한다", () => {
    useAppStore.setState({
      imageInfo: svgInfo,
      imageDetails: svgDetails,
      histogramData: null,
      exifData: null,
      exifError: null,
      showExifPanel: true
    })

    render(<ExifPanel />)

    expect(screen.getByText("histogram.vectorUnavailable")).not.toBeNull()
    expect(screen.getByText("details.colorVector")).not.toBeNull()
  })

  it("래스터는 기존 문구를 유지한다", () => {
    useAppStore.setState({
      imageInfo: {
        file_path: "/pics/a.jpg",
        file_name: "a.jpg",
        file_size: 123,
        mime_type: "image/jpeg",
        width: 2000,
        height: 1000
      },
      imageDetails: { ...svgDetails, file_path: "/pics/a.jpg" },
      histogramData: null,
      exifData: null,
      exifError: null,
      showExifPanel: true
    })

    render(<ExifPanel />)

    expect(screen.getByText("histogram.unavailable")).not.toBeNull()
  })
})
