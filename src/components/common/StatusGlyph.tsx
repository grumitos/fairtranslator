import { cn } from "@/lib/utils";
import type { EntryStatus } from "@/types/domain";

/*
 * Estado de una entrada por forma, no solo por color: aro vacío (sin traducir), medio relleno
 * (borrador), aro con punto (revisado) y círculo de marca con check (final).
 */
export function StatusGlyph({ status, className }: { status: EntryStatus; className?: string }) {
  const common = cn("size-3.5 shrink-0", className);

  switch (status) {
    case "untranslated":
      return (
        <svg viewBox="0 0 16 16" aria-hidden="true" className={common}>
          <circle cx="8" cy="8" r="6" fill="none" stroke="var(--line-chip)" strokeWidth={1.5} />
        </svg>
      );
    case "draft":
      return (
        <svg viewBox="0 0 16 16" aria-hidden="true" className={common}>
          <circle cx="8" cy="8" r="6" fill="none" stroke="var(--ink-3)" strokeWidth={1.5} />
          <path d="M8 2a6 6 0 0 0 0 12z" fill="var(--ink-3)" />
        </svg>
      );
    case "reviewed":
      return (
        <svg viewBox="0 0 16 16" aria-hidden="true" className={common}>
          <circle cx="8" cy="8" r="6" fill="none" stroke="var(--ink)" strokeWidth={1.5} />
          <circle cx="8" cy="8" r="2.5" fill="var(--ink)" />
        </svg>
      );
    case "final":
      return (
        <svg viewBox="0 0 16 16" aria-hidden="true" className={common}>
          <circle cx="8" cy="8" r="7" fill="var(--brand)" />
          <path
            d="M4.9 8.2l2.1 2.1 4.1-4.5"
            fill="none"
            stroke="var(--brand-ink)"
            strokeWidth={1.8}
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      );
  }
}
