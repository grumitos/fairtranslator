import * as React from "react"
import { XIcon } from "lucide-react"
import { Dialog as SheetPrimitive } from "radix-ui"

import { cn } from "@/lib/utils"
import i18n from "@/lib/i18n"

function Sheet({ ...props }: React.ComponentProps<typeof SheetPrimitive.Root>) {
  return <SheetPrimitive.Root data-slot="sheet" {...props} />
}

function SheetTrigger({
  ...props
}: React.ComponentProps<typeof SheetPrimitive.Trigger>) {
  return <SheetPrimitive.Trigger data-slot="sheet-trigger" {...props} />
}

function SheetClose({
  ...props
}: React.ComponentProps<typeof SheetPrimitive.Close>) {
  return <SheetPrimitive.Close data-slot="sheet-close" {...props} />
}

function SheetPortal({
  ...props
}: React.ComponentProps<typeof SheetPrimitive.Portal>) {
  return <SheetPrimitive.Portal data-slot="sheet-portal" {...props} />
}

function SheetOverlay({
  className,
  ...props
}: React.ComponentProps<typeof SheetPrimitive.Overlay>) {
  return (
    <SheetPrimitive.Overlay
      data-slot="sheet-overlay"
      className={cn("fixed inset-0 top-12 z-50 bg-scrim", className)}
      {...props}
    />
  )
}

/*
 * Panel lateral del kit: 440 px, fondo bg-1, sombra hacia la izquierda y entrada con
 * desplazamiento y fundido (las animaciones viven en index.css, por data-slot y data-state).
 * Queda por debajo de la barra de título para que la ventana se pueda mover y cerrar.
 */
function SheetContent({
  className,
  children,
  side = "right",
  showCloseButton = true,
  ...props
}: React.ComponentProps<typeof SheetPrimitive.Content> & {
  side?: "top" | "right" | "bottom" | "left"
  showCloseButton?: boolean
}) {
  return (
    <SheetPortal>
      <SheetOverlay />
      <SheetPrimitive.Content
        data-slot="sheet-content"
        data-side={side}
        className={cn(
          "fixed z-50 flex flex-col overflow-hidden overscroll-contain bg-surface-1 text-ink outline-none",
          side === "right" &&
            "top-12 right-0 bottom-0 w-[var(--sheet-width,440px)] max-w-[calc(100vw-2rem)] border-l border-line-field shadow-drawer-left",
          side === "left" &&
            "top-12 bottom-0 left-0 w-[var(--sheet-width,440px)] max-w-[calc(100vw-2rem)] border-r border-line-field shadow-drawer-right",
          side === "top" && "inset-x-0 top-12 h-auto border-b border-line-field",
          side === "bottom" && "inset-x-0 bottom-0 h-auto border-t border-line-field",
          className
        )}
        {...props}
      >
        {children}
        {showCloseButton && (
          <SheetPrimitive.Close className="absolute top-[18px] right-5 inline-flex size-[30px] items-center justify-center rounded-md text-ink-2 transition-colors duration-(--motion-fast) outline-none hover:bg-surface-3 hover:text-ink focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ink disabled:pointer-events-none">
            <XIcon className="size-4" strokeWidth={1.7} />
            <span className="sr-only">{i18n.t("common:close")}</span>
          </SheetPrimitive.Close>
        )}
      </SheetPrimitive.Content>
    </SheetPortal>
  )
}

function SheetHeader({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="sheet-header"
      className={cn("flex shrink-0 flex-col gap-1 border-b border-line py-[18px] pr-16 pl-5", className)}
      {...props}
    />
  )
}

function SheetBody({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="sheet-body"
      className={cn("flex min-h-0 flex-1 flex-col gap-5 overflow-y-auto px-5 py-[18px]", className)}
      {...props}
    />
  )
}

function SheetFooter({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="sheet-footer"
      className={cn("mt-auto flex shrink-0 items-center justify-end gap-2 border-t border-line px-5 py-4", className)}
      {...props}
    />
  )
}

function SheetTitle({
  className,
  ...props
}: React.ComponentProps<typeof SheetPrimitive.Title>) {
  return (
    <SheetPrimitive.Title
      data-slot="sheet-title"
      className={cn("text-[15px] font-semibold text-ink", className)}
      {...props}
    />
  )
}

function SheetDescription({
  className,
  ...props
}: React.ComponentProps<typeof SheetPrimitive.Description>) {
  return (
    <SheetPrimitive.Description
      data-slot="sheet-description"
      className={cn("text-xs text-ink-3", className)}
      {...props}
    />
  )
}

export {
  Sheet,
  SheetTrigger,
  SheetClose,
  SheetContent,
  SheetHeader,
  SheetBody,
  SheetFooter,
  SheetTitle,
  SheetDescription,
}
