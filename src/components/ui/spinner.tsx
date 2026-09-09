import { cn } from "cn"
import { CircleNotch } from "@phosphor-icons/react"

function Spinner({ className, ...props }: React.ComponentProps<"svg">) {
  return (
    <CircleNotch data-slot="spinner" role="status" aria-label="Loading" className={cn("size-4 animate-spin", className)} {...props} />
  )
}

export { Spinner }
