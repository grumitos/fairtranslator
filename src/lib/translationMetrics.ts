import type { TranslationEntry } from "@/types/domain";
import { hasTranslationContent } from "@/lib/translationState";

export interface TranslationProgressStats {
  total: number;
  translated: number;
  pending: number;
  percent: number;
}

export function isTranslatableEntry(entry: TranslationEntry): boolean {
  return entry.original.trim() !== "";
}

export function isPendingTranslationEntry(entry: TranslationEntry): boolean {
  return (
    isTranslatableEntry(entry)
    && (entry.status === "untranslated" || !hasTranslationContent(entry.translation))
  );
}

export function isTranslatedEntry(entry: TranslationEntry): boolean {
  return (
    isTranslatableEntry(entry)
    && entry.status !== "untranslated"
    && hasTranslationContent(entry.translation)
  );
}

export function countTranslationProgress(entries: TranslationEntry[]): TranslationProgressStats {
  let total = 0;
  let translated = 0;

  for (const entry of entries) {
    if (!isTranslatableEntry(entry)) {
      continue;
    }

    total += 1;
    if (entry.status !== "untranslated" && hasTranslationContent(entry.translation)) {
      translated += 1;
    }
  }

  return {
    total,
    translated,
    pending: total - translated,
    percent: total > 0 ? Math.round((translated / total) * 100) : 0,
  };
}

export function countPendingTranslationEntries(entries: TranslationEntry[]): number {
  let pending = 0;

  for (const entry of entries) {
    if (isPendingTranslationEntry(entry)) {
      pending += 1;
    }
  }

  return pending;
}
