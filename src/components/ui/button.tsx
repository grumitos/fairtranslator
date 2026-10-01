import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { Slot } from "radix-ui"

import { cn } from "@/lib/utils"

const buttonVariants = cva(
  "inline-flex shrink-0 items-center justify-center gap-[7px] rounded-lg border text-sm font-medium whitespace-nowrap transition-[background-color,border-color,color,filter] duration-(--motion-fast) outline-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ink disabled:cursor-not-allowed aria-invalid:border-danger [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-[15px]",
  {
    variants: {
      variant: {
        // Relleno de tinta: acciones de confirmación que no son la acción principal de la vista.
        default:
          "border-ink bg-ink font-semibold text-surface-0 hover:border-ink-hover hover:bg-ink-hover disabled:border-line-dim disabled:bg-transparent disabled:text-ink-4",
        // Relleno de marca: solo la acción principal de cada vista.
        brand:
          "border-brand bg-brand font-semibold text-brand-ink hover:brightness-110 disabled:border-line-dim disabled:bg-transparent disabled:text-ink-4",
        destructive:
          "border-line-strong bg-transparent text-danger hover:bg-danger-bg disabled:border-line-dim disabled:text-ink-4",
        outline:
          "border-line-strong bg-transparent text-ink hover:bg-surface-2 disabled:border-line-dim disabled:text-ink-4",
        secondary:
          "border-transparent bg-surface-3 text-ink hover:bg-surface-4 disabled:bg-transparent disabled:text-ink-4",
        ghost:
          "border-transparent bg-transparent text-ink-2 hover:bg-surface-3 hover:text-ink disabled:text-ink-4",
        link: "border-transparent text-ink underline-offset-4 hover:underline disabled:text-ink-4",
      },
      size: {
        default: "h-8 px-3",
        xs: "h-6 gap-1 rounded-md px-2 text-xs [&_svg:not([class*='size-'])]:size-3",
        sm: "h-7 gap-1.5 rounded-md px-2.5 text-[12.5px] [&_svg:not([class*='size-'])]:size-[14px]",
        lg: "h-[34px] px-3.5",
        icon: "size-[30px] rounded-md [&_svg:not([class*='size-'])]:size-4",
        "icon-xs": "size-6 rounded-md [&_svg:not([class*='size-'])]:size-3",
        "icon-sm": "size-7 rounded-md [&_svg:not([class*='size-'])]:size-[15px]",
        "icon-lg": "size-9 rounded-md [&_svg:not([class*='size-'])]:size-4",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
)

function Button({
  className,
  variant = "default",
  size = "default",
  asChild = false,
  ...props
}: React.ComponentProps<"button"> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean
  }) {
  const Comp = asChild ? Slot.Root : "button"

  return (
    <Comp
      data-slot="button"
      data-variant={variant}
      data-size={size}
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  )
}

export { Button, buttonVariants }
