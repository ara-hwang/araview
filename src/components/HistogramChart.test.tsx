import { render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key })
}))

import { HistogramChart } from "@/components/HistogramChart"

const solid = (value: number, at: number) => {
  const bins = Array.from({ length: 256 }, () => 0)
  bins[at] = value
  return bins
}

describe("HistogramChart", () => {
  it("renders one area path per channel with an accessible label", () => {
    const { container } = render(
      <HistogramChart
        data={{
          r: solid(10, 255),
          g: solid(5, 128),
          b: solid(3, 0),
          sampled_pixels: 18
        }}
      />
    )
    const svg = screen.getByRole("img", { name: "histogram.chartLabel" })
    expect(svg).not.toBeNull()
    expect(container.querySelectorAll("svg path")).toHaveLength(3)
    expect(screen.getByText("histogram.red")).not.toBeNull()
    expect(screen.getByText("histogram.green")).not.toBeNull()
    expect(screen.getByText("histogram.blue")).not.toBeNull()
  })
})
