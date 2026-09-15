import { useId } from "react"
import { useTranslation } from "react-i18next"
import type { ImageHistogram } from "@/types"
import { histogramChannelPath, histogramMax } from "@/utils/imageDetails"

const CHART_WIDTH = 256
const CHART_HEIGHT = 64

const CHANNELS = [
  { key: "r", color: "#ef4444", labelKey: "histogram.red" },
  { key: "g", color: "#22c55e", labelKey: "histogram.green" },
  { key: "b", color: "#3b82f6", labelKey: "histogram.blue" }
] as const

export function HistogramChart({ data }: { data: ImageHistogram }) {
  const { t } = useTranslation()
  const titleId = useId()
  const max = histogramMax(data)

  return (
    <figure className="space-y-1.5">
      <svg
        role="img"
        aria-labelledby={titleId}
        viewBox={`0 0 ${CHART_WIDTH} ${CHART_HEIGHT}`}
        className="bg-muted/40 h-16 w-full rounded-md"
        preserveAspectRatio="none"
      >
        <title id={titleId}>{t("histogram.chartLabel")}</title>
        {CHANNELS.map(({ key, color }) => (
          <path
            key={key}
            d={histogramChannelPath(data[key], max, CHART_WIDTH, CHART_HEIGHT)}
            fill={color}
            fillOpacity={0.45}
            stroke={color}
            strokeWidth={1}
            vectorEffect="non-scaling-stroke"
          />
        ))}
      </svg>
      <figcaption className="flex gap-3 text-xs">
        {CHANNELS.map(({ key, color, labelKey }) => (
          <span
            key={key}
            className="text-muted-foreground inline-flex items-center gap-1"
          >
            <span aria-hidden="true" className="inline-block size-2">
              <svg viewBox="0 0 8 8" className="block size-2">
                <circle cx="4" cy="4" r="4" fill={color} />
              </svg>
            </span>
            {t(labelKey)}
          </span>
        ))}
      </figcaption>
    </figure>
  )
}
