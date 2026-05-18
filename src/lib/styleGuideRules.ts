export function normalizeStyleGuideRules(items: string[]): string[] {
  const seen = new Set<string>();
  const normalized: string[] = [];

  for (const raw of items) {
    const text = raw.trim();
    if (!text || seen.has(text)) {
      continue;
    }

    seen.add(text);
    normalized.push(text);
  }

  return normalized;
}

export function areStyleGuideRulesEqual(left: string[], right: string[]): boolean {
  return left.length === right.length && left.every((item, index) => item === right[index]);
}
