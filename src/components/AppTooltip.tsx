import type { ReactElement, ReactNode } from "react"

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"

type AppTooltipProps = {
  /** 툴팁 본문. null/빈 문자열이면 트리거만 그대로 렌더한다. */
  content: ReactNode
  side?: React.ComponentProps<typeof TooltipContent>["side"]
  align?: React.ComponentProps<typeof TooltipContent>["align"]
  children: ReactElement
}

/**
 * 헤더 툴바 등 아이콘 버튼용 툴팁 래퍼.
 * 네이티브 `title` 대신 shadcn/Base UI 툴팁을 쓴다.
 *
 * 설계 메모:
 * - disabled 트리거에는 span 래퍼를 쓰지 않는다. 래퍼가 끼면 ButtonGroup의
 *   `[&>[data-slot]]` 자식 선택자가 래퍼를 잡아 모서리 병합이 깨지고,
 *   Base UI Toggle 계열은 `focusableWhenDisabled`를 지원하지 않는다.
 *   그래서 비활성 트리거에는 `title`을 병행한다 (AppTooltip을 쓰는 쪽에서
 *   title도 함께 넘기면 브라우저가 커스텀 툴팁 표시 중에는 네이티브를
 *   자동으로 억제해 중복 표시가 없다).
 * - Base UI 툴팁은 의도적으로 `role="tooltip"`/`aria-describedby`를 붙이지
 *   않고 시각 보조로만 쓴다. 접근성 이름은 트리거의 `aria-label`이 담당한다
 *   (툴팁 본문과 같은 문구를 유지할 것).
 * - 트리거는 ref를 받을 수 있어야 한다: `Button`, 네이티브 `button`,
 *   그리고 React 19 ref-as-prop 덕분에 props를 그대로 펼치는 `Toggle`,
 *   `ToggleGroupItem`, `ButtonGroupText`.
 */
export function AppTooltip({
  content,
  side = "bottom",
  align = "center",
  children
}: AppTooltipProps) {
  if (content == null || content === "") return children
  return (
    <Tooltip>
      <TooltipTrigger render={children} />
      <TooltipContent side={side} align={align}>
        {content}
      </TooltipContent>
    </Tooltip>
  )
}
