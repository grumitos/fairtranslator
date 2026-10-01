import type { EntryStatus, TranslationEntry } from "@/types/domain";
import { hasTranslationContent } from "@/lib/translationState";

export function isTranslatableEntry(entry: TranslationEntry): boolean {
  return entry.original.trim() !== "";
}

export function isTranslatedEntry(entry: TranslationEntry): boolean {
  return (
    isTranslatableEntry(entry)
    && entry.status !== "untranslated"
    && hasTranslationContent(entry.translation)
  );
}

export interface FileStats {
  /** Translatable entries (non-empty original). */
  total: number;
  translated: number;
  pending: number;
  /** Translatable entries per status. */
  byStatus: Record<EntryStatus, number>;
}

// Cada edición crea un array de entradas nuevo solo para su archivo (immer comparte el resto), así
// que los conteos por archivo se reutilizan y solo se recalcula el archivo editado.
const fileStatsCache = new WeakMap<TranslationEntry[], FileStats>();

export function fileStats(entries: TranslationEntry[]): FileStats {
  const cached = fileStatsCache.get(entries);
  if (cached) return cached;
  const stats: FileStats = { total: 0, translated: 0, pending: 0, byStatus: { untranslated: 0, draft: 0, reviewed: 0, final: 0 } };
  for (const entry of entries) {
    if (!isTranslatableEntry(entry)) continue;
    stats.total += 1;
    stats.byStatus[entry.status] += 1;
    if (isTranslatedEntry(entry)) stats.translated += 1;
    else stats.pending += 1;
  }
  fileStatsCache.set(entries, stats);
  return stats;
}

/** Suma de los conteos por archivo (vista "Todas las entradas"). */
export function projectStats(files: Array<{ entries: TranslationEntry[] }>): FileStats {
  const sum: FileStats = { total: 0, translated: 0, pending: 0, byStatus: { untranslated: 0, draft: 0, reviewed: 0, final: 0 } };
  for (const file of files) {
    const stats = fileStats(file.entries);
    sum.total += stats.total;
    sum.translated += stats.translated;
    sum.pending += stats.pending;
    for (const status of Object.keys(sum.byStatus) as EntryStatus[]) sum.byStatus[status] += stats.byStatus[status];
  }
  return sum;
}
