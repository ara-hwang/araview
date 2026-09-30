import { cn } from "cn"
import { useEffect, useRef, useState, type KeyboardEvent } from "react"

import { computeListWindow, scrollTopToReveal } from "@/utils/listWindow"

export type LicenseListItem = {
  key: string
  title: string
  subtitle: string
}

type LicenseListProps = {
  items: LicenseListItem[]
  selectedKey: string
  onSelect: (key: string) => void
  label: string
}

const ROW_HEIGHT = 52
const OVERSCAN = 6

// 가상화된 단일 선택 목록. 보이는 행만 DOM에 두고 위/아래 화살표, Home/End로 이동한다.
export function LicenseList({ items, selectedKey, onSelect, label }: LicenseListProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const [scrollTop, setScrollTop] = useState(0)
  const [viewportHeight, setViewportHeight] = useState(400)

  useEffect(() => {
    const el = containerRef.current
    if (!el) return
    setViewportHeight(el.clientHeight)
    const observer = new ResizeObserver(() => setViewportHeight(el.clientHeight))
    observer.observe(el)
    return () => observer.disconnect()
  }, [])

  const selectedIndex = Math.max(
    0,
    items.findIndex((item) => item.key === selectedKey)
  )
  const { start, end, totalHeight } = computeListWindow({
    scrollTop,
    viewportHeight,
    itemCount: items.length,
    rowHeight: ROW_HEIGHT,
    overscan: OVERSCAN
  })

  const select = (index: number) => {
    const clamped = Math.min(items.length - 1, Math.max(0, index))
    const item = items[clamped]
    if (!item) return
    onSelect(item.key)
    const el = containerRef.current
    if (el) {
      el.scrollTop = scrollTopToReveal(clamped, el.scrollTop, el.clientHeight, ROW_HEIGHT)
    }
  }

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const page = Math.max(1, Math.floor(viewportHeight / ROW_HEIGHT) - 1)
    const targets: Record<string, number> = {
      ArrowDown: selectedIndex + 1,
      ArrowUp: selectedIndex - 1,
      PageDown: selectedIndex + page,
      PageUp: selectedIndex - page,
      Home: 0,
      End: items.length - 1
    }
    const target = targets[event.key]
    if (target === undefined) return
    event.preventDefault()
    select(target)
  }

  return (
    <div
      ref={containerRef}
      role="listbox"
      aria-label={label}
      aria-activedescendant={items[selectedIndex] ? optionId(selectedIndex) : undefined}
      tabIndex={0}
      onKeyDown={handleKeyDown}
      onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
      className="relative min-h-0 flex-1 overflow-y-auto scheme-light outline-none focus-visible:ring-1 focus-visible:ring-ring focus-visible:ring-inset dark:scheme-dark"
    >
      <div style={{ height: totalHeight }}>
        {items.slice(start, end).map((item, offset) => {
          const index = start + offset
          const active = item.key === selectedKey
          return (
            <div
              key={item.key}
              id={optionId(index)}
              role="option"
              aria-selected={active}
              aria-setsize={items.length}
              aria-posinset={index + 1}
              onClick={() => select(index)}
              style={{ top: index * ROW_HEIGHT, height: ROW_HEIGHT }}
              className={cn(
                "absolute inset-x-0 flex cursor-default flex-col justify-center px-3 text-left",
                active ? "bg-primary/10 text-primary" : "hover:bg-accent"
              )}
            >
              <span className="truncate text-sm font-medium">{item.title}</span>
              <span className="truncate text-xs text-muted-foreground">{item.subtitle}</span>
            </div>
          )
        })}
      </div>
    </div>
  )
}

const optionId = (index: number) => `license-option-${index}`
