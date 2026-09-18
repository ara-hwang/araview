import * as React from "react"
import { Toast as ToastPrimitive, type ToastManagerAddOptions } from "@base-ui/react/toast"
import { useTranslation } from "react-i18next"
import { cn } from "cn"

import { Button } from "@/components/ui/button"
import {
  Check,
  CheckCircle,
  CircleNotch,
  Copy,
  Info,
  Warning,
  X,
  XCircle
} from "@phosphor-icons/react"

const toastManager = ToastPrimitive.createToastManager()

type ToastHelperOptions = Omit<ToastManagerAddOptions<object>, "title" | "type" | "description" | "timeout"> & {
  description?: React.ReactNode
  /** sonner-style duration (ms). base-ui 명칭으로는 timeout이다. */
  duration?: number
  timeout?: number
}

function addWithType(
  type: string,
  title: React.ReactNode,
  options?: ToastHelperOptions
): string {
  const { description, duration, timeout, ...rest } = options ?? {}
  return toastManager.add({
    title,
    description,
    type,
    timeout: timeout ?? duration,
    ...rest
  })
}

/**
 * sonner `toast.success/error/info` 호출부와 호환되는 헬퍼.
 * `duration`은 base-ui `timeout`으로 매핑된다.
 */
const toast = Object.assign(toastManager, {
  success: (title: React.ReactNode, options?: ToastHelperOptions) =>
    addWithType("success", title, options),
  info: (title: React.ReactNode, options?: ToastHelperOptions) =>
    addWithType("info", title, options),
  warning: (title: React.ReactNode, options?: ToastHelperOptions) =>
    addWithType("warning", title, options),
  error: (title: React.ReactNode, options?: ToastHelperOptions) =>
    addWithType("error", title, options)
})

function ToastProvider({ ...props }: ToastPrimitive.Provider.Props) {
  return <ToastPrimitive.Provider {...props} />
}

function ToastPortal({ ...props }: ToastPrimitive.Portal.Props) {
  return <ToastPrimitive.Portal data-slot="toast-portal" {...props} />
}

function ToastViewport({ className, ...props }: ToastPrimitive.Viewport.Props) {
  return (
    <ToastPrimitive.Viewport
      data-slot="toast-viewport"
      className={cn(
        "pointer-events-none fixed inset-x-4 bottom-4 z-[100] mx-auto w-auto max-w-sm outline-none sm:right-4 sm:left-auto sm:mx-0 sm:w-full",
        className
      )}
      {...props}
    />
  )
}

