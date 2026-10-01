import { memo, useMemo, type ReactNode } from "react";
import { cn } from "@/lib/utils";
import { splitSpecialTokens } from "./controlCodes";

/*
 * Texto de una entrada con los códigos de control en mono y color de marca (el traductor debe
 * conservarlos) y, si hay búsqueda activa, las coincidencias resaltadas en tinta.
 */
function highlight(text: string, regex: RegExp | null, keyPrefix: string): ReactNode[] {
  if (!regex || !text) return [text];

  const parts: ReactNode[] = [];
  const re = new RegExp(regex.source, regex.flags.includes("g") ? regex.flags : `${regex.flags}g`);
  let lastIndex = 0;
  let match: RegExpExecArray | null;
  let count = 0;

  while ((match = re.exec(text)) !== null) {
    if (match.index > lastIndex) parts.push(text.slice(lastIndex, match.index));
    if (match[0].length > 0) {
      parts.push(
        <mark key={`${keyPrefix}-${count++}`} className="rounded-[3px] bg-ink px-px text-surface-0">
          {match[0]}
        </mark>,
      );
    }
    lastIndex = re.lastIndex;
    if (match[0].length === 0) re.lastIndex += 1;
  }
  if (lastIndex < text.length) parts.push(text.slice(lastIndex));
  return parts;
}

export const ControlCodeText = memo(function ControlCodeText({
  text,
  regex = null,
  className,
  placeholder = "—",
}: {
  text: string;
  regex?: RegExp | null;
  className?: string;
  placeholder?: string;
}) {
  const segments = useMemo(() => splitSpecialTokens(text), [text]);

  if (!text) {
    return <span className={cn("text-ink-4", className)}>{placeholder}</span>;
  }

  return (
    <span className={className}>
      {segments.map((segment, index) =>
        segment.kind === "code" ? (
          <span
            key={index}
            className="rounded-sm bg-brand-soft px-1 py-px font-mono text-[12px] leading-none text-brand-text [overflow-wrap:anywhere]"
          >
            {segment.text}
          </span>
        ) : (
          <span key={index}>{highlight(segment.text, regex, String(index))}</span>
        ),
      )}
    </span>
  );
});
