import { Command, MagnifyingGlass } from "@phosphor-icons/react"
import { useEffect, useId, useMemo, useRef } from "react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog"
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty"
import { ScrollArea } from "@/components/ui/scroll-area"
import { COMMAND_GROUP_ORDER } from "@/constants/commands"
import {
  useCommandPaletteHost,
  usePaletteStore,
  type ResolvedPaletteCommand
} from "@/hooks/useCommandPalette"
import { cn } from "@/lib/utils"

function scrollActiveIntoView(el: HTMLButtonElement | null) {
  el?.scrollIntoView({ block: "nearest" })
}

function CommandRow({
  item,
  index,
  active,
  onActiveIndexChange,
  onRun
}: {
  item: ResolvedPaletteCommand
  index: number
  active: boolean
  onActiveIndexChange: (index: number) => void
  onRun: (item: ResolvedPaletteCommand) => void
}) {
  return (
    <li>
      <button
        ref={(el) => {
          if (active) scrollActiveIntoView(el)
        }}
        type="button"
        role="option"
        aria-selected={active}
        disabled={!item.enabled}
        onMouseEnter={() => onActiveIndexChange(index)}
        onClick={() => onRun(item)}
        className={cn(
          "flex w-full items-center justify-between gap-2 rounded-lg px-2 py-1.5 text-left text-sm",
          "focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none",
          active && item.enabled && "bg-accent text-accent-foreground",
          !item.enabled && "opacity-50"
        )}
      >
        <span className="min-w-0 flex-1 truncate">{item.label}</span>
        {item.shortcutLabel && (
          <kbd className="shrink-0 font-mono text-xs text-muted-foreground tabular-nums">
            {item.shortcutLabel}
          </kbd>
        )}
      </button>
    </li>
  )
}

function GroupedList({
  items,
  baseIndex = 0,
  activeIndex,
  onActiveIndexChange,
  onRun,
  listboxId
}: {
  items: ResolvedPaletteCommand[]
  baseIndex?: number
  activeIndex: number
  onActiveIndexChange: (index: number) => void
  onRun: (item: ResolvedPaletteCommand) => void
  listboxId: string
}) {
  const { t } = useTranslation()
  const tx = t as unknown as (key: string) => string

  const groups = useMemo(() => {
    const map = new Map<string, ResolvedPaletteCommand[]>()
    items.forEach((item) => {
      const list = map.get(item.def.group)
      if (list) list.push(item)
      else map.set(item.def.group, [item])
    })
    return COMMAND_GROUP_ORDER.flatMap((group) => {
      const list = map.get(group)
      return list ? [{ group, list }] : []
    })
  }, [items])

  if (items.length === 0) {
    return (
      <Empty>
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <MagnifyingGlass />
          </EmptyMedia>
          <EmptyTitle>{tx("palette.noResult")}</EmptyTitle>
          <EmptyDescription>{tx("palette.noResultHint")}</EmptyDescription>
        </EmptyHeader>
      </Empty>
    )
  }

  let offset = baseIndex
  return (
    <div role="listbox" id={listboxId} aria-label={tx("palette.title")}>
      {groups.map(({ group, list }) => {
        const start = offset
        offset += list.length
        return (
          <div key={group}>
            <p aria-hidden className="px-2 pt-2 pb-1 text-xs font-medium text-muted-foreground">
              {tx(`palette.group.${group}`)}
            </p>
            <ul>
              {list.map((item, i) => {
                const index = start + i
                return (
                  <CommandRow
                    key={item.def.id}
                    item={item}
                    index={index}
                    active={index === activeIndex}
                    onActiveIndexChange={onActiveIndexChange}
                    onRun={onRun}
                  />
                )
              })}
            </ul>
          </div>
        )
      })}
    </div>
  )
}

export function CommandPalette() {
  const { t } = useTranslation()
  const tx = t as unknown as (key: string) => string
  const open = usePaletteStore((s) => s.open)
  const query = usePaletteStore((s) => s.query)
  const { filtered, recentCount, activeIndex, run } = useCommandPaletteHost()
  const listboxId = useId()
  const inputRef = useRef<HTMLInputElement | null>(null)

  const recent = filtered.slice(0, recentCount)
  const rest = filtered.slice(recentCount)
  const setActiveIndex = (i: number) => usePaletteStore.getState().setActiveIndex(i)
  const handleRun = (item: ResolvedPaletteCommand) => {
    if (item.enabled) run(item.def.id)
  }

  useEffect(() => {
    if (open) {
      const frame = requestAnimationFrame(() => inputRef.current?.focus())
      return () => cancelAnimationFrame(frame)
    }
  }, [open])

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault()
      if (filtered.length > 0) {
        usePaletteStore.getState().setActiveIndex((activeIndex + 1) % filtered.length)
      }
    } else if (e.key === "ArrowUp") {
      e.preventDefault()
      if (filtered.length > 0) {
        usePaletteStore
          .getState()
          .setActiveIndex((activeIndex - 1 + filtered.length) % filtered.length)
      }
    } else if (e.key === "Enter") {
      e.preventDefault()
      const item = filtered[activeIndex]
      if (item && item.enabled) run(item.def.id)
    }
  }

  return (
    // modal="trap-focus": 타이틀바를 덮는 투명 전체화면 백드롭을 없앤다 (App.css 주석 참고)
    <Dialog
      open={open}
      modal="trap-focus"
      onOpenChange={(isOpen) => usePaletteStore.getState().setOpen(isOpen)}
    >
      <DialogContent
        showCloseButton={false}
        className="gap-0 overflow-hidden p-0 no-drag sm:max-w-lg"
      >
        <DialogTitle className="sr-only">{tx("palette.title")}</DialogTitle>
        <DialogDescription className="sr-only">{tx("palette.desc")}</DialogDescription>
        <div className="flex items-center gap-2 border-b px-3">
          <Command className="size-4 shrink-0 text-muted-foreground" />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => usePaletteStore.getState().setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            role="combobox"
            aria-expanded
            aria-controls={listboxId}
            aria-autocomplete="list"
            placeholder={tx("palette.search")}
            aria-label={tx("palette.search")}
            className="h-11 w-full bg-transparent text-sm outline-none placeholder:text-muted-foreground"
          />
          {query && (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => usePaletteStore.getState().setQuery("")}
            >
              {tx("palette.clear")}
            </Button>
          )}
        </div>
        <ScrollArea className="max-h-[50vh] p-2">
          {recent.length > 0 && (
            <div>
              <p aria-hidden className="px-2 pt-2 pb-1 text-xs font-medium text-muted-foreground">
                {tx("palette.recent")}
              </p>
              <ul>
                {recent.map((item, i) => (
                  <CommandRow
                    key={item.def.id}
                    item={item}
                    index={i}
                    active={i === activeIndex}
                    onActiveIndexChange={setActiveIndex}
                    onRun={handleRun}
                  />
                ))}
              </ul>
            </div>
          )}
          <GroupedList
            items={rest}
            baseIndex={recentCount}
            activeIndex={activeIndex}
            onActiveIndexChange={(i) => usePaletteStore.getState().setActiveIndex(i)}
            onRun={(item) => {
              if (item.enabled) run(item.def.id)
            }}
            listboxId={listboxId}
          />
        </ScrollArea>
      </DialogContent>
    </Dialog>
  )
}