function Toast({ className, ...props }: ToastPrimitive.Root.Props) {
  return (
    <ToastPrimitive.Root
      data-slot="toast"
      className={cn(
        "group/toast pointer-events-auto absolute right-0 bottom-0 z-[calc(1000-var(--toast-index))] w-full origin-bottom rounded-2xl border bg-popover text-popover-foreground shadow-lg will-change-transform outline-none select-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50",
        "[--gap:0.75rem] [--height:var(--toast-frontmost-height,var(--toast-height))] [--offset-y:calc(var(--toast-offset-y)*-1+calc(var(--toast-index)*var(--gap)*-1)+var(--toast-swipe-movement-y))] [--peek:0.75rem] [--scale:calc(max(0,1-(var(--toast-index)*0.1)))] [--shrink:calc(1-var(--scale))]",
        "h-(--height) [transform:translateX(var(--toast-swipe-movement-x))_translateY(calc(var(--toast-swipe-movement-y)-(var(--toast-index)*var(--peek))-(var(--shrink)*var(--height))))_scale(var(--scale))] [transition:transform_500ms_cubic-bezier(0.22,1,0.36,1),opacity_500ms,height_150ms]",
        "after:absolute after:top-full after:left-0 after:h-[calc(var(--gap)+1px)] after:w-full after:content-['']",
        "data-expanded:h-(--toast-height) data-expanded:[transform:translateX(var(--toast-swipe-movement-x))_translateY(var(--offset-y))]",
        "data-limited:opacity-0 data-starting-style:[transform:translateY(150%)]",
        "[&[data-ending-style]:not([data-limited]):not([data-swipe-direction])]:[transform:translateY(150%)]",
        "data-ending-style:data-[swipe-direction=down]:[transform:translateY(calc(var(--toast-swipe-movement-y)+150%))]",
        "data-ending-style:data-[swipe-direction=left]:[transform:translateX(calc(var(--toast-swipe-movement-x)-150%))_translateY(var(--offset-y))]",
        "data-ending-style:data-[swipe-direction=right]:[transform:translateX(calc(var(--toast-swipe-movement-x)+150%))_translateY(var(--offset-y))]",
        "data-ending-style:data-[swipe-direction=up]:[transform:translateY(calc(var(--toast-swipe-movement-y)-150%))]",
        "data-expanded:data-ending-style:data-[swipe-direction=down]:[transform:translateY(calc(var(--toast-swipe-movement-y)+150%))]",
        "data-expanded:data-ending-style:data-[swipe-direction=left]:[transform:translateX(calc(var(--toast-swipe-movement-x)-150%))_translateY(var(--offset-y))]",
        "data-expanded:data-ending-style:data-[swipe-direction=right]:[transform:translateX(calc(var(--toast-swipe-movement-x)+150%))_translateY(var(--offset-y))]",
        "data-expanded:data-ending-style:data-[swipe-direction=up]:[transform:translateY(calc(var(--toast-swipe-movement-y)-150%))]",
        className
      )}
      {...props}
    />
  )
}

function ToastContent({ className, ...props }: ToastPrimitive.Content.Props) {
  return (
    <ToastPrimitive.Content
      data-slot="toast-content"
      className={cn(
        "flex h-full items-center gap-3 overflow-hidden p-4 transition-opacity duration-250 ease-[cubic-bezier(0.22,1,0.36,1)] data-behind:opacity-0 data-expanded:opacity-100",
        className
      )}
      {...props}
    />
  )
}

function ToastTitle({ className, ...props }: ToastPrimitive.Title.Props) {
  return (
    <ToastPrimitive.Title
      data-slot="toast-title"
      className={cn("text-sm font-medium", className)}
      {...props}
    />
  )
}

function ToastDescription({
  className,
  ...props
}: ToastPrimitive.Description.Props) {
  return (
    <ToastPrimitive.Description
      data-slot="toast-description"
      className={cn("text-sm text-muted-foreground", className)}
      {...props}
    />
  )
}

function ToastAction({
  className,
  render = <Button variant="outline" size="sm" />,
  ...props
}: ToastPrimitive.Action.Props) {
  return (
    <ToastPrimitive.Action
      data-slot="toast-action"
      render={render}
      className={cn("shrink-0", className)}
      {...props}
    />
  )
}

function ToastClose({
  className,
  children,
  render = <Button variant="ghost" size="icon-sm" />,
  ...props
}: ToastPrimitive.Close.Props) {
  const { t } = useTranslation()
  return (
    <ToastPrimitive.Close
      data-slot="toast-close"
      aria-label={t("toast.close")}
      render={render}
      className={cn(
        "relative shrink-0 text-muted-foreground after:absolute after:-inset-2 after:content-[''] hover:text-foreground",
        className
      )}
      {...props}
    >
      {children ?? <X aria-hidden="true" />}
    </ToastPrimitive.Close>
  )
}

function ToastIcon({ type }: { type: string | undefined }) {
  let icon: React.ReactNode = null

  if (type === "success") {
    icon = <CheckCircle aria-hidden="true" />
  }

  if (type === "info") {
    icon = <Info aria-hidden="true" />
  }

  if (type === "warning") {
    icon = <Warning aria-hidden="true" />
  }

  if (type === "error") {
    icon = <XCircle className="text-destructive" aria-hidden="true" />
  }

  if (type === "loading") {
    icon = <CircleNotch className="animate-spin" aria-hidden="true" />
  }

  if (!icon) {
    return null
  }

  return (
    <span
      data-slot="toast-icon"
      className="shrink-0 [&_svg]:pointer-events-none [&_svg:not([class*='size-'])]:size-4"
    >
      {icon}
    </span>
  )
}

