/**
 * Highlights regex matches in text by wrapping them in <mark> elements.
 * Extracted from TranslationGrid to keep components under 200 lines.
 */
export function HighlightedText({
  text,
  regex,
  className,
}: {
  text: string;
  regex: RegExp | null;
  className?: string;
}) {
  if (!regex || !text) {
    return <span className={className}>{text || "\u2014"}</span>;
  }

  const parts: { text: string; highlighted: boolean }[] = [];
  let lastIndex = 0;
  let match: RegExpExecArray | null;
  // Clone regex so we don't mutate the shared one
  const re = new RegExp(regex.source, regex.flags);

  while ((match = re.exec(text)) !== null) {
    if (match.index > lastIndex) {
      parts.push({ text: text.slice(lastIndex, match.index), highlighted: false });
    }
    parts.push({ text: match[0], highlighted: true });
    lastIndex = re.lastIndex;
    // Guard against zero-length matches causing infinite loops
    if (match[0].length === 0) {
      re.lastIndex++;
    }
  }
  if (lastIndex < text.length) {
    parts.push({ text: text.slice(lastIndex), highlighted: false });
  }

  if (parts.length === 0) {
    return <span className={className}>{text}</span>;
  }

  return (
    <span className={className}>
      {parts.map((part, i) =>
        part.highlighted ? (
          <mark
            key={i}
            className="bg-yellow-300 text-foreground dark:bg-yellow-600 rounded-sm px-0.5"
          >
            {part.text}
          </mark>
        ) : (
          <span key={i}>{part.text}</span>
        ),
      )}
    </span>
  );
}
