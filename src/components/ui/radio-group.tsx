import * as React from "react";
import { RadioGroup as BaseUIRadioGroup, Radio } from "@base-ui/react";
import { cn } from "@/lib/utils";

const RadioGroupRoot = React.forwardRef<
  HTMLDivElement,
  React.ComponentProps<typeof BaseUIRadioGroup>
>(({ className, ...props }, ref) => (
  <BaseUIRadioGroup
    ref={ref}
    className={cn("flex flex-col gap-2", className)}
    {...props}
  />
));
RadioGroupRoot.displayName = "RadioGroupRoot";

const RadioGroupItem = React.forwardRef<
  HTMLSpanElement,
  React.ComponentProps<typeof Radio.Root>
>(({ className, children, ...props }, ref) => (
  <Radio.Root
    ref={ref}
    className={cn(
      "flex items-center gap-2 cursor-pointer group",
      className
    )}
    {...props}
  >
    <span className="relative flex h-4 w-4 items-center justify-center rounded-full border border-[hsl(var(--border))] bg-transparent group-data-[checked]:border-[hsl(var(--primary))] transition-colors">
      <Radio.Indicator className="block h-2 w-2 rounded-full bg-[hsl(var(--primary))]" />
    </span>
    {children}
  </Radio.Root>
));
RadioGroupItem.displayName = "RadioGroupItem";

export { RadioGroupRoot, RadioGroupItem };