function toCopyText(value: React.ReactNode): string {
  if (value === null || value === undefined || typeof value === "boolean") return ""
  if (typeof value === "string" || typeof value === "number") return String(value)
  if (Array.isArray(value)) return value.map(toCopyText).join("")
  if (React.isValidElement(value)) {
    const props = value.props as { children?: React.ReactNode }
    return toCopyText(props.children)
  }
  return ""
}

/** 오류 토스트의 제목+설명을 클립보드에 복사하는 아이콘 버튼 */
function ToastCopyButton({
  title,
  description
}: {
  title?: React.ReactNode
  description?: React.ReactNode
}) {
  const { t } = useTranslation()
  const [copied, setCopied] = React.useState(false)
  const timerRef = React.useRef<number | null>(null)

  React.useEffect(
    () => () => {
      if (timerRef.current !== null) window.clearTimeout(timerRef.current)
    },
    []
  )

  const text = [toCopyText(title), toCopyText(description)].filter(Boolean).join("\n")
  if (!text) return null

  const label = copied ? t("toast.copied") : t("toast.copyMessage")

  const handleCopy = async (e: React.SyntheticEvent) => {
    e.stopPropagation()
    try {
      await navigator.clipboard.writeText(text)
    } catch {
      try {
        const ta = document.createElement("textarea")
        ta.value = text
        ta.style.position = "fixed"
        ta.style.opacity = "0"
        document.body.appendChild(ta)
        ta.select()
        document.execCommand("copy")
        document.body.removeChild(ta)
      } catch {
        return
      }
    }
    setCopied(true)
    if (timerRef.current !== null) window.clearTimeout(timerRef.current)
    timerRef.current = window.setTimeout(() => setCopied(false), 1500)
  }

  return (
    <Button
      type="button"
      variant="ghost"
      size="icon-sm"
      data-slot="toast-copy"
      data-base-ui-swipe-ignore
      aria-label={label}
      title={label}
      onClick={(e) => void handleCopy(e)}
      className="relative shrink-0 text-muted-foreground after:absolute after:-inset-2 after:content-[''] hover:text-foreground"
    >
      {copied ? <Check aria-hidden="true" /> : <Copy aria-hidden="true" />}
    </Button>
  )
}

function ToastList() {
  const { toasts } = ToastPrimitive.useToastManager()

  return toasts.map((toastItem) => (
    <Toast key={toastItem.id} toast={toastItem}>
      <ToastContent>
        <ToastIcon type={toastItem.type} />
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <ToastTitle />
          <ToastDescription />
        </div>
        {toastItem.type === "error" && (
          <ToastCopyButton title={toastItem.title} description={toastItem.description} />
        )}
        <ToastAction />
        <ToastClose />
      </ToastContent>
    </Toast>
  ))
}

function Toaster({
  children,
  toastManager: manager = toastManager,
  ...props
}: ToastPrimitive.Provider.Props) {
  return (
    <ToastProvider toastManager={manager} {...props}>
      {children}
      <ToastPortal>
        <ToastViewport>
          <ToastList />
        </ToastViewport>
      </ToastPortal>
    </ToastProvider>
  )
}

const createToastManager = ToastPrimitive.createToastManager
const useToastManager = ToastPrimitive.useToastManager

export {
  Toaster,
  Toast,
  ToastAction,
  ToastClose,
  ToastContent,
  ToastCopyButton,
  ToastDescription,
  ToastPortal,
  ToastProvider,
  ToastTitle,
  ToastViewport,
  createToastManager,
  toast,
  toCopyText,
  useToastManager
}
