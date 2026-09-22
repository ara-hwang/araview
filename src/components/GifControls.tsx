import { CaretLeft, CaretRight, Pause, Play } from "@phosphor-icons/react"
import { useTranslation } from "react-i18next"
import { useShallow } from "zustand/react/shallow"

import { AppTooltip } from "@/components/AppTooltip"
import { Button } from "@/components/ui/button"
import { ButtonGroup, ButtonGroupText } from "@/components/ui/button-group"
import { formatShortcutDisplay } from "@/constants/shortcuts"
import { stepGifFrameBy, toggleGifPlayback, useGifStore } from "@/store/gifStore"
import { useSettingsStore } from "@/store/settingsStore"

/**
 * GIF 전용 재생 컨트롤. 제어 가능한 GIF(단일 보기 + 다중 프레임 + 디코더 지원)일
 * 때만 나타나며, 좁은 창에서는 숨고 단축키/명령 팔레트로 조작한다.
 */
export function GifControls() {
  const { t } = useTranslation()
  const { active, playing, frame, frameCount } = useGifStore(
    useShallow((state) => ({
      active: state.active,
      playing: state.playing,
      frame: state.frame,
      frameCount: state.frameCount
    }))
  )
  const shortcuts = useSettingsStore((state) => state.shortcuts)

  if (!active || frameCount <= 1) return null

  const withShortcut = (label: string, binding: string) =>
    binding ? `${label} (${formatShortcutDisplay(binding)})` : label
  const playLabel = playing ? t("menu.gifPause") : t("menu.gifPlay")

  return (
    <ButtonGroup aria-label={t("header.gifGroup")} className="hidden no-drag min-[1024px]:flex">
      <AppTooltip content={withShortcut(t("menu.gifPrevFrame"), shortcuts.gifPrevFrame)}>
        <Button
          variant="outline"
          onClick={() => stepGifFrameBy(-1)}
          title={withShortcut(t("menu.gifPrevFrame"), shortcuts.gifPrevFrame)}
          aria-label={withShortcut(t("menu.gifPrevFrame"), shortcuts.gifPrevFrame)}
        >
          <CaretLeft data-icon="inline-start" />
        </Button>
      </AppTooltip>
      <AppTooltip content={withShortcut(playLabel, shortcuts.toggleGifPlayback)}>
        <Button
          variant="outline"
          onClick={toggleGifPlayback}
          title={withShortcut(playLabel, shortcuts.toggleGifPlayback)}
          aria-label={withShortcut(playLabel, shortcuts.toggleGifPlayback)}
        >
          {playing ? (
            <Pause data-icon="inline-start" weight="fill" />
          ) : (
            <Play data-icon="inline-start" weight="fill" />
          )}
        </Button>
      </AppTooltip>
      <AppTooltip content={withShortcut(t("menu.gifNextFrame"), shortcuts.gifNextFrame)}>
        <Button
          variant="outline"
          onClick={() => stepGifFrameBy(1)}
          title={withShortcut(t("menu.gifNextFrame"), shortcuts.gifNextFrame)}
          aria-label={withShortcut(t("menu.gifNextFrame"), shortcuts.gifNextFrame)}
        >
          <CaretRight data-icon="inline-start" />
        </Button>
      </AppTooltip>
      <ButtonGroupText
        className="tabular-nums no-drag"
        aria-label={t("header.gifFrame", { index: frame + 1, total: frameCount })}
      >
        {frame + 1}/{frameCount}
      </ButtonGroupText>
    </ButtonGroup>
  )
}
