import * as React from "react"

import { cn } from "@/lib/utils"

function Input({ className, type, ...props }: React.ComponentProps<"input">) {
  return (
    <input
      type={type}
      data-slot="input"
      className={cn(
        "h-8 w-full min-w-0 rounded-md border border-line-field bg-surface-2 px-2.5 text-sm text-ink transition-[border-color,color] duration-(--motion-fast) outline-none placeholder:text-ink-4 file:inline-flex file:h-7 file:border-0 file:bg-transparent file:text-sm file:font-medium file:text-ink disabled:cursor-not-allowed disabled:border-line-dim disabled:bg-transparent disabled:text-ink-4",
        "hover:border-line-strong focus-visible:border-line-chip focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ink",
        "aria-invalid:border-danger",
        className
      )}
      {...props}
    />
  )
}

export { Input }
