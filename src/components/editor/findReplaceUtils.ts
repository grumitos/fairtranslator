import type { TranslationEntry } from "@/types/domain";

/** Represents one match: which entry and which field */
export interface FindMatch {
  filePath: string;
  entryId: string;
  /** The entry when the match was computed; matches are recomputed on every project change. */
  entry: TranslationEntry;
  field: "original" | "translation";
  /** Flat index in data array for scrolling */
  dataIndex: number;
}

/**
 * Build a RegExp from the user's find query + options.
 * Returns null if the query is empty or invalid.
 */
export function buildFindRegex(
  query: string,
  matchCase: boolean,
  useRegex: boolean,
): RegExp | null {
  if (!query) return null;
  try {
    const flags = matchCase ? "g" : "gi";
    const pattern = useRegex ? query : query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    return new RegExp(pattern, flags);
  } catch {
    return null;
  }
}
