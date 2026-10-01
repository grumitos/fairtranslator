import { cn } from "@/lib/utils";

const RADIUS = 5.25;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

// Avance de un archivo en 14 px: arco de marca sobre una pista; completo, un círculo con check.
export function ProgressRing({
  done,
  total,
  className,
}: {
  done: number;
  total: number;
  className?: string;
}) {
  const ratio = total > 0 ? Math.min(1, Math.max(0, done / total)) : 0;
  const common = cn("size-3.5 shrink-0", className);

  if (total > 0 && done >= total) {
    return (
      <svg viewBox="0 0 14 14" aria-hidden="true" className={common}>
        <circle cx="7" cy="7" r="6.5" fill="var(--brand)" />
        <path
          d="M4.2 7.2l1.9 1.9 3.7-4"
          fill="none"
          stroke="var(--brand-ink)"
          strokeWidth={1.6}
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
    );
  }

  return (
    <svg viewBox="0 0 14 14" aria-hidden="true" className={common}>
      <circle cx="7" cy="7" r={RADIUS} fill="none" stroke="var(--bg-4)" strokeWidth={1.75} />
      {ratio > 0 && (
        <circle
          cx="7"
          cy="7"
          r={RADIUS}
          fill="none"
          stroke="var(--brand)"
          strokeWidth={1.75}
          strokeLinecap="round"
          strokeDasharray={`${(CIRCUMFERENCE * ratio).toFixed(2)} ${CIRCUMFERENCE.toFixed(2)}`}
          transform="rotate(-90 7 7)"
        />
      )}
    </svg>
  );
}
