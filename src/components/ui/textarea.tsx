import * as React from "react"

import { cn } from "@/lib/utils"

function Textarea({ className, ...props }: React.ComponentProps<"textarea">) {
  return (
    <textarea
      data-slot="textarea"
      className={cn(
        "flex field-sizing-content min-h-16 w-full rounded-md border border-line-field bg-surface-2 px-3 py-2 text-sm leading-relaxed text-ink transition-[border-color,color] duration-(--motion-fast) outline-none placeholder:text-ink-4 hover:border-line-strong focus-visible:border-line-chip focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ink disabled:cursor-not-allowed disabled:border-line-dim disabled:bg-transparent disabled:text-ink-4 aria-invalid:border-danger",
        className
      )}
      {...props}
    />
  )
}

export { Textarea }
