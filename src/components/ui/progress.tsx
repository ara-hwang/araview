import * as React from "react";
import { cn } from "@/lib/utils";

interface ProgressProps extends React.ComponentPropsWithoutRef<"div"> {
  value?: number;
  /** 트랙 위를 움직이는 원형 핸들 스타일 (썸) */
  variant?: "fill" | "thumb";
}

const Progress = React.forwardRef<HTMLDivElement, ProgressProps>(
  ({ className, value = 0, variant = "fill", ...props }, ref) => {
    const pct = Math.min(100, Math.max(0, value));
    const isThumb = variant === "thumb";

    return (
      <div
        ref={ref}
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(value)}
        className={cn(
          "relative w-full overflow-visible rounded-full",
          isThumb
            ? "h-1.5 px-1.5 bg-[hsl(var(--muted))] border border-[hsl(var(--border))]"
            : "h-4 bg-[hsl(var(--secondary))]",
          className
        )}
        {...props}
      >
        {isThumb ? (
          <div
            className="absolute top-1/2 size-3 rounded-full bg-[hsl(var(--primary))] shadow-sm transition-all pointer-events-none"
            style={{
              left: `calc(0.375rem + (100% - 0.75rem) * ${pct / 100})`,
              transform: "translate(-50%, -50%)",
            }}
          />
        ) : (
          <div
            className="h-full flex-1 bg-[hsl(var(--primary))] transition-all rounded-full"
            style={{ width: `${pct}%` }}
          />
        )}
      </div>
    );
  }
);
Progress.displayName = "Progress";

export { Progress };
