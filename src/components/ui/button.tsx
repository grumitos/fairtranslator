import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { Slot } from "radix-ui"

import { cn } from "@/lib/utils"

const buttonVariants = cva(
  "inline-flex shrink-0 items-center justify-center gap-2 rounded-md text-sm font-medium whitespace-nowrap border border-transparent transition-[background-color,border-color,color,box-shadow,transform,opacity] duration-150 outline-none active:translate-y-px focus-visible:border-ring focus-visible:ring-[2px] focus-visible:ring-ring/40 disabled:pointer-events-none disabled:translate-y-0 disabled:opacity-45 aria-invalid:border-destructive aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
  {
    variants: {
      variant: {
        default: "border-primary/80 bg-primary text-primary-foreground shadow-xs hover:bg-primary/92",
        destructive:
          "border-destructive/80 bg-destructive text-destructive-foreground shadow-xs hover:bg-destructive/90 focus-visible:ring-destructive/20 dark:bg-destructive/70 dark:focus-visible:ring-destructive/40",
        outline:
          "border-input bg-card/80 shadow-xs hover:border-ring/35 hover:bg-accent hover:text-accent-foreground dark:bg-input/30 dark:hover:bg-input/50",
        secondary:
          "border-border bg-secondary text-secondary-foreground shadow-xs hover:border-ring/25 hover:bg-secondary/80",
        ghost:
          "hover:border-border hover:bg-accent hover:text-accent-foreground dark:hover:bg-accent/55",
        link: "text-primary underline-offset-4 hover:underline",
      },
      size: {
        default: "h-8 px-3 py-1.5 has-[>svg]:px-2.5",
        xs: "h-6 gap-1 rounded-md px-2 text-xs has-[>svg]:px-1.5 [&_svg:not([class*='size-'])]:size-3",
        sm: "h-7 gap-1.5 rounded-md px-2.5 text-xs has-[>svg]:px-2",
        lg: "h-9 rounded-md px-5 has-[>svg]:px-3.5",
        icon: "size-8",
        "icon-xs": "size-6 rounded-md [&_svg:not([class*='size-'])]:size-3",
        "icon-sm": "size-7",
        "icon-lg": "size-9",
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
