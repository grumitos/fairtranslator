import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

// Pista de atajo. Es decorativa (el atajo también está en el title), así que no entra en el nombre accesible.
export function Kbd({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <kbd
      aria-hidden="true"
      className={cn(
        "inline-flex h-[18px] items-center rounded-sm border border-line-strong px-[5px] font-mono text-[10.5px] text-ink-3",
        className,
      )}
    >
      {children}
    </kbd>
  );
}
