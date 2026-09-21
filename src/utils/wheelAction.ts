import type { WheelAction, WheelMap, WheelSlot } from "@/constants/shortcuts"

export type WheelEventLike = {
  deltaY: number
  ctrlKey: boolean
  shiftKey: boolean
  altKey: boolean
}

/** 휠 이벤트를 설정 슬롯 이름으로 변환한다. */
export function toWheelSlot(e: WheelEventLike): WheelSlot {
  const direction = e.deltaY < 0 ? "wheelUp" : "wheelDown"
  if (e.ctrlKey) return `ctrl+${direction}` as WheelSlot
  if (e.shiftKey) return `shift+${direction}` as WheelSlot
  if (e.altKey) return `alt+${direction}` as WheelSlot
  return direction
}

/** 설정된 휠 동작을 해석한다. 세로 이동이 없거나 none이면 null. */
export function resolveWheelAction(e: WheelEventLike, wheel: WheelMap): WheelAction | null {
  if (e.deltaY === 0) return null
  const action = wheel[toWheelSlot(e)] ?? "none"
  return action === "none" ? null : action
}
