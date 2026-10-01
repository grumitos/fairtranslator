"use client"

import * as React from "react"
import { Label as LabelPrimitive } from "radix-ui"

import { cn } from "@/lib/utils"

function Label({
  className,
  ...props
}: React.ComponentProps<typeof LabelPrimitive.Root>) {
  return (
    <LabelPrimitive.Root
      data-slot="label"
      className={cn(
        "flex items-center gap-2 text-xs leading-none text-ink-3 select-none group-data-[disabled=true]:pointer-events-none group-data-[disabled=true]:text-ink-4 peer-disabled:cursor-not-allowed peer-disabled:text-ink-4",
        className
      )}
      {...props}
    />
  )
}

export { Label }
